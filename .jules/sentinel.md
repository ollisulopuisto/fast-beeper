# Sentinel's Journal - Critical Learnings Only

## 2026-11-01 - External URL Scheme Validation at Desktop Launcher Hook
**Vulnerability:** The Linux detached link opener (`DetachedLinks`) intercepted egui's `OpenUrl` commands and forwarded them directly to `open::that_detached` without verifying the URL scheme. If untrusted schemes (e.g. `file://`, `javascript:`, or desktop protocol handlers) were routed through `OpenUrl`, they would be passed directly to the desktop launcher.
**Learning:** Even when URL filtering is applied in certain action dispatch handlers (e.g., in `App::update` for chat links), egui UI components (such as hyperlinks or plugins) can emit `OpenUrl` commands directly into `output.platform_output.commands`. Defense-in-depth requires validation at the launcher plugin boundary itself.
**Prevention:** Always validate URLs against `crate::safety::external_url` at the `DetachedLinks::output_hook` boundary prior to invoking system launchers.
