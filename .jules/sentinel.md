# Sentinel's Journal - Critical Learnings Only

## 2025-05-18 - DetachedLinks OpenUrl Interception Bypassing Safety Boundary
**Vulnerability:** The `DetachedLinks` egui plugin in `src/opener.rs` intercepted `egui::OutputCommand::OpenUrl` commands to open links detached (avoiding defunct child processes on Unix), but invoked the opener directly without validating `link.url` against `crate::safety::external_url`. Any URL emitted through egui output commands (such as widgets or hyperlinks) could execute arbitrary schemes (e.g. `file://`, `javascript:`, local custom handlers) on desktop launchers.
**Learning:** Application link handling had defense checks in `Action::OpenUrl` in `src/app.rs`, but secondary link dispatch sinks like egui plugins (`egui::plugin::Plugin`) handle low-level output events directly and must independently enforce the same protocol boundary validations.
**Prevention:** Never assume intercepted framework commands or events have passed prior domain validation; always validate URLs against `crate::safety::external_url` at the launcher boundary.
