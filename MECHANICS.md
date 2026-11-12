# Rules sheet

The numbers Cleanerio uses to decide what counts as large, old, unused or unhealthy. They are this
project's own rules: change them in `src/pages.rs` and `src/scan.rs`, and update the tests there.

| Rule | Value | Used in |
|---|---|---|
| low disk warning share | 0.1 of disk | activity |\n| low disk warning choices | 2,5,10,20,30,50,100 GB | activity |\n| unknown disk size fallback | 1000000000000 bytes | activity |\n| trash size alert | 2000000000 bytes | cleanup |\n| large file minimum | 50000001 bytes | my files |\n| size group small | 50,000,000 < size <= 500,000,000 | my files |\n| size group average | 500,000,000 < size <= 1,000,000,000 | my files |\n| size group huge | size > 999,999,999 | my files |\n| old file age | 365 days | my files |\n| last-opened filters | 7, 30, 182, 365 days | my files |\n| fresh duplicates window | 90 days | my files |\n| unused app age | 180 days | applications |\n| space map style | bubbles | space map |\n| old download age | 90 days | my files |\n| duplicate quick-compare bytes | 65536 bytes | my files |\n| threat families in built-in database | 19 | protection |\n| threat categories | Trojan, Adware, Spyware, Miner, Unwanted | protection |\n| health score maximum | 100 | health |\n| health level edges | Excellent >89, Good >59, Fair >39, Requires Attention >0, Critical 0 | health |\n| penalty out of disk space | 20 | health |\n| penalty no recent smart scan | 10 | health |\n| penalty unused apps | 10 | health |\n| penalty trash overfill | 5 | health |\n| penalty uptime | 5 | health |\n| penalty no recent threat scan | 5 | health |\n| penalty critical battery | 80 | health |\n| restart suggested after | 7 days | health |\n| recent scan window | 7 days | health |\n| memory tile warning | 0.85 share | health |

Safety rules:

- Everything removed goes to the Trash after a confirmation; only "Empty Trash" deletes.
- Protected folders (the home folder, Library, Documents, /Applications, /System and the like) are never moved to the Trash.
- Name-only threat matches and name-only app leftovers are never ticked automatically.
- "Last opened" comes from the date macOS records, not from file access times.

Tests: `cargo test` checks the size groups, age filters, thresholds, health levels and low-disk limit at their edges.
