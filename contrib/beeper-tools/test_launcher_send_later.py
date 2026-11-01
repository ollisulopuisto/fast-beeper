import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = (ROOT / 'packaging/macos/BeeperLauncher.swift').read_text()


class SendLaterSafetyTests(unittest.TestCase):
    def test_forward_failure_drops_cached_token_before_launch(self):
        # An occupied local 23376 must not receive a previously synced token:
        # the cached file is removed, and removal happens before the client
        # binary is launched.
        removal = SOURCE.index('removeItem(at: profile.appendingPathComponent("config/scheduler-token")')
        launch = SOURCE.index('Bundle.main.bundleURL.appendingPathComponent("Contents/MacOS/zapfast-client")')
        guard = SOURCE.index('if !schedulerForwarded {')
        self.assertLess(guard, removal)
        self.assertLess(removal, launch)

    def test_token_sync_only_with_own_forward(self):
        # The READY marker proves our SSH session owns the forward before any
        # token is read from the Server Mac; the sync stays inside that gate.
        gate = SOURCE.index('if schedulerForwarded {')
        query = SOURCE.index('let tokenQuery = ssh(connection, extra: [])')
        self.assertIn('printf READY; cat >/dev/null', SOURCE)
        self.assertIn('marker == Data("READY".utf8)', SOURCE)
        self.assertIn('ExitOnForwardFailure=yes', SOURCE)
        self.assertLess(gate, query)

    def test_token_never_on_argv_or_in_logs(self):
        self.assertNotIn('arguments! += [schedulerToken]', SOURCE)
        self.assertNotIn('\\(schedulerToken)', SOURCE)
        self.assertIn('.posixPermissions: 0o600', SOURCE)
        self.assertIn('schedulerToken.count <= 256', SOURCE)


if __name__ == '__main__':
    unittest.main()
