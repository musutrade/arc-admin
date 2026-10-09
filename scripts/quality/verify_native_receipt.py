#!/usr/bin/env python3
"""Exercise rejection against a real completed collection, preserving originals."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import time

from rust_collector import canonical, decode, sha
from rust_host import signed
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--host-result', type=Path, required=True)
    parser.add_argument('--harness-bin', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--private-key', type=Path, required=True,
                        help='existing shadow host key, used only to sign the expired negative request')
    args = parser.parse_args()
    host, out, binary = args.host_result.resolve(), args.output.resolve(), args.harness_bin.resolve()
    receipt = decode((host / 'host-receipt.json').read_bytes())
    if receipt['collection_exit_code'] != 0:
        raise ValueError('negative verification requires a successful real collection')
    if sha(binary.read_bytes()) != receipt['harness_binary_sha256']:
        raise ValueError('verification binary changed')
    out.mkdir(parents=True, exist_ok=False)
    source = host / 'source'
    artifacts = source / '.harness-gate/evidence'
    results = []

    def evaluate(name, source_root=source, artifact_root=artifacts, expected=host / 'expected.json'):
        argv = [str(binary), 'quality', 'evaluate', '--project', str(host / 'project.json'),
                '--policy', str(host / 'policy.json'), '--evidence', str(host / 'evidence.json'),
                '--expected', str(expected), '--source-root', str(source_root),
                '--artifact-root', str(artifact_root), '--output', str(out / (name + '-report.json'))]
        result = subprocess.run(argv, capture_output=True, text=True, timeout=120)
        log = result.stdout + result.stderr
        (out / (name + '.log')).write_text(log)
        report_path = out / (name + '-report.json')
        reasons = []
        if report_path.exists():
            report = decode(report_path.read_bytes())
            reasons = [r['reason'] for r in report['policy_result']['results']
                       if r['state'] == 'measurement_error' and r['subject'] is None]
        tokens = {'tampered-artifact': ['digest mismatch'], 'changed-source': ['digest mismatch'],
                  'missing-artifact': ['missing', 'No such file', 'not found'],
                  'stale-context': ['stale commit/base/target/run evidence']}[name]
        diagnostic = log + '\n'.join(reasons)
        # Distinguish input-integrity rejection from the original per-subject
        # quality failures; a nonzero policy exit alone does not prove rejection.
        if result.returncode == 0 or not any(token in diagnostic for token in tokens):
            raise AssertionError('integrity rejection not observed: ' + name)
        results.append({'case': name, 'exit_code': result.returncode,
                        'diagnostic': diagnostic.strip(), 'log_sha256': sha(log.encode())})

    changed = out / 'changed-artifacts'
    shutil.copytree(artifacts, changed)
    (changed / 'file-0.json').write_text('{"fabricated":true}')
    evaluate('tampered-artifact', artifact_root=changed)
    missing = out / 'missing-artifacts'
    shutil.copytree(artifacts, missing)
    (missing / 'file-0.json').unlink()
    evaluate('missing-artifact', artifact_root=missing)
    changed_source = out / 'changed-source'
    shutil.copytree(source / 'backend', changed_source / 'backend')
    first = decode((host / 'evidence.json').read_bytes())[0]['subject']['path']
    with (changed_source / first).open('a') as stream:
        stream.write('\n// controlled negative source mutation\n')
    evaluate('changed-source', source_root=changed_source)
    expected = decode((host / 'expected.json').read_bytes())
    expected['run'] += '-stale'
    (out / 'stale-expected.json').write_bytes(canonical(expected))
    evaluate('stale-context', expected=out / 'stale-expected.json')

    trusted = decode((host / 'trusted-keys.json').read_bytes())[0]
    (out / 'public-key.json').write_bytes(canonical(trusted))
    for name in ['tampered-signature', 'expired-request']:
        request = decode((source / '.harness-gate/backend-request.json').read_bytes())
        now = time.time_ns() // 1_000_000
        request['issued_at_ms'] = now if name == 'tampered-signature' else now - 7200000
        request['expires_at_ms'] = now + 60000 if name == 'tampered-signature' else now - 3600000
        if name == 'expired-request':
            key_path = args.private_key
            if not key_path.is_file() or key_path.is_symlink() or key_path.stat().st_mode & 0o077:
                raise ValueError('negative signing requires the existing private host key (0600)')
            signed(request, Ed25519PrivateKey.from_private_bytes(key_path.read_bytes()))
        # Deliberately invalidate authenticated fields. Signature/freshness
        # validation must reject before the collector can touch its artifacts.
        request_path = out / (name + '.json')
        request_path.write_bytes(canonical(request))
        result = subprocess.run([str(binary), 'adapter', 'run', '--request', str(request_path),
                                 '--trusted-key', str(out / 'public-key.json')],
                                text=True, capture_output=True, timeout=30)
        log = result.stdout + result.stderr
        (out / (name + '.log')).write_text(log)
        token = 'signature' if name == 'tampered-signature' else 'expir'
        if result.returncode == 0 or token not in log.lower():
            raise AssertionError('authenticated rejection not observed: ' + name)
        results.append({'case': name, 'exit_code': result.returncode,
                        'diagnostic': log.strip(), 'log_sha256': sha(log.encode())})
    report = {'schema': 'arc-admin-native-negative-checks/v1',
              'native_manifest_sha256': receipt['native_manifest_sha256'],
              'harness_binary_sha256': receipt['harness_binary_sha256'],
              'cases': results, 'negative_checks_passed': True,
              'authority_transfer_permitted': False}
    (out / 'negative-checks.json').write_bytes(canonical(report) + b'\n')
    print(json.dumps({'negative_checks_passed': len(results), 'authority_transfer_permitted': False}))


if __name__ == '__main__':
    main()
