import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = (ROOT / 'packaging/macos/BeeperLauncher.swift').read_text()


class SchedulerInstallTests(unittest.TestCase):
    # Source-contract tests: they pin the safety-relevant structure of the
    # install step; the install itself runs natively after explicit approval.
    def test_install_only_after_explicit_approval(self):
        offer = SOURCE.index('func offerSchedulerInstall')
        declined = SOURCE.index('scheduler-install-declined')
        alert = SOURCE.index('alert.runModal()', offer)
        install = SOURCE.index('installScheduler(connection, bundled: bundled', offer)
        self.assertLess(offer, declined)
        self.assertLess(declined, alert)
        self.assertLess(alert, install)
        self.assertIn('alertThirdButtonReturn', SOURCE)
        self.assertIn('alertFirstButtonReturn else { return }', SOURCE)

    def test_install_checks_remote_architecture(self):
        self.assertIn('"uname -m"', SOURCE)
        self.assertIn('== "arm64"', SOURCE)

    def test_tokens_travel_on_stdin_never_argv(self):
        self.assertIn('stdin: Data(beeperToken.utf8)', SOURCE)
        self.assertNotIn('\\(beeperToken)', SOURCE)
        self.assertIn('chmod 600 "$APP_SUPPORT/scheduler-token"', SOURCE)
        self.assertNotIn('arguments! += [beeperToken]', SOURCE)

    def test_install_uses_launchd_and_verifies_health_with_token(self):
        self.assertIn('launchctl load "$PLIST"', SOURCE)
        self.assertIn('schedulerHealthScript', SOURCE)
        self.assertNotIn('Authorization: Bearer', SOURCE)

    def test_decline_has_a_discoverable_retry_route(self):
        self.assertIn('scheduler-install-declined', SOURCE)
        self.assertIn('CommandLine.arguments.contains("--reconfigure")', SOURCE)
        self.assertIn('relaunch while holding Option', SOURCE)

    def test_partial_failure_reports_stage_not_noop(self):
        self.assertIn('func installFailed(_ connection: Connection, stage: String)', SOURCE)
        self.assertNotIn('Nothing else changed on', SOURCE)


if __name__ == '__main__':
    unittest.main()
