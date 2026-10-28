#!/usr/bin/env python3
"""Package a compiled trial binary. No builds, credentials or publication."""
import argparse
import hashlib
import json
import pathlib
import shutil
import tempfile
import zipfile

TARGETS = {'aarch64-apple-darwin', 'x86_64-apple-darwin',
           'x86_64-unknown-linux-gnu', 'x86_64-pc-windows-msvc'}


def package(root, binary, output, target, commit):
    if target not in TARGETS or len(commit) != 40 or any(c not in '0123456789abcdef' for c in commit):
        raise ValueError('Expected a supported target and exact commit SHA')
    if not binary.is_file():
        raise ValueError('Compiled binary missing')
    output.mkdir(parents=True, exist_ok=True)
    name = f'fast-beeper-{target}-{commit[:12]}'
    archive = output / (name + '.zip')
    if archive.exists():
        raise FileExistsError('Archive already exists; use a clean output directory')
    with tempfile.TemporaryDirectory() as tmp:
        stage = pathlib.Path(tmp) / name
        stage.mkdir()
        destination = stage / ('zapfast.exe' if 'windows' in target else 'zapfast')
        shutil.copy2(binary, destination)
        if 'windows' not in target:
            destination.chmod(0o755)
        tools = stage / 'contrib' / 'beeper-tools'
        tools.mkdir(parents=True)
        for file in ('run_client.py', 'check_endpoint.py'):
            shutil.copy2(root / 'contrib' / 'beeper-tools' / file, tools / file)
        for file in ('LICENSE', 'THIRD-PARTY-NOTICES.md', 'BACKEND.md', 'HEADLESS-SETUP.md'):
            shutil.copy2(root / file, stage / file)
        shutil.copy2(root / 'BINARY-BUILDS.md', stage / 'README.md')
        (stage / 'build-info.json').write_text(json.dumps({
            'commit': commit, 'target': target, 'server_bundled': False,
            'native_runtime_tested': False, 'signed_notarized': False,
        }, indent=2) + '\n')
        manifest = {str(p.relative_to(stage)).replace('\\', '/'):
                    hashlib.sha256(p.read_bytes()).hexdigest()
                    for p in sorted(stage.rglob('*')) if p.is_file()}
        (stage / 'checksums.json').write_text(json.dumps(manifest, indent=2) + '\n')
        with zipfile.ZipFile(archive, 'x', compression=zipfile.ZIP_DEFLATED) as z:
            for path in sorted(stage.rglob('*')):
                if path.is_file():
                    # Preserve runnable POSIX permissions even when tests/package
                    # creation happen on Windows, where chmod is limited.
                    info = zipfile.ZipInfo.from_file(path, str(path.relative_to(stage.parent)))
                    info.create_system = 3
                    mode = 0o755 if path == destination and 'windows' not in target else 0o644
                    info.external_attr = (0o100000 | mode) << 16
                    info.compress_type = zipfile.ZIP_DEFLATED
                    z.writestr(info, path.read_bytes())
    archive.with_suffix('.zip.sha256').write_text(hashlib.sha256(archive.read_bytes()).hexdigest() + '  ' + archive.name + '\n')
    return archive


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=pathlib.Path)
    parser.add_argument('--output', required=True, type=pathlib.Path)
    parser.add_argument('--target', required=True, choices=sorted(TARGETS))
    parser.add_argument('--commit', required=True)
    args = parser.parse_args()
    root = pathlib.Path(__file__).resolve().parents[2]
    print(package(root, args.binary, args.output, args.target, args.commit))


if __name__ == '__main__':
    main()
