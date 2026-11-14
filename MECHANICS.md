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

Windows rules (`src/windows_scan.rs`): temporary files must be at least 7 days old;
installer files at least 90 days old; large files at least 100 MB. Duplicate candidates
are 1 MB to 2 GB and are compared by a complete SHA-256 hash after a quick prefix pass.
The scan examines up to 150,000 personal files. All Windows results start unselected;
Recycle Bin actions protect folder roots and reject symbolic links and junctions.

Tests: `cargo test` checks the size groups, age filters, thresholds, health levels and low-disk limit at their edges.

## The original's other features (added 2026-11-14)
Feature lists come from the original's text tables (JunkCleanupModule, ProtectionModule, PerformanceModule, ApplicationsModule, OrganizeModule, ToolboxModule) and file lists from its AIToolScanning binary.
| Feature | How ours works | Source / estimate |
|---|---|---|
| AI Clutter | caches, logs, telemetry and old versions of Claude, Codex, Gemini/Antigravity, Cursor, opencode and the Chrome DevTools MCP profile; local models (Ollama, LM Studio) listed unticked | paths from AIToolScanning strings; model folders estimated |
| iPhone/iPad backups | ~/Library/Application Support/MobileSync/Backup, unticked | JunkCleanupModule "iOS Device Backups" |
| Broken preferences | ~/Library/Preferences/*.plist that are empty or unreadable | JunkCleanupModule "Broken Preferences" |
| Trash bins on other drives | /Volumes/*/.Trashes/<your id> | "Mounted Trash Bins" |
| Duplicate Finder | same size → first 64 KB → whole file; files of 100 KB or more | method and minimum estimated |
| Similar Images | 9×8 difference fingerprint; ≤ 6 differing bits = similar; biggest kept; up to 3000 pictures (JPEG/PNG) | estimated |
| Privacy | browser history, cookies, autofill, caches (Safari, Chrome, Firefox), recent-items lists, saved Wi-Fi networks; browser must be closed | ProtectionModule "Privacy Items", "Wi-Fi Networks" |
| App Permissions | read from the user's permission database when Full Disk Access is given, otherwise links to each System Settings page; "Take back" uses tccutil | ProtectionModule "Application Permissions" (17 categories) |
| Maintenance tasks added | Free up purgeable space, Thin Time Machine snapshots, Speed up Mail (vacuum Envelope Index), Repair disk permissions | PerformanceModule task titles; commands are ours |
| Run maintenance scripts | not added: macOS on this Mac no longer has the `periodic` tool | PerformanceModule "periodic_scripts" |
| App Updater | App Store apps: public App Store lookup; others: their Sparkle update feed | ApplicationsModule "App Updater"; sources ours |
| App leftovers | Library items named like an app id (com.x.y) whose app isn't installed and Spotlight can't find; unticked | ApplicationsModule "App Leftovers" |
| Browser Care | lists Safari and Chrome tabs (only if open), marks duplicate addresses, closes chosen tabs | ToolboxModule "Browser Care" |
| Screen Cleaning | full-screen black; three clicks or Esc to leave | ToolboxModule "Screen Cleaning Mode" |
| Language files, universal binaries | not added: removing them breaks app signatures on current macOS | safety choice |
