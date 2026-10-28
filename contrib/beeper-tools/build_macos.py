#!/usr/bin/env python3
"""Check or build a local Mac trial bundle. No installs, account changes or publication."""
import argparse
import hashlib
import json
import pathlib
import platform
import shutil
import subprocess
import sys


def plan(root, machine):
    targets = {'arm64': 'aarch64-apple-darwin', 'x86_64': 'x86_64-apple-darwin'}
    if machine not in targets:
        raise ValueError('Unsupported Mac architecture')
    target = targets[machine]
    return {'target': target, 'build': ['cargo', 'build', '--locked', '--release', '--bin', 'zapfast', '--target', target, '--target-dir', str(root / 'target')],
            'binary': root / 'target' / target / 'release' / 'zapfast'}


def build(root, destination, spec):
    # Refuse existing destinations before running a build; never overwrite a trial.
    if destination.exists():
        raise ValueError('Output already exists; choose a new directory')
    subprocess.run(spec['build'], cwd=root, check=True)
    if not spec['binary'].is_file():
        raise ValueError('Build returned without the expected binary')
    # Assemble separately, then rename only a complete source-backed trial.
    import tempfile
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.zapfast-trial-', dir=destination.parent) as tmp:
        stage = pathlib.Path(tmp) / 'trial'
        stage.mkdir()
        shutil.copy2(spec['binary'], stage / 'zapfast')
        scripts = stage / 'contrib' / 'beeper-tools'
        scripts.mkdir(parents=True)
        for name in ('run_client.py', 'check_endpoint.py'):
            shutil.copy2(root / 'contrib' / 'beeper-tools' / name, scripts / name)
        for name in ('BACKEND.md', 'HEADLESS-SETUP.md', 'MAC-TRIAL.md', 'LICENSE', 'THIRD-PARTY-NOTICES.md'):
            shutil.copy2(root / name, stage / name)
        (stage / 'build-info.json').write_text(json.dumps({'architecture': spec['target'], 'native_runtime_tested': False,
            'server_bundled': False, 'signing_notarization_verified': False}, indent=2) + '\n')
        manifest = {str(path.relative_to(stage)): hashlib.sha256(path.read_bytes()).hexdigest()
                    for path in sorted(stage.rglob('*')) if path.is_file()}
        (stage / 'checksums.json').write_text(json.dumps(manifest, indent=2) + '\n')
        stage.rename(destination)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--build', action='store_true', help='Compile and assemble trial; default only checks prerequisites')
    parser.add_argument('--out', type=pathlib.Path, help='New output directory, required with --build')
    args = parser.parse_args()
    if platform.system() != 'Darwin':
        print('This helper runs on macOS only. No build, install or changes performed.', file=sys.stderr)
        return 1
    root = pathlib.Path(__file__).resolve().parents[2]
    try:
        spec = plan(root, platform.machine())
        for executable in ('cargo', 'rustc', 'xcrun'):
            if not shutil.which(executable):
                raise ValueError('Missing prerequisite: ' + executable + '. Install it separately; this helper installs nothing.')
        subprocess.run(['xcrun', '--find', 'clang'], check=True, stdout=subprocess.DEVNULL)
        if args.build:
            if args.out is None:
                raise ValueError('--build requires --out NEW_DIRECTORY')
            build(root, args.out.expanduser().resolve(), spec)
            print('Local trial assembled. Native launch, Keychain, Server, sign-in and bridge acceptance still required.')
        else:
            print('Prerequisite commands found for ' + spec['target'] + '. No compile/runtime/dependency compatibility claim.')
            print('No server, login, account, service or network configuration was changed.')
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print('Mac trial preparation failed: ' + str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
