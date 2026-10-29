#!/usr/bin/env python3
"""Create a local unsigned trial DMG. No build, upload, install or account changes."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import platform
import plistlib
import re
import struct
import subprocess
import tempfile
import zipfile

spec = importlib.util.spec_from_file_location('package_binary', Path(__file__).with_name('package_binary.py'))
package_binary = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package_binary)

TARGET_CPUS = {'aarch64-apple-darwin': 0x0100000C, 'x86_64-apple-darwin': 0x01000007}


def validate_binary(binary, target):
    if target not in TARGET_CPUS:
        raise ValueError('Expected a single supported Mac target')
    with binary.open('rb') as stream:
        header = stream.read(32)
    if len(header) != 32 or header[:4] != b'\xcf\xfa\xed\xfe':
        raise ValueError('Expected a little-endian 64-bit Mach-O executable, not a fixture or universal binary')
    _, cpu, _, kind = struct.unpack('<IIII', header[:16])
    if cpu != TARGET_CPUS[target] or kind != 2:
        raise ValueError('Mach-O architecture/executable type does not match target')


def verify_mounted(image, expected_name, target):
    """Read-only mount and verify the actual image payload; always detach."""
    with tempfile.TemporaryDirectory(prefix='.fast-beeper-verify-') as tmp:
        mount = Path(tmp) / 'volume'
        result = subprocess.run(
            ['hdiutil', 'attach', '-readonly', '-nobrowse', '-plist',
             '-mountpoint', str(mount), str(image)],
            check=True, stdout=subprocess.PIPE)
        # An attach succeeded, so cleanup is required even for malformed output.
        try:
            entities = plistlib.loads(result.stdout).get('system-entities', [])
            if not any(item.get('mount-point') == str(mount) for item in entities):
                raise ValueError('DMG did not mount at the expected location')
            trial = mount / expected_name
            if sorted(p.name for p in mount.iterdir()) != [expected_name] or not trial.is_dir():
                raise ValueError('Unexpected disk image root payload')
            paths = list(trial.rglob('*'))
            if any(p.is_symlink() for p in paths):
                raise ValueError('Disk image payload must not contain symlinks')
            manifest = json.loads((trial / 'checksums.json').read_text())
            actual = {p.relative_to(trial).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
                      for p in paths if p.is_file() and p.name != 'checksums.json'}
            if manifest != actual:
                raise ValueError('Mounted disk image checksums or file set differ')
            validate_binary(trial / 'zapfast', target)
            info = json.loads((trial / 'build-info.json').read_text())
            if info.get('target') != target or info.get('installable_app') is not False:
                raise ValueError('Mounted disk image metadata differs')
        finally:
            subprocess.run(['hdiutil', 'detach', str(mount)], check=True)


def create(root, binary, output, target, commit, version):
    if platform.system() != 'Darwin':
        raise ValueError('DMG creation requires macOS hdiutil; nothing created')
    if not re.fullmatch(r'[0-9]+(?:\.[0-9]+){1,3}(?:-[a-zA-Z0-9.]+)?', version):
        raise ValueError('Expected a numeric release version, optionally with a prerelease suffix')
    if output.exists() or output.is_symlink():
        raise FileExistsError('Output exists; choose a new directory')
    validate_binary(binary, target)
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.fast-beeper-dmg-', dir=output.parent) as tmp:
        tmp = Path(tmp)
        stage = tmp / 'publish'
        stage.mkdir()
        archive = package_binary.package(root, binary, tmp / 'portable', target, commit)
        payload = tmp / 'payload'
        payload.mkdir()
        with zipfile.ZipFile(archive) as bundle:
            bundle.extractall(payload)
        trial = next(payload.iterdir())
        (trial / 'zapfast').chmod(0o755)
        (trial / 'TRIAL-DMG.txt').write_text(
            'Fast-beeper ' + version + '\n\n'
            'Unsigned, unnotarized development trial. Not an installable .app.\n'
            'Copy the entire trial directory to a writable location before running.\n'
            'Use the Python Beeper launcher in README.md, not the bare binary.\n'
            'Python 3 or an existing uv installation is required. Nothing installs it.\n'
            'An existing running Desktop or Server endpoint is required. Local Server sign-in is available with --sign-in.\n'
            'Server email-code and recovery-key entry stay inside the app with --sign-in. The launcher defaults to read-only.\n'
            'No Server, credentials, account data or profile is bundled.\n'
            'No runtime, signing, notarization or minimum macOS compatibility acceptance.\n')
        info = json.loads((trial / 'build-info.json').read_text())
        info.update(version=version, package_kind='unsigned-trial-dmg', installable_app=False)
        (trial / 'build-info.json').write_text(json.dumps(info, indent=2) + '\n')
        hashes = {p.relative_to(trial).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
                  for p in sorted(trial.rglob('*')) if p.is_file() and p.name != 'checksums.json'}
        (trial / 'checksums.json').write_text(json.dumps(hashes, indent=2) + '\n')
        name = f'fast-beeper-{version}-{target}-{commit[:12]}.dmg'
        image = stage / name
        subprocess.run(['hdiutil', 'create', '-format', 'UDZO', '-fs', 'HFS+',
                        '-volname', 'Fast Beeper Trial', '-srcfolder', str(payload), str(image)],
                       check=True)
        if not image.is_file() or not image.stat().st_size:
            raise ValueError('hdiutil returned without a nonempty DMG')
        subprocess.run(['hdiutil', 'verify', str(image)], check=True)
        verify_mounted(image, trial.name, target)
        (stage / (name + '.sha256')).write_text(hashlib.sha256(image.read_bytes()).hexdigest() + '  ' + name + '\n')
        stage.rename(output)
    return output / name


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path, help='New directory, never overwritten')
    parser.add_argument('--target', required=True, choices=sorted(TARGET_CPUS))
    parser.add_argument('--commit', required=True)
    parser.add_argument('--version', required=True)
    args = parser.parse_args()
    try:
        print(create(Path(__file__).resolve().parents[2], args.binary, args.output,
                     args.target, args.commit, args.version))
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, 'DMG preparation failed: ' + str(error) + '\n')


if __name__ == '__main__':
    main()
