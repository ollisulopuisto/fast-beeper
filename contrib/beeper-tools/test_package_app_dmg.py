import importlib.util
import plistlib
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('package_app_dmg', Path(__file__).with_name('package_app_dmg.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class FinderAppTests(unittest.TestCase):
    def test_native_assembly_uses_real_bundle_and_beeper_mode(self):
        with tempfile.TemporaryDirectory() as directory:
            app = Path(directory) / 'ZapFast Beeper.app'
            def run(command, **kwargs):
                self.assertEqual(command[-1], 'beeper')
                self.assertEqual(command[1], str(ROOT / 'packaging/macos/bundle.sh'))
                (app / 'Contents/MacOS').mkdir(parents=True)
                (app / 'Contents/Resources').mkdir()
                (app / 'Contents/Resources/zapfast.icns').write_bytes(b'fixture')
                (app / 'Contents/MacOS/zapfast-client').write_bytes(b'fixture')
                (app / 'Contents/Info.plist').write_bytes(plistlib.dumps({
                    'CFBundleIdentifier': 'fi.sulopuisto.fast-beeper',
                    'CFBundleExecutable': 'zapfast', 'CFBundleVersion': '2026.10.30.1'}))
            with patch.object(module.platform, 'system', return_value='Darwin'), patch.object(module.subprocess, 'run', side_effect=run):
                module.assemble(ROOT, Path('/tmp/native-zapfast'), app, '2026.10.30.1')

    def test_non_mac_does_not_fake_native_app(self):
        with patch.object(module.platform, 'system', return_value='Linux'), patch.object(module.subprocess, 'run') as run:
            with self.assertRaises(ValueError):
                module.assemble(ROOT, Path('/binary'), Path('/new.app'), '1.0')
            run.assert_not_called()

    def test_native_launcher_is_not_a_terminal_or_shell_wrapper(self):
        source = (ROOT / 'packaging/macos/BeeperLauncher.swift').read_text()
        for forbidden in ['Terminal.app', '/bin/bash', '/bin/sh', 'getpass', 'UserDefaults.standard.set(token']:
            self.assertNotIn(forbidden, source)
        for required in ['BatchMode=yes', 'StrictHostKeyChecking=yes', 'ExitOnForwardFailure=yes',
                         '127.0.0.1:23374:127.0.0.1:23374', 'target["id"] as? String == "server"',
                         'env["ZAPFAST_BACKEND"] = "beeper"', 'env["BEEPER_SIGN_IN"] = "0"',
                         '.zapfast-beeper-prototype', 'defer { stop(tunnel) }']:
            self.assertIn(required, source)
        self.assertIn('let host: String', source)
        connection = source.split('struct Connection: Codable {', 1)[1].split('}', 1)[0]
        self.assertNotIn('token', connection)

    def test_original_icon_and_non_terminal_bundle_identity(self):
        info = plistlib.loads((ROOT / 'packaging/macos/Info.plist').read_bytes())
        self.assertEqual(info['CFBundlePackageType'], 'APPL')
        self.assertEqual(info['CFBundleIconFile'], 'zapfast')
        self.assertGreater((ROOT / 'packaging/macos/icon-1024.png').stat().st_size, 10000)
