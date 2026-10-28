import contextlib
import importlib.util
import io
import pathlib
import tempfile
import unittest
from unittest import mock
spec = importlib.util.spec_from_file_location('mac_build', pathlib.Path(__file__).with_name('build_macos.py'))
mac_build = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mac_build)

class MacBuildTests(unittest.TestCase):
    def test_architecture_plan_is_locked_and_explicit(self):
        for machine, target in [('arm64','aarch64-apple-darwin'), ('x86_64','x86_64-apple-darwin')]:
            plan = mac_build.plan(pathlib.Path('/fixture'),machine)
            self.assertEqual(plan['target'],target)
            self.assertIn('--locked',plan['build'])
            self.assertEqual(plan['build'][plan['build'].index('--target-dir')+1],'/fixture/target')
        with self.assertRaises(ValueError): mac_build.plan(pathlib.Path('/fixture'),'unknown')

    def test_existing_output_refused_before_build(self):
        with tempfile.TemporaryDirectory() as tmp, mock.patch.object(mac_build.subprocess,'run') as run:
            root = pathlib.Path(tmp)
            with self.assertRaises(ValueError): mac_build.build(root,root,mac_build.plan(root,'arm64'))
            run.assert_not_called()

    def test_non_mac_never_builds(self):
        with mock.patch('sys.argv',['build_macos.py','--build','--out','unused']), mock.patch.object(mac_build.platform,'system',return_value='Linux'), mock.patch.object(mac_build.subprocess,'run') as run, contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(mac_build.main(),1)
            run.assert_not_called()

    def test_missing_binary_does_not_publish_partial_trial(self):
        with tempfile.TemporaryDirectory() as tmp, mock.patch.object(mac_build.subprocess,'run'):
            root=pathlib.Path(tmp); out=root/'trial'
            with self.assertRaises(ValueError):mac_build.build(root,out,mac_build.plan(root,'arm64'))
            self.assertFalse(out.exists())

    def test_successful_assembly_preserves_launcher_root_and_honest_status(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.check_assembly(pathlib.Path(tmp))

    def test_assembly_through_symlinked_parent_keeps_the_same_launcher_root(self):
        with tempfile.TemporaryDirectory() as tmp:
            base=pathlib.Path(tmp); real=base/'real';real.mkdir();alias=base/'alias'
            try:
                alias.symlink_to(real, target_is_directory=True)
            except OSError as error:
                self.skipTest(f'Directory symlinks unavailable: {error}')
            self.check_assembly(alias)

    def check_assembly(self, parent):
        import json
        with mock.patch.object(mac_build.subprocess,'run'):
            root=parent/'source';root.mkdir();out=parent/'trial'
            spec=mac_build.plan(root,'arm64');spec['binary'].parent.mkdir(parents=True);spec['binary'].write_text('fixture only')
            scripts=root/'contrib'/'beeper-tools';scripts.mkdir(parents=True)
            for name in ('run_client.py','check_endpoint.py'):(scripts/name).write_text('fixture')
            for name in ('BACKEND.md','HEADLESS-SETUP.md','MAC-TRIAL.md','LICENSE','THIRD-PARTY-NOTICES.md'):(root/name).write_text('fixture')
            mac_build.build(root,out,spec)
            self.assertEqual((out/'contrib'/'beeper-tools'/'run_client.py').resolve().parents[2],out.resolve())
            info=json.loads((out/'build-info.json').read_text())
            self.assertFalse(info['native_runtime_tested']);self.assertFalse(info['server_bundled'])
            import hashlib
            hashes=json.loads((out/'checksums.json').read_text())
            self.assertEqual(hashes['zapfast'],hashlib.sha256((out/'zapfast').read_bytes()).hexdigest())
            self.assertIn('contrib/beeper-tools/run_client.py',hashes)
