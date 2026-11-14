//! More to clean and check: AI clutter, broken preferences, similar images, privacy items, app
//! permissions, leftovers of deleted apps, app updates and browser tabs.
//! Nothing here deletes files by itself; removal goes through the Trash like everywhere else.
//! The app updater is the one part of Cleanerio that uses the internet, and only when asked.

use crate::scan::{children, days_since, dir_size, file_name, home, short, BigFile, JunkItem, Progress};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------- AI clutter

/// Cache, log and old-version places of AI tools (relative to the home folder).
const AI_CACHES: &[&str] = &[
    ".cache/chrome-devtools-mcp/chrome-profile/Default/Cache",
    ".cache/chrome-devtools-mcp/chrome-profile/Default/Code Cache",
    ".cache/chrome-devtools-mcp/chrome-profile/Default/GPUCache",
    ".cache/chrome-devtools-mcp/chrome-profile/Default/DawnCache",
    ".cache/chrome-devtools-mcp/chrome-profile/Default/GrShaderCache",
    ".cache/chrome-devtools-mcp/chrome-profile/Default/Service Worker/CacheStorage",
    ".cache/chrome-devtools-mcp/chrome-profile/GraphiteDawnCache",
    ".cache/chrome-devtools-mcp/chrome-profile/component_crx_cache",
    ".cache/chrome-devtools-mcp/chrome-profile/extensions_crx_cache",
    ".claude/shell-snapshots",
    ".claude/telemetry",
    ".claude/stats-cache.json",
    ".claude/mcp-needs-auth-cache.json",
    ".claude/plugins/install-counts-cache.json",
    ".codex/cache/codex_apps_server_info",
    ".codex/cache/codex_apps_tools",
    ".gemini/antigravity-browser-profile/Default/Cache",
    ".gemini/antigravity-browser-profile/Default/Code Cache",
    ".gemini/antigravity-browser-profile/Default/GPUCache",
    ".gemini/antigravity-browser-profile/Default/DawnGraphiteCache",
    ".gemini/antigravity-browser-profile/Default/DawnWebGPUCache",
    ".gemini/antigravity-browser-profile/Default/Service Worker/CacheStorage",
    ".gemini/antigravity-browser-profile/GraphiteDawnCache",
    ".gemini/antigravity-browser-profile/component_crx_cache",
    ".gemini/antigravity-browser-profile/extensions_crx_cache",
    ".local/share/opencode/log",
    "Library/Logs/Claude",
    "Library/Caches/com.anthropic.claudefordesktop",
    "Library/Caches/com.openai.codex",
    "Library/Caches/claude-cli-nodejs",
];

/// Folders holding a tool's versions; all but the newest are clutter.
const AI_VERSION_DIRS: &[&str] = &[".local/share/claude/versions", ".local/share/cursor-agent/versions"];

/// Downloaded local AI models (big, listed but never ticked).
const AI_MODEL_DIRS: &[&str] = &[".ollama/models/blobs", ".lmstudio/models", ".cache/lm-studio/models"];

pub fn ai_clutter(p: &Progress) -> Vec<JunkItem> {
    let h = home();
    let mut out = vec![];
    let mut add = |path: PathBuf, selected: bool, label: Option<String>| {
        let size = dir_size(&path, Some(p));
        if size > 0 {
            out.push(JunkItem { name: label.unwrap_or_else(|| short(&path)), path, size, selected, in_use: false });
        }
    };
    for rel in AI_CACHES {
        let path = h.join(rel);
        if std::fs::symlink_metadata(&path).is_ok() {
            p.set(format!("Looking at {}", short(&path)));
            add(path, true, None);
        }
    }
    for rel in AI_VERSION_DIRS {
        let mut versions: Vec<(std::time::SystemTime, PathBuf)> = children(&h.join(rel))
            .into_iter()
            .filter_map(|v| std::fs::metadata(&v).and_then(|m| m.modified()).ok().map(|t| (t, v)))
            .collect();
        versions.sort();
        versions.pop(); // keep the newest
        for (_, v) in versions {
            add(v.clone(), true, Some(format!("Old version: {}", short(&v))));
        }
    }
    for rel in AI_MODEL_DIRS {
        let path = h.join(rel);
        if path.exists() {
            p.set(format!("Measuring {}", short(&path)));
            add(path.clone(), false, Some(format!("Local AI models: {}", short(&path))));
        }
    }
    out.sort_by(|a, b| b.size.cmp(&a.size));
    out
}

// ---------------------------------------------------------------- broken preferences

pub fn broken_prefs(p: &Progress) -> Vec<JunkItem> {
    let mut out = vec![];
    for f in children(&home().join("Library/Preferences")) {
        if p.cancelled() {
            break;
        }
        if f.extension().is_none_or(|e| e != "plist") {
            continue;
        }
        let Ok(md) = std::fs::metadata(&f) else { continue };
        if !md.is_file() {
            continue;
        }
        // an empty file or one that isn't a property list can't be read by its app
        if md.len() == 0 || plist::Value::from_file(&f).is_err() {
            // not ticked: the file may only be unreadable to us, or half-written right now
            out.push(JunkItem { name: file_name(&f), size: md.len().max(1), path: f, selected: false, in_use: false });
        }
    }
    out
}

// ---------------------------------------------------------------- similar images

/// Two pictures are "similar" when their 64-bit difference fingerprints differ in at most this
/// many bits.
pub const SIMILAR_MAX_BITS: u32 = 6;
/// Most pictures compared in one scan.
pub const SIMILAR_MAX_IMAGES: usize = 3000;

#[derive(Clone)]
pub struct ImageGroup {
    pub files: Vec<BigFile>,
}

fn dhash(path: &Path) -> Option<u64> {
    let img = image::open(path).ok()?;
    let small = img.resize_exact(9, 8, image::imageops::FilterType::Triangle).to_luma8();
    let mut h = 0u64;
    for y in 0..8 {
        for x in 0..8 {
            let a = small.get_pixel(x, y)[0];
            let b = small.get_pixel(x + 1, y)[0];
            h = (h << 1) | u64::from(a > b);
        }
    }
    Some(h)
}

pub fn similar_images(roots: Vec<PathBuf>, p: &Progress) -> Vec<ImageGroup> {
    let mut files = vec![];
    let mut stack = roots;
    while let Some(dir) = stack.pop() {
        if p.cancelled() || files.len() >= SIMILAR_MAX_IMAGES {
            break;
        }
        for c in children(&dir) {
            let name = file_name(&c);
            if name.starts_with('.') || name.ends_with(".photoslibrary") || name.ends_with(".app") {
                continue;
            }
            // links are not followed: one pointing back up the tree would never end
            let Ok(md) = std::fs::symlink_metadata(&c) else { continue };
            if md.file_type().is_symlink() {
                continue;
            }
            if md.is_dir() {
                if c != home().join("Library") {
                    stack.push(c);
                }
                continue;
            }
            let ext = c.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            if matches!(ext.as_str(), "jpg" | "jpeg" | "png") && md.len() > 20_000 && md.len() < 40_000_000 {
                files.push((c, md));
            }
        }
    }
    let total = files.len();
    let mut hashed = vec![];
    for (i, (path, md)) in files.into_iter().enumerate() {
        if p.cancelled() {
            return vec![];
        }
        if i % 10 == 0 {
            p.set(format!("Comparing pictures ({}/{total})", i + 1));
        }
        if let Some(h) = dhash(&path) {
            let age = md.modified().map(days_since).unwrap_or(0);
            hashed.push((h, BigFile { name: file_name(&path), size: md.len(), age_days: age, kind: "Picture".into(), path, selected: false }));
        }
    }
    // group each picture with the first earlier group it is close to
    let mut groups: Vec<(u64, Vec<BigFile>)> = vec![];
    for (h, f) in hashed {
        match groups.iter_mut().find(|(g, _)| (g ^ h).count_ones() <= SIMILAR_MAX_BITS) {
            Some((_, v)) => v.push(f),
            None => groups.push((h, vec![f])),
        }
    }
    let mut out: Vec<ImageGroup> = groups
        .into_iter()
        .filter(|(_, v)| v.len() > 1)
        .map(|(_, mut v)| {
            // keep the biggest (usually the best quality); offer the rest
            v.sort_by(|a, b| b.size.cmp(&a.size));
            ImageGroup { files: v }
        })
        .collect();
    out.sort_by(|a, b| b.files.len().cmp(&a.files.len()));
    out
}

// ---------------------------------------------------------------- privacy

#[derive(Clone)]
pub struct PrivacyItem {
    pub browser: &'static str,
    pub what: &'static str,
    pub paths: Vec<PathBuf>,
    pub size: u64,
    pub selected: bool,
}

/// Process names of each browser, to make sure it's closed before its files are moved.
pub fn browser_running(browser: &str) -> bool {
    let names: &[&str] = match browser {
        "Safari" => &["Safari"],
        "Google Chrome" => &["Google Chrome"],
        "Firefox" => &["firefox", "Firefox"],
        _ => &[],
    };
    if names.is_empty() {
        return false;
    }
    let mut sys = sysinfo::System::new().expect("system information");
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    sys.processes().values().any(|pr| names.iter().any(|n| pr.name().to_string_lossy() == *n))
}

pub fn privacy_items(p: &Progress) -> Vec<PrivacyItem> {
    let h = home();
    let mut out = vec![];
    let mut push = |browser: &'static str, what: &'static str, paths: Vec<PathBuf>| {
        let paths: Vec<PathBuf> = paths.into_iter().filter(|x| std::fs::symlink_metadata(x).is_ok()).collect();
        if paths.is_empty() {
            return;
        }
        let size = paths.iter().map(|x| dir_size(x, None)).sum();
        out.push(PrivacyItem { browser, what, paths, size, selected: false });
    };
    p.set("Looking at Safari…");
    push("Safari", "Browsing history", vec![h.join("Library/Safari/History.db"), h.join("Library/Safari/History.db-wal"), h.join("Library/Safari/History.db-shm")]);
    push("Safari", "Cookies", vec![h.join("Library/Cookies/Cookies.binarycookies"), h.join("Library/Containers/com.apple.Safari/Data/Library/Cookies/Cookies.binarycookies")]);
    push("Safari", "Downloads history", vec![h.join("Library/Safari/Downloads.plist")]);
    push("Safari", "Cached pages", vec![h.join("Library/Caches/com.apple.Safari")]);
    p.set("Looking at Chrome…");
    let chrome = h.join("Library/Application Support/Google/Chrome");
    let profiles: Vec<PathBuf> = children(&chrome).into_iter().filter(|d| d.join("Preferences").exists()).collect();
    let each = |names: &[&str]| -> Vec<PathBuf> { profiles.iter().flat_map(|pr| names.iter().map(move |n| pr.join(n))).collect() };
    push("Google Chrome", "Browsing history", each(&["History", "History-journal", "Visited Links", "Top Sites"]));
    push("Google Chrome", "Cookies", each(&["Network/Cookies", "Network/Cookies-journal", "Cookies", "Cookies-journal"]));
    push("Google Chrome", "Autofill data", each(&["Web Data", "Web Data-journal"]));
    push("Google Chrome", "Cached pages", vec![h.join("Library/Caches/Google/Chrome")]);
    p.set("Looking at Firefox…");
    let ff: Vec<PathBuf> = children(&h.join("Library/Application Support/Firefox/Profiles"));
    let each = |names: &[&str]| -> Vec<PathBuf> { ff.iter().flat_map(|pr| names.iter().map(move |n| pr.join(n))).collect() };
    push("Firefox", "Cookies", each(&["cookies.sqlite"]));
    push("Firefox", "Form history", each(&["formhistory.sqlite"]));
    push("Firefox", "Cached pages", vec![h.join("Library/Caches/Firefox")]);
    p.set("Looking at recent items…");
    let recent: Vec<PathBuf> = children(&h.join("Library/Application Support/com.apple.sharedfilelist"))
        .into_iter()
        .filter(|x| file_name(x).contains("RecentDocuments") || file_name(x).contains("RecentApplications") || file_name(x).contains("RecentServers"))
        .collect();
    push("Your Mac", "Recent items lists", recent);
    out
}

/// Saved Wi-Fi networks (shown so old ones can be forgotten).
pub fn wifi_networks() -> Vec<String> {
    let out = std::process::Command::new("networksetup").args(["-listpreferredwirelessnetworks", "en0"]).output();
    let Ok(o) = out else { return vec![] };
    String::from_utf8_lossy(&o.stdout).lines().skip(1).map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()
}

/// A value as one word for the shell: inside single quotes, with any single quote written as '\''.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Text as an AppleScript string. The backslash is escaped first: otherwise a name holding \"
/// would end the string early and the rest would run as script.
fn applescript_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

pub fn forget_wifi(name: &str) -> Result<(), String> {
    let cmd = format!("networksetup -removepreferredwirelessnetwork en0 {}", shell_quote(name));
    let script = format!("do shell script {} with administrator privileges", applescript_string(&cmd));
    let o = std::process::Command::new("osascript").args(["-e", &script]).output().map_err(|e| e.to_string())?;
    if o.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&o.stderr).trim().to_string()) }
}

// ---------------------------------------------------------------- app permissions

/// Permission kinds (macOS privacy categories), their System Settings page and tccutil name.
pub const PERMISSIONS: &[(&str, &str, &str)] = &[
    ("Camera", "Privacy_Camera", "Camera"),
    ("Microphone", "Privacy_Microphone", "Microphone"),
    ("Screen recording", "Privacy_ScreenCapture", "ScreenCapture"),
    ("Accessibility", "Privacy_Accessibility", "Accessibility"),
    ("Full Disk Access", "Privacy_AllFiles", "SystemPolicyAllFiles"),
    ("Files and folders", "Privacy_FilesAndFolders", "SystemPolicyDocumentsFolder"),
    ("Input monitoring", "Privacy_ListenEvent", "ListenEvent"),
    ("Automation", "Privacy_Automation", "AppleEvents"),
    ("Location", "Privacy_LocationServices", "Location"),
    ("Contacts", "Privacy_Contacts", "AddressBook"),
    ("Calendars", "Privacy_Calendars", "Calendar"),
    ("Reminders", "Privacy_Reminders", "Reminders"),
    ("Photos", "Privacy_Photos", "Photos"),
    ("Bluetooth", "Privacy_Bluetooth", "BluetoothAlways"),
    ("Developer tools", "Privacy_DevTools", "DeveloperTool"),
    ("Media and Apple Music", "Privacy_Media", "MediaLibrary"),
    ("App management", "Privacy_AppBundles", "SystemPolicyAppBundles"),
];

pub fn open_privacy_pane(anchor: &str) {
    let _ = std::process::Command::new("open").arg(format!("x-apple.systempreferences:com.apple.preference.security?{anchor}")).spawn();
}

/// (category, app id, allowed) from the user's permissions database, when macOS lets us read it
/// (it needs Full Disk Access).
pub fn granted_permissions() -> Result<Vec<(String, String, bool)>, String> {
    let db = home().join("Library/Application Support/com.apple.TCC/TCC.db");
    let o = std::process::Command::new("sqlite3")
        .args(["-readonly", "-separator", "\t"])
        .arg(&db)
        .arg("select service, client, auth_value from access order by service, client")
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err("macOS keeps this list private until the app has Full Disk Access.".into());
    }
    let names: HashMap<String, &str> = PERMISSIONS.iter().map(|(n, _, t)| (format!("kTCCService{t}"), *n)).collect();
    Ok(String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter_map(|l| {
            let mut it = l.split('\t');
            let svc = it.next()?;
            let client = it.next()?.to_string();
            let allowed = it.next()?.trim() == "2";
            Some((names.get(svc).map(|s| s.to_string()).unwrap_or_else(|| svc.trim_start_matches("kTCCService").to_string()), client, allowed))
        })
        .collect())
}

/// Takes back one app's permission; macOS asks the app again next time.
pub fn reset_permission(category: &str, app_id: &str) -> Result<(), String> {
    let tcc = PERMISSIONS.iter().find(|p| p.0 == category).map(|p| p.2).unwrap_or(category);
    let o = std::process::Command::new("tccutil").args(["reset", tcc, app_id]).output().map_err(|e| e.to_string())?;
    if o.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&o.stderr).trim().to_string()) }
}

// ---------------------------------------------------------------- leftovers of deleted apps

fn looks_like_bundle_id(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() >= 3 && parts.iter().all(|p| !p.is_empty()) && parts[0].chars().all(|c| c.is_ascii_lowercase())
}

fn installed_ids() -> HashSet<String> {
    let mut ids = HashSet::new();
    let mut stack = vec![PathBuf::from("/Applications"), PathBuf::from("/System/Applications"), home().join("Applications")];
    while let Some(d) = stack.pop() {
        for c in children(&d) {
            if c.extension().is_some_and(|e| e == "app") {
                if let Ok(v) = plist::Value::from_file(c.join("Contents/Info.plist")) {
                    if let Some(id) = v.as_dictionary().and_then(|d| d.get("CFBundleIdentifier")).and_then(|v| v.as_string()) {
                        ids.insert(id.to_lowercase());
                    }
                }
            } else if c.is_dir() && d.ends_with("Applications") {
                stack.push(c); // one level of sub-folders (Utilities and the like)
            }
        }
    }
    ids
}

/// Asks Spotlight whether an app with this id is anywhere on the Mac.
fn app_exists_anywhere(id: &str) -> bool {
    let q = format!("kMDItemCFBundleIdentifier == '{}'", id.replace('\'', ""));
    std::process::Command::new("mdfind").arg(q).output().map(|o| o.status.success() && !o.stdout.is_empty()).unwrap_or(false)
}

/// Settings and caches named after apps that are no longer installed.
pub fn app_leftovers(p: &Progress) -> Vec<(String, Vec<(PathBuf, u64)>)> {
    // Spotlight is what tells us an app is gone. If it can't even find the Finder, it is
    // switched off or still indexing, and every installed app would look deleted.
    if !app_exists_anywhere("com.apple.finder") {
        return vec![];
    }
    let h = home();
    let ids = installed_ids();
    let mut by_id: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for d in ["Library/Application Support", "Library/Caches", "Library/Preferences", "Library/Containers", "Library/Saved Application State", "Library/HTTPStorages", "Library/WebKit", "Library/Logs"] {
        for c in children(&h.join(d)) {
            let f = file_name(&c);
            let stem = f.trim_end_matches(".plist").trim_end_matches(".savedstate").trim_end_matches(".binarycookies").to_lowercase();
            if !looks_like_bundle_id(&stem) || stem.starts_with("com.apple.") || stem.starts_with("group.") {
                continue;
            }
            // a helper of an installed app (com.x.app.helper) belongs to that app
            if ids.iter().any(|i| stem == *i || stem.starts_with(&format!("{i}.")) || i.starts_with(&format!("{stem}."))) {
                continue;
            }
            by_id.entry(stem).or_default().push(c);
        }
    }
    let total = by_id.len();
    let mut out = vec![];
    for (i, (id, paths)) in by_id.into_iter().enumerate() {
        if p.cancelled() {
            break;
        }
        p.set(format!("Checking leftovers ({}/{total}) · {id}", i + 1));
        if app_exists_anywhere(&id) {
            continue;
        }
        let items: Vec<(PathBuf, u64)> = paths.into_iter().map(|x| {
            let s = dir_size(&x, None);
            (x, s)
        }).collect();
        out.push((id, items));
    }
    out.sort_by(|a, b| b.1.iter().map(|x| x.1).sum::<u64>().cmp(&a.1.iter().map(|x| x.1).sum::<u64>()));
    out
}

// ---------------------------------------------------------------- app updates

#[derive(Clone)]
pub struct AppUpdate {
    pub name: String,
    pub path: PathBuf,
    pub current: String,
    pub latest: String,
    /// Where to get it: the App Store page or the developer's download.
    pub link: String,
    pub source: &'static str,
}

fn curl(url: &str) -> Option<String> {
    let o = std::process::Command::new("curl").args(["-sL", "--max-time", "8", "-A", "Cleanerio", url]).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).to_string())
}

/// Compares dotted version numbers part by part ("1.10" is newer than "1.9").
pub fn newer(latest: &str, current: &str) -> bool {
    let parse = |s: &str| -> Vec<u64> { s.split(|c: char| !c.is_ascii_digit()).filter(|x| !x.is_empty()).map(|x| x.parse().unwrap_or(0)).collect() };
    let (a, b) = (parse(latest), parse(current));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    false
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let i = tag.find(&format!("{name}=\""))? + name.len() + 2;
    Some(tag[i..].split('"').next()?.to_string())
}

fn elem(xml: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let i = xml.find(&open)? + open.len();
    Some(xml[i..].split('<').next()?.trim().to_string())
}

/// True for the only kinds of link the updater will open: a web page or the App Store.
pub fn safe_link(link: &str) -> bool {
    link.starts_with("https://") || link.starts_with("macappstore://")
}

/// Newest version in a Sparkle update feed (the newest item by version).
fn sparkle_latest(feed: &str) -> Option<(String, String)> {
    let mut best: Option<(String, String)> = None;
    for item in feed.split("<item").skip(1) {
        let enc = item.split("<enclosure").nth(1).map(|e| e.split('>').next().unwrap_or("").to_string()).unwrap_or_default();
        let v = elem(item, "sparkle:shortVersionString")
            .or_else(|| attr(&enc, "sparkle:shortVersionString"))
            .or_else(|| elem(item, "sparkle:version"))
            .or_else(|| attr(&enc, "sparkle:version"))?;
        let link = attr(&enc, "url").unwrap_or_default();
        if best.as_ref().is_none_or(|(b, _)| newer(&v, b)) {
            best = Some((v, link));
        }
    }
    best
}

pub fn app_updates(p: &Progress) -> Vec<AppUpdate> {
    let mut out = vec![];
    let apps: Vec<PathBuf> = [PathBuf::from("/Applications"), home().join("Applications")]
        .iter()
        .flat_map(|d| children(d))
        .filter(|a| a.extension().is_some_and(|e| e == "app"))
        .collect();
    let total = apps.len();
    for (i, a) in apps.into_iter().enumerate() {
        if p.cancelled() {
            break;
        }
        let name = file_name(&a).trim_end_matches(".app").to_string();
        p.set(format!("Checking {name} ({}/{total})", i + 1));
        let Ok(v) = plist::Value::from_file(a.join("Contents/Info.plist")) else { continue };
        let Some(d) = v.as_dictionary() else { continue };
        let get = |k: &str| d.get(k).and_then(|v| v.as_string()).unwrap_or("").to_string();
        let (id, current) = (get("CFBundleIdentifier"), get("CFBundleShortVersionString"));
        if id.is_empty() || current.is_empty() || id.starts_with("com.apple.") {
            continue;
        }
        let feed = get("SUFeedURL");
        if a.join("Contents/_MASReceipt").exists() {
            // App Store app: ask the public App Store lookup
            let Some(j) = curl(&format!("https://itunes.apple.com/lookup?bundleId={id}&entity=macSoftware")) else { continue };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&j) else { continue };
            let Some(r) = v.get("results").and_then(|r| r.get(0)) else { continue };
            let latest = r.get("version").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let link = r.get("trackViewUrl").and_then(|x| x.as_str()).unwrap_or("").replace("https://", "macappstore://");
            if newer(&latest, &current) {
                out.push(AppUpdate { name, path: a, current, latest, link, source: "App Store" });
            }
        } else if feed.starts_with("https://") {
            let Some(x) = curl(&feed) else { continue };
            if let Some((latest, link)) = sparkle_latest(&x) {
                if newer(&latest, &current) {
                    out.push(AppUpdate { name, path: a, current, latest, link, source: "Developer" });
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------- browser tabs

#[derive(Clone)]
pub struct Tab {
    pub browser: String,
    pub window: usize,
    pub index: usize,
    pub title: String,
    pub url: String,
    pub duplicate: bool,
    pub selected: bool,
}

const TABS_JS: &str = r#"
var out = [];
['Safari', 'Google Chrome'].forEach(function (name) {
  var app = Application(name);
  if (!app.running()) return;
  app.windows().forEach(function (w, wi) {
    try {
      w.tabs().forEach(function (t, ti) {
        out.push({browser: name, window: wi, index: ti, title: name === 'Safari' ? t.name() : t.title(), url: t.url() || ''});
      });
    } catch (e) {}
  });
});
JSON.stringify(out);
"#;

/// Open tabs of Safari and Chrome (only browsers that are already running are asked).
pub fn browser_tabs() -> Result<Vec<Tab>, String> {
    let o = std::process::Command::new("osascript").args(["-l", "JavaScript", "-e", TABS_JS]).output().map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).trim().to_string());
    }
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).map_err(|e| e.to_string())?;
    let mut seen = HashSet::new();
    let mut tabs = vec![];
    for t in v.as_array().cloned().unwrap_or_default() {
        let url = t["url"].as_str().unwrap_or("").to_string();
        let dup = !url.is_empty() && !seen.insert(url.clone());
        tabs.push(Tab {
            browser: t["browser"].as_str().unwrap_or("").into(),
            window: t["window"].as_u64().unwrap_or(0) as usize,
            index: t["index"].as_u64().unwrap_or(0) as usize,
            title: t["title"].as_str().unwrap_or("").into(),
            url,
            duplicate: dup,
            selected: dup,
        });
    }
    Ok(tabs)
}

/// Closes the chosen tabs and returns how many were closed. A tab is closed only if the
/// address at its place is still the one that was listed; if tabs were opened or closed in
/// between, the place may hold a different page, and that one is left alone. Last ones go
/// first, so the places of the others stay right.
pub fn close_tabs(mut tabs: Vec<Tab>) -> Result<usize, String> {
    tabs.sort_by(|a, b| (b.browser.clone(), b.window, b.index).cmp(&(a.browser.clone(), a.window, a.index)));
    let wanted: Vec<serde_json::Value> =
        tabs.iter().map(|t| serde_json::json!({"browser": t.browser, "window": t.window, "index": t.index, "url": t.url})).collect();
    let js = format!(
        "var wanted = {}; var closed = 0;\nwanted.forEach(function (t) {{ try {{ var tab = Application(t.browser).windows[t.window].tabs[t.index]; if ((tab.url() || '') === t.url) {{ tab.close(); closed++; }} }} catch (e) {{}} }});\nclosed;",
        serde_json::Value::Array(wanted)
    );
    let o = std::process::Command::new("osascript").args(["-l", "JavaScript", "-e", &js]).output().map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim().parse().unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(newer("1.10", "1.9"));
        assert!(!newer("2.0", "2.0.0"));
        assert!(newer("2.0.1", "2.0"));
    }

    #[test]
    fn sparkle() {
        let x = r#"<rss><channel><item><title>2</title><enclosure url="https://x/a.zip" sparkle:shortVersionString="2.1" sparkle:version="210"/></item>
        <item><sparkle:shortVersionString>2.3</sparkle:shortVersionString><enclosure url="https://x/b.zip"/></item></channel></rss>"#;
        assert_eq!(sparkle_latest(x).unwrap().0, "2.3");
    }

    #[test]
    fn quoting_for_the_password_prompt() {
        assert_eq!(shell_quote("Cafe's WiFi"), "'Cafe'\\''s WiFi'");
        // a name built to end the AppleScript string early stays inside it
        let s = applescript_string("x\\\" & do shell script \"id");
        assert_eq!(s, "\"x\\\\\\\" & do shell script \\\"id\"");
    }

    #[test]
    fn only_web_and_app_store_links_open() {
        assert!(safe_link("https://example.com/a.dmg") && safe_link("macappstore://apps.apple.com/app/id1"));
        assert!(!safe_link("file:///Applications/Calculator.app") && !safe_link("http://example.com/a.dmg"));
    }

    #[test]
    fn bundle_ids() {
        assert!(looks_like_bundle_id("com.acme.paint"));
        assert!(!looks_like_bundle_id("Paint"));
        assert!(!looks_like_bundle_id("com.acme"));
    }
}
