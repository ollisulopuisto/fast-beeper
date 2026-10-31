import importlib.util
from pathlib import Path
import unittest
import subprocess
import tempfile
import json
import sys

spec = importlib.util.spec_from_file_location('scope', Path(__file__).with_name('ci-scope.py'))
scope = importlib.util.module_from_spec(spec)
spec.loader.exec_module(scope)


class ScopeTests(unittest.TestCase):
    def test_real_git_diff_includes_deletions_and_literal_odd_filenames(self):
        script = Path(__file__).with_name('ci-scope.py').resolve()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            def git(*args):
                return subprocess.check_output(['git', '-C', tmp, *args], stderr=subprocess.DEVNULL).decode().strip()
            git('init')
            git('config', 'user.email', 'test@example.invalid')
            git('config', 'user.name', 'Test')
            (root / 'src').mkdir()
            file = root / 'src' / 'odd $filename.rs'
            file.write_text('first')
            git('add', '.')
            git('commit', '-m', 'base')
            base = git('rev-parse', 'HEAD')
            file.unlink()
            git('add', '-u')
            git('commit', '-m', 'remove')
            result = subprocess.check_output([sys.executable, str(script), '--base', base], cwd=tmp)
            self.assertEqual(json.loads(result), dict(portable=True, native=True, audit=False, nix=False))
            same = subprocess.check_output([sys.executable, str(script), '--base', 'HEAD'], cwd=tmp)
            self.assertFalse(any(json.loads(same).values()))

    def test_docs_need_no_build(self):
        self.assertFalse(any(scope.classify(['CHECKPOINT.md', 'docs/image.png']).values()))

    def test_python_packaging_is_portable_not_four_native_builds(self):
        self.assertEqual(scope.classify(['contrib/beeper-tools/package_dmg.py']),
                         dict(portable=True, native=False, audit=False, nix=False))

    def test_runtime_keeps_native_without_nix_fanout(self):
        for path in ('src/app.rs', 'assets/icon.png', 'tests/native.rs', 'packaging/macos/bundle.sh'):
            self.assertTrue(scope.classify([path])['native'])
            self.assertTrue(scope.classify([path])['portable'])
            self.assertFalse(scope.classify([path])['nix'])

    def test_dependencies_and_ci_routing_require_all_checks(self):
        for path in ('Cargo.lock', 'Cargo.toml', 'rust-toolchain.toml', 'build.rs', '.github/workflows/ci.yml', '.github/scripts/ci-scope.py'):
            self.assertTrue(all(scope.classify([path]).values()))

    def test_nix_changes_are_checked(self):
        for path in ('flake.lock', 'flake.nix', 'packaging/flatpak/manifest.yml'):
            self.assertTrue(scope.classify([path])['nix'])

    def test_unknown_implementation_fails_open_and_mixed_scope_unions(self):
        self.assertTrue(scope.classify(['new-code/file'])['native'])
        self.assertTrue(scope.classify(['README.md', 'src/app.rs'])['native'])
        self.assertTrue(all(scope.classify([], full=True).values()))


if __name__ == '__main__':
    unittest.main()
