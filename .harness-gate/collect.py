#!/usr/bin/python3
"""Project-owned replay transport for host-reviewed native measurement bundles.

The signed request pins the bundle. Its original capture context is retained in
raw artifacts; no native measurement or policy decision is made by this bridge.
"""
import argparse
import hashlib
import json
from pathlib import Path
import sys


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def regular(root, name):
    path = Path(name)
    if path.is_absolute() or not path.parts or any(p in ('.', '..') for p in path.parts):
        raise ValueError('unsafe bundle path')
    current = root
    for part in path.parts:
        current /= part
        if current.is_symlink():
            raise ValueError('symlink in measurement bundle')
    if not current.is_file():
        raise ValueError('missing measurement input: ' + name)
    return current


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--bundle', type=Path, required=True)
    parser.add_argument('--sha256', required=True)
    args = parser.parse_args()
    if args.bundle.is_symlink() or sha(args.bundle) != args.sha256:
        raise ValueError('native bundle digest mismatch')
    bundle = json.loads(args.bundle.read_text())
    request = json.load(sys.stdin)
    if bundle['schema'] != 'arc-native-replay/v1':
        raise ValueError('unknown native replay schema')
    if request['input'] != bundle['input'] or request['config_digest'] != bundle['config_digest']:
        raise ValueError('native replay binding mismatch')
    if request['invocation_id'] != bundle['response']['invocation_id']:
        raise ValueError('stale native replay invocation')
    workspace = Path(request['input']['workspace_root'])
    for name, digest in bundle['sources'].items():
        if sha(regular(workspace, name)) != digest:
            raise ValueError('native source/configuration changed: ' + name)
    copies = []
    for item in bundle['response']['artifacts']:
        source = regular(args.bundle.parent, item['path'])
        if sha(source) != item['sha256'] or source.stat().st_size != item['bytes']:
            raise ValueError('native artifact digest mismatch')
        destination = Path(request['artifact_root']) / item['path']
        if destination.exists() or destination.is_symlink():
            raise ValueError('native artifact destination is not fresh')
        copies.append((source, destination))
    for source, destination in copies:
        destination.parent.mkdir(parents=True, exist_ok=True)
        with destination.open('xb') as stream:
            stream.write(source.read_bytes())
    json.dump(bundle['response'], sys.stdout, separators=(',', ':'))


if __name__ == '__main__':
    main()
