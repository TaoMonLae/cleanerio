//! The Windows side of Cleanerio: where temporary files, caches, installed programs and
//! startup entries live on Windows, and the commands that stand in for the Mac-only ones.
//!
//! Like `linux.rs`, this compiles on every system; `scan.rs` calls it only when `ON` is true.
//! Windows is asked through PowerShell (which answers in JSON) so no Windows-only code is
//! needed here.

use crate::scan::{children, dir_size, file_name, home, quiet, AppInfo, JunkGroup, JunkItem, JunkKind, Progress, Task};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// True in the Windows build.
pub const ON: bool = cfg!(target_os = "windows");

/// %LOCALAPPDATA%
pub fn local() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(|| home().join("AppData/Local"))
}
/// %APPDATA%
pub fn roaming() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| home().join("AppData/Roaming"))
}
fn windows_dir() -> PathBuf {
    std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\Windows"))
}

/// Lower case with forward slashes, so paths compare the way Windows compares them.
fn norm(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/").trim_end_matches('/').to_lowercase()
}
fn under(path: &str, folder: &str) -> bool {
    path == folder || path.starts_with(&format!("{folder}/"))
}

/// Runs a PowerShell script and returns what it printed (UTF-8).
fn powershell(script: &str) -> Option<String> {
    let full = format!("[Console]::OutputEncoding=[Text.Encoding]::UTF8; {script}");
    let out = quiet("powershell").args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &full]).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).to_string())
}
/// PowerShell prints one object as an object and several as a list; this always gives a list.
fn json_list(text: &str) -> Vec<serde_json::Value> {
    match serde_json::from_str::<serde_json::Value>(text.trim()) {
        Ok(serde_json::Value::Array(a)) => a,
        Ok(v @ serde_json::Value::Object(_)) => vec![v],
        _ => vec![],
    }
}
fn text_of(v: &serde_json::Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").trim().to_string()
}
/// A value inside single quotes in a PowerShell script.
fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

// ---------------------------------------------------------------- protected folders

/// Folders Windows or the person's account can't work without.
pub fn is_protected(path: &Path) -> bool {
    let p = norm(path);
    let h = norm(&home());
    // drive roots such as "c:"
    if p.len() <= 2 {
        return true;
    }
    let win = norm(&windows_dir());
    if under(&p, &win) {
        return true;
    }
    let drive = &win[..win.find('/').unwrap_or(win.len())];
    for top in ["program files", "program files (x86)", "programdata", "users", "$recycle.bin", "system volume information", "recovery"] {
        if p == format!("{drive}/{top}") {
            return true;
        }
    }
    // other people's profiles
    if Path::new(&p).parent().is_some_and(|d| norm(d) == format!("{drive}/users")) {
        return true;
    }
    const HOME: &[&str] = &[
        "documents", "desktop", "downloads", "videos", "music", "pictures", "onedrive", "appdata", "appdata/local", "appdata/roaming", "appdata/locallow",
        "appdata/local/microsoft", "appdata/roaming/microsoft", "appdata/local/packages", "appdata/local/programs", "appdata/local/temp",
    ];
    if HOME.iter().any(|d| p == format!("{h}/{d}")) {
        return true;
    }
    // keys, saved passwords and Windows' own settings, with everything inside them
    const INSIDE: &[&str] = &[".ssh", ".gnupg", "appdata/roaming/microsoft/credentials", "appdata/roaming/microsoft/protect", "appdata/local/microsoft/credentials", "appdata/roaming/microsoft/windows"];
    INSIDE.iter().any(|d| under(&p, &format!("{h}/{d}")))
}

// ---------------------------------------------------------------- junk

fn item(name: &str, path: PathBuf, on: bool, p: &Progress) -> Option<JunkItem> {
    if !path.exists() {
        return None;
    }
    p.set(format!("Looking at {name}"));
    let size = dir_size(&path, Some(p));
    (size > 0).then(|| JunkItem { name: name.to_string(), path, size, selected: on, in_use: false })
}

/// One entry per folder inside `dir`, biggest first.
fn items_in(dir: &Path, on: bool, p: &Progress, keep: impl Fn(&Path) -> bool) -> Vec<JunkItem> {
    let mut out: Vec<JunkItem> = children(dir)
        .into_iter()
        .filter(|c| keep(c))
        .filter_map(|c| {
            if p.cancelled() {
                return None;
            }
            let size = dir_size(&c, Some(p));
            (size > 0).then(|| JunkItem { name: file_name(&c), path: c, size, selected: on, in_use: false })
        })
        .collect();
    out.sort_by(|a, b| b.size.cmp(&a.size));
    out
}

/// True when nothing changed the file, or anything inside the folder, during the last day.
/// A folder's own date doesn't change when a file deep inside it does, so the files are
/// checked too. A folder too big to check in full counts as in use.
fn untouched_for_a_day(path: &Path) -> bool {
    let old = |p: &Path| std::fs::symlink_metadata(p).and_then(|m| m.modified()).map(crate::scan::days_since).is_ok_and(|d| d >= 1);
    if !old(path) {
        return false;
    }
    let mut stack = vec![path.to_path_buf()];
    let mut seen = 0;
    while let Some(dir) = stack.pop() {
        if !std::fs::symlink_metadata(&dir).is_ok_and(|m| m.is_dir()) {
            continue;
        }
        for c in children(&dir) {
            seen += 1;
            if seen > 5000 || !old(&c) {
                return false;
            }
            stack.push(c);
        }
    }
    true
}

/// Browser profile folders ("Default", "Profile 1"…) inside a Chrome-family data folder.
fn profiles(user_data: &Path) -> Vec<PathBuf> {
    children(user_data).into_iter().filter(|c| c.join("Preferences").exists()).collect()
}

pub fn scan_junk(p: &Progress) -> Vec<JunkGroup> {
    let (h, l) = (home(), local());
    let running = crate::linux::running_names();
    let is_running = |names: &[&str]| names.iter().any(|n| running.contains(*n) || running.contains(&format!("{n}.exe")));
    let mut groups = vec![];
    let group = |kind, title, desc, items: Vec<JunkItem>| JunkGroup { kind, title, desc, icon: "", items, expanded: false, note: None };

    p.set("Looking for temporary files…");
    // files a running program made today are left alone
    let temp = items_in(&std::env::temp_dir(), true, p, untouched_for_a_day);

    p.set("Looking for cache files…");
    let mut caches: Vec<JunkItem> = vec![];
    let browsers: [(&str, PathBuf, &[&str]); 3] = [
        ("Chrome", l.join("Google/Chrome/User Data"), &["chrome"]),
        ("Edge", l.join("Microsoft/Edge/User Data"), &["msedge"]),
        ("Brave", l.join("BraveSoftware/Brave-Browser/User Data"), &["brave"]),
    ];
    for (name, data, procs) in browsers {
        for prof in profiles(&data) {
            for sub in ["Cache", "Code Cache", "GPUCache"] {
                if let Some(mut it) = item(&format!("{name} · {} · {sub}", file_name(&prof)), prof.join(sub), true, p) {
                    if is_running(procs) {
                        it.in_use = true;
                        it.selected = false;
                    }
                    caches.push(it);
                }
            }
        }
    }
    for prof in children(&l.join("Mozilla/Firefox/Profiles")) {
        if let Some(mut it) = item(&format!("Firefox · {}", file_name(&prof)), prof.join("cache2"), true, p) {
            if is_running(&["firefox"]) {
                it.in_use = true;
                it.selected = false;
            }
            caches.push(it);
        }
    }
    for (name, path) in [
        ("Internet cache", l.join("Microsoft/Windows/INetCache")),
        ("Graphics shader cache", l.join("D3DSCache")),
        ("NVIDIA shader cache", l.join("NVIDIA/DXCache")),
        ("NVIDIA OpenGL cache", l.join("NVIDIA/GLCache")),
        ("Teams cache", roaming().join("Microsoft/Teams/Cache")),
        ("Discord cache", roaming().join("discord/Cache")),
        ("Spotify cache", l.join("Spotify/Data")),
    ] {
        caches.extend(item(name, path, true, p));
    }
    caches.sort_by(|a, b| b.size.cmp(&a.size));
    let mut all = temp;
    for it in all.iter_mut() {
        it.name = format!("Temp · {}", it.name);
    }
    all.extend(caches);
    all.sort_by(|a, b| b.size.cmp(&a.size));
    groups.push(group(JunkKind::Caches, "Temporary and cache files", "Files programs keep to load faster or forgot to remove. Programs rebuild them when needed.", all));

    p.set("Looking for crash reports…");
    let mut logs = items_in(&l.join("CrashDumps"), true, p, |_| true);
    logs.extend(items_in(&l.join("Microsoft/Windows/WER/ReportArchive"), true, p, |_| true));
    logs.extend(items_in(&l.join("Microsoft/Windows/WER/ReportQueue"), true, p, |_| true));
    groups.push(group(JunkKind::Logs, "Crash reports", "Records Windows keeps after a program stops working. Useful only for troubleshooting.", logs));

    p.set("Looking for developer leftovers…");
    let mut dev: Vec<JunkItem> = vec![];
    for (name, path) in [
        ("Cargo downloads", h.join(".cargo/registry/cache")),
        ("Cargo sources", h.join(".cargo/registry/src")),
        ("npm cache", l.join("npm-cache/_cacache")),
        ("npm cache (old location)", roaming().join("npm-cache/_cacache")),
        ("pip cache", l.join("pip/Cache")),
        ("Gradle caches", h.join(".gradle/caches")),
        ("Go module downloads", h.join("go/pkg/mod/cache")),
    ] {
        dev.extend(item(name, path, false, p));
    }
    dev.sort_by(|a, b| b.size.cmp(&a.size));
    groups.push(group(JunkKind::Developer, "Developer junk", "Downloaded packages that coding tools fetch again when a project needs them.", dev));

    p.set("Looking for old installers…");
    let installer = |c: &Path| {
        let ext = c.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let name = file_name(c).to_lowercase();
        matches!(ext.as_str(), "msi" | "msix" | "msixbundle" | "appx" | "iso")
            || (ext == "exe" && ["setup", "install"].iter().any(|w| name.contains(w)))
    };
    let mut inst = items_in(&h.join("Downloads"), false, p, installer);
    inst.extend(items_in(&h.join("Desktop"), false, p, installer));
    groups.push(group(JunkKind::Installers, "Old installers", "Setup programs and disk images left in Downloads or on the Desktop.", inst));

    p.set("Looking in the Recycle Bin…");
    groups.push(group(JunkKind::Trash, "Recycle Bin", "Files you already moved to the Recycle Bin. Emptying it frees the space for good.", recycle_bin_items()));
    groups
}

/// What is in the Recycle Bin: files with their size (folders are listed without one).
#[cfg(not(target_os = "macos"))]
fn recycle_bin_items() -> Vec<JunkItem> {
    let Ok(items) = trash::os_limited::list() else { return vec![] };
    let mut out: Vec<JunkItem> = items
        .iter()
        .map(|it| {
            let size = match trash::os_limited::metadata(it).map(|m| m.size) {
                Ok(trash::TrashItemSize::Bytes(b)) => b,
                _ => 0,
            };
            JunkItem { name: it.name.to_string_lossy().to_string(), path: it.original_path(), size, selected: false, in_use: false }
        })
        .collect();
    out.sort_by(|a, b| b.size.cmp(&a.size));
    out
}
#[cfg(target_os = "macos")]
fn recycle_bin_items() -> Vec<JunkItem> {
    vec![]
}

/// Bytes held by the files in the Recycle Bin.
pub fn recycle_bin_size() -> u64 {
    recycle_bin_items().iter().map(|i| i.size).sum()
}

/// Empties the Recycle Bin for good.
#[cfg(not(target_os = "macos"))]
pub fn empty_recycle_bin() -> Result<(), String> {
    let items = trash::os_limited::list().map_err(|e| e.to_string())?;
    trash::os_limited::purge_all(items).map_err(|e| e.to_string())
}
#[cfg(target_os = "macos")]
pub fn empty_recycle_bin() -> Result<(), String> {
    Err("not available on this system".into())
}

// ---------------------------------------------------------------- startup entries

/// The folder whose shortcuts open when the person signs in.
fn startup_folder() -> PathBuf {
    roaming().join("Microsoft/Windows/Start Menu/Programs/Startup")
}
/// The registry keys whose values run at sign-in.
const RUN_KEYS: &[(&str, &str)] = &[
    ("HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run", "Starts when you sign in"),
    ("HKLM:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run", "Starts for every user"),
    ("HKLM:\\Software\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Run", "Starts for every user"),
];

/// A startup entry kept in the registry is written as a made-up path: "registry:<key>::<name>".
const REG: &str = "registry:";
fn registry_path(key: &str, name: &str) -> PathBuf {
    PathBuf::from(format!("{REG}{key}::{name}"))
}
/// (key, value name) when the path stands for a registry startup entry.
pub fn registry_entry(path: &Path) -> Option<(String, String)> {
    let s = path.to_string_lossy();
    let rest = s.strip_prefix(REG)?;
    let (key, name) = rest.split_once("::")?;
    Some((key.to_string(), name.to_string()))
}

/// The program a startup command starts: `"C:\Program Files\App\app.exe" --minimized` gives
/// the path between the quotes; without quotes, everything up to ".exe".
pub fn command_program(cmd: &str) -> String {
    let cmd = cmd.trim();
    if let Some(rest) = cmd.strip_prefix('"') {
        return rest.split('"').next().unwrap_or("").to_string();
    }
    let lower = cmd.to_lowercase();
    match lower.find(".exe") {
        Some(i) => cmd[..i + 4].to_string(),
        None => cmd.split_whitespace().next().unwrap_or("").to_string(),
    }
}

pub struct StartupEntry {
    pub path: PathBuf,
    pub name: String,
    pub program: String,
    pub place: &'static str,
}

/// Everything that starts by itself: registry Run values and Startup folder shortcuts.
pub fn startup_entries() -> Vec<StartupEntry> {
    let mut out = vec![];
    let keys = RUN_KEYS.iter().map(|k| ps_quote(k.0)).collect::<Vec<_>>().join(",");
    let script = format!(
        "$o=@(); foreach($k in {keys}){{ $p=Get-ItemProperty -Path $k -ErrorAction SilentlyContinue; if($p){{ $p.PSObject.Properties | Where-Object {{ $_.Name -notlike 'PS*' }} | ForEach-Object {{ $o += [pscustomobject]@{{Key=$k;Name=$_.Name;Command=[string]$_.Value}} }} }} }}; $o | ConvertTo-Json -Compress"
    );
    for v in powershell(&script).map(|t| json_list(&t)).unwrap_or_default() {
        let (key, name) = (text_of(&v, "Key"), text_of(&v, "Name"));
        if name.is_empty() {
            continue;
        }
        let place = RUN_KEYS.iter().find(|k| k.0 == key).map(|k| k.1).unwrap_or("Starts when you sign in");
        out.push(StartupEntry { path: registry_path(&key, &name), program: command_program(&text_of(&v, "Command")), name, place });
    }
    for f in children(&startup_folder()) {
        let name = file_name(&f);
        if name.eq_ignore_ascii_case("desktop.ini") {
            continue;
        }
        let shown = name.trim_end_matches(".lnk").to_string();
        out.push(StartupEntry { path: f, name: shown, program: String::new(), place: "Your Startup folder" });
    }
    out
}

/// Notes a removed registry startup entry in Cleanerio's own folder, so it can be put back by hand.
fn remember_removed(key: &str, name: &str) {
    let script = format!("(Get-ItemProperty -Path {} -Name {} -ErrorAction SilentlyContinue).{}", ps_quote(key), ps_quote(name), ps_quote(name));
    let command = powershell(&script).unwrap_or_default();
    let file = crate::scan::app_data_dir().join("removed-startup-entries.txt");
    if let Some(d) = file.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(file) {
        let _ = writeln!(f, "{key}\t{name}\t{}", command.trim());
    }
}

/// Removes a registry startup entry. The command it ran is noted first (see `remember_removed`).
pub fn remove_registry_entry(key: &str, name: &str) -> Result<(), String> {
    remember_removed(key, name);
    let script = format!("Remove-ItemProperty -Path {} -Name {} -ErrorAction Stop", ps_quote(key), ps_quote(name));
    let full = format!("[Console]::OutputEncoding=[Text.Encoding]::UTF8; {script}");
    match quiet("powershell").args(["-NoProfile", "-NonInteractive", "-Command", &full]).output() {
        Ok(o) if o.status.success() => Ok(()),
        Ok(_) if key.starts_with("HKLM") => Err("this entry is set for every user; removing it needs an administrator".into()),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr).lines().next().unwrap_or("could not be removed").trim().to_string()),
        Err(e) => Err(e.to_string()),
    }
}

/// The person's own startup entries, by name.
pub fn login_items() -> Result<Vec<String>, String> {
    let mut v: Vec<String> = startup_entries().into_iter().filter(|e| !e.path.to_string_lossy().contains("HKLM")).map(|e| e.name).collect();
    v.sort_by_key(|n| n.to_lowercase());
    v.dedup();
    Ok(v)
}
pub fn remove_login_item(name: &str) -> Result<(), String> {
    let entry = startup_entries()
        .into_iter()
        .find(|e| e.name == name && !e.path.to_string_lossy().contains("HKLM"))
        .ok_or("That startup entry is no longer there.")?;
    match registry_entry(&entry.path) {
        Some((key, value)) => remove_registry_entry(&key, &value),
        None => trash::delete(&entry.path).map_err(|e| e.to_string()),
    }
}

// ---------------------------------------------------------------- running programs

/// The programs the person opened that use the most memory: (id, name, memory, processor share).
pub fn heavy_apps() -> Vec<(sysinfo::Pid, String, u64, f32)> {
    let Ok(mut sys) = sysinfo::System::new() else { return vec![] };
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let win = norm(&windows_dir());
    const SHELL: &[&str] = &["explorer.exe", "searchhost.exe", "startmenuexperiencehost.exe", "shellexperiencehost.exe", "textinputhost.exe", "widgets.exe", "sihost.exe", "ctfmon.exe", "securityhealthsystray.exe"];
    let mut v: Vec<_> = sys
        .processes()
        .iter()
        .filter(|(pid, p)| {
            let name = p.name().to_string_lossy().to_lowercase();
            pid.as_u32() != std::process::id() && !SHELL.contains(&name.as_str()) && p.exe().is_some_and(|e| !under(&norm(e), &win) && !norm(e).contains("/windowsapps/microsoft."))
        })
        .map(|(pid, p)| (*pid, p.name().to_string_lossy().trim_end_matches(".exe").to_string(), p.memory(), p.cpu_usage()))
        .collect();
    v.sort_by(|a, b| b.2.cmp(&a.2));
    v.truncate(8);
    v
}

/// Asks a program to close its windows (it can save its work first).
pub fn ask_to_quit(pid: u32) {
    let _ = quiet("taskkill").args(["/PID", &pid.to_string()]).output();
}

// ---------------------------------------------------------------- applications

/// Leftovers are looked for in these folders of the person's profile.
fn leftover_candidates() -> Vec<PathBuf> {
    [roaming(), local(), home().join("AppData/LocalLow")].iter().flat_map(|d| children(d)).filter(|c| !is_protected(c)).collect()
}

const APPS_SCRIPT: &str = "$k='HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*','HKLM:\\SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*','HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*'; Get-ItemProperty $k -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -and -not $_.SystemComponent -and -not $_.ParentKeyName } | Select-Object DisplayName,DisplayVersion,InstallLocation,EstimatedSize,PSChildName,Publisher | ConvertTo-Json -Compress";

/// One installed program as Windows lists it under "Installed apps".
struct Installed {
    name: String,
    version: String,
    location: String,
    size: u64,
    id: String,
}

fn parse_installed(json: &str) -> Vec<Installed> {
    let mut seen = HashSet::new();
    let mut out: Vec<Installed> = json_list(json)
        .iter()
        .filter_map(|v| {
            let name = text_of(v, "DisplayName");
            if name.is_empty() || !seen.insert(name.to_lowercase()) {
                return None;
            }
            // EstimatedSize is in kilobytes
            let size = v.get("EstimatedSize").and_then(|x| x.as_u64()).unwrap_or(0) * 1024;
            Some(Installed { name, version: text_of(v, "DisplayVersion"), location: text_of(v, "InstallLocation").trim_matches('"').to_string(), size, id: text_of(v, "PSChildName") })
        })
        .collect();
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

pub fn scan_apps(p: &Progress) -> Vec<AppInfo> {
    p.set("Asking Windows for the installed programs…");
    let installed = powershell(APPS_SCRIPT).map(|t| parse_installed(&t)).unwrap_or_default();
    let lib = leftover_candidates();
    let mut out = vec![];
    for a in installed {
        if p.cancelled() {
            break;
        }
        p.set(format!("Measuring {}", a.name));
        let path = PathBuf::from(&a.location);
        let size = if a.size > 0 {
            a.size
        } else if !a.location.is_empty() && path.is_dir() {
            dir_size(&path, Some(p))
        } else {
            0
        };
        let leftovers: Vec<(PathBuf, u64)> = crate::scan::find_leftovers(&a.name, "", &lib).into_iter().map(|l| {
            let s = dir_size(&l, None);
            (l, s)
        }).collect();
        // matched by name only, so nothing starts ticked
        let skip = leftovers.iter().map(|l| l.0.clone()).collect();
        out.push(AppInfo {
            name: a.name,
            path,
            bundle_id: a.id,
            version: a.version,
            size,
            leftovers,
            skip,
            last_used_days: 0,
            managed: Some("To remove this program, use Windows Settings › Apps › Installed apps. Cleanerio can remove the files it keeps in your profile.".to_string()),
        });
    }
    out
}

/// How many programs are installed (for the Smart Scan). Windows doesn't record when a
/// program was last opened, so none are reported as unused.
pub fn apps_summary() -> (usize, Vec<String>) {
    (powershell(APPS_SCRIPT).map(|t| parse_installed(&t).len()).unwrap_or(0), vec![])
}

// ---------------------------------------------------------------- maintenance

pub fn tasks() -> Vec<Task> {
    vec![
        Task { title: "Flush DNS cache", desc: "Fixes websites that won't load after network changes.", icon: "", admin: false, cmd: "ipconfig /flushdns", selected: true, last: None },
        Task { title: "Rebuild the icon cache", desc: "Fixes icons that are blank or show the wrong picture.", icon: "", admin: false, cmd: "ie4uinit.exe -show", selected: false, last: None },
        Task { title: "Restart Explorer", desc: "Fixes a stuck taskbar, Start menu or folder window without signing out.", icon: "", admin: false, cmd: "taskkill /F /IM explorer.exe & start explorer.exe", selected: false, last: None },
        Task { title: "Clean up Windows Update files", desc: "Removes old versions of updated system files for good. Can take several minutes.", icon: "", admin: true, cmd: "Dism.exe /Online /Cleanup-Image /StartComponentCleanup", selected: false, last: None },
        Task { title: "Check system files", desc: "Looks for damaged Windows files and repairs them. Can take ten minutes or more.", icon: "", admin: true, cmd: "sfc /scannow", selected: false, last: None },
    ]
}

/// Runs one command the way the Command Prompt would.
pub fn run_plain(cmd: &str) -> std::io::Result<std::process::Output> {
    quiet("cmd").args(["/C", cmd]).output()
}

/// Runs commands as administrator; Windows asks for permission once.
pub fn run_admin(cmds: &[String]) -> String {
    let joined = cmds.join(" & ");
    let script = format!(
        "try {{ $p = Start-Process -FilePath 'cmd.exe' -ArgumentList '/C',{} -Verb RunAs -WindowStyle Hidden -Wait -PassThru; exit $p.ExitCode }} catch {{ exit 1223 }}",
        ps_quote(&joined)
    );
    match quiet("powershell").args(["-NoProfile", "-NonInteractive", "-Command", &script]).output() {
        Ok(o) if o.status.success() => "Done".to_string(),
        Ok(o) if o.status.code() == Some(1223) => "Cancelled".to_string(),
        Ok(o) => format!("Finished with code {}", o.status.code().unwrap_or(-1)),
        Err(e) => format!("Failed: {e}"),
    }
}

// ---------------------------------------------------------------- cloud folders

/// Folders that cloud apps keep in the person's profile.
pub fn cloud_roots() -> Vec<(String, PathBuf)> {
    const NAMES: &[&str] = &["dropbox", "google drive", "googledrive", "my drive", "iclouddrive", "nextcloud", "owncloud", "pclouddrive", "mega", "megasync", "proton drive", "box", "sync"];
    children(&home())
        .into_iter()
        .filter(|c| c.is_dir())
        .filter(|c| {
            let n = file_name(c).to_lowercase();
            n == "onedrive" || n.starts_with("onedrive - ") || n.starts_with("dropbox (") || NAMES.contains(&n.as_str())
        })
        .map(|c| (file_name(&c), c))
        .collect()
}

// ---------------------------------------------------------------- battery, network, windows

/// Battery and Wi-Fi are asked for on a background thread (PowerShell takes a second to
/// start), and the last answer is handed out meanwhile.
static STATUS: Mutex<(Option<(u8, String)>, Option<String>, bool)> = Mutex::new((None, None, false));
static STATUS_READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// True once the first battery and Wi-Fi answer has arrived.
pub fn status_ready() -> bool {
    STATUS_READY.load(std::sync::atomic::Ordering::Relaxed)
}

fn refresh_status() {
    {
        let mut s = STATUS.lock().unwrap();
        if s.2 {
            return; // already asking
        }
        s.2 = true;
    }
    std::thread::spawn(|| {
        let battery = powershell("Get-CimInstance Win32_Battery | Select-Object -First 1 EstimatedChargeRemaining,BatteryStatus | ConvertTo-Json -Compress").and_then(|t| {
            let v = json_list(&t).into_iter().next()?;
            let pct = v.get("EstimatedChargeRemaining")?.as_u64()?.min(100) as u8;
            // 1 = running on the battery; 2 = plugged in
            let state = match v.get("BatteryStatus").and_then(|x| x.as_u64()) {
                Some(1) => "On battery",
                Some(2) if pct >= 100 => "Charged",
                Some(3) => "Charged",
                _ => "Charging",
            };
            Some((pct, state.to_string()))
        });
        let wifi = quiet("netsh").args(["wlan", "show", "interfaces"]).output().ok().and_then(|o| {
            String::from_utf8_lossy(&o.stdout).lines().find_map(|l| {
                let (k, v) = l.split_once(':')?;
                (k.trim() == "SSID").then(|| v.trim().to_string()).filter(|s| !s.is_empty())
            })
        });
        let mut s = STATUS.lock().unwrap();
        *s = (battery, wifi, false);
        STATUS_READY.store(true, std::sync::atomic::Ordering::Relaxed);
    });
}
pub fn battery() -> Option<(u8, String)> {
    refresh_status();
    STATUS.lock().unwrap().0.clone()
}
pub fn wifi_name() -> Option<String> {
    STATUS.lock().unwrap().1.clone()
}

/// Shows a file selected in File Explorer. A registry startup entry has no file to show.
pub fn reveal(p: &Path) {
    if registry_entry(p).is_some() {
        return;
    }
    let _ = quiet("explorer").arg(format!("/select,{}", p.display())).spawn();
}

/// Opens Windows' own storage overview.
pub fn open_storage_settings() {
    let _ = quiet("cmd").args(["/C", "start", "", "ms-settings:storagesense"]).spawn();
}

/// Asks for a file with the standard Windows file window.
pub fn pick_file() -> Option<PathBuf> {
    let script = "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.OpenFileDialog; $d.Title = 'Choose a threat database (.json)'; $d.Filter = 'JSON files (*.json)|*.json'; if ($d.ShowDialog() -eq 'OK') { $d.FileName }";
    let full = format!("[Console]::OutputEncoding=[Text.Encoding]::UTF8; {script}");
    let out = quiet("powershell").args(["-NoProfile", "-STA", "-Command", &full]).output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !s.is_empty()).then(|| PathBuf::from(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_of_a_startup_command() {
        assert_eq!(command_program("\"C:\\Program Files\\App\\app.exe\" --minimized"), "C:\\Program Files\\App\\app.exe");
        assert_eq!(command_program("C:\\Program Files\\App\\App.EXE /background"), "C:\\Program Files\\App\\App.EXE");
        assert_eq!(command_program("rundll32 shell32.dll,Control_RunDLL"), "rundll32");
        assert_eq!(command_program(""), "");
    }

    #[test]
    fn registry_entries_round_trip() {
        let p = registry_path("HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run", "My App");
        assert_eq!(registry_entry(&p), Some(("HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run".to_string(), "My App".to_string())));
        assert!(registry_entry(Path::new("C:\\Users\\tao\\file.txt")).is_none());
    }

    #[test]
    fn installed_programs_from_json() {
        let one = r#"{"DisplayName":"Paint Pro","DisplayVersion":"2.1","InstallLocation":"C:\\Program Files\\Paint Pro\\","EstimatedSize":2048,"PSChildName":"{ABC}","Publisher":"Acme"}"#;
        let v = parse_installed(one);
        assert_eq!(v.len(), 1);
        assert_eq!((v[0].name.as_str(), v[0].version.as_str(), v[0].size), ("Paint Pro", "2.1", 2048 * 1024));
        let many = format!("[{one},{one},{{\"DisplayName\":\"Editor\",\"EstimatedSize\":null,\"PSChildName\":\"Editor\"}}]");
        let v = parse_installed(&many);
        assert_eq!(v.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["Editor", "Paint Pro"]);
        assert_eq!(v[0].size, 0);
        assert!(parse_installed("").is_empty());
    }

    #[test]
    fn a_fresh_file_keeps_its_folder() {
        let dir = std::env::temp_dir().join(format!("cleanerio-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("deep")).unwrap();
        std::fs::write(dir.join("deep/new.txt"), "x").unwrap();
        assert!(!untouched_for_a_day(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn quoting_for_powershell() {
        assert_eq!(ps_quote("it's"), "'it''s'");
    }

    #[test]
    fn windows_paths_compare_without_case() {
        assert!(under(&norm(Path::new("C:\\Windows\\System32")), &norm(Path::new("c:\\windows"))));
        assert!(!under(&norm(Path::new("C:\\WindowsApps")), &norm(Path::new("c:\\windows"))));
    }
}
