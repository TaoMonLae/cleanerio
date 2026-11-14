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

## More features (added 2026-11-14)

These are Mac-only unless noted. The thresholds marked "our choice" are judgement calls, not measurements.

| Feature | How it works | Notes |
|---|---|---|
| AI Clutter | caches, logs, telemetry and old versions of AI coding tools and their browser profiles; local models (Ollama, LM Studio) listed unticked | the newest version of each tool is kept |
| iPhone/iPad backups | `~/Library/Application Support/MobileSync/Backup`, unticked | |
| Xcode Archives | listed under Developer junk, unticked | Xcode can't regenerate them |
| Broken preferences | `~/Library/Preferences/*.plist` that are empty or unreadable, unticked | a file may be unreadable only because it is being written |
| Trash bins on other drives | `/Volumes/*/.Trashes/<your id>` | |
| Duplicate Finder (all systems) | same size → first 64 KB → whole file; files of 100 KB or more; cloud files that aren't downloaded are skipped | minimum size is our choice |
| Similar Images (all systems) | 9×8 difference fingerprint; ≤ 6 differing bits = similar; biggest kept; up to 3000 pictures (JPEG/PNG); links are not followed | thresholds are our choice |
| Privacy | browser history, cookies, autofill, caches (Safari, Chrome, Firefox), recent-items lists, saved Wi-Fi networks; the browser must be closed | removed items go to the Trash and stay readable there until it is emptied |
| App Permissions | read from the user's permission database when Full Disk Access is given, otherwise links to each System Settings page; "Take back" uses `tccutil` | |
| Maintenance tasks added | Free up purgeable space, Thin Time Machine snapshots, Speed up Mail (vacuum the message index), Repair disk permissions | |
| App Updater | App Store apps: Apple's public lookup; others: their Sparkle update feed. Only https and App Store links are opened | the only feature that uses the internet; runs only when asked |
| Leftovers of deleted apps | Library items named like an app id (com.x.y) whose app isn't installed and Spotlight can't find; unticked | nothing is listed when Spotlight is off |
| Browser Care | lists Safari and Chrome tabs (only if open), marks duplicate addresses, closes chosen tabs | a tab is closed only if its address is still the one listed |
| Screen Cleaning (all systems) | full-screen black; three clicks or Esc to leave | |
| Language files, universal binaries | not added: removing them breaks app signatures on current macOS | safety choice |
