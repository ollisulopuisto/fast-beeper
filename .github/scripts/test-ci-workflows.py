"""Regression tests for literal workflow routing without third-party modules."""
from pathlib import Path
import json
import os
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


def workflow(name):
    return (ROOT / '.github' / 'workflows' / name).read_text()


def run_block(text, marker):
    block = text.split(marker, 1)[1]
    lines = []
    for line in block.splitlines():
        if line and not line.startswith('          '):
            break
        lines.append(line[10:] if line.startswith('          ') else line)
    return '\n'.join(lines)


class WorkflowTests(unittest.TestCase):
    def test_trial_is_explicit_and_selects_only_requested_native_target(self):
        text = workflow('trial-binaries.yml')
        trigger = text.split('on:', 1)[1].split('permissions:', 1)[0]
        self.assertNotIn('pull_request:', trigger)
        self.assertNotIn('push:', trigger)
        self.assertIn('workflow_dispatch:', trigger)
        block = run_block(text, '        run: |\n')
        targets = ['aarch64-apple-darwin', 'x86_64-apple-darwin',
                   'x86_64-unknown-linux-gnu', 'x86_64-pc-windows-msvc']
        for target in targets + ['all', 'invalid']:
            with tempfile.NamedTemporaryFile() as out:
                env = dict(os.environ, TARGET=target, GITHUB_OUTPUT=out.name)
                result = subprocess.run(['bash', '-eu', '-c', block], env=env, capture_output=True)
                if target == 'invalid':
                    self.assertNotEqual(result.returncode, 0)
                else:
                    self.assertEqual(result.returncode, 0, result.stderr)
                    rows = json.loads(Path(out.name).read_text().split('=', 1)[1])['include']
                    self.assertEqual([r['target'] for r in rows], targets if target == 'all' else [target])
        self.assertIn('fromJSON(needs.select.outputs.matrix)', text)
        self.assertIn('FAST_BEEPER_UPLOAD_ARTIFACTS', text)

    def test_ci_selection_modes(self):
        text = workflow('ci.yml')
        block = run_block(text.split('id: scope', 1)[1], '        run: |\n')
        for event, scope, expected in [
            ('schedule', '', dict(portable=False, native=False, audit=True, nix=False)),
            ('workflow_dispatch', 'fast', dict(portable=True, native=False, audit=False, nix=False)),
            ('workflow_dispatch', 'full', dict(portable=True, native=True, audit=True, nix=True)),
            ('merge_group', '', dict(portable=True, native=True, audit=True, nix=True)),
            ('push', '', dict(portable=True, native=True, audit=True, nix=True)),
        ]:
            with tempfile.NamedTemporaryFile() as out:
                env = dict(os.environ, EVENT_NAME=event, REQUESTED_SCOPE=scope,
                           BASE='0' * 40, HEAD='HEAD', GITHUB_OUTPUT=out.name)
                result = subprocess.run(['bash', '-eu', '-c', block], cwd=ROOT, env=env, capture_output=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                actual = dict(line.split('=') for line in Path(out.name).read_text().splitlines())
                self.assertEqual(actual, {k: str(v).lower() for k, v in expected.items()})

    def test_expensive_checks_follow_selection_and_portable_failure(self):
        text = workflow('ci.yml')
        self.assertEqual(text.count('needs: [changes, portable]'), 6)
        self.assertIn('!cancelled() && !failure()', text)
        self.assertIn("vars.FAST_BEEPER_UPLOAD_ARTIFACTS == 'true'", text)
        self.assertIn('cancel-in-progress: true', workflow('flatpak.yml'))
        self.assertIn("upload-artifact: ${{ vars.FAST_BEEPER_UPLOAD_ARTIFACTS == 'true' }}", workflow('flatpak.yml'))


if __name__ == '__main__':
    unittest.main()
