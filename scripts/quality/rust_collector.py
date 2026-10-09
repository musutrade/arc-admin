#!/usr/bin/python3
"""Signed adapter for retained, same-run native Rust measurements. No approval."""
import gzip
import hashlib
import json
from pathlib import Path
import sys

METRICS = ('coverage.line', 'coverage.region', 'risk.crap')


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False,
                      allow_nan=False).encode()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def decode(data):
    def unique(pairs):
        obj = {}
        for key, value in pairs:
            if key in obj:
                raise ValueError('duplicate JSON key: ' + key)
            obj[key] = value
        return obj
    return json.loads(data, object_pairs_hook=unique)


def checked(path, expected):
    if path.is_symlink() or not path.is_file():
        raise ValueError('native input is not a regular file: ' + str(path))
    data = path.read_bytes()
    if sha(data) != expected:
        raise ValueError('native input digest mismatch: ' + str(path))
    return data


def describe(raw):
    if 'host: x86_64-unknown-linux-gnu' not in raw['tools']['rustc']:
        raise ValueError('this native pilot requires the declared x86_64 Linux target')
    series = {
        'name': 'arc-admin-native-file-coverage',
        'collector': {'name': 'arc-admin-rust-native', 'version': '1'},
        'tool': {'name': 'cargo-llvm-cov', 'version': sha(canonical(raw['tools']))},
        'rule': {'name': 'llvm-file-summary-unfiltered', 'version': '1'},
        'runtime': {'name': 'rust', 'version': sha(raw['tools']['rustc'].encode())},
        'source_identity': {'name': 'subject-identity', 'version': '1'},
        'normalization': {'name': 'exact-native-counters', 'version': '1'},
        'target': 'x86_64-unknown-linux-gnu',
        'metrics': [{'name': metric, 'type': 'rational' if metric == 'risk.crap' else 'ratio'}
                    for metric in METRICS],
    }
    series['id'] = 'measurement-series/v1:' + sha(canonical(series))
    subjects = []
    for path, digest in sorted(raw['source_inventory'].items()):
        subject = {'identity_version': 'subject-identity/v1', 'component': 'backend',
                   'target': series['target'], 'boundary': 'source', 'kind': 'file/v1',
                   'path': path, 'discriminator': path, 'source_sha256': digest}
        subject['id'] = 'subject-identity/v1:' + sha(canonical({'project': 'arc-admin', **subject}))
        subject['metadata'] = {'scope': 'LLVM file summaries; includes inline test code; not certified production/function coverage'}
        subjects.append(subject)
    return series, subjects


def validate_native(sample, expected_sha, workspace):
    raw = decode(checked(sample / 'native.json', expected_sha))
    if raw.get('schema') != 'arc-admin-rust-native/v1' or not raw.get('source_inventory'):
        raise ValueError('unsupported native contract or empty inventory')
    paths = set(raw['source_inventory'])
    discovered = {str(p.relative_to(workspace)) for p in (workspace / 'backend/src').rglob('*.rs')}
    if paths != discovered:
        raise ValueError('source inventory changed')
    for path, digest in raw['source_inventory'].items():
        if not path.startswith('backend/src/') or '..' in Path(path).parts:
            raise ValueError('invalid native source path')
        checked(workspace / path, digest)
        checked(sample / 'source' / path, digest)
    execution = decode(checked(sample / 'execution.json', raw['execution_sha256']))
    if (execution['commit'], execution['run'], execution['exit_code']) != (raw['commit'], raw['run'], 0):
        raise ValueError('native tests did not pass in the expected run')
    checked(sample / 'tests.log', raw['tests_sha256'])
    llvm_bytes = gzip.decompress((sample / 'llvm.json.gz').read_bytes())
    if sha(llvm_bytes) != raw['llvm_sha256']:
        raise ValueError('LLVM export digest mismatch')
    llvm = decode(llvm_bytes)
    if llvm.get('type') != 'llvm.coverage.json.export' or len(llvm['data']) != 1:
        raise ValueError('invalid LLVM export')
    observed, native_roots = {}, set()
    for row in llvm['data'][0]['files']:
        filename = row['filename']
        matches = [name for name in paths if filename.endswith('/' + name)]
        if not matches:
            continue
        if len(matches) != 1:
            raise ValueError('ambiguous native source path')
        name = matches[0]
        native_roots.add(filename[:-len(name)])
        if name in observed:
            raise ValueError('duplicate native file')
        observed[name] = {}
        for metric in ('lines', 'regions'):
            counts = row['summary'][metric]
            covered, total = counts['covered'], counts['count']
            if type(covered) is not int or type(total) is not int or not 0 <= covered <= total:
                raise ValueError('invalid native counters')
            observed[name][metric] = {'covered': covered, 'total': total}
    if len(native_roots) != 1:
        raise ValueError('mixed or missing native workspace roots')
    if raw['coverage'] != {'files': observed, 'missing': sorted(paths - set(observed))}:
        raise ValueError('normalized counts do not match the raw LLVM export')
    if raw['risk_state'] != 'measurement_error':
        raise ValueError('uncertified CRAP cannot claim a numeric result')
    return raw


def collect(request):
    if (request.get('protocol_version'), request.get('result_schema_version'),
        request.get('input', {}).get('schema'), request.get('input', {}).get('project')) != (
            2, '1', 'harness-project-collector-request/v1', 'arc-admin'):
        raise ValueError('unexpected signed collector protocol/project')
    sample, expected_sha = request['args']
    sample = Path(sample).resolve()
    workspace = Path(request['input']['workspace_root']).resolve()
    raw = validate_native(sample, expected_sha, workspace)
    context = request['input']['context']
    if (context['commit'], context['run']) != (raw['commit'], raw['run']):
        raise ValueError('native source/run differs from signed context')
    series, subjects = describe(raw)
    bindings = sorted((s['id'], m, series['id']) for s in subjects for m in METRICS)
    received = [(b['subject'], b['capability'], b['series']) for b in request['input']['bindings']]
    if received != bindings or request['input']['collector'] != series['collector']:
        raise ValueError('signed bindings do not match discovered native subjects')
    out = Path(request['artifact_root']).resolve()
    if str(out) != request['input']['output_root'] or list(out.iterdir()):
        raise ValueError('artifact root must match the request and be empty')
    records, artifacts = [], []
    for index, subject in enumerate(subjects):
        source = {'path': subject['path'], 'sha256': subject['source_sha256']}
        row = raw['coverage']['files'].get(subject['path'])
        content = {'source': source, 'native': row, 'native_manifest_sha256': expected_sha,
                   'llvm_sha256': raw['llvm_sha256'], 'context': context,
                   'risk_state': raw['risk_state'], 'risk_reason': raw['risk_reason']}
        path = out / f'file-{index}.json'
        path.write_bytes(canonical(content))
        names = [path.name]
        if index == 0:
            names += ['native.json', 'llvm.json.gz', 'execution.json', 'tests.log']
            for name in names[1:]:
                (out / name).write_bytes((sample / name).read_bytes())
        refs = [{'id': name, 'kind': 'raw', 'path': name, 'sha256': sha((out / name).read_bytes()),
                 'bytes': (out / name).stat().st_size,
                 'media_type': 'application/gzip' if name.endswith('.gz') else 'text/plain',
                 'context': context, 'source': source} for name in names]
        capabilities, metrics = [], []
        for metric, native in [('coverage.line', 'lines'), ('coverage.region', 'regions'), ('risk.crap', None)]:
            counts = row[native] if row and native else None
            state = ('supported' if counts['total'] else 'not_applicable') if counts else 'measurement_error'
            capabilities.append({'metric': metric, 'state': state, 'artifacts': names,
                'reason': raw['risk_reason'] if native is None else 'Unfiltered native LLVM file counts; missing rows are measurement errors.'})
            if state == 'supported':
                metrics.append({'name': metric, 'value': {'type': 'ratio', **counts}, 'artifacts': names})
        records.append({'schema': 'harness-evidence/v1', 'id': 'backend-' + sha(subject['path'].encode())[:24], 'project': 'arc-admin',
                        'component': 'backend', 'collector': series['collector'], 'series': series,
                        'subject': subject, 'context': context, 'source': source, 'metrics': metrics,
                        'capabilities': capabilities, 'artifacts': refs, 'status': 'measurement_error'})
        artifacts += refs
    return {'schema_version': '1', 'status': 'PASS', 'invocation_id': request['invocation_id'],
            'artifacts': artifacts, 'collection': {'schema': 'harness-project-collector-response/v1',
                                                 'evidence': records, 'error': None}}


if __name__ == '__main__':
    try:
        print(canonical(collect(decode(sys.stdin.buffer.read()))).decode())
    except (KeyError, OSError, ValueError) as error:
        print('native collector rejected input: ' + str(error), file=sys.stderr)
        raise SystemExit(1)
