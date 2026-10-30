# Sign-in source checks

`cargo test --manifest-path contrib/signin-check/Cargo.toml`

Compiles the actual production `src/beeper_signin.rs` with real egui, reqwest,
zeroize and OS-keyring libraries, avoiding the unrelated WhatsApp stack.
Only the app's environment/config wrapper and the platform-store constructor are
adapted here. HTTP fixtures use synthetic loopback credentials. This does not
prove Beeper Server interoperability, macOS Keychain behavior, GUI integration,
network availability, or a native DMG.
