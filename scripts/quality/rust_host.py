#!/usr/bin/env python3
"""Local shadow host: provision fresh signed inputs, collect, and retain failures.

The host owns its private key. This pilot does not accept a production baseline
or replace Arc-Admin's existing full/hook gates.
"""
import argparse
import base64
import copy
import json
import os
from pathlib import Path
import shutil
import subprocess
import time
import uuid

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives import serialization

from rust_collector import canonical, decode, describe, METRICS, sha, validate_native


def write(path, value):
    path.write_bytes(canonical(value) + b'\n')


def invoke(binary, args, log):
    with log.open('w') as stream:
        return subprocess.run([str(binary), *map(str, args)], stdout=stream,
                              stderr=subprocess.STDOUT, timeout=300).returncode


def key_at(path):
    if path.exists():
        if path.is_symlink() or path.stat().st_mode & 0o077:
            raise ValueError('private key must be a regular host-owned file with mode 0600')
        return Ed25519PrivateKey.from_private_bytes(path.read_bytes())
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    key = Ed25519PrivateKey.generate()
    data = key.private_bytes(serialization.Encoding.Raw, serialization.PrivateFormat.Raw,
                             serialization.NoEncryption())
    with os.fdopen(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), 'wb') as stream:
        stream.write(data)
    return key


def configuration(root, harness, raw, series):
    directory = root / '.harness-gate'
    directory.mkdir(exist_ok=False)
    reference = harness / 'docs/dogfood/arc-admin'
    shutil.copy2(reference / 'import/flow.toml', directory / 'flow.toml')
    shutil.copy2(reference / 'quality/packs/backend/policy.json', directory / 'policy.json')
    rules = decode((directory / 'policy.json').read_bytes())['rules']
    lines = ['version = 1', '[project]', 'id = "arc-admin"', 'name = "arc-admin"',
             '[components.backend]', 'flow_components = ["backend"]',
             'source_roots = ["backend/src"]', 'artifact_root = ".harness-gate/evidence"',
             '[subjects.backend-files]', 'component = "backend"', 'kind = "file"',
             'selection = { kind = "explicit", paths = ' + json.dumps(sorted(raw['source_inventory'])) + ' }',
             '[collectors.backend]', 'protocol = "harness-collector-request/v1"',
             'request = ".harness-gate/backend-request.json"']
    for metric in METRICS:
        lines += ['[[collectors.backend.produces]]', 'target = { kind = "component", id = "backend" }',
                  'capability = ' + json.dumps(metric), 'series = ' + json.dumps(series['id'])]
    for rule in rules:
        lines += ['[policies.' + json.dumps(rule['id']) + ']', 'policy_file = ".harness-gate/policy.json"',
                  'rule = ' + json.dumps(rule['id']),
                  'expectation = { target = { kind = "component", id = "backend" }, capability = '
                  + json.dumps(rule['metric']) + ', series = ' + json.dumps(series['id']) + ' }']
    lines += ['[profiles.full]', 'assurance = "partial"', 'collectors = ["backend"]',
              'policies = ' + json.dumps([r['id'] for r in rules]),
              '[profiles.hook]', 'assurance = "partial"', 'collectors = []', 'policies = []',
              '[baseline]', 'required = true',
              'provider = { kind = "git", reference = "origin/main", merge_base = true }',
              '[reporting]', 'output = ".harness-gate/reports"', 'formats = ["human", "json"]']
    (directory / 'quality.toml').write_text('\n'.join(lines) + '\n')
    write(directory / 'backend-request.json', {})
    (directory / 'evidence').mkdir()
    return directory


def signed(request, key):
    # Match the public adapter-v2 signing contract's struct field order. Nested
    # arbitrary JSON maps are canonical/sorted, as serde_json::Value is in Rust.
    fields = ('protocol_version', 'result_schema_version', 'adapter', 'invocation_id', 'step_id',
              'timeout_ms', 'config_digest', 'artifact_root', 'nonce', 'issued_at_ms', 'expires_at_ms',
              'args', 'environment', 'capabilities', 'input')
    payload = {'domain': 'harness-gate/adapter-request/v2',
               **{field: copy.deepcopy(request[field]) for field in fields}}
    adapter = request['adapter']
    payload['adapter'] = {field: adapter[field] for field in ('name', 'version', 'executable', 'source_digest')}
    payload['adapter']['signature'] = {field: adapter['signature'][field] for field in ('algorithm', 'key_id')}
    payload['capabilities'] = {field: sorted(request['capabilities'][field])
                               for field in ('network', 'resources', 'environment')}
    payload['environment'] = dict(sorted(request['environment'].items()))
    payload['input'] = decode(canonical(payload['input']))
    signature = key.sign(json.dumps(payload, ensure_ascii=False, separators=(',', ':')).encode())
    request['adapter']['signature']['value'] = base64.b64encode(signature).decode()
    return request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sample', type=Path, required=True)
    parser.add_argument('--repository', type=Path, required=True,
                        help='host-controlled Git checkout used to authenticate source identity')
    parser.add_argument('--harness-source', type=Path, required=True)
    parser.add_argument('--harness-bin', type=Path, required=True)
    parser.add_argument('--private-key', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    sample, out = args.sample.resolve(), args.output.resolve()
    repository = args.repository.resolve()
    if any(args.private_key.resolve().is_relative_to(path) for path in (sample, out, repository)):
        raise ValueError('private key must remain outside all source/evidence directories')
    out.mkdir(parents=True, exist_ok=False)
    root = out / 'source'
    shutil.copytree(sample / 'source', root)
    native_sha = sha((sample / 'native.json').read_bytes())
    raw = validate_native(sample, native_sha, root)
    expected_commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repository, text=True).strip()
    if raw['commit'] != expected_commit:
        raise ValueError('native commit differs from the host-controlled checkout')
    tracked = subprocess.check_output(['git', 'ls-tree', '-r', '--name-only', expected_commit,
                                      '--', 'backend/src'], cwd=repository, text=True).splitlines()
    if set(raw['source_inventory']) != {path for path in tracked if path.endswith('.rs')}:
        raise ValueError('native inventory omits or adds Git-tracked Rust source')
    for path, digest in raw['source_inventory'].items():
        original = subprocess.check_output(['git', 'show', expected_commit + ':' + path], cwd=repository)
        if sha(original) != digest:
            raise ValueError('native source is not the expected Git object: ' + path)
    base = subprocess.check_output(['git', 'merge-base', expected_commit, 'origin/main'],
                                   cwd=repository, text=True).strip()
    series, subjects = describe(raw)
    config = configuration(root, args.harness_source.resolve(), raw, series)
    state = {'schema': 'quality-trusted-state/v1', 'profile': 'full',
        'expected': {'commit': expected_commit, 'base_commit': base, 'target': series['target'], 'run': raw['run']},
        'components': {'backend': {'id': 'backend', 'path': 'backend', 'metadata': {'language': 'rust'},
            'targets': [{'id': series['target'], 'boundaries': ['source'], 'metadata': {}}],
            'source_boundaries': [{'id': 'source', 'path': 'backend/src', 'role': 'production', 'metadata': {}}]}},
        'subject_kinds': {'file': 'file/v1'}, 'relationship_kinds': {},
        'subjects': {'backend-files': subjects}, 'series': {'backend': series},
        'artifact_root': '.harness-gate/evidence', 'artifacts': {},
        'selection': None, 'mappings': None, 'exceptions': []}
    def pin():
        state['config_files'] = {'.harness-gate/' + name: sha((config / name).read_bytes())
                                for name in ['flow.toml', 'quality.toml', 'policy.json', 'backend-request.json']}
        write(out / 'state.json', state)
    pin()
    binary = args.harness_bin.resolve()
    rc = invoke(binary, ['quality', 'compile', '--repository-root', root, '--state', out / 'state.json',
                         '--output', out / 'compiled.json'], out / 'compile.log')
    if rc:
        raise RuntimeError('trusted compilation failed; see compile.log')
    inputs = decode((out / 'compiled.json').read_bytes())
    binding = {'schema': 'quality-collector-binding/v1',
        'config_files': {k: v for k, v in state['config_files'].items() if not k.endswith('backend-request.json')},
        **{key: inputs[key] for key in ['project', 'policy', 'expected', 'selection', 'mappings', 'exceptions']},
        'series': state['series'], 'profile': state['profile']}
    key = key_at(args.private_key.resolve())
    public = key.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
    key_id = 'arc-admin-local-shadow-' + sha(public)[:12]
    write(out / 'trusted-keys.json', [{'key_id': key_id, 'public_key': base64.b64encode(public).decode()}])
    executable = Path(__file__).with_name('rust_collector.py').resolve()
    executable.chmod(0o755)
    now = time.time_ns() // 1_000_000
    request = {'protocol_version': 2, 'result_schema_version': '1',
        'adapter': {'name': series['collector']['name'], 'version': '1', 'executable': str(executable),
                    'source_digest': sha(executable.read_bytes()),
                    'signature': {'algorithm': 'ed25519', 'key_id': key_id, 'value': ''}},
        'invocation_id': raw['run'], 'step_id': 'backend', 'timeout_ms': 120000,
        'config_digest': sha(canonical(binding)), 'artifact_root': str(config / 'evidence'),
        'nonce': uuid.uuid4().hex, 'issued_at_ms': now, 'expires_at_ms': now + 300000,
        'args': [str(sample), native_sha], 'environment': {},
        'capabilities': {'network': [], 'resources': [], 'environment': []},
        'input': {'schema': 'harness-project-collector-request/v1', 'project': 'arc-admin',
                  'collector': series['collector'], 'context': state['expected'], 'workspace_root': str(root),
                  'output_root': str(config / 'evidence'), 'selection': inputs['selection'],
                  'bindings': [{'subject': sid, 'capability': metric, 'series': series['id']}
                               for sid, metric in sorted((s['id'], m) for s in subjects for m in METRICS)]}}
    write(config / 'backend-request.json', signed(request, key))
    pin()
    rc = invoke(binary, ['quality', 'collect', '--repository-root', root, '--state', out / 'state.json',
                        '--trusted-keys', out / 'trusted-keys.json', '--output', out / 'collection.json'],
                out / 'collection.log')
    write(out / 'host-receipt.json', {'collection_exit_code': rc, 'authority_transfer_permitted': False,
          'trust_scope': 'local shadow only; no accepted baseline or production trust anchor',
          'harness_binary_sha256': sha(binary.read_bytes()), 'native_manifest_sha256': native_sha})
    if rc:
        raise RuntimeError('signed native collection failed; see collection.log')
    collection = decode((out / 'collection.json').read_bytes())
    for name in ['project', 'policy', 'expected']:
        write(out / (name + '.json'), collection['inputs'][name])
    write(out / 'evidence.json', collection['evidence'])
    evaluation = ['quality', 'evaluate', '--source-root', root, '--artifact-root', config / 'evidence',
                  '--output', out / 'policy-report.json', '--evidence', out / 'evidence.json']
    for name in ['project', 'policy', 'expected']:
        evaluation += ['--' + name, out / (name + '.json')]
    rc = invoke(binary, evaluation, out / 'evaluation.log')
    write(out / 'evaluation-receipt.json', {'exit_code': rc, 'accepted': False,
          'note': 'No base comparison; partial backend shadow, uncertified CRAP remains measurement_error.'})
    print(json.dumps({'collection_exit_code': 0, 'policy_exit_code': rc,
                      'subjects': len(subjects), 'authority_transfer_permitted': False}))
    raise SystemExit(rc)


if __name__ == '__main__':
    main()
