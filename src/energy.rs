//! Whether the OS may pause ZapFast to save power.
//!
//! By default it may not: messaging and the managed connection must keep
//! working while the window is in the background, so ZapFast holds an
//! NSProcessInfo activity that opts it out of App Nap. The System setting
//! trades that for battery savings.

/// Lets the OS pause ZapFast (`true`), or keeps it running (`false`).
pub fn set_app_nap_allowed(allowed: bool) {
    imp::set_allowed(allowed);
}

/// Whether ZapFast must hold the power assertion: the mirror of the
/// setting, kept as its own function so the default is testable.
pub(crate) fn assertion_required(allow_app_nap: bool) -> bool {
    !allow_app_nap
}

#[cfg(target_os = "macos")]
mod imp {
    use std::sync::Mutex;

    use objc2_foundation::{NSActivityOptions, NSObjectProtocol, NSProcessInfo, NSString};

    /// The held activity, made sendable for the static: the token is only
    /// ever handed back to NSProcessInfo, never inspected.
    struct SendableActivity(
        objc2::rc::Retained<objc2::runtime::ProtocolObject<dyn NSObjectProtocol>>,
    );
    unsafe impl Send for SendableActivity {}

    /// endActivity is always called before release: dropping the object
    /// alone does not end the assertion.
    static ACTIVITY: Mutex<Option<SendableActivity>> = Mutex::new(None);

    pub(super) fn set_allowed(allowed: bool) {
        let mut slot = ACTIVITY
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match (allowed, slot.take()) {
            (true, Some(activity)) => unsafe {
                NSProcessInfo::processInfo().endActivity(&activity.0);
            },
            (false, None) => {
                let reason =
                    NSString::from_str("Messages keep arriving while ZapFast is in the background");
                let activity = NSProcessInfo::processInfo().beginActivityWithOptions_reason(
                    NSActivityOptions::UserInitiatedAllowingIdleSystemSleep,
                    &reason,
                );
                *slot = Some(SendableActivity(activity));
            }
            (_, activity) => *slot = activity,
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub(super) fn set_allowed(_allowed: bool) {}
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_assertion_is_the_default_and_only_lifted_by_the_setting() {
        assert!(super::assertion_required(false));
        assert!(!super::assertion_required(true));
    }
}
