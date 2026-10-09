#!/usr/bin/python3
"""Prepare authenticated full/hook state from an explicitly pinned native bundle.

This host utility grants no authority to downloaded measurements: the operator
must review and supply the bundle digest. It rejects changed source/tool inputs.
Native capture context remains in the raw artifacts; this is explicit replay.
"""
import argparse
import base64
import copy
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import tomllib


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def fingerprint(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()).hexdigest()


def read(path):
    return json.loads(path.read_text())


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + '\n')


def command(args):
    return subprocess.check_output([str(v) for v in args], text=True)


def git(root, *args):
    return command(['git', '-C', root, *args]).strip()


def checked(root, name, digest):
    path = Path(name)
    if path.is_absolute() or '..' in path.parts:
        raise ValueError('unsafe native path')
    current = root
    for part in path.parts:
        current /= part
        if current.is_symlink():
            raise ValueError('symlink in trusted input')
    if sha(current) != digest:
        raise ValueError('native input changed: ' + name)
    return current


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--measurements', required=True, type=Path)
    parser.add_argument('--sha256', required=True)
    parser.add_argument('--core', default='harness-gate')
    parser.add_argument('--profile', choices=['full', 'hook'], default='full')
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parent.parent)
    parser.add_argument('--scope-json', type=Path, help='exact Core scope JSON for a partial/staged invocation')
    parser.add_argument('--baseline', action='store_true', help='accept exact-source replay at the real origin/main merge base')
    args = parser.parse_args()
    root = args.root.resolve()
    measurement = args.measurements.resolve()
    if sha(measurement) != args.sha256:
        raise ValueError('untrusted native bundle digest')
    bundle = read(measurement)
    if bundle['schema'] != 'arc-native-measurements/v1':
        raise ValueError('unknown native measurement bundle')
    for name, digest in bundle['sources'].items():
        checked(root, name, digest)
    for name, digest in bundle['files'].items():
        checked(measurement.parent, name, digest)
    config = root / '.harness-gate'
    quality = tomllib.loads((config / 'quality.toml').read_text())
    if quality['baseline'] != {'required': True, 'provider': {'kind': 'git', 'reference': 'origin/main', 'merge_base': True}}:
        raise ValueError('project baseline contract changed')
    for name, digest in bundle['configuration'].items():
        checked(root, name, digest)
    runtime = config / 'runtime'
    runtime.mkdir(exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix='arc-quality-host-'))
    os.chmod(output, 0o700)
    context = {'target': bundle['target'], 'commit': git(root, 'rev-parse', 'HEAD'), 'base_commit': git(root, 'merge-base', 'HEAD', 'origin/main'), 'run': 'arc-quality-host-' + str(time.time_ns())}
    state = copy.deepcopy(bundle['state'])
    state.update(profile=args.profile, expected=context, artifacts={}, selection={'changed_subject': sorted(bundle['state']['subjects']), 'critical_subject': []})
    if args.scope_json:
        scope = read(args.scope_json)
        state['selection']['changed_subject'] = sorted(alias for alias, declaration in quality['subjects'].items() if set(quality['components'][declaration['component']]['flow_components']) & set(scope['components']) and (scope['mode'] in ('all', 'components') or any(subject['path'] in scope['changed_files'] for subject in state['subjects'][alias])))
    state['series'] = bundle['state']['series'] if args.profile == 'full' else {}
    artifacts = root / state['artifact_root']
    if artifacts.exists() and any(p.is_file() for p in artifacts.rglob('*')):
        raise ValueError('artifact root is not fresh; retain the previous run separately')
    for component in quality['components'].values():
        (root / component['artifact_root']).mkdir(parents=True, exist_ok=True)
    selected = quality['profiles'][args.profile]['collectors']
    requests = {name: root / quality['collectors'][name]['request'] for name in selected}
    for path in requests.values():
        write(path, {})
    fixed = sorted({'.harness-gate/flow.toml', '.harness-gate/quality.toml', *(v['policy_file'] for v in quality['policies'].values())})
    statepath = runtime / (args.profile + '-state.json')

    def pin():
        state['config_files'] = {name: sha(root / name) for name in fixed + [str(p.relative_to(root)) for p in requests.values()]}
        write(statepath, state)

    pin()
    compiled_path = output / 'compiled.json'
    command([args.core, 'quality', 'compile', '--repository-root', root, '--state', statepath, '--output', compiled_path])
    compiled = read(compiled_path)
    binding = {'schema': 'quality-collector-binding/v1', 'config_files': {name: sha(root / name) for name in fixed}, 'series': state['series'], 'profile': args.profile}
    binding.update({name: compiled[name] for name in ('project', 'policy', 'expected', 'selection', 'mappings', 'exceptions')})
    digest = fingerprint(binding)
    key = output / 'private.pem'
    public = output / 'public.der'
    command(['openssl', 'genpkey', '-algorithm', 'ED25519', '-out', key])
    command(['openssl', 'pkey', '-in', key, '-pubout', '-outform', 'DER', '-out', public])
    write(runtime / 'trusted-keys.json', [{'key_id': 'arc-admin-quality-host', 'public_key': base64.b64encode(public.read_bytes()[12:]).decode()}])
    for name in selected:
        records = copy.deepcopy(bundle['records'][name])
        copies = {}
        for record in records:
            record['context'] = context
            for artifact in record['artifacts']:
                artifact['context'] = context
                copies[artifact['path']] = artifact
            if 'contract' in record:
                record['contract']['baseline']['commit'] = context['base_commit']
                path = record['subject']['path']
                original = subprocess.check_output(['git', '-C', str(root), 'show', context['base_commit'] + ':' + path])
                if hashlib.sha256(original).hexdigest() != record['source']['sha256']:
                    raise ValueError('API baseline changed; new native contract measurement required')
        for path, artifact in copies.items():
            target = output / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(checked(measurement.parent, path, artifact['sha256']).read_bytes())
        series = state['series'][name]
        inner = {'schema': 'harness-project-collector-request/v1', 'project': 'arc-admin', 'collector': series['collector'], 'context': context, 'workspace_root': str(root), 'output_root': str(artifacts), 'selection': compiled['selection'], 'bindings': sorted([{'subject': r['subject']['id'], 'capability': c['metric'], 'series': series['id']} for r in records for c in r['capabilities']], key=lambda v: (v['subject'], v['capability'], v['series']))}
        response = {'schema_version': '1', 'status': 'PASS', 'invocation_id': context['run'], 'artifacts': list(copies.values()), 'collection': {'schema': 'harness-project-collector-response/v1', 'error': None, 'evidence': records}}
        path = output / (name + '.json')
        write(path, {'schema': 'arc-native-replay/v1', 'config_digest': digest, 'input': inner, 'sources': bundle['sources'], 'response': response})
        now = int(time.time() * 1000)
        executable = config / 'collect.py'
        request = {'protocol_version': 2, 'result_schema_version': '1', 'adapter': series['collector'] | {'executable': str(executable), 'source_digest': sha(executable), 'signature': {'algorithm': 'ed25519', 'key_id': 'arc-admin-quality-host', 'value': ''}}, 'invocation_id': context['run'], 'step_id': name, 'timeout_ms': 300000, 'config_digest': digest, 'artifact_root': str(artifacts), 'nonce': context['run'] + '-' + name, 'issued_at_ms': now, 'expires_at_ms': now + 3600000, 'args': ['--bundle', str(path), '--sha256', sha(path)], 'environment': {}, 'capabilities': {'network': [], 'resources': [], 'environment': []}, 'input': inner}
        unsigned = {'domain': 'harness-gate/adapter-request/v2', **copy.deepcopy(request)}
        del unsigned['adapter']['signature']['value']
        unsigned['input'] = json.loads(json.dumps(inner, sort_keys=True))
        message = output / (name + '-signed.json')
        message.write_text(json.dumps(unsigned, separators=(',', ':'), ensure_ascii=False))
        signature = output / (name + '.sig')
        command(['openssl', 'pkeyutl', '-sign', '-rawin', '-inkey', key, '-in', message, '-out', signature])
        request['adapter']['signature']['value'] = base64.b64encode(signature.read_bytes()).decode()
        write(requests[name], request)
    pin()
    if args.baseline:
        if args.profile != 'full' or context['base_commit'] == context['commit']:
            raise ValueError('baseline requires a distinct real origin/main merge base')
        prepare_baseline(args, root, runtime, output, bundle, measurement, state, fixed, requests)
    write(output / 'receipt.json', {'schema': 'arc-quality-host-receipt/v1', 'measurement_sha256': args.sha256, 'original_contexts': bundle['original_contexts'], 'context': context, 'profile': args.profile, 'state': str(statepath), 'baseline_requested': args.baseline, 'native_recompile': False, 'configuration_sha256': state['config_files']})
    print(output)


def prepare_baseline(args, root, runtime, output, bundle, measurement, head, fixed, requests):
    base = copy.deepcopy(head)
    base['expected'] = dict(head['expected'], commit=head['expected']['base_commit'], base_commit=git(root, 'rev-parse', head['expected']['base_commit'] + '^'), run=head['expected']['run'] + '-base')
    directory = runtime / 'accepted-baseline'
    if directory.exists():
        raise ValueError('accepted baseline already exists; retain it before a new acceptance')
    directory.mkdir()
    files = {}
    names = set(fixed + [str(p.relative_to(root)) for p in requests.values()])
    names.update(s['path'] for ss in base['subjects'].values() for s in ss)
    for name in sorted(names):
        data = subprocess.check_output(['git', '-C', str(root), 'show', base['expected']['commit'] + ':' + name])
        path = directory / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        files[name] = sha(path)
    base['config_files'] = {name: files[name] for name in head['config_files']}
    for name, digest in bundle['sources'].items():
        data = subprocess.check_output(['git', '-C', str(root), 'show', base['expected']['commit'] + ':' + name])
        if hashlib.sha256(data).hexdigest() != digest:
            raise ValueError('baseline source/tool input changed: ' + name)
    records = copy.deepcopy([r for rs in bundle['records'].values() for r in rs])
    for record in records:
        record['context'] = base['expected']
        if 'contract' in record:
            record['contract']['baseline']['commit'] = base['expected']['base_commit']
            # This series only permits byte-identical accepted API lineage.
            path = record['subject']['path']
            old = subprocess.check_output(['git', '-C', str(root), 'show', base['expected']['base_commit'] + ':' + path])
            if hashlib.sha256(old).hexdigest() != record['source']['sha256']:
                raise ValueError('baseline parent contract changed; measure it explicitly')
        for artifact in record['artifacts']:
            artifact['context'] = base['expected']
            name = base['artifact_root'] + '/' + artifact['path']
            path = directory / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(checked(measurement.parent, artifact['path'], artifact['sha256']).read_bytes())
            base['artifacts'][artifact['path']] = artifact['sha256']
            files[name] = artifact['sha256']
    manifest = directory / 'manifest.json'
    write(manifest, {'schema': 'quality-baseline-manifest/v1', 'state': base, 'evidence': records, 'files': files})
    write(runtime / 'full-baseline-request.json', {'schema': 'quality-baseline-request/v1', 'state': base, 'manifest': str(manifest.relative_to(root)), 'manifest_sha256': sha(manifest)})


if __name__ == '__main__':
    main()
