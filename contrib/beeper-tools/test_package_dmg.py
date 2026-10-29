import hashlib
import importlib.util
import json
from pathlib import Path
import struct
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location('package_dmg', Path(__file__).with_name('package_dmg.py'))
dmg = importlib.util.module_from_spec(spec)
spec.loader.exec_module(dmg)


class DmgTests(unittest.TestCase):
    def fixture(self, root):
        binary = root / 'binary'
        binary.write_bytes(struct.pack('<IIIIIIII', 0xFEEDFACF, 0x0100000C, 0, 2, 0, 0, 0, 0))
        scripts = root / 'contrib/beeper-tools'
        scripts.mkdir(parents=True)
        for name in ('run_client.py', 'check_endpoint.py'):
            (scripts / name).write_text('fixture')
        for name in ('LICENSE', 'THIRD-PARTY-NOTICES.md', 'BACKEND.md', 'HEADLESS-SETUP.md', 'BINARY-BUILDS.md'):
            (root / name).write_text('fixture')
        return binary

    def call(self, root, binary, **kwargs):
        return dmg.create(root, binary, root / 'out', kwargs.get('target', 'aarch64-apple-darwin'),
                          kwargs.get('commit', 'a' * 40), kwargs.get('version', '2026.10.29.1-trial'))

    def test_non_mac_does_not_run_or_create_output(self):
        with tempfile.TemporaryDirectory() as tmp, mock.patch.object(dmg.platform, 'system', return_value='Linux'), mock.patch.object(dmg.subprocess, 'run') as run:
            root = Path(tmp)
            with self.assertRaises(ValueError):
                self.call(root, self.fixture(root))
            run.assert_not_called()
            self.assertFalse((root / 'out').exists())

    def test_architecture_type_and_version_are_checked(self):
        with tempfile.TemporaryDirectory() as tmp, mock.patch.object(dmg.platform, 'system', return_value='Darwin'), mock.patch.object(dmg.subprocess, 'run') as run:
            root = Path(tmp)
            binary = self.fixture(root)
            for kwargs in ({'target': 'x86_64-apple-darwin'}, {'target': 'linux'}, {'version': '../bad'}, {'commit': 'bad'}):
                with self.assertRaises(ValueError):
                    self.call(root, binary, **kwargs)
            binary.write_text('not a native binary')
            with self.assertRaises(ValueError):
                self.call(root, binary)
            run.assert_not_called()

    def test_existing_output_is_preserved(self):
        with tempfile.TemporaryDirectory() as tmp, mock.patch.object(dmg.platform, 'system', return_value='Darwin'), mock.patch.object(dmg.subprocess, 'run') as run:
            root = Path(tmp)
            binary = self.fixture(root)
            (root / 'out').mkdir()
            (root / 'out/keep').write_text('keep')
            with self.assertRaises(FileExistsError):
                self.call(root, binary)
            self.assertEqual((root / 'out/keep').read_text(), 'keep')
            run.assert_not_called()

    def test_packaging_plan_and_payload_checksums(self):
        with tempfile.TemporaryDirectory() as tmp, mock.patch.object(dmg.platform, 'system', return_value='Darwin'):
            root = Path(tmp)
            binary = self.fixture(root)
            before = binary.read_bytes()
            calls = []
            def run(command, check):
                calls.append(command)
                if command[1] == 'create':
                    payload = Path(command[command.index('-srcfolder') + 1])
                    trial = next(payload.iterdir())
                    info = json.loads((trial / 'build-info.json').read_text())
                    self.assertFalse(info['installable_app'])
                    self.assertFalse(info['signed_notarized'])
                    self.assertFalse(info['native_runtime_tested'])
                    self.assertIn('not the bare binary', (trial / 'TRIAL-DMG.txt').read_text())
                    hashes = json.loads((trial / 'checksums.json').read_text())
                    for relative, digest in hashes.items():
                        self.assertEqual(hashlib.sha256((trial / relative).read_bytes()).hexdigest(), digest)
                    self.assertEqual((trial / 'zapfast').stat().st_mode & 0o777, 0o755)
                    Path(command[-1]).write_bytes(b'mocked image, not a real DMG')
            with mock.patch.object(dmg.subprocess, 'run', side_effect=run):
                image = self.call(root, binary)
            self.assertEqual([c[1] for c in calls], ['create', 'verify'])
            self.assertTrue(image.exists())
            self.assertIn(hashlib.sha256(image.read_bytes()).hexdigest(), image.with_suffix('.dmg.sha256').read_text())
            self.assertEqual(binary.read_bytes(), before)

    def test_failure_and_empty_image_do_not_publish(self):
        for mode in ('fail', 'empty', 'verify-fail'):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as tmp, mock.patch.object(dmg.platform, 'system', return_value='Darwin'):
                root = Path(tmp)
                binary = self.fixture(root)
                def run(command, check):
                    if mode == 'fail' or (mode == 'verify-fail' and command[1] == 'verify'):
                        raise OSError('fixture hdiutil failure')
                    if mode == 'verify-fail':
                        Path(command[-1]).write_bytes(b'mocked image')
                with mock.patch.object(dmg.subprocess, 'run', side_effect=run):
                    with self.assertRaises((OSError, ValueError)):
                        self.call(root, binary)
                self.assertFalse((root / 'out').exists())


if __name__ == '__main__':
    unittest.main()
