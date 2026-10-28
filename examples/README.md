# Targeted diagnostics

merged_checks uses the app's headless constructor and real settings/model/UI
library. The development run used production sources with unrelated transport
and native-call leaves stubbed. Running this example from the full source tree
requires the full app dependencies; the example is not proof of native Mac runtime.

merged_perf compares cloned vs borrowed timeline sort on generic synthetic data.
Run with `cargo run --example merged_perf -- owned 10000` and then `refs 10000`.
Debug measurements are not release app frame times or whole-stack memory results.

## Additional synthetic run, October 28

Linux debug build, 512 repeats of `fixture ` per message, separate processes:

| Mode | Rows | Ordering microseconds | Peak process RSS KiB |
|---|---:|---:|---:|
| Borrowed refs | 10000 | 2655 | 50300 |
| Owned payload clone | 10000 | 24086 | 96892 |
| Borrowed refs | 50000 | 13729 | 237412 |
| Owned payload clone | 50000 | 156644 | 470596 |

One run per case, not a statistical benchmark. RSS includes the source fixture and process, not an incremental allocation measurement. This demonstrates the avoided payload copy in ordering only, not whole-app memory, Beeper Server resources, UI rendering performance or Mac performance.

`chip_scroll_checks` now checks wrapped controls within220px; the previous horizontal-scroll behavior is superseded.

`search_state_checks` uses real App event handling to check stale query/chat errors are ignored, a current failure ends the pending state, and a successful response clears the error. Separate native GUI fixtures verify HTTP503-to-success rendering. These are targeted diagnostics, not the full GUI suite.

`refusal_state_checks` verifies real App handling restores unsupported image pixels, caption and quote. It uses no live transport and does not claim UI pixel or native Mac validation. Some diagnostics start optional background helpers that are terminated on exit.

`late_send_checks` uses real App event handling to check late Beeper text/caption failures retain copyable content when a newer current or parked draft exists, without overwriting it. Empty composers still restore normally. Retention uses the existing dismissible error list, not a durable recovery archive; errors can be evicted by later errors. This check is not UI pixel or native Mac validation.

`repair_checks` checks network slots1/10, unchanged plain digits, exact pinned/list modifier separation, missing-slot no-op and unreplied toggling using real App/UI code. Mac hardware keyboard remains user-trial work.

`cargo run --locked --example queue_checks`: synthetic App diagnostic for local Reply later persistence, exact origin/account confirmation, linked-person clearing and queue filtering. No live-account writes.
