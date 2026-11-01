"""Execute the shipped health script and extracted installer shell, no live SSH."""
import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = (ROOT / 'packaging/macos/BeeperLauncher.swift').read_text()
HEALTH = ROOT / 'packaging/macos/scheduler-health.sh'


class SetupExecutionTests(unittest.TestCase):
    def sandbox(self, root):
        home = root / 'home'
        home.mkdir()
        bin_dir = root / 'bin'
        bin_dir.mkdir()
        return home, bin_dir, dict(os.environ, HOME=str(home), PATH=str(bin_dir) + ':' + os.environ['PATH'])

    def test_health_token_is_file_only_and_cleanup_always_runs(self):
        for fails in (False, True):
            with self.subTest(fails=fails), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                home, bin_dir, env = self.sandbox(root)
                token = home / 'Library/Application Support/zapfast/scheduler-token'
                token.parent.mkdir(parents=True)
                token.write_text('FAKE-TEST-CREDENTIAL')
                env['CAPTURE'] = str(root / 'argv')
                env['FAIL_CURL'] = '1' if fails else '0'
                curl = bin_dir / 'curl'
                curl.write_text('''#!/usr/bin/env python3
import os, sys
from pathlib import Path
Path(os.environ['CAPTURE']).write_text(repr(sys.argv))
header = next(a[1:] for a in sys.argv if a.startswith('@'))
assert Path(header).read_text() == 'Authorization: Bearer FAKE-TEST-CREDENTIAL\\n'
assert Path(header).stat().st_mode & 0o777 == 0o600
if os.environ['FAIL_CURL'] == '1': sys.exit(7)
print('{"ok":true}')
''')
                curl.chmod(0o755)
                result = subprocess.run(['sh', '-c', HEALTH.read_text().strip() + ' && echo HEALTHY'], env=env, capture_output=True)
                self.assertEqual(result.returncode == 0, not fails)
                argv = (root / 'argv').read_text()
                self.assertNotIn('FAKE-TEST-CREDENTIAL', argv)
                self.assertNotIn(b'FAKE-TEST-CREDENTIAL', result.stdout + result.stderr)
                header_path = re.search(r"'@([^']+)'", argv).group(1)
                self.assertFalse(Path(header_path).exists())
                self.assertFalse(Path(header_path + '.response').exists())

    def test_copy_failure_does_not_replace_existing_binary(self):
        match = re.search(r'let copy = remote\(connection, """\n(.*?)\n        """, stdin: binaryData\)', SOURCE, re.S)
        self.assertIsNotNone(match)
        script = 'set -e\n' + '\n'.join(line[8:] for line in match.group(1).splitlines())
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            home, bin_dir, env = self.sandbox(root)
            binary = home / 'Library/Application Support/zapfast/bin/zapfast-scheduler'
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b'old-working-daemon')
            broken_cat = bin_dir / 'cat'
            broken_cat.write_text('#!/bin/sh\nprintf partial\nexit 9\n')
            broken_cat.chmod(0o755)
            result = subprocess.run(['sh', '-c', script], env=env, input=b'new-daemon', capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(binary.read_bytes(), b'old-working-daemon')
            self.assertEqual(list(binary.parent.glob('scheduler.*')), [])

    def test_remote_command_follows_destination_not_ssh_options(self):
        # Source-contract check alongside executed shell tests. No SSH is run.
        body = SOURCE.split('func remote(', 1)[1].split('func installScheduler(', 1)[0]
        self.assertIn('let process = ssh(connection, extra: [])', body)
        self.assertIn('process.arguments! += ["set -e\\n" + script]', body)
        self.assertNotIn('extra: [script]', body)


if __name__ == '__main__':
    unittest.main()
