# Optional queue counts

## Approved wireframe

    Chats    [All 12] [Unread 3] [Reply later 5] [Read later 2]
    Filters

Existing icons remain in their cells. Counts are small upper-right badges,
not text labels replacing the icons. No added row, cell width or height.

Settings > Chats > Show queue counts is On by default. Off restores the
current icon-only control. This setting is independent of unread-message
counts. Zero has no badge; large values show 99+, with the exact count in
the tooltip and accessibility label.

## Meaning

The maintainer confirmed chat counts for now, rather than individual messages.
Each badge says how many local chat-list items that queue would show with the
current query and scope. Network/account, category, favorites, labels, inbox,
archive and locked-folder rules are the list's existing rules. Linked people
count once. Reply later and Read later retain their hidden-inbox exception.
This is not a remote total over unloaded message pages. Merely showing counts
does not change the selected queue, mark anything read, or send any request.

The shared filtering pass avoids the list's sorting and linked preview/history
work. Off skips counting entirely. The control is searchable in offline mode.

Reproduce with the synthetic queue-counts demo page. Roll back by reverting
the queue-count preference PR; no data migration or external effects.
