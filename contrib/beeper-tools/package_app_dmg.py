#!/usr/bin/env python3
"""Build an unsigned native Finder app/DMG from an existing native Mac binary."""
import argparse
import hashlib
import json
import platform
import plistlib
import re
import subprocess
import tempfile
from pathlib import Path


def assemble(root, binary, app, version):
    if platform.system() != 'Darwin':
        raise ValueError('Native app assembly requires macOS')
    if not re.fullmatch(r'[0-9]+(?:\.[0-9]+){1,3}', version):
        raise ValueError('Expected numeric app version')
    if app.exists() or app.is_symlink():
        raise FileExistsError('App output exists')
    subprocess.run(['bash', str(root / 'packaging/macos/bundle.sh'),
                    str(binary.resolve()), str(app.resolve()), version, 'beeper'], check=True)
    info = plistlib.loads((app / 'Contents/Info.plist').read_bytes())
    if (info.get('CFBundleIdentifier') != 'fi.sulopuisto.fast-beeper'
            or info.get('CFBundleExecutable') != 'zapfast'
            or info.get('CFBundleVersion') != version
            or not (app / 'Contents/Resources/zapfast.icns').is_file()
            or not (app / 'Contents/MacOS/zapfast-client').is_file()):
        raise ValueError('Beeper app metadata or payload did not match')


def create(root, binary, output, version, commit):
    if output.exists() or output.is_symlink():
        raise FileExistsError('Output exists')
    if not re.fullmatch(r'[0-9a-f]{40}', commit):
        raise ValueError('Expected exact source commit')
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=output.parent) as temporary:
        stage = Path(temporary)
        payload = stage / 'payload'
        payload.mkdir()
        assemble(root, binary, payload / 'ZapFast Beeper.app', version)
        (payload / 'Applications').symlink_to('/Applications')
        (payload / 'READ-ME.txt').write_text(
            'Drag ZapFast Beeper.app to Applications, then open it.\n'
            'Unsigned/ad-hoc signed tester build, not notarized.\n'
            'The first-run window needs your existing Server Mac SSH host and username.\n'
            'Existing SSH key/host trust is required. Close the old manual port23374 tunnel first.\n'
            'No passwords or API credentials are bundled or saved by the launcher.\n'
            'Server stays on the other Mac; no install, network relink or account reset.\n'
            'Choose write mode explicitly; it defaults to read-only.\n')
        publish = stage / 'publish'
        publish.mkdir()
        image = publish / f'ZapFast-Beeper-{version}-{commit[:12]}.dmg'
        subprocess.run(['hdiutil', 'create', '-format', 'UDZO', '-volname',
                        'ZapFast Beeper', '-srcfolder', str(payload), str(image)], check=True)
        subprocess.run(['hdiutil', 'verify', str(image)], check=True)
        mount = stage / 'mounted'
        subprocess.run(['hdiutil', 'attach', '-readonly', '-nobrowse', '-mountpoint',
                        str(mount), str(image)], check=True)
        try:
            app = mount / 'ZapFast Beeper.app'
            if not (app / 'Contents/Resources/zapfast.icns').is_file():
                raise ValueError('Mounted app icon missing')
            subprocess.run(['codesign', '--verify', '--strict', '--deep', str(app)], check=True)
            subprocess.run(['lipo', str(app / 'Contents/MacOS/zapfast-client'), '-verify_arch', 'arm64'], check=True)
        finally:
            subprocess.run(['hdiutil', 'detach', str(mount)], check=True)
        (publish / (image.name + '.sha256')).write_text(
            hashlib.sha256(image.read_bytes()).hexdigest() + '  ' + image.name + '\n')
        (publish / 'build-info.json').write_text(json.dumps({
            'commit': commit, 'version': version, 'installable_app': True,
            'signed': 'ad-hoc', 'notarized': False, 'native_runtime_verified': False,
            'server_bundled': False,
        }, indent=2) + '\n')
        publish.rename(output)
    return output


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--commit', required=True)
    args = parser.parse_args()
    create(Path(__file__).resolve().parents[2], args.binary, args.output, args.version, args.commit)
