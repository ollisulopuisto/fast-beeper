# macOS activation

The delegate handlers use architecture-correct fallback encodings when protocol
metadata is unavailable. This path is active by default and covered by native
registration tests.

Direct native restoration of every minimized Solmu window is experimental and
off by default. To test it, launch the app process with the environment variable
`SOLMU_EXPERIMENTAL_RESTORE_MINIMIZED=1`. Any other value or no variable keeps it
off. Finder launches do not normally inherit terminal environment variables.

This opt-in can restore multiple minimized windows, not only the main window.
It has not passed real closed/minimized Cmd-Tab acceptance. Removing the variable
returns to the default behavior. No profile or message data is changed.
