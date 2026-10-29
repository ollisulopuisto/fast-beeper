#!/usr/bin/env python3
"""Select check families from tracked changed paths. Unknown code fails open."""
import argparse
import json
from pathlib import Path
import subprocess


def classify(paths, full=False):
    flags = dict(portable=False, native=False, audit=False, nix=False)
    if full:
        return dict.fromkeys(flags, True)
    for path in paths:
        if path.startswith(('docs/',)) or path.endswith('.md'):
            continue
        if path.startswith('contrib/beeper-tools/'):
            flags['portable'] = True
        elif path in ('flake.nix', 'flake.lock') or path.startswith('packaging/flatpak/'):
            flags['nix'] = True
            flags['native'] = True
        elif path in ('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'build.rs'):
            flags.update(dict.fromkeys(flags, True))
        elif path.startswith(('src/', 'assets/', 'tests/', 'examples/', 'packaging/macos/', 'packaging/windows/')):
            flags['portable'] = flags['native'] = flags['nix'] = True
        elif path.startswith('harness/'):
            flags['portable'] = True
        elif path in ('.github/workflows/ci.yml', '.github/scripts/ci-scope.py', '.github/scripts/test-ci-scope.py', '.github/scripts/test-ci-workflows.py'):
            flags.update(dict.fromkeys(flags, True))
        elif path.startswith('.github/') or path in ('.gitignore', '.gitattributes', 'LICENSE', 'THIRD-PARTY-NOTICES.md'):
            continue
        else:
            # New or unclassified implementation must not silently lose coverage.
            flags['portable'] = flags['native'] = flags['nix'] = True
    return flags


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base')
    parser.add_argument('--head', default='HEAD')
    parser.add_argument('--full', action='store_true')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    if args.full:
        result = classify([], full=True)
    elif not args.base or set(args.base) == {'0'}:
        result = classify([], full=True)
    else:
        # Include deletions/renames; argv, never shell interpolation of filenames.
        data = subprocess.check_output(['git', 'diff', '--name-only', '-z', args.base, args.head])
        result = classify(data.decode().rstrip('\0').split('\0') if data else [])
    print(json.dumps(result, sort_keys=True))
    if args.output:
        with args.output.open('a') as out:
            for name, value in result.items():
                out.write(f'{name}={str(value).lower()}\n')


if __name__ == '__main__':
    main()
