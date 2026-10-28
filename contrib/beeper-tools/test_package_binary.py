import importlib.util
import hashlib
import json
import pathlib
import tempfile
import unittest
import zipfile

HERE = pathlib.Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('package_binary', HERE / 'package_binary.py')
package_binary = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package_binary)


class PackageTests(unittest.TestCase):
    def test_complete_portable_archive_and_checksums(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            binary = root / 'zapfast'
            binary.write_bytes(b'fixture executable')
            binary.chmod(0o755)
            archive = package_binary.package(HERE.parents[1], binary, root / 'dist',
                                            'aarch64-apple-darwin', 'a' * 40)
            with zipfile.ZipFile(archive) as z:
                names = z.namelist()
                prefix = names[0].split('/')[0] + '/'
                self.assertIn(prefix + 'zapfast', names)
                self.assertIn(prefix + 'contrib/beeper-tools/run_client.py', names)
                metadata = json.loads(z.read(prefix + 'build-info.json'))
                self.assertFalse(metadata['server_bundled'])
                self.assertFalse(metadata['native_runtime_tested'])
                self.assertEqual(metadata['commit'], 'a' * 40)
                manifest = json.loads(z.read(prefix + 'checksums.json'))
                self.assertIn('zapfast', manifest)
                for relative, digest in manifest.items():
                    self.assertEqual(hashlib.sha256(z.read(prefix + relative)).hexdigest(), digest)
                self.assertEqual(archive.with_suffix('.zip.sha256').read_text().split()[0],
                                 hashlib.sha256(archive.read_bytes()).hexdigest())
                self.assertEqual((z.getinfo(prefix + 'zapfast').external_attr >> 16) & 0o777, 0o755)
                self.assertFalse(any('token' in name.lower() for name in names))

    def test_rejects_unknown_target_missing_binary_and_existing_output(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            binary = root / 'zapfast'
            with self.assertRaises(ValueError):
                package_binary.package(HERE.parents[1], binary, root / 'dist', 'unknown', 'a' * 40)
            with self.assertRaises(ValueError):
                package_binary.package(HERE.parents[1], binary, root / 'dist', 'x86_64-pc-windows-msvc', 'a' * 40)
            binary.write_bytes(b'fixture')
            archive = package_binary.package(HERE.parents[1], binary, root / 'dist', 'x86_64-pc-windows-msvc', 'a' * 40)
            with zipfile.ZipFile(archive) as z:
                self.assertTrue(any(name.endswith('/zapfast.exe') for name in z.namelist()))
            with self.assertRaises(FileExistsError):
                package_binary.package(HERE.parents[1], binary, root / 'dist', 'x86_64-pc-windows-msvc', 'a' * 40)


if __name__ == '__main__':
    unittest.main()
