//! Everything that looks at the disk: junk, background items, apps, big files and folder sizes.
//! Nothing here deletes anything; removal always goes through the Trash (see `trash_paths`).

use std::collections::HashSet;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

// ---------------------------------------------------------------- background jobs

pub struct Progress {
    status: Arc<Mutex<String>>,
    cancel: Arc<AtomicBool>,
    stage: Arc<AtomicUsize>,
}

impl Progress {
    pub fn set(&self, s: impl Into<String>) {
        *self.status.lock().unwrap_or_else(|e| e.into_inner()) = s.into();
    }
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
    pub fn stage(&self, n: usize) {
        self.stage.store(n, Ordering::Relaxed);
    }
}

pub struct Job<T> {
    status: Arc<Mutex<String>>,
    result: Arc<Mutex<Option<T>>>,
    cancel: Arc<AtomicBool>,
    stage: Arc<AtomicUsize>,
    failed: Arc<AtomicBool>,
}

impl<T: Send + 'static> Job<T> {
    pub fn spawn(f: impl FnOnce(&Progress) -> T + Send + 'static) -> Self {
        let status = Arc::new(Mutex::new(String::from("Starting…")));
        let result = Arc::new(Mutex::new(None));
        let cancel = Arc::new(AtomicBool::new(false));
        let stage = Arc::new(AtomicUsize::new(0));
        let p = Progress { status: status.clone(), cancel: cancel.clone(), stage: stage.clone() };
        let r = result.clone();
        let failed = Arc::new(AtomicBool::new(false));
        let fail = failed.clone();
        std::thread::spawn(move || {
            // a crash in the work must not leave the screen waiting forever
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&p))) {
                Ok(out) => *r.lock().unwrap_or_else(|e| e.into_inner()) = Some(out),
                Err(_) => fail.store(true, Ordering::Relaxed),
            }
        });
        Job { status, result, cancel, stage, failed }
    }
    /// The work crashed and will never give a result.
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }
    /// The person pressed Stop.
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
    /// A result is waiting to be collected.
    pub fn finished(&self) -> bool {
        self.result.lock().unwrap_or_else(|e| e.into_inner()).is_some()
    }
    pub fn stage(&self) -> usize {
        self.stage.load(Ordering::Relaxed)
    }
    pub fn status(&self) -> String {
        self.status.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    pub fn take(&self) -> Option<T> {
        self.result.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------- helpers

pub fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

/// Space a file really takes on disk.
fn alloc_size(md: &std::fs::Metadata) -> u64 {
    md.blocks() * 512
}

/// Total size of a file or folder, without following links.
pub fn dir_size(path: &Path, p: Option<&Progress>) -> u64 {
    let Ok(md) = std::fs::symlink_metadata(path) else { return 0 };
    if !md.is_dir() {
        return alloc_size(&md);
    }
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    let mut seen = 0u32;
    while let Some(dir) = stack.pop() {
        if p.is_some_and(|p| p.cancelled()) {
            break;
        }
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let Ok(md) = e.metadata() else { continue };
            if md.is_dir() {
                stack.push(e.path());
            } else {
                total += alloc_size(&md);
            }
            seen += 1;
            if seen % 2000 == 0 {
                if let Some(p) = p {
                    p.set(format!("Measuring {}", short(&dir)));
                }
            }
        }
    }
    total
}

pub fn short(p: &Path) -> String {
    let h = home();
    match p.strip_prefix(&h) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => p.display().to_string(),
    }
}

pub fn file_name(p: &Path) -> String {
    p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| p.display().to_string())
}

pub fn children(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(rd) => rd.flatten().map(|e| e.path()).collect(),
        Err(_) => vec![],
    };
    v.retain(|p| file_name(p) != ".DS_Store" && file_name(p) != ".localized");
    v
}

pub fn days_since(t: SystemTime) -> u64 {
    SystemTime::now().duration_since(t).map(|d| d.as_secs() / 86_400).unwrap_or(0)
}

/// Folders that macOS or the person's account can't work without. Nothing here is ever moved
/// to the Trash, whichever screen asks (see `trash_paths`).
pub fn is_protected(path: &Path) -> bool {
    let h = home();
    // the home folder and everything above it, up to "/"
    if h.starts_with(path) {
        return true;
    }
    const SYSTEM: &[&str] = &["/System", "/usr", "/bin", "/sbin"];
    if SYSTEM.iter().any(|s| path.starts_with(s)) {
        return true;
    }
    const TOP: &[&str] = &["/Library", "/Applications", "/Users", "/private", "/etc", "/var", "/tmp", "/opt", "/cores", "/Volumes", "/dev"];
    if TOP.iter().any(|s| path == Path::new(s)) {
        return true;
    }
    // other people's home folders and the first level of /Library
    if path.parent().is_some_and(|p| p == Path::new("/Users") || p == Path::new("/Library")) {
        return true;
    }
    // the standard folders in the home folder, and the first level of ~/Library
    const HOME: &[&str] = &["Library", "Documents", "Desktop", "Downloads", "Movies", "Music", "Pictures", "Public", "Applications", ".Trash"];
    if HOME.iter().any(|d| path == h.join(d)) {
        return true;
    }
    path.parent().is_some_and(|p| p == h.join("Library"))
}

/// Places the space map never walks into: other drives, network shares and the second view
/// of the startup disk (counting it would double every size).
fn outside_this_disk(path: &Path) -> bool {
    const SKIP: &[&str] = &["/System/Volumes", "/Volumes", "/dev", "/net", "/home"];
    SKIP.iter().any(|s| path == Path::new(s))
}

/// Seconds since 1970 for a date written as "2026-06-01 10:22:33 +0000" (what `mdls` prints).
pub fn parse_mdls_date(s: &str) -> Option<u64> {
    let mut parts = s.split_whitespace();
    let mut d = parts.next()?.split('-').map(|x| x.parse::<i64>());
    let (y, m, day) = (d.next()?.ok()?, d.next()?.ok()?, d.next()?.ok()?);
    let mut t = parts.next()?.split(':').map(|x| x.parse::<i64>());
    let (h, mi, sec) = (t.next()?.ok()?, t.next()?.ok()?, t.next()?.ok()?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&day) {
        return None;
    }
    // days from 1970-01-01 (proleptic Gregorian calendar)
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400 + h * 3600 + mi * 60 + sec).ok()
}

/// Days since the item was last opened, as macOS itself recorded it (the "Last opened" date
/// Finder shows). None when macOS has no record. File access times are not used for this:
/// backups, indexing and this app's own scans change them.
pub fn last_opened_days(path: &Path) -> Option<u64> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let out = std::process::Command::new("mdls").args(["-raw", "-name", "kMDItemLastUsedDate"]).arg(path).output().ok()?;
    let then = parse_mdls_date(String::from_utf8_lossy(&out.stdout).trim())?;
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).ok()?.as_secs();
    Some(now.saturating_sub(then) / 86_400)
}

/// Last opened if macOS knows it; otherwise the newer of the file's own dates, as before.
fn idle_days(path: &Path, md: Option<&std::fs::Metadata>) -> u64 {
    last_opened_days(path)
        .or_else(|| md.and_then(|m| m.accessed().ok().max(m.modified().ok())).map(days_since))
        .unwrap_or(0)
}

/// Bundle ids (lower case) of the apps that are running now.
pub fn running_bundle_ids() -> HashSet<String> {
    let mut ids = HashSet::new();
    let Ok(mut sys) = sysinfo::System::new() else { return ids };
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let mut seen: HashSet<String> = HashSet::new();
    for proc_ in sys.processes().values() {
        let Some(exe) = proc_.exe() else { continue };
        let exe = exe.to_string_lossy().to_string();
        let Some(i) = exe.find(".app/Contents/MacOS/") else { continue };
        let app = exe[..i + 4].to_string();
        if !seen.insert(app.clone()) {
            continue;
        }
        if let Ok(v) = plist::Value::from_file(Path::new(&app).join("Contents/Info.plist")) {
            if let Some(id) = v.as_dictionary().and_then(|d| d.get("CFBundleIdentifier")).and_then(|v| v.as_string()) {
                ids.insert(id.to_lowercase());
            }
        }
    }
    ids
}

/// True when a program inside this app bundle is running.
pub fn app_is_running(app: &Path) -> bool {
    let Ok(mut sys) = sysinfo::System::new() else { return false };
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    sys.processes().values().any(|p| p.exe().is_some_and(|e| e.starts_with(app)))
}

/// A cache folder belongs to a running app when it is named after the app's bundle id.
pub fn cache_in_use(folder_name: &str, running: &HashSet<String>) -> bool {
    let n = folder_name.to_lowercase();
    running.iter().any(|id| n == *id || n.starts_with(&format!("{id}.")))
}

/// The launchd label of a login agent file in a LaunchAgents folder.
fn agent_label(path: &Path) -> Option<String> {
    let in_agents = path.parent().and_then(|p| p.file_name()).is_some_and(|n| n == "LaunchAgents");
    (in_agents && path.extension().is_some_and(|e| e == "plist")).then(|| plist_program(path).0)
}

/// Stops a login agent that is loaded, so removing its file takes effect now and not only
/// after the next login. Failure is fine: it may simply not be loaded.
fn unload_agent(label: &str) {
    if !cfg!(target_os = "macos") {
        return;
    }
    if let Ok(md) = std::fs::metadata(home()) {
        let _ = std::process::Command::new("launchctl").arg("bootout").arg(format!("gui/{}/{label}", md.uid())).output();
    }
}

/// The apps the person opened that use the most memory: (id, name, memory, processor share).
/// macOS's own programs and this app are left out, so they can't be quit by mistake.
/// Looking at every running program takes a moment, so this runs on a background thread.
pub fn heavy_apps() -> Vec<(sysinfo::Pid, String, u64, f32)> {
    let Ok(mut sys) = sysinfo::System::new() else { return vec![] };
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let mut v: Vec<_> = sys
        .processes()
        .iter()
        .filter(|(pid, p)| {
            pid.as_u32() != std::process::id()
                && p.exe().is_some_and(|e| {
                    let e = e.to_string_lossy();
                    e.contains(".app/Contents/MacOS/") && !e.starts_with("/System/") && !e.starts_with("/usr/")
                })
        })
        .map(|(pid, p)| (*pid, p.name().to_string_lossy().to_string(), p.memory(), p.cpu_usage()))
        .collect();
    v.sort_by(|a, b| b.2.cmp(&a.2));
    v.truncate(8);
    v
}

/// Asks a program to close (the polite signal; it can save its work first).
pub fn ask_to_quit(pid: u32) {
    let _ = std::process::Command::new("kill").arg("-TERM").arg(pid.to_string()).output();
}

// ---------------------------------------------------------------- junk

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum JunkKind {
    Caches,
    Logs,
    Developer,
    Installers,
    Mail,
    Trash,
    Ai,
    Backups,
    BrokenPrefs,
}

#[derive(Clone)]
pub struct JunkItem {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub selected: bool,
    /// Belongs to an app that is running now; not ticked, because the app is still using it.
    pub in_use: bool,
}

#[derive(Clone)]
pub struct JunkGroup {
    pub kind: JunkKind,
    pub title: &'static str,
    pub desc: &'static str,
    pub icon: &'static str,
    pub items: Vec<JunkItem>,
    pub expanded: bool,
    pub note: Option<String>,
}

impl JunkGroup {
    pub fn total(&self) -> u64 {
        self.items.iter().map(|i| i.size).sum()
    }
    pub fn selected_size(&self) -> u64 {
        self.items.iter().filter(|i| i.selected).map(|i| i.size).sum()
    }
}

fn junk_items(dirs: &[PathBuf], filter: impl Fn(&Path) -> bool, on: bool, p: &Progress) -> (Vec<JunkItem>, Option<String>) {
    let mut out = vec![];
    let mut blocked = false;
    for d in dirs {
        if !d.exists() {
            continue;
        }
        if std::fs::read_dir(d).is_err() {
            blocked = true;
            continue;
        }
        for c in children(d) {
            if p.cancelled() {
                break;
            }
            if !filter(&c) {
                continue;
            }
            p.set(format!("Looking at {}", short(&c)));
            let size = dir_size(&c, Some(p));
            if size == 0 {
                continue;
            }
            out.push(JunkItem { name: file_name(&c), path: c, size, selected: on, in_use: false });
        }
    }
    out.sort_by(|a, b| b.size.cmp(&a.size));
    let note = blocked.then(|| "macOS didn't let us look here. Give this app Full Disk Access in System Settings › Privacy & Security to include it.".to_string());
    (out, note)
}

pub fn scan_junk(p: &Progress) -> Vec<JunkGroup> {
    let h = home();
    let mut groups = vec![];

    p.set("Looking for cache files…");
    let (mut items, note) = junk_items(&[h.join("Library/Caches")], |_| true, true, p);
    let running = running_bundle_ids();
    for it in items.iter_mut() {
        if cache_in_use(&it.name, &running) {
            it.in_use = true;
            it.selected = false;
        }
    }
    groups.push(JunkGroup {
        kind: JunkKind::Caches,
        title: "Cache files",
        desc: "Temporary files apps keep to load faster. Apps rebuild them when needed.",
        icon: "🗄",
        items,
        expanded: false,
        note,
    });

    p.set("Looking for log files…");
    let (items, note) = junk_items(&[h.join("Library/Logs")], |_| true, true, p);
    groups.push(JunkGroup {
        kind: JunkKind::Logs,
        title: "Log files",
        desc: "Records of app activity that are mostly useful for troubleshooting.",
        icon: "📄",
        items,
        expanded: false,
        note,
    });

    p.set("Looking for developer leftovers…");
    let (items, note) = junk_items(
        &[
            h.join("Library/Developer/Xcode/DerivedData"),
            h.join("Library/Developer/Xcode/iOS DeviceSupport"),
            h.join("Library/Developer/CoreSimulator/Caches"),
            h.join("Library/Developer/Xcode/iOS Device Logs"),
            h.join("Library/Developer/Xcode/Archives"),
        ],
        |_| true,
        true,
        p,
    );
    groups.push(JunkGroup {
        kind: JunkKind::Developer,
        title: "Developer junk",
        desc: "Build products and device support files that coding tools can regenerate.",
        icon: "🛠",
        items,
        expanded: false,
        note,
    });

    p.set("Looking for mail downloads…");
    let (items, note) = junk_items(
        &[
            h.join("Library/Containers/com.apple.mail/Data/Library/Mail Downloads"),
            h.join("Library/Mail Downloads"),
        ],
        |_| true,
        false,
        p,
    );
    groups.push(JunkGroup {
        kind: JunkKind::Mail,
        title: "Mail downloads",
        desc: "Copies of email attachments you opened. The originals stay in your mail.",
        icon: "✉",
        items,
        expanded: false,
        note,
    });

    p.set("Looking for old installers…");
    let (items, note) = junk_items(
        &[h.join("Downloads"), h.join("Desktop")],
        |c| {
            let ext = c.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            matches!(ext.as_str(), "dmg" | "pkg" | "mpkg" | "xip" | "iso")
        },
        false,
        p,
    );
    groups.push(JunkGroup {
        kind: JunkKind::Installers,
        title: "Old installers",
        desc: "Disk images and installer packages left in Downloads or on the Desktop.",
        icon: "💿",
        items,
        expanded: false,
        note,
    });

    p.set("Looking for AI clutter…");
    groups.push(JunkGroup {
        kind: JunkKind::Ai,
        title: "AI clutter",
        desc: "Caches, logs and old versions left by AI tools. Downloaded local models are listed but not ticked.",
        icon: "",
        items: crate::extra::ai_clutter(p),
        expanded: false,
        note: None,
    });

    p.set("Looking for phone backups…");
    let (items, note) = junk_items(&[h.join("Library/Application Support/MobileSync/Backup")], |_| true, false, p);
    groups.push(JunkGroup {
        kind: JunkKind::Backups,
        title: "iPhone and iPad backups",
        desc: "Local backups of your devices. Keep the newest if you rely on it.",
        icon: "",
        items,
        expanded: false,
        note,
    });

    p.set("Looking for broken settings files…");
    groups.push(JunkGroup {
        kind: JunkKind::BrokenPrefs,
        title: "Broken preferences",
        desc: "Settings files that can't be read anymore. Apps make fresh ones when needed.",
        icon: "",
        items: crate::extra::broken_prefs(p),
        expanded: false,
        note: None,
    });

    p.set("Looking in the Trash…");
    let mut bins = vec![h.join(".Trash")];
    // Trash bins on other drives (the original's "Mounted Trash Bins")
    let uid = std::fs::metadata(&h).map(|m| m.uid()).unwrap_or(501);
    for v in children(Path::new("/Volumes")) {
        bins.push(v.join(".Trashes").join(uid.to_string()));
    }
    let (items, note) = junk_items(&bins, |_| true, false, p);
    groups.push(JunkGroup {
        kind: JunkKind::Trash,
        title: "Trash",
        desc: "Files you already moved to the Trash. Emptying it frees the space for good.",
        icon: "🗑",
        items,
        expanded: false,
        note,
    });
    groups
}

// ---------------------------------------------------------------- protection

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verdict {
    Threat,
    Review,
    Ok,
}

#[derive(Clone)]
pub struct AgentItem {
    pub path: PathBuf,
    pub label: String,
    pub program: String,
    pub place: &'static str,
    pub verdict: Verdict,
    pub reason: String,
    /// Adware, Trojan, Spyware, Miner or Unwanted when the threat database matched.
    pub category: String,
    pub selected: bool,
    /// Process id when the threat is running right now.
    pub pid: Option<u32>,
}

pub fn plist_program(path: &Path) -> (String, String) {
    let Ok(v) = plist::Value::from_file(path) else { return (file_name(path), String::new()) };
    let Some(d) = v.as_dictionary() else { return (file_name(path), String::new()) };
    let label = d.get("Label").and_then(|v| v.as_string()).unwrap_or("").to_string();
    let program = d
        .get("Program")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string())
        .or_else(|| {
            d.get("ProgramArguments")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_string())
                .map(|s| s.to_string())
        })
        .unwrap_or_default();
    (if label.is_empty() { file_name(path) } else { label }, program)
}

pub fn judge(label: &str, program: &str, file: &str) -> (Verdict, String) {
    let all = format!("{label} {program} {file}").to_lowercase();
    if let Some((fam, w)) = crate::threats::load_cached().match_words_pub(&all) {
        return (Verdict::Threat, format!("{} ({}) · matches “{w}”", fam.0, fam.1));
    }
    if !program.is_empty() {
        let pp = Path::new(program);
        if program.starts_with("/tmp") || program.starts_with("/private/tmp") || program.starts_with("/Users/Shared") {
            return (Verdict::Review, "Runs a program from a shared or temporary folder".into());
        }
        if pp.components().any(|c| {
            let s = c.as_os_str().to_string_lossy();
            s.starts_with('.') && s.len() > 1 && s != ".."
        }) {
            return (Verdict::Review, "Runs a program hidden in a dot-folder".into());
        }
        if pp.is_absolute() && !pp.exists() {
            return (Verdict::Review, "Points to a program that no longer exists".into());
        }
    }
    (Verdict::Ok, "Looks normal".into())
}

pub fn scan_agents(p: &Progress) -> Vec<AgentItem> {
    crate::threats::scan(&crate::threats::load(), p)
}

// ---------------------------------------------------------------- applications

#[derive(Clone)]
pub struct AppInfo {
    pub name: String,
    pub path: PathBuf,
    pub bundle_id: String,
    pub version: String,
    pub size: u64,
    pub leftovers: Vec<(PathBuf, u64)>,
    /// Leftovers that are not ticked for removal. Files matched only by the app's name (not
    /// its bundle id) start here: a folder called "Steam" may hold a whole games library.
    pub skip: HashSet<PathBuf>,
    pub last_used_days: u64,
}

impl AppInfo {
    pub fn total(&self) -> u64 {
        self.size + self.leftovers.iter().map(|l| l.1).sum::<u64>()
    }
    /// The leftovers that are ticked.
    pub fn chosen(&self) -> Vec<(PathBuf, u64)> {
        self.leftovers.iter().filter(|l| !self.skip.contains(&l.0)).cloned().collect()
    }
    /// The app plus its ticked leftovers.
    pub fn chosen_total(&self) -> u64 {
        self.size + self.chosen().iter().map(|l| l.1).sum::<u64>()
    }
}

/// Leftovers matched by the app's name only; these are not ticked by default.
pub fn name_only_leftovers(bundle_id: &str, leftovers: &[(PathBuf, u64)]) -> HashSet<PathBuf> {
    leftovers
        .iter()
        .filter(|l| find_leftovers("", bundle_id, std::slice::from_ref(&l.0)).is_empty())
        .map(|l| l.0.clone())
        .collect()
}

pub const LEFTOVER_DIRS: &[&str] = &[
    "Library/Application Support",
    "Library/Caches",
    "Library/Preferences",
    "Library/Containers",
    "Library/Group Containers",
    "Library/Saved Application State",
    "Library/Logs",
    "Library/HTTPStorages",
    "Library/WebKit",
    "Library/Cookies",
    "Library/LaunchAgents",
];

pub fn find_leftovers(name: &str, bundle_id: &str, lib: &[PathBuf]) -> Vec<PathBuf> {
    let name_l = name.to_lowercase();
    let id_l = bundle_id.to_lowercase();
    lib.iter()
        .filter(|p| {
            let f = file_name(p).to_lowercase();
            let stem = f.trim_end_matches(".plist").trim_end_matches(".savedstate").trim_end_matches(".binarycookies");
            (!id_l.is_empty() && (stem == id_l || stem.starts_with(&format!("{id_l}.")) || stem.ends_with(&format!(".{id_l}"))))
                || (name_l.len() > 2 && stem == name_l)
        })
        .cloned()
        .collect()
}

pub fn scan_apps(p: &Progress) -> Vec<AppInfo> {
    let h = home();
    let lib: Vec<PathBuf> = LEFTOVER_DIRS.iter().flat_map(|d| children(&h.join(d))).collect();
    let mut bundles = vec![];
    for dir in [PathBuf::from("/Applications"), h.join("Applications")] {
        for a in children(&dir) {
            if a.extension().is_some_and(|e| e == "app") {
                bundles.push(a);
            }
        }
    }
    let mut out = vec![];
    for a in bundles {
        if p.cancelled() {
            break;
        }
        let name = file_name(&a).trim_end_matches(".app").to_string();
        p.set(format!("Measuring {name}"));
        let (mut bundle_id, mut version) = (String::new(), String::new());
        if let Ok(v) = plist::Value::from_file(a.join("Contents/Info.plist")) {
            if let Some(d) = v.as_dictionary() {
                bundle_id = d.get("CFBundleIdentifier").and_then(|v| v.as_string()).unwrap_or("").to_string();
                version = d.get("CFBundleShortVersionString").and_then(|v| v.as_string()).unwrap_or("").to_string();
            }
        }
        if bundle_id.starts_with("com.apple.") {
            continue; // built into macOS
        }
        let size = dir_size(&a, Some(p));
        let leftovers = find_leftovers(&name, &bundle_id, &lib).into_iter().map(|l| {
            let s = dir_size(&l, None);
            (l, s)
        });
        let last_used_days = idle_days(&a, std::fs::metadata(&a).ok().as_ref());
        let leftovers: Vec<(PathBuf, u64)> = leftovers.collect();
        let skip = name_only_leftovers(&bundle_id, &leftovers);
        out.push(AppInfo { name, path: a, bundle_id, version, size, leftovers, skip, last_used_days });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

// ---------------------------------------------------------------- large & old files

#[derive(Clone)]
pub struct BigFile {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub age_days: u64,
    pub kind: String,
    pub selected: bool,
}

pub fn kind_of(p: &Path) -> String {
    let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "mov" | "mp4" | "m4v" | "mkv" | "avi" | "webm" => "Video",
        "mp3" | "m4a" | "wav" | "aiff" | "flac" | "aac" => "Audio",
        "jpg" | "jpeg" | "png" | "heic" | "tiff" | "gif" | "raw" | "psd" => "Picture",
        "zip" | "rar" | "7z" | "tar" | "gz" | "xz" | "tgz" => "Archive",
        "dmg" | "pkg" | "iso" | "xip" => "Installer",
        "pdf" | "doc" | "docx" | "pages" | "key" | "ppt" | "pptx" | "xls" | "xlsx" | "numbers" => "Document",
        "app" => "App",
        _ => "Other",
    }
    .to_string()
}

pub fn scan_big(root: PathBuf, min_bytes: u64, p: &Progress) -> Vec<BigFile> {
    let h = home();
    let skip: HashSet<PathBuf> = [h.join("Library"), h.join(".Trash")].into_iter().collect();
    let mut out = vec![];
    let mut stack = vec![root];
    let mut n = 0u64;
    while let Some(dir) = stack.pop() {
        if p.cancelled() {
            break;
        }
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let path = e.path();
            let name = file_name(&path);
            if name.starts_with('.') || skip.contains(&path) {
                continue;
            }
            let Ok(md) = e.metadata() else { continue };
            n += 1;
            if n % 3000 == 0 {
                p.set(format!("{n} files checked · {}", short(&dir)));
            }
            if md.is_dir() {
                if name.ends_with(".app") || name.ends_with(".photoslibrary") || name.ends_with(".musiclibrary") {
                    continue;
                }
                stack.push(path);
            } else if md.len() >= min_bytes {
                out.push(BigFile {
                    kind: kind_of(&path),
                    name,
                    size: md.len(),
                    age_days: idle_days(&path, Some(&md)),
                    path,
                    selected: false,
                });
            }
        }
    }
    out.sort_by(|a, b| b.size.cmp(&a.size));
    out
}

// ---------------------------------------------------------------- space map

pub struct Node {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub is_dir: bool,
    pub children: Vec<Node>,
    pub items: u64,
}

const KEEP: usize = 40;

fn build_node(path: &Path, p: &Progress, depth: usize, counter: &mut u64) -> Node {
    let name = file_name(path);
    let Ok(md) = std::fs::symlink_metadata(path) else {
        return Node { name, path: path.into(), size: 0, is_dir: false, children: vec![], items: 0 };
    };
    if !md.is_dir() {
        return Node { name, path: path.into(), size: alloc_size(&md), is_dir: false, children: vec![], items: 1 };
    }
    *counter += 1;
    if *counter % 300 == 0 {
        p.set(format!("Mapping {}", short(path)));
    }
    let mut kids: Vec<Node> = vec![];
    if !p.cancelled() {
        if let Ok(rd) = std::fs::read_dir(path) {
            for e in rd.flatten() {
                let child = e.path();
                if outside_this_disk(&child) {
                    continue;
                }
                kids.push(build_node(&child, p, depth + 1, counter));
            }
        }
    }
    let size = kids.iter().map(|k| k.size).sum();
    let items = kids.iter().map(|k| k.items).sum::<u64>() + 1;
    kids.sort_by(|a, b| b.size.cmp(&a.size));
    if kids.len() > KEEP {
        let rest: Vec<Node> = kids.split_off(KEEP - 1);
        let rs: u64 = rest.iter().map(|k| k.size).sum();
        let ri: u64 = rest.iter().map(|k| k.items).sum();
        kids.push(Node {
            name: format!("{} smaller items", rest.len()),
            path: path.into(),
            size: rs,
            is_dir: false,
            children: vec![],
            items: ri,
        });
    }
    // Drop deep detail we will never draw to save memory.
    if depth > 12 {
        kids.clear();
    }
    Node { name, path: path.into(), size, is_dir: true, children: kids, items }
}

pub fn scan_space(root: PathBuf, p: &Progress) -> Node {
    let mut c = 0;
    build_node(&root, p, 0, &mut c)
}

// ---------------------------------------------------------------- actions

#[derive(Default)]
pub struct ActionReport {
    pub removed: Vec<PathBuf>,
    pub failed: Vec<(PathBuf, String)>,
    pub freed: u64,
    pub message: String,
}

/// Moves files to the Trash. Protected folders are refused.
pub fn trash_paths(paths: Vec<(PathBuf, u64)>, p: &Progress) -> ActionReport {
    let mut r = ActionReport::default();
    let mut todo: Vec<(PathBuf, u64, Option<String>)> = vec![];
    for (path, size) in paths {
        if is_protected(&path) {
            r.failed.push((path, "protected folder".into()));
        } else if std::fs::symlink_metadata(&path).is_err() {
            r.removed.push(path); // already gone
        } else {
            let agent = agent_label(&path);
            todo.push((path, size, agent));
        }
    }
    let done = |r: &mut ActionReport, path: PathBuf, size: u64, agent: Option<String>| {
        if let Some(label) = agent {
            unload_agent(&label);
        }
        r.freed += size;
        r.removed.push(path);
    };
    // All together first: one request to the Finder instead of one per file.
    if todo.len() > 1 {
        p.set(format!("Moving {} items to the Trash…", todo.len()));
        if trash::delete_all(todo.iter().map(|t| &t.0)).is_ok() {
            for (path, size, agent) in todo {
                done(&mut r, path, size, agent);
            }
            return r;
        }
    }
    // One by one, so a single failure doesn't stop the rest.
    let n = todo.len();
    for (i, (path, size, agent)) in todo.into_iter().enumerate() {
        p.set(format!("Moving to Trash ({}/{n}) {}", i + 1, file_name(&path)));
        if std::fs::symlink_metadata(&path).is_err() {
            done(&mut r, path, size, agent); // the attempt above already moved it
            continue;
        }
        match trash::delete(&path) {
            Ok(()) => done(&mut r, path, size, agent),
            Err(e) => r.failed.push((path, e.to_string())),
        }
    }
    r
}

pub fn empty_trash(p: &Progress, size: u64) -> ActionReport {
    p.set("Emptying the Trash…");
    // list what is in the Trash first: afterwards there is nothing left to list
    let before = children(&home().join(".Trash"));
    let _ = size; // already counted when the files were moved to the Trash
    let out = std::process::Command::new("osascript")
        .args(["-e", "tell application \"Finder\" to empty trash"])
        .output();
    let mut r = ActionReport::default();
    match out {
        Ok(o) if o.status.success() => {
            r.removed = before;
            r.message = "The Trash is empty.".into();
        }
        Ok(o) => r.message = format!("Couldn't empty the Trash: {}", String::from_utf8_lossy(&o.stderr).trim()),
        Err(e) => r.message = format!("Couldn't empty the Trash: {e}"),
    }
    r
}

// ---------------------------------------------------------------- maintenance

pub struct Task {
    pub title: &'static str,
    pub desc: &'static str,
    pub icon: &'static str,
    pub admin: bool,
    pub cmd: &'static str,
    pub selected: bool,
    pub last: Option<String>,
}

/// Smallest file the duplicate finder compares (estimated; the original's value wasn't found).
pub const DUPLICATE_MIN_BYTES: u64 = 100_000;

pub fn tasks() -> Vec<Task> {
    vec![
        Task { title: "Free up memory", desc: "Rarely needed: macOS manages memory well on its own.", icon: "🧠", admin: true, cmd: "purge", selected: false, last: None },
        Task { title: "Flush DNS cache", desc: "Fixes websites that won't load after network changes.", icon: "🌐", admin: true, cmd: "dscacheutil -flushcache; killall -HUP mDNSResponder", selected: false, last: None },
        Task { title: "Reindex Spotlight", desc: "Rebuilds the search index. Search is slow for an hour or more while it works.", icon: "🔍", admin: true, cmd: "mdutil -E /", selected: false, last: None },
        Task { title: "Rebuild Launch Services", desc: "Fixes duplicate entries in “Open With” menus. May do little on recent macOS.", icon: "📋", admin: false, cmd: "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -kill -r -domain local -domain system -domain user", selected: false, last: None },
        Task { title: "Clear Quick Look cache", desc: "Refreshes file previews that show the wrong picture.", icon: "👁", admin: false, cmd: "qlmanage -r cache", selected: true, last: None },
        Task { title: "Free up purgeable space", desc: "Thins the local snapshots macOS keeps, so space it holds back becomes free.", icon: "", admin: true, cmd: "tmutil thinlocalsnapshots / 999999999999 4", selected: false, last: None },
        Task { title: "Thin Time Machine snapshots", desc: "Deletes the local Time Machine snapshots on this disk. Backups on your backup drive stay.", icon: "", admin: true, cmd: "tmutil deletelocalsnapshots /", selected: false, last: None },
        Task { title: "Speed up Mail", desc: "Compacts Mail's message index so searching and opening mail is quicker. Quit Mail first.", icon: "", admin: false, cmd: "if pgrep -x Mail >/dev/null; then echo 'Quit Mail first' >&2; exit 1; fi; for f in ~/Library/Mail/V*/MailData/'Envelope Index'; do [ -f \"$f\" ] && sqlite3 \"$f\" vacuum; done", selected: false, last: None },
        Task { title: "Repair disk permissions", desc: "Resets the permissions of your home folder's files to what macOS expects.", icon: "", admin: false, cmd: "diskutil resetUserPermissions / $(id -u)", selected: false, last: None },
        Task { title: "Restart Dock and Finder", desc: "Fixes a stuck Dock or Finder window without logging out.", icon: "🔄", admin: false, cmd: "killall Dock; killall Finder", selected: false, last: None },
    ]
}

/// Runs the chosen commands. The ones that need an administrator are run together, so the
/// password is asked once.
pub fn run_tasks(cmds: Vec<(usize, bool, String)>, p: &Progress) -> Vec<(usize, String)> {
    let mut results = vec![];
    let admin: Vec<_> = cmds.iter().filter(|c| c.1).collect();
    if !admin.is_empty() {
        p.set("Waiting for your password…");
        let joined = admin.iter().map(|c| c.2.clone()).collect::<Vec<_>>().join("; ");
        let script = format!("do shell script \"{}\" with administrator privileges", joined.replace('"', "\\\""));
        let out = std::process::Command::new("osascript").args(["-e", &script]).output();
        let msg = match out {
            Ok(o) if o.status.success() => "Done".to_string(),
            Ok(o) => {
                let e = String::from_utf8_lossy(&o.stderr).to_string();
                if e.contains("-128") { "Cancelled".into() } else { format!("Failed: {}", e.trim()) }
            }
            Err(e) => format!("Failed: {e}"),
        };
        for c in &admin {
            results.push((c.0, msg.clone()));
        }
    }
    for c in cmds.iter().filter(|c| !c.1) {
        p.set(format!("Running task {}…", c.0 + 1));
        let out = std::process::Command::new("sh").args(["-c", &c.2]).output();
        let msg = match out {
            Ok(o) if o.status.success() => "Done".to_string(),
            Ok(o) => format!("Finished with a warning: {}", String::from_utf8_lossy(&o.stderr).lines().next().unwrap_or("")),
            Err(e) => format!("Failed: {e}"),
        };
        results.push((c.0, msg));
    }
    results
}

pub fn human(b: u64) -> String {
    let b = b as f64;
    if b < 1000.0 {
        format!("{} bytes", b as u64)
    } else if b < 1e6 {
        format!("{:.0} KB", b / 1e3)
    } else if b < 1e9 {
        format!("{:.1} MB", b / 1e6)
    } else if b < 1e12 {
        format!("{:.2} GB", b / 1e9)
    } else {
        format!("{:.2} TB", b / 1e12)
    }
}

// ---------------------------------------------------------------- duplicates

#[derive(Clone)]
pub struct DupFile {
    pub path: PathBuf,
    pub age_days: u64,
    pub selected: bool,
}

#[derive(Clone)]
pub struct DupGroup {
    pub size: u64,
    pub name: String,
    pub files: Vec<DupFile>,
}

fn hash_file(path: &Path, limit: Option<u64>) -> Option<u64> {
    use std::hash::Hasher;
    use std::io::Read;
    let mut f = std::fs::File::open(path).ok()?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut read = 0u64;
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        h.write(&buf[..n]);
        read += n as u64;
        if limit.is_some_and(|l| read >= l) {
            break;
        }
    }
    Some(h.finish())
}

/// True when two files hold exactly the same bytes.
fn same_bytes(a: &Path, b: &Path) -> bool {
    use std::io::Read;
    let (Ok(mut fa), Ok(mut fb)) = (std::fs::File::open(a), std::fs::File::open(b)) else { return false };
    let mut ba = vec![0u8; 256 * 1024];
    let mut bb = vec![0u8; 256 * 1024];
    loop {
        let Ok(n) = fa.read(&mut ba) else { return false };
        let mut got = 0;
        while got < n {
            match fb.read(&mut bb[got..n]) {
                Ok(0) | Err(_) => return false,
                Ok(m) => got += m,
            }
        }
        if n == 0 {
            // a must be finished; b must be too
            return matches!(fb.read(&mut bb[..1]), Ok(0));
        }
        if ba[..n] != bb[..n] {
            return false;
        }
    }
}

/// Files with the same size are compared by a quick look at their start, then in full, and
/// finally byte for byte. Two names for one file (hard links) count once.
pub fn scan_duplicates(root: PathBuf, p: &Progress) -> Vec<DupGroup> {
    let mut by_size: std::collections::HashMap<u64, Vec<PathBuf>> = Default::default();
    let skip: HashSet<PathBuf> = [home().join("Library"), home().join(".Trash")].into_iter().collect();
    let mut stack = vec![root];
    let mut n = 0u64;
    let mut seen_files: HashSet<(u64, u64)> = HashSet::new();
    while let Some(dir) = stack.pop() {
        if p.cancelled() {
            return vec![];
        }
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let path = e.path();
            let name = file_name(&path);
            if name.starts_with('.') || skip.contains(&path) {
                continue;
            }
            let Ok(md) = e.metadata() else { continue };
            if md.is_dir() {
                if !name.ends_with(".app") && !name.ends_with(".photoslibrary") {
                    stack.push(path);
                }
            } else if md.len() >= DUPLICATE_MIN_BYTES {
                if !seen_files.insert((md.dev(), md.ino())) {
                    continue; // another name for a file already listed
                }
                n += 1;
                if n % 2000 == 0 {
                    p.set(format!("{n} files listed · {}", short(&dir)));
                }
                by_size.entry(md.len()).or_default().push(path);
            }
        }
    }
    let candidates: Vec<(u64, Vec<PathBuf>)> = by_size.into_iter().filter(|(_, v)| v.len() > 1).collect();
    let total = candidates.len();
    let mut out = vec![];
    for (i, (size, paths)) in candidates.into_iter().enumerate() {
        if p.cancelled() {
            break;
        }
        p.set(format!("Comparing files ({}/{total})", i + 1));
        let mut quick: std::collections::HashMap<u64, Vec<PathBuf>> = Default::default();
        for path in paths {
            if let Some(h) = hash_file(&path, Some(64 * 1024)) {
                quick.entry(h).or_default().push(path);
            }
        }
        for (_, same) in quick.into_iter().filter(|(_, v)| v.len() > 1) {
            let mut full: std::collections::HashMap<u64, Vec<PathBuf>> = Default::default();
            for path in same {
                if let Some(h) = hash_file(&path, None) {
                    full.entry(h).or_default().push(path);
                }
            }
            for (_, mut group) in full.into_iter().filter(|(_, v)| v.len() > 1) {
                let first = group[0].clone();
                group.retain(|g| *g == first || same_bytes(&first, g));
                if group.len() < 2 {
                    continue;
                }
                let mut files: Vec<DupFile> = group
                    .into_iter()
                    .map(|path| {
                        let age = std::fs::metadata(&path).and_then(|m| m.modified()).map(days_since).unwrap_or(0);
                        DupFile { path, age_days: age, selected: false }
                    })
                    .collect();
                files.sort_by_key(|f| f.age_days);
                out.push(DupGroup { size, name: file_name(&files[0].path), files });
            }
        }
    }
    out.sort_by(|a, b| (b.size * (b.files.len() as u64 - 1)).cmp(&(a.size * (a.files.len() as u64 - 1))));
    out
}

// ---------------------------------------------------------------- downloads

pub fn scan_downloads(p: &Progress) -> Vec<BigFile> {
    let mut out = vec![];
    for c in children(&home().join("Downloads")) {
        if p.cancelled() {
            break;
        }
        p.set(format!("Looking at {}", file_name(&c)));
        let md = std::fs::symlink_metadata(&c).ok();
        let idle = idle_days(&c, md.as_ref());
        out.push(BigFile {
            kind: if md.is_some_and(|m| m.is_dir()) { "Folder".into() } else { kind_of(&c) },
            name: file_name(&c),
            size: dir_size(&c, None),
            age_days: idle,
            path: c,
            selected: false,
        });
    }
    out.sort_by(|a, b| b.age_days.cmp(&a.age_days).then(b.size.cmp(&a.size)));
    out
}

// ---------------------------------------------------------------- cloud

pub struct CloudFolder {
    pub name: String,
    pub path: PathBuf,
    pub local: u64,
    pub icloud: bool,
    pub files: Vec<BigFile>,
}

pub fn scan_cloud(p: &Progress) -> Vec<CloudFolder> {
    let h = home();
    let mut roots = vec![];
    let icloud = h.join("Library/Mobile Documents/com~apple~CloudDocs");
    if icloud.exists() {
        roots.push(("iCloud Drive".to_string(), icloud, true));
    }
    for c in children(&h.join("Library/CloudStorage")) {
        let name = file_name(&c).replace('-', " · ");
        roots.push((name, c, false));
    }
    let mut out = vec![];
    for (name, path, is_icloud) in roots {
        if p.cancelled() {
            break;
        }
        p.set(format!("Measuring {name}"));
        let local = dir_size(&path, Some(p));
        let mut files = scan_big(path.clone(), 20_000_000, p);
        // only files actually stored on this Mac
        files.retain(|f| std::fs::metadata(&f.path).map(|m| m.blocks() > 0).unwrap_or(false));
        files.truncate(60);
        out.push(CloudFolder { name, path, local, icloud: is_icloud, files });
    }
    out
}

/// Removes the downloaded copy of iCloud Drive files; they stay in iCloud.
pub fn evict(paths: Vec<(PathBuf, u64)>, p: &Progress) -> ActionReport {
    let mut r = ActionReport::default();
    for (path, size) in paths {
        p.set(format!("Freeing {}", file_name(&path)));
        match std::process::Command::new("brctl").arg("evict").arg(&path).output() {
            Ok(o) if o.status.success() => {
                r.freed += size;
                r.removed.push(path);
            }
            Ok(o) => r.failed.push((path, String::from_utf8_lossy(&o.stderr).to_string())),
            Err(e) => r.failed.push((path, e.to_string())),
        }
    }
    r.message = format!("Freed {} on this Mac · files are still in iCloud", human(r.freed));
    r
}

// ---------------------------------------------------------------- login items

pub fn login_items() -> Result<Vec<String>, String> {
    let out = std::process::Command::new("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to set n to name of every login item",
            "-e",
            "set AppleScript's text item delimiters to linefeed",
            "-e",
            "return n as text",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(if s.is_empty() { vec![] } else { s.lines().map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect() })
}

pub fn remove_login_item(name: &str) -> Result<(), String> {
    let script = format!("tell application \"System Events\" to delete login item \"{}\"", name.replace('"', ""));
    let out = std::process::Command::new("osascript").args(["-e", &script]).output().map_err(|e| e.to_string())?;
    if out.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) }
}

// ---------------------------------------------------------------- quick looks for the smart scan

pub struct AppsSummary {
    pub total: usize,
    pub unused: Vec<String>,
}

pub fn apps_summary() -> AppsSummary {
    let mut total = 0;
    let mut unused = vec![];
    for dir in [PathBuf::from("/Applications"), home().join("Applications")] {
        for a in children(&dir) {
            if a.extension().is_some_and(|e| e == "app") {
                total += 1;
                let days = idle_days(&a, std::fs::metadata(&a).ok().as_ref());
                if days > crate::pages::UNUSED_APP_DAYS {
                    unused.push(file_name(&a).trim_end_matches(".app").to_string());
                }
            }
        }
    }
    AppsSummary { total, unused }
}

pub fn clutter_roots() -> Vec<PathBuf> {
    let h = home();
    ["Downloads", "Desktop", "Documents", "Movies", "Music", "Pictures"].iter().map(|d| h.join(d)).filter(|p| p.exists()).collect()
}

// ---------------------------------------------------------------- activity history

fn activity_file() -> PathBuf {
    home().join("Library/Application Support/Cleanerio/activity.tsv")
}

pub fn log_activity(what: &str, bytes: u64) {
    let f = activity_file();
    if let Some(d) = f.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let ts = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let line = format!("{ts}\t{bytes}\t{}\n", what.replace(['\t', '\n'], " "));
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(f) {
        let _ = file.write_all(line.as_bytes());
    }
}

pub fn read_activity() -> Vec<(u64, u64, String)> {
    let s = std::fs::read_to_string(activity_file()).unwrap_or_default();
    let mut v: Vec<_> = s
        .lines()
        .filter_map(|l| {
            let mut it = l.splitn(3, '\t');
            Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?, it.next()?.to_string()))
        })
        .collect();
    v.reverse();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn judges() {
        assert!(judge("com.pirrit.agent", "", "").0 == Verdict::Threat);
        assert!(judge("com.x.y", "/tmp/abc", "").0 == Verdict::Review);
        assert!(judge("com.x.y", "/Users/me/.hidden/run", "").0 == Verdict::Review);
        assert!(judge("com.x.y", "/usr/bin/true", "").0 == Verdict::Ok);
    }

    #[test]
    fn protected_folders() {
        let h = home();
        for p in [PathBuf::from("/"), PathBuf::from("/Applications"), PathBuf::from("/System/Library"), h.clone(), h.join("Library"), h.join("Documents"), h.join("Library/Caches")] {
            assert!(is_protected(&p), "{} should be protected", p.display());
        }
        for p in [h.join("Library/Caches/com.acme.paint"), h.join("Downloads/old.dmg"), PathBuf::from("/Applications/Paint.app"), PathBuf::from("/Library/LaunchDaemons/com.x.plist")] {
            assert!(!is_protected(&p), "{} should be removable", p.display());
        }
        let p = Progress { status: Default::default(), cancel: Default::default(), stage: Default::default() };
        let r = trash_paths(vec![(h.join("Documents"), 1)], &p);
        assert!(r.removed.is_empty() && r.failed.len() == 1);
    }

    #[test]
    fn hard_links_are_not_duplicates() {
        let d = std::env::temp_dir().join(format!("cleanerio-link-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("one.bin"), vec![3u8; 600 * 1024]).unwrap();
        std::fs::hard_link(d.join("one.bin"), d.join("same.bin")).unwrap();
        let p = Progress { status: Default::default(), cancel: Default::default(), stage: Default::default() };
        let g = scan_duplicates(d.clone(), &p);
        std::fs::remove_dir_all(&d).unwrap();
        assert!(g.is_empty());
    }

    #[test]
    fn dates_and_running_apps() {
        assert_eq!(parse_mdls_date("1970-01-02 00:00:00 +0000"), Some(86_400));
        assert_eq!(parse_mdls_date("2026-11-12 13:00:00 +0000"), Some(1_794_488_400));
        assert_eq!(parse_mdls_date("(null)"), None);
        let running: HashSet<String> = ["com.google.chrome".to_string()].into_iter().collect();
        assert!(cache_in_use("com.google.Chrome", &running));
        assert!(cache_in_use("com.google.Chrome.helper", &running));
        assert!(!cache_in_use("com.spotify.client", &running));
        assert_eq!(agent_label(Path::new("/h/Library/LaunchAgents/com.x.plist")).as_deref(), Some("com.x.plist"));
        assert!(agent_label(Path::new("/h/Library/Preferences/com.x.plist")).is_none());
    }

    #[test]
    fn low_disk_limits() {
        const GB: u64 = 1_000_000_000;
        assert_eq!(low_disk_limit(494 * GB), 50 * GB);
        assert_eq!(low_disk_limit(64 * GB), 5 * GB);
        assert_eq!(low_disk_limit(1000 * GB), 100 * GB);
        assert_eq!(low_disk_limit(4000 * GB), 100 * GB);
        assert_eq!(low_disk_limit(1), 100 * GB); // unknown size counts as 1 TB
    }

    #[test]
    fn crashed_job_reports_failure() {
        let j: Job<u32> = Job::spawn(|_| panic!("boom"));
        for _ in 0..200 {
            if j.failed() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(j.failed() && !j.finished() && j.take().is_none());
        let ok: Job<u32> = Job::spawn(|_| 7);
        for _ in 0..200 {
            if ok.finished() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(ok.take(), Some(7));
    }

    #[test]
    fn leftovers() {
        let lib = vec![
            PathBuf::from("/h/Library/Preferences/com.acme.paint.plist"),
            PathBuf::from("/h/Library/Caches/com.acme.paint"),
            PathBuf::from("/h/Library/Application Support/Paint"),
            PathBuf::from("/h/Library/Application Support/Other"),
        ];
        assert_eq!(find_leftovers("Paint", "com.acme.paint", &lib).len(), 3);
        let found: Vec<(PathBuf, u64)> = find_leftovers("Paint", "com.acme.paint", &lib).into_iter().map(|p| (p, 1)).collect();
        let skip = name_only_leftovers("com.acme.paint", &found);
        assert_eq!(skip.len(), 1);
        assert!(skip.contains(&PathBuf::from("/h/Library/Application Support/Paint")));
    }

    #[test]
    fn duplicates() {
        let d = std::env::temp_dir().join(format!("cleanerio-dup-{}", std::process::id()));
        std::fs::create_dir_all(d.join("a")).unwrap();
        let data = vec![7u8; 600 * 1024];
        std::fs::write(d.join("one.bin"), &data).unwrap();
        std::fs::write(d.join("a/two.bin"), &data).unwrap();
        let mut other = data.clone();
        other[500_000] = 1;
        std::fs::write(d.join("three.bin"), &other).unwrap();
        let p = Progress { status: Default::default(), cancel: Default::default(), stage: Default::default() };
        let g = scan_duplicates(d.clone(), &p);
        std::fs::remove_dir_all(&d).unwrap();
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].files.len(), 2);
    }

    #[test]
    fn sizes() {
        assert_eq!(human(1_500_000_000), "1.50 GB");
        assert_eq!(human(999), "999 bytes");
    }
}

/// Free space below which the app warns: of the choices 2, 5, 10, 20, 30, 50 and 100 GB, the one
/// closest to 10% of the startup disk (an unknown size counts as 1 TB).
pub fn low_disk_limit(disk_total: u64) -> u64 {
    let cap = if disk_total <= 1 { 1_000_000_000_000 } else { disk_total };
    let target = cap as f64 * 0.10;
    [2u64, 5, 10, 20, 30, 50, 100]
        .iter()
        .map(|g| g * 1_000_000_000)
        .min_by(|a, b| (*a as f64 - target).abs().partial_cmp(&(*b as f64 - target).abs()).unwrap())
        .unwrap()
}

/// Battery percent and state from `pmset -g batt`; None on a Mac without a battery.
pub fn battery() -> Option<(u8, String)> {
    let out = std::process::Command::new("pmset").args(["-g", "batt"]).output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).to_string();
    let line = s.lines().find(|l| l.contains("InternalBattery"))?;
    let pct: u8 = line.split('%').next()?.rsplit(|c: char| !c.is_ascii_digit()).next()?.parse().ok()?;
    let state = if line.contains("discharging") {
        "On battery"
    } else if line.contains("charging") {
        "Charging"
    } else {
        "Charged"
    };
    Some((pct, state.to_string()))
}

/// Name of the Wi-Fi network, when macOS shares it.
pub fn wifi_name() -> Option<String> {
    let out = std::process::Command::new("ipconfig").args(["getsummary", "en0"]).output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).to_string();
    s.lines()
        .find_map(|l| l.trim().strip_prefix("SSID : "))
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty() && x != "<redacted>")
}

/// Bytes per second as short text.
pub fn rate(b: f64) -> String {
    if b < 1.0 {
        "Zero KB/sec".into()
    } else if b < 1000.0 {
        format!("{} bytes/sec", b as u64)
    } else if b < 1e6 {
        format!("{:.0} KB/sec", b / 1e3)
    } else {
        format!("{:.1} MB/sec", b / 1e6)
    }
}

pub const SMART_SCAN_LOG: &str = "Ran Smart Scan";
pub const THREAT_SCAN_LOG: &str = "Ran threat scan";

/// Battery condition as System Information reports it ("Normal", "Service Recommended"...).
pub fn battery_condition() -> Option<String> {
    let out = std::process::Command::new("system_profiler").arg("SPPowerDataType").output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).to_string();
    s.lines().find_map(|l| l.trim().strip_prefix("Condition: ").map(|x| x.trim().to_string()))
}

/// Days since the newest activity entry with this text, if any.
pub fn days_since_logged(history: &[(u64, u64, String)], what: &str) -> Option<u64> {
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    history.iter().filter(|h| h.2 == what).map(|h| now.saturating_sub(h.0) / 86_400).min()
}
