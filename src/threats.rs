//! The threat database and the malware check: known families of Mac malware (trojans, adware,
//! spyware, crypto miners) matched against background items, apps, browser extensions, running
//! programs, known file locations and file fingerprints (SHA-256).
//!
//! A built-in database ships with the app; a newer one can be loaded from a file and is kept in
//! ~/Library/Application Support/Cleanerio/threats.json.

use crate::scan::{children, file_name, home, short, AgentItem, Progress, Verdict};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const BUILT_IN: &str = include_str!("../threats.json");

#[derive(Clone, Serialize, Deserialize)]
pub struct Family {
    pub name: String,
    pub category: String,
    #[serde(default)]
    pub words: Vec<String>,
    #[serde(default)]
    pub paths: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Database {
    pub version: u64,
    pub updated: String,
    pub families: Vec<Family>,
    /// SHA-256 (lower-case hex) → family name.
    #[serde(default)]
    pub sha256: HashMap<String, String>,
}

impl Database {
    pub fn signatures(&self) -> usize {
        self.families.iter().map(|f| f.words.len() + f.paths.len()).sum::<usize>() + self.sha256.len()
    }
    fn family(&self, name: &str) -> Option<&Family> {
        self.families.iter().find(|f| f.name == name)
    }
    /// First family whose words appear in the text.
    fn match_words(&self, text: &str) -> Option<(&Family, String)> {
        let t = text.to_lowercase();
        for f in &self.families {
            if let Some(w) = f.words.iter().find(|w| !w.is_empty() && t.contains(&w.to_lowercase())) {
                return Some((f, w.clone()));
            }
        }
        None
    }
}

fn user_file() -> PathBuf {
    crate::scan::app_data_dir().join("threats.json")
}

/// The newest of the built-in database and the saved one.
pub fn load() -> Database {
    let built: Database = serde_json::from_str(BUILT_IN).expect("built-in threat database");
    match std::fs::read_to_string(user_file()).ok().and_then(|s| serde_json::from_str::<Database>(&s).ok()) {
        Some(u) if u.version >= built.version => u,
        _ => built,
    }
}

/// Checks a database file and saves it as the one to use. Returns the loaded database.
pub fn install(from: &Path) -> Result<Database, String> {
    let text = std::fs::read_to_string(from).map_err(|e| e.to_string())?;
    let db: Database = serde_json::from_str(&text).map_err(|e| format!("not a threat database ({e})"))?;
    if db.families.is_empty() {
        return Err("the file has no threat families".into());
    }
    let f = user_file();
    if let Some(d) = f.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    std::fs::write(&f, text).map_err(|e| e.to_string())?;
    Ok(db)
}

/// Asks the person for a database file with the standard macOS file picker.
pub fn pick_file() -> Option<PathBuf> {
    if crate::linux::ON {
        return crate::linux::pick_file();
    }
    let out = std::process::Command::new("osascript")
        .args(["-e", "POSIX path of (choose file with prompt \"Choose a threat database (.json)\")"])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !s.is_empty()).then(|| PathBuf::from(s))
}

fn sha256_file(p: &Path) -> Option<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let md = std::fs::metadata(p).ok()?;
    if !md.is_file() || md.len() > 200_000_000 {
        return None;
    }
    let mut f = std::fs::File::open(p).ok()?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Some(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn expand(p: &str) -> PathBuf {
    match p.strip_prefix("~/") {
        Some(rest) => home().join(rest),
        None => PathBuf::from(p),
    }
}

fn hit(path: PathBuf, label: String, program: String, place: &'static str, fam: &Family, why: String) -> AgentItem {
    AgentItem {
        path,
        label,
        program,
        place,
        verdict: Verdict::Threat,
        reason: format!("{} ({}) · {}", fam.name, fam.category, why),
        category: fam.category.clone(),
        // a name match can be wrong, so the person ticks it; only fingerprints are pre-ticked
        selected: false,
        pid: None,
    }
}

/// A file fingerprint match is certain enough to tick for removal.
fn sure(mut item: AgentItem) -> AgentItem {
    item.selected = true;
    item
}

/// The full malware check.
pub fn scan(db: &Database, p: &Progress) -> Vec<AgentItem> {
    let h = home();
    let mut out: Vec<AgentItem> = vec![];

    // 1. background items (launch agents and daemons)
    let places: [(PathBuf, &'static str); 3] = [
        (h.join("Library/LaunchAgents"), "Your login agents"),
        (PathBuf::from("/Library/LaunchAgents"), "All users' agents"),
        (PathBuf::from("/Library/LaunchDaemons"), "System helpers"),
    ];
    let places: Vec<(PathBuf, &'static str)> = if crate::linux::ON { crate::linux::startup_places() } else { places.into_iter().collect() };
    for (dir, place) in places {
        for f in children(&dir) {
            if p.cancelled() {
                return out;
            }
            let wanted = if crate::linux::ON { crate::linux::is_startup_file(&f) } else { f.extension().is_some_and(|e| e == "plist") };
            if !wanted {
                continue;
            }
            p.set(format!("Checking {}", short(&f)));
            let (label, program) = if crate::linux::ON { crate::linux::entry_program(&f) } else { crate::scan::plist_program(&f) };
            let text = format!("{label} {program} {}", file_name(&f));
            if let Some((fam, w)) = db.match_words(&text) {
                out.push(hit(f, label, program, place, fam, format!("matches “{w}”")));
                continue;
            }
            if let Some(fam) = sha256_file(Path::new(&program)).and_then(|s| db.sha256.get(&s)).and_then(|n| db.family(n)) {
                out.push(sure(hit(f, label, program, place, fam, "file fingerprint matches".into())));
                continue;
            }
            let (verdict, reason) = crate::scan::judge(&label, &program, &file_name(&f));
            out.push(AgentItem { path: f, label, program, place, verdict, reason, category: String::new(), selected: false, pid: None });
        }
    }

    // 2. apps: names, bundle ids and main program fingerprints
    for dir in [PathBuf::from("/Applications"), h.join("Applications")] {
        for a in children(&dir) {
            if p.cancelled() {
                return out;
            }
            if a.extension().map(|e| e != "app").unwrap_or(true) {
                continue;
            }
            p.set(format!("Checking {}", file_name(&a)));
            let info = plist::Value::from_file(a.join("Contents/Info.plist")).ok();
            let d = info.as_ref().and_then(|v| v.as_dictionary());
            let id = d.and_then(|d| d.get("CFBundleIdentifier")).and_then(|v| v.as_string()).unwrap_or("").to_string();
            let exe = d.and_then(|d| d.get("CFBundleExecutable")).and_then(|v| v.as_string()).unwrap_or("").to_string();
            let name = file_name(&a);
            if let Some((fam, w)) = db.match_words(&format!("{name} {id}")) {
                out.push(hit(a, name, id, "Applications", fam, format!("matches “{w}”")));
            } else if !db.sha256.is_empty() && !exe.is_empty() {
                if let Some(fam) = sha256_file(&a.join("Contents/MacOS").join(&exe)).and_then(|s| db.sha256.get(&s)).and_then(|n| db.family(n)) {
                    out.push(sure(hit(a, name, id, "Applications", fam, "file fingerprint matches".into())));
                }
            }
        }
    }

    // 3. browser extensions (Chrome-family browsers keep a manifest per extension)
    p.set("Checking browser extensions…");
    let browsers = if crate::linux::ON {
        [("google-chrome", "Chrome"), ("microsoft-edge", "Edge"), ("BraveSoftware/Brave-Browser", "Brave"), ("chromium", "Chrome")].to_vec()
    } else {
        [("Google/Chrome", "Chrome"), ("Microsoft Edge", "Edge"), ("BraveSoftware/Brave-Browser", "Brave")].to_vec()
    };
    let support = if crate::linux::ON { crate::linux::config_home() } else { h.join("Library/Application Support") };
    for (dir, browser) in browsers {
        let base = support.join(dir);
        for profile in children(&base) {
            for ext in children(&profile.join("Extensions")) {
                for ver in children(&ext) {
                    let Ok(m) = std::fs::read_to_string(ver.join("manifest.json")) else { continue };
                    let name = serde_json::from_str::<serde_json::Value>(&m)
                        .ok()
                        .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
                        .unwrap_or_else(|| file_name(&ext));
                    if let Some((fam, w)) = db.match_words(&format!("{name} {}", file_name(&ext))) {
                        let place: &'static str = match browser {
                            "Chrome" => "Chrome extension",
                            "Edge" => "Edge extension",
                            _ => "Brave extension",
                        };
                        out.push(hit(ext.clone(), name.clone(), String::new(), place, fam, format!("matches “{w}”")));
                    }
                }
            }
        }
    }

    // 4. running programs (miners and spyware that are already active)
    p.set("Checking running programs…");
    let mut sys = sysinfo::System::new().expect("system information");
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    for (pid, proc_) in sys.processes() {
        let name = proc_.name().to_string_lossy().to_string();
        let exe = proc_.exe().map(|e| e.to_path_buf()).unwrap_or_default();
        if let Some((fam, w)) = db.match_words(&format!("{name} {}", exe.display())) {
            let mut item = hit(exe.clone(), name, exe.display().to_string(), "Running now", fam, format!("matches “{w}”"));
            item.pid = Some(pid.as_u32());
            out.push(item);
        }
    }

    // 5. known file locations
    p.set("Checking known hiding places…");
    for fam in &db.families {
        for loc in &fam.paths {
            let path = expand(loc);
            if std::fs::symlink_metadata(&path).is_ok() && !out.iter().any(|i| i.path == path) {
                out.push(hit(path.clone(), file_name(&path), String::new(), "Known location", fam, format!("found at {}", short(&path))));
            }
        }
    }

    out.sort_by(|a, b| a.verdict.cmp(&b.verdict).then(a.label.to_lowercase().cmp(&b.label.to_lowercase())));
    out
}

/// The database, loaded once (used by the quick name check in `scan::judge`).
pub fn load_cached() -> &'static Database {
    static DB: std::sync::OnceLock<Database> = std::sync::OnceLock::new();
    DB.get_or_init(load)
}

impl Database {
    /// (family name, category) and the word that matched.
    pub fn match_words_pub(&self, text: &str) -> Option<((String, String), String)> {
        self.match_words(text).map(|(f, w)| ((f.name.clone(), f.category.clone()), w))
    }
}
