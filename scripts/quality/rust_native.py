#!/usr/bin/env python3
"""Arc-Admin native Rust measurements; never grants quality acceptance."""
import argparse
from datetime import datetime, timezone
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import time
import uuid


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False,
                      allow_nan=False).encode()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def load(path):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError('duplicate JSON key: ' + key)
            result[key] = value
        return result
    return json.loads(Path(path).read_text(), object_pairs_hook=unique)


def write(path, value):
    Path(path).write_bytes(canonical(value) + b'\n')


def command(argv, cwd=None):
    return subprocess.check_output(argv, cwd=cwd, text=True).strip()


def inventory(root):
    return {str(p.relative_to(root)): digest(p.read_bytes())
            for p in sorted((root / 'backend/src').rglob('*.rs')) if p.is_file()}


def native_rows(llvm, source, expected):
    if llvm.get('type') != 'llvm.coverage.json.export' or len(llvm['data']) != 1:
        raise ValueError('unexpected LLVM export schema')
    rows = {}
    for row in llvm['data'][0]['files']:
        path = Path(row['filename'])
        if not path.is_relative_to(source):
            continue
        relative = str(path.relative_to(source))
        if relative not in expected:
            continue
        if relative in rows:
            raise ValueError('duplicate native source: ' + relative)
        values = {}
        for metric in ('lines', 'regions'):
            raw = row['summary'][metric]
            covered, total = raw['covered'], raw['count']
            if type(covered) is not int or type(total) is not int or not 0 <= covered <= total:
                raise ValueError('invalid native counters')
            values[metric] = {'covered': covered, 'total': total}
        rows[relative] = values
    # A source without an LLVM row is retained as missing, never synthesized 0/0.
    return {'files': rows, 'missing': sorted(set(expected) - set(rows))}


def sample(args):
    root, out = args.repository.resolve(), args.output.resolve()
    if out.is_relative_to(root):
        raise ValueError('measurement output must be outside the repository')
    if command(['git', 'diff', 'HEAD', '--', 'backend', 'rust-toolchain.toml'], root):
        raise ValueError('commit backend/toolchain changes before native sampling')
    out.mkdir(parents=True, exist_ok=False)
    sampler_bytes = Path(__file__).read_bytes()
    (out / 'sampler.py').write_bytes(sampler_bytes)
    source = out / 'source'
    source.mkdir()
    commit = command(['git', 'rev-parse', 'HEAD'], root)
    archive = subprocess.check_output(['git', 'archive', commit], cwd=root)
    (out / 'source.tar.gz').write_bytes(gzip.compress(archive, mtime=0))
    with tarfile.open(fileobj=io.BytesIO(archive)) as bundle:
        bundle.extractall(source, filter='data')
    expected = inventory(source)
    if not expected:
        raise ValueError('empty backend source inventory')
    ast_errors = []
    for path in expected:
        result = subprocess.run([str(args.analyzer.resolve()), str(source / path)],
                                text=True, capture_output=True, check=False)
        if result.returncode:
            ast_errors.append({'path': path, 'error': result.stderr.strip()})
    write(out / 'ast-preflight.json', {'files': len(expected), 'errors': ast_errors})
    env = {k: v for k, v in os.environ.items() if k not in
           ('DATABASE_URL', 'TEST_DATABASE_URL', 'RUSTFLAGS', 'LLVM_PROFILE_FILE',
            'CARGO_ENCODED_RUSTFLAGS', 'RUSTUP_TOOLCHAIN') and not k.startswith('GIT_')}
    env.update(CARGO_TARGET_DIR=str(out / 'build'), CARGO_BUILD_JOBS='2')
    llvm_tools = {}
    for name, key in [('llvm-cov', 'LLVM_COV'), ('llvm-profdata', 'LLVM_PROFDATA')]:
        executable = (args.llvm_tools / name).resolve()
        version = command([str(executable), '--version'])
        rust_version = command(['rustc', '-vV'], source).split('LLVM version: ', 1)[1].strip()
        if rust_version not in version:
            raise ValueError('LLVM tool version differs from the pinned Rust compiler')
        env[key] = str(executable)
        llvm_tools[name] = {'version': version, 'sha256': digest(executable.read_bytes())}
    name = 'arc-quality-' + uuid.uuid4().hex[:16]
    run = uuid.uuid4().hex
    started = datetime.now(timezone.utc).isoformat()
    tick = time.monotonic()
    subprocess.run(['docker', 'run', '--detach', '--rm', '--name', name,
                    '-e', 'POSTGRES_USER=arc_admin_test', '-e', 'POSTGRES_PASSWORD=arc_admin_test',
                    '-e', 'POSTGRES_DB=arc_admin_test', '-p', '127.0.0.1::5432',
                    'postgres:16-alpine'], check=True, stdout=subprocess.DEVNULL)
    exit_code = None
    argv = []
    try:
        deadline = time.monotonic() + 45
        while subprocess.run(['docker', 'exec', name, 'pg_isready', '-U', 'arc_admin_test'],
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode:
            if time.monotonic() >= deadline:
                raise RuntimeError('disposable PostgreSQL did not become ready')
            time.sleep(1)
        port = command(['docker', 'port', name, '5432/tcp']).rsplit(':', 1)[1]
        env['TEST_DATABASE_URL'] = f'postgres://arc_admin_test:arc_admin_test@127.0.0.1:{port}/arc_admin_test'
        argv = ['cargo', 'llvm-cov', '--manifest-path', 'backend/Cargo.toml', '--locked',
                '--json', '--output-path', str(out / 'llvm.json')]
        with (out / 'tests.log').open('w') as log:
            exit_code = subprocess.run(argv, cwd=source, env=env, stdout=log,
                                       stderr=subprocess.STDOUT, timeout=1800).returncode
    finally:
        subprocess.run(['docker', 'rm', '--force', name], check=True, stdout=subprocess.DEVNULL)
        write(out / 'execution.json', {'commit': commit, 'run': run, 'started_at': started,
              'exit_code': exit_code, 'argv': argv, 'elapsed_seconds': time.monotonic() - tick,
              'database': 'disposable PostgreSQL 16, loopback only', 'cleanup': 'completed'})
    if exit_code:
        raise RuntimeError('native tests failed; retained log and execution receipt')
    if inventory(source) != expected:
        raise ValueError('native source changed during measurement')
    llvm_bytes = (out / 'llvm.json').read_bytes()
    rows = native_rows(json.loads(llvm_bytes), source, expected)
    raw = {'schema': 'arc-admin-rust-native/v1', 'commit': commit, 'run': run,
           'source_inventory': expected, 'coverage': rows, 'ast_errors': ast_errors,
           'risk_state': 'measurement_error',
           'risk_reason': 'Whole-backend function/LLVM mapping is not certified; no CRAP values emitted.',
           'llvm_sha256': digest(llvm_bytes), 'tests_sha256': digest((out / 'tests.log').read_bytes()),
           'execution_sha256': digest((out / 'execution.json').read_bytes()),
           'tools': {'rustc': command(['rustc', '-vV'], source),
                     'cargo_llvm_cov': command(['cargo', 'llvm-cov', '--version'], source),
                     'llvm': llvm_tools,
                     'analyzer_sha256': digest(args.analyzer.read_bytes()),
                     'collector_sha256': digest(sampler_bytes)}}
    write(out / 'native.json', raw)
    (out / 'llvm.json.gz').write_bytes(gzip.compress(llvm_bytes, mtime=0))
    print(json.dumps({'native_tests': 'pass', 'source_files': len(expected),
                      'measured_files': len(rows['files']), 'missing_files': rows['missing'],
                      'risk': 'measurement_error', 'output': str(out)}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repository', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--analyzer', type=Path, required=True)
    parser.add_argument('--llvm-tools', type=Path, required=True)
    sample(parser.parse_args())


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
