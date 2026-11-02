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




def service_script():
    """Rebuild the exact service-start script the launcher ships over SSH.

    Walks the Swift multiline string in BeeperLauncher.swift: plain text
    keeps its content (minus the 8-space literal indentation) and the
    `\(schedulerHealthScript)` interpolation splices in the shipped probe.
    """
    health = HEALTH.read_text().strip()
    open_marker = 'let service = remote(connection, """'
    i1 = SOURCE.index(open_marker) + len(open_marker)
    i2 = SOURCE.index('echo RUNNING""")', i1)
    block = SOURCE[i1:i2]
    assert '"""' not in block, 'single literal: interpolation, not concatenation'
    lines = block.split('\n')
    script = 'set -e' + '\n'.join(line[8:] if line.startswith('        ') else line for line in lines)
    script = script.replace('\\(schedulerHealthScript)', health)
    return script + 'echo RUNNING\n'


class ServiceStartTests(unittest.TestCase):
    """Execute the service-start script with stubbed launchd/curl, no live SSH."""

    def sandbox(self, root):
        home = root / 'home'
        home.mkdir()
        bin_dir = root / 'bin'
        bin_dir.mkdir()
        return home, bin_dir, dict(os.environ, HOME=str(home), PATH=str(bin_dir) + ':' + os.environ['PATH'])

    def install_stubs(self, bin_dir, curl_succeed_after, gui_loaded=False, gui_bootstrap_ok=False):
        launchctl = bin_dir / 'launchctl'
        launchctl.write_text("""#!/bin/sh
echo "$@" >> "$CAPTURE"
case "$1" in
    print)
        case "$2" in
            gui/*) [ "%s" = yes ] || exit 1 ;;
            user/*) exit 1 ;;
        esac
        ;;
    bootstrap)
        case "$2" in
            gui/*) [ "%s" = yes ] || exit 5 ;;
        esac
        ;;
esac
exit 0
""" % ('yes' if gui_loaded else 'no', 'yes' if gui_bootstrap_ok else 'no'))
        launchctl.chmod(0o755)
        curl = bin_dir / 'curl'
        curl.write_text("""#!/usr/bin/env python3
import os, sys
from pathlib import Path
counter = Path(os.environ['CURL_COUNTER'])
count = int(counter.read_text() or '0') + 1 if counter.exists() else 1
counter.write_text(str(count))
if count < int(os.environ['CURL_SUCCEED_AFTER']):
    sys.exit(7)
print('{"ok":true}')
""")
        curl.chmod(0o755)
        # Instant sleep so the poll loops do not slow the test down.
        sleep = bin_dir / 'sleep'
        sleep.write_text('#!/bin/sh\nexit 0\n')
        sleep.chmod(0o755)

    def run_service(self, succeed_after, gui_loaded=False, gui_bootstrap_ok=False):
        import tempfile
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            home, bin_dir, env = self.sandbox(root)
            self.install_stubs(bin_dir, succeed_after, gui_loaded, gui_bootstrap_ok)
            env['CAPTURE'] = str(root / 'launchctl-argv')
            env['CURL_COUNTER'] = str(root / 'curl-count')
            env['CURL_SUCCEED_AFTER'] = str(succeed_after)
            app_support = home / 'Library/Application Support/zapfast'
            (app_support / 'bin').mkdir(parents=True)
            (app_support / 'scheduler-token').write_text('FAKE-TEST-CREDENTIAL')
            (app_support / 'beeper-token').write_text('FAKE-TEST-CREDENTIAL')
            (app_support / 'scheduler.log').write_text('MARKER-LOG-LINE\n')
            result = subprocess.run(['sh', '-c', service_script()], env=env, capture_output=True, text=True)
            capture_path = root / 'launchctl-argv'
            capture = capture_path.read_text() if capture_path.exists() else ''
            plist = home / 'Library/LaunchAgents/com.zapfast.scheduler.plist'
            plist_text = plist.read_text() if plist.is_file() else None
            return result, capture, plist_text

    def test_healthy_service_is_left_untouched(self):
        # The scheduler already answers: no launchd mutation at all.
        result, capture, plist_text = self.run_service(succeed_after=1)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('RUNNING', result.stdout)
        self.assertEqual(capture, '')
        self.assertIsNotNone(plist_text)

    def test_cold_start_uses_gui_then_user_domain_and_polls(self):
        result, capture, plist_text = self.run_service(succeed_after=4)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('RUNNING', result.stdout)
        self.assertIn('print gui/', capture)
        self.assertIn('print user/', capture)
        self.assertIn('bootstrap gui/', capture)
        self.assertIn('bootstrap user/', capture)
        self.assertNotIn('bootout', capture)
        self.assertNotIn(' load ', ' ' + capture.replace(chr(10), ' ') + ' ')
        self.assertIsNotNone(plist_text)
        self.assertIn('com.zapfast.scheduler', plist_text)
        self.assertIn('ZAPFAST_CONTROL_TOKEN_FILE', plist_text)

    def test_loaded_but_unhealthy_agent_is_restarted_in_its_own_domain(self):
        result, capture, _ = self.run_service(succeed_after=3, gui_loaded=True, gui_bootstrap_ok=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('RUNNING', result.stdout)
        self.assertIn('print gui/', capture)
        self.assertIn('bootout gui/', capture)
        self.assertIn('bootstrap gui/', capture)
        self.assertNotIn('user/', capture)

    def test_failure_reports_remote_stderr_and_log_tail(self):
        result, _, _ = self.run_service(succeed_after=99)
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn('RUNNING', result.stdout)
        self.assertIn('scheduler still not healthy', result.stderr)
        self.assertIn('MARKER-LOG-LINE', result.stderr)

    def test_launcher_surfaces_remote_stderr_in_the_failure_dialog(self):
        # Source contract: remote() returns stderr, and the failure dialog
        # shows the Server Mac's own words instead of a bare stage name.
        self.assertIn('process.standardError = err', SOURCE)
        self.assertIn('return (process.terminationStatus, data, errData)', SOURCE)
        self.assertIn('func installFailed(_ connection: Connection, stage: String, detail: String = "")', SOURCE)
        self.assertIn('The Server Mac said:', SOURCE)
        self.assertIn('detail: detail', SOURCE)

    def test_health_probe_gives_mktemp_an_explicit_template(self):
        # Portability contract: template operand keeps the probe working on
        # mktemp variants without a default-template mode (older BSD).
        self.assertRegex(HEALTH.read_text(), r'mktemp "\$\{TMPDIR:-/tmp\}/[^"]*XXXXXX"')


if __name__ == '__main__':
    unittest.main()
