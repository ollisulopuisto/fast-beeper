# Reply later and live-page retention

This follows repair1105. The combined PR includes that repair once; the second commit adds the queue.

- Cmd+Shift+L adds/removes the open conversation from a local persistent queue. The Reply later chip shows it; read state does not clear it.
- Manually linked people count as one queued item. Successful send confirmation for an exact configured member clears the linked item. Origin, account and chat identity are preserved. Unrelated accounts and services cannot clear it.
- Only explicitly successful local send confirmation clears an item automatically. Failed, pending and sparse unconfirmed echoes do not. If a bridge does not expose send success, remove the item manually. Sends from other clients are not used to auto-clear this queue.
- The queue respects network/account selection, lock and archive boundaries. It overrides per-account inbox hiding so a read queued conversation remains visible. Global pins do not leak nonqueued chats into this filter.
- Unread, Unreplied and Reply later filters are mutually exclusive. Search/network scope remains available.
- In Beeper mode the inherited immediate app-lock shortcut moves to Option+Cmd+Shift+L (Alt+Ctrl+Shift+L elsewhere). Cmd+Shift+L remains lock outside Beeper mode. Menu locking is unchanged.
- Live polling now retains the received page even if its continuation cursor stalls. The warning and last-good cursor stay; this does not establish why a live service returned that cursor or guarantee all updates arrived.

## Validation

166 Rust adapter/model/safety tests pass against synthetic fixtures. Real App diagnostic checks settings save/load, failed/foreign confirmation retention, exact linked-member confirmation and filtered-row count. GUI library/binary compile in diagnostic build with unrelated native transport helpers stubbed. Synthetic 1100x780 and 700x700 screenshots inspected. No user messages or screenshots are included.

Native patched Mac compilation, real sends, OS keychain/restart and full inherited GUI test suite remain unverified. Never resend a failed draft automatically.

API contract checked: https://developers.beeper.com/desktop-api-reference/resources/messages/methods/list
