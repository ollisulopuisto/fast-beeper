//! Touch ID for the app lock, enforced by the keychain itself.
//!
//! The app-lock password lives in a keychain item whose SecAccessControl
//! requires the currently enrolled biometry for every read: the system shows
//! the Touch ID prompt as part of the read and refuses without it. Touch ID
//! never bypasses the password check - a successful read fills the password
//! field and the same slow PBKDF2 verifier gates the unlock. The item is
//! forgotten when the setting is turned off or the password changes, and the
//! lock screen always keeps the plain password field.

use std::sync::mpsc::Receiver;

use zeroize::Zeroizing;

/// The keychain entry holding the app lock password for Touch ID unlocks.
#[cfg(target_os = "macos")]
const SERVICE: &str = "rocks.zapfast.AppLock";
#[cfg(target_os = "macos")]
const ACCOUNT: &str = "app-lock-password";

/// Whether this build can offer Touch ID at all. Hardware and enrolment are
/// checked per read; an absent sensor simply fails back to the password.
pub(crate) fn supported() -> bool {
    cfg!(target_os = "macos")
}

/// Whether a saved password waits for Touch ID unlocks.
pub(crate) fn has_saved_password() -> bool {
    #[cfg(target_os = "macos")]
    return imp::has_saved_password();
    #[cfg(not(target_os = "macos"))]
    return false;
}

/// Saves the app lock password after a successful password unlock, while
/// the Touch ID setting is on. Any older entry is replaced.
pub(crate) fn save_password(password: &str) -> anyhow::Result<()> {
    imp::save_password(password)
}

/// Forgets the saved app lock password.
pub(crate) fn delete_password() -> anyhow::Result<()> {
    imp::delete_password()
}

/// Reads the saved password under the system's biometric prompt, off the
/// interface thread: the keychain shows Touch ID and blocks until it
/// answers. The answer is the password, or `None` when the prompt was
/// cancelled, the biometry failed, or no entry exists.
#[cfg(target_os = "macos")]
pub(crate) fn evaluate(reason: &str) -> Receiver<Option<Zeroizing<String>>> {
    imp::evaluate(reason)
}

/// On other platforms there is nothing to read; the prompt fails at once.
#[cfg(not(target_os = "macos"))]
pub(crate) fn evaluate(_reason: &str) -> Receiver<Option<Zeroizing<String>>> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let _ = sender.send(None);
    receiver
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub(super) fn save_password(_password: &str) -> anyhow::Result<()> {
        anyhow::bail!("Touch ID unlock needs macOS")
    }

    pub(super) fn delete_password() -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use std::sync::mpsc::Receiver;

    use objc2_core_foundation::{CFBoolean, CFData, CFDictionary, CFRetained, CFString, CFType};
    use objc2_security::{
        SecAccessControl, SecAccessControlCreateFlags, SecItemAdd, SecItemCopyMatching,
        SecItemDelete, kSecAttrAccessControl, kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
        kSecAttrAccount, kSecAttrService, kSecClass, kSecClassGenericPassword, kSecReturnData,
        kSecUseAuthenticationUI, kSecUseAuthenticationUIFail, kSecUseOperationPrompt,
        kSecValueData,
    };
    use zeroize::Zeroizing;

    use super::{ACCOUNT, SERVICE};

    const ERR_SEC_SUCCESS: i32 = 0;
    const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;
    /// The item's biometric set no longer matches; it is invalidated.
    const ERR_SEC_AUTH_FAILED: i32 = -25293;
    /// The item exists but may not be read without authentication UI.
    const ERR_SEC_INTERACTION_NOT_ALLOWED: i32 = -25308;

    /// The identity half of every query: this app's one app-lock entry.
    fn query(extra: &[(&'static CFString, &CFType)]) -> CFRetained<CFDictionary<CFType, CFType>> {
        // The kSec* constants are extern statics; reading them is unsafe.
        unsafe { query_inner(extra) }
    }

    unsafe fn query_inner(
        extra: &[(&'static CFString, &CFType)],
    ) -> CFRetained<CFDictionary<CFType, CFType>> {
        let service = CFString::from_static_str(SERVICE);
        let account = CFString::from_static_str(ACCOUNT);
        let mut keys: Vec<&CFType> = vec![
            AsRef::<CFType>::as_ref(kSecClass),
            AsRef::<CFType>::as_ref(kSecAttrService),
            AsRef::<CFType>::as_ref(kSecAttrAccount),
        ];
        let mut values: Vec<&CFType> = vec![
            AsRef::<CFType>::as_ref(kSecClassGenericPassword),
            AsRef::<CFType>::as_ref(&*service),
            AsRef::<CFType>::as_ref(&*account),
        ];
        for (key, value) in extra {
            keys.push(AsRef::<CFType>::as_ref(*key));
            values.push(value);
        }
        CFDictionary::<CFType, CFType>::from_slices(&keys, &values)
    }

    /// A read that never prompts: success or interaction-not-allowed means
    /// the entry exists, not-found means it does not.
    // kSecUseAuthenticationUI is deprecated in favor of an LAContext
    // auth-context; the key still works and keeps this simple read LA-free.
    #[allow(deprecated)]
    pub(super) fn has_saved_password() -> bool {
        let query = query(&[(
            unsafe { kSecUseAuthenticationUI },
            AsRef::<CFType>::as_ref(unsafe { kSecUseAuthenticationUIFail }),
        )]);
        // No return type is asked for, so nothing comes back to release.
        let status = unsafe { SecItemCopyMatching(query.as_opaque(), std::ptr::null_mut()) };
        matches!(
            status,
            ERR_SEC_SUCCESS | ERR_SEC_INTERACTION_NOT_ALLOWED | ERR_SEC_AUTH_FAILED
        )
    }

    pub(super) fn save_password(password: &str) -> anyhow::Result<()> {
        // A re-save replaces; a leftover entry with older flags goes too.
        let _ = delete_password();
        let access = unsafe {
            SecAccessControl::with_flags(
                None,
                AsRef::<CFType>::as_ref(kSecAttrAccessibleWhenUnlockedThisDeviceOnly),
                SecAccessControlCreateFlags::BiometryCurrentSet,
                std::ptr::null_mut(),
            )
        }
        .ok_or_else(|| anyhow::anyhow!("The system refused a Touch ID access control"))?;
        let data = CFData::from_bytes(password.as_bytes());
        let attributes = query(&[
            (unsafe { kSecValueData }, AsRef::<CFType>::as_ref(&*data)),
            (
                unsafe { kSecAttrAccessControl },
                AsRef::<CFType>::as_ref(&*access),
            ),
        ]);
        let status = unsafe { SecItemAdd(attributes.as_opaque(), std::ptr::null_mut()) };
        if status != ERR_SEC_SUCCESS {
            anyhow::bail!("The OS keychain could not save the app-lock password ({status})");
        }
        Ok(())
    }

    pub(super) fn delete_password() -> anyhow::Result<()> {
        let status = unsafe { SecItemDelete(query(&[]).as_opaque()) };
        if status != ERR_SEC_SUCCESS && status != ERR_SEC_ITEM_NOT_FOUND {
            anyhow::bail!("The OS keychain could not forget the app-lock password ({status})");
        }
        Ok(())
    }

    /// The blocking, prompt-showing read; runs on its own thread.
    // kSecUseOperationPrompt is deprecated in favor of an LAContext
    // auth-context; the key still renders the reason in the system prompt.
    #[allow(deprecated)]
    pub(super) fn evaluate(reason: &str) -> Receiver<Option<Zeroizing<String>>> {
        let (sender, receiver) = std::sync::mpsc::channel();
        let reason = reason.to_owned();
        std::thread::Builder::new()
            .name("app-lock-touch-id".into())
            .spawn(move || {
                let prompt = CFString::from_str(&reason);
                let query = query(&[
                    (
                        unsafe { kSecReturnData },
                        AsRef::<CFType>::as_ref(CFBoolean::new(true)),
                    ),
                    (
                        unsafe { kSecUseOperationPrompt },
                        AsRef::<CFType>::as_ref(&*prompt),
                    ),
                ]);
                let mut result: *const CFType = std::ptr::null();
                let status = unsafe { SecItemCopyMatching(query.as_opaque(), &mut result) };
                // A successful read returns the data retained for us.
                let data: Option<CFRetained<CFData>> = if status == ERR_SEC_SUCCESS {
                    std::ptr::NonNull::new(result.cast_mut().cast::<CFData>())
                        .map(|ptr| unsafe { CFRetained::from_raw(ptr) })
                } else {
                    None
                };
                let password: Option<Zeroizing<String>> = if let Some(data) = data {
                    String::from_utf8(data.to_vec()).ok().map(Zeroizing::new)
                } else {
                    // User cancel, failed biometry, invalidated or missing
                    // entry: all fall back to typing the password.
                    if status == ERR_SEC_AUTH_FAILED {
                        let _ = delete_password();
                    }
                    None
                };
                let _ = sender.send(password);
            });
        receiver
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn unsupported_platforms_fail_the_prompt_at_once() {
        if super::supported() {
            return;
        }
        assert_eq!(super::evaluate("unlock").recv().unwrap(), None);
    }
}
