import importlib.util
from pathlib import Path
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('check_adapter', HERE / 'check_adapter.py')
check_adapter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check_adapter)


class AdapterCheckTests(unittest.TestCase):
    def fixture(self, root):
        (root / 'harness/src').mkdir(parents=True)
        (root / 'harness/src/leaf.rs').write_text('explicit stub')
        for relative in check_adapter.LIVE_MODULES:
            source = root / 'src' / relative
            source.parent.mkdir(parents=True, exist_ok=True)
            source.write_text('current ' + relative)
            stale = root / 'harness/src' / relative
            stale.parent.mkdir(parents=True, exist_ok=True)
            stale.write_text('stale snapshot')

    def test_current_modules_replace_stale_copies_and_keep_stubs(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.fixture(root)
            (root / 'harness/target').mkdir()
            (root / 'harness/target/not-source').write_text('build output')
            destination = root / 'staged'
            check_adapter.stage(root, destination)
            for relative in check_adapter.LIVE_MODULES:
                self.assertEqual((destination / 'src' / relative).read_bytes(),
                                 (root / 'src' / relative).read_bytes())
            self.assertEqual((destination / 'src/leaf.rs').read_text(), 'explicit stub')
            self.assertFalse((destination / 'target').exists())
            self.assertEqual((root / 'harness/src/backend/beeper.rs').read_text(),
                             'stale snapshot')

    def test_missing_active_source_fails_instead_of_using_snapshot(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.fixture(root)
            (root / 'src/backend/beeper.rs').unlink()
            with self.assertRaises(FileNotFoundError):
                check_adapter.stage(root, root / 'staged')


if __name__ == '__main__':
    unittest.main()
