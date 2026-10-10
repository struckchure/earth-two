#!/usr/bin/env python3
"""Inventory actual candidates and compare identical gzip-9 transfer estimates.

Sidecars and TypeScript declarations are excluded from runtime payloads. Rust
assets are requested individually; Go's assets are in raylib.data. Transfer
figures cover the complete asset corpus, not a measured first-page download.
"""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import platform
import subprocess
import zlib


def digest(data):
    return hashlib.sha256(data).hexdigest()


def asset_inventory(root):
    return {str(p.relative_to(root)): {'bytes': p.stat().st_size, 'sha256': digest(p.read_bytes())}
            for p in sorted(root.rglob('*')) if p.is_file()}


def web_inventory(root):
    files = {}
    sidecars = 0
    for p in sorted(root.rglob('*')):
        if not p.is_file():
            continue
        rel = str(p.relative_to(root))
        if p.suffix in ('.gz', '.br'):
            sidecars += p.stat().st_size
            continue
        if p.name.endswith('.d.ts'):
            continue
        data = p.read_bytes()
        category = 'assets' if rel.startswith('assets/') or p.suffix == '.data' else 'runtime'
        files[rel] = {'category': category, 'bytes': len(data),
                      'gzip9_bytes': len(gzip.compress(data, compresslevel=9, mtime=0)),
                      'sha256': digest(data)}
    totals = {c: {k: sum(v[k] for v in files.values() if v['category'] == c)
                  for k in ('bytes', 'gzip9_bytes')} for c in ('runtime', 'assets')}
    return {'files': files, 'totals': totals, 'existing_sidecar_bytes': sidecars,
            'directory_bytes': sum(p.stat().st_size for p in root.rglob('*') if p.is_file())}


def wasm_sections(path):
    data = path.read_bytes()
    if data[:8] != b'\0asm\1\0\0\0':
        raise ValueError(f'not WASM: {path}')
    i = 8
    def leb():
        nonlocal i
        result = shift = 0
        while True:
            byte = data[i]
            i += 1
            result |= (byte & 127) << shift
            if byte < 128:
                return result
            shift += 7
    result = []
    while i < len(data):
        kind = data[i]
        i += 1
        size = leb()
        end = i + size
        if end > len(data):
            raise ValueError('truncated WASM section')
        name = ''
        if kind == 0:
            length = leb()
            name = data[i:i+length].decode('utf8')
        result.append({'id': kind, 'name': name, 'bytes': size})
        i = end
    return result


def native_inventory(path):
    data = path.read_bytes()
    return {'path': str(path), 'bytes': len(data), 'sha256': digest(data),
            'file': subprocess.check_output(['file', str(path)], text=True).strip()}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('baseline', type=Path)
    p.add_argument('--rust-web', type=Path, default=Path('build/rust-web'))
    p.add_argument('--rust-native', type=Path, default=Path('build/rust/release/earth-two-client'))
    p.add_argument('--out', type=Path, required=True)
    a = p.parse_args()
    if a.out.exists():
        p.error('output already exists; preserve prior measurements')
    go_root = a.baseline / 'reference/build'
    go_assets = asset_inventory(go_root / 'assets')
    rust_assets = asset_inventory(a.rust_web / 'assets')
    if not go_assets or not rust_assets:
        p.error('both packed asset trees are required')
    differing = [s for s in sorted(go_assets.keys() | rust_assets.keys()) if go_assets.get(s) != rust_assets.get(s)]
    result = {'platform': platform.platform(), 'compression': {'algorithm': 'gzip', 'level': 9, 'mtime': 0, 'zlib': zlib.ZLIB_VERSION},
              'go_commit': json.loads((a.baseline / 'run.json').read_text())['reference_commit'],
              'rust_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
              'assets': {'identical': not differing, 'differing_paths': differing, 'go_files': len(go_assets), 'rust_files': len(rust_assets)},
              'go_web': web_inventory(go_root / 'web'), 'rust_web': web_inventory(a.rust_web),
              'rust_wasm_sections': wasm_sections(a.rust_web / 'earth_two_client_bg.wasm'),
              'native': {},
              'scope': 'As-built candidates. Full asset corpus; not observed cold-start network transfer. Identical Python gzip-9 estimates are separate from shipping sidecars. Native executable sizes exclude libraries, launcher, installer metadata and assets. No installed-package parity claim.'}
    for name, path in [('go', a.baseline / 'earth-two-reference'), ('rust', a.rust_native)]:
        if path.is_file():
            result['native'][name] = native_inventory(path)
    a.out.parent.mkdir(parents=True, exist_ok=True)
    a.out.write_text(json.dumps(result, indent=2) + '\n')
    print('Packed assets identical:', result['assets']['identical'])
    for name in ('go_web', 'rust_web'):
        print(name, result[name]['totals'])
    print('native', result['native'])


if __name__ == '__main__':
    main()
