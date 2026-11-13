//! The Linux side of Cleanerio: where caches, the Trash, apps and startup entries live on a
//! Linux desktop, and the commands that stand in for the Mac-only ones in `scan.rs`.
//!
//! Everything here compiles on both systems; `scan.rs` calls it only when `ON` is true.

use crate::scan::{children, dir_size, file_name, home, AppInfo, Progress, Task};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// True in the Linux build.
pub const ON: bool = cfg!(target_os = "linux");

fn xdg(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home().join(fallback))
}
pub fn cache_home() -> PathBuf {
    xdg("XDG_CACHE_HOME", ".cache")
}
pub fn config_home() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config")
}
pub fn data_home() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share")
}
/// Where the desktop keeps trashed files.
pub fn trash_dir() -> PathBuf {
    data_home().join("Trash/files")
}

// ---------------------------------------------------------------- protected folders

/// Folders Linux or the person's account can't work without.
pub fn is_protected(path: &Path) -> bool {
    let h = home();
    const SYSTEM: &[&str] = &[
        "/usr", "/bin", "/sbin", "/lib", "/lib32", "/lib64", "/libx32", "/boot", "/proc", "/sys", "/dev", "/run", "/etc", "/var", "/snap", "/srv",
    ];
    if SYSTEM.iter().any(|s| path.starts_with(s)) {
        return true;
    }
    const TOP: &[&str] = &["/opt", "/tmp", "/home", "/root", "/mnt", "/media", "/lost+found"];
    if TOP.iter().any(|s| path == Path::new(s)) {
        return true;
    }
    if path.parent().is_some_and(|p| p == Path::new("/home")) {
        return true;
    }
    // the standard folders in the home folder
    const HOME: &[&str] = &[
        "Documents", "Desktop", "Downloads", "Videos", "Music", "Pictures", "Public", "Templates", "Applications", "snap", ".config", ".cache", ".local",
        ".local/share", ".local/state", ".local/bin", ".local/share/applications", ".local/share/Trash", ".local/share/flatpak", ".var", ".var/app",
        ".mozilla", ".thunderbird",
    ];
    if HOME.iter().any(|d| path == h.join(d)) || [cache_home(), config_home(), data_home()].iter().any(|d| path == d) {
        return true;
    }
    // keys and passwords, and everything inside them
    const SECRETS: &[&str] = &[".ssh", ".gnupg", ".pki", ".password-store", ".local/share/keyrings", ".local/share/kwalletd"];
    SECRETS.iter().any(|d| path.starts_with(h.join(d)))
}

/// Places the space map never walks into: kernel views, other drives and network shares.
pub fn outside_this_disk(path: &Path) -> bool {
    const SKIP: &[&str] = &["/proc", "/sys", "/dev", "/run", "/mnt", "/media", "/snap", "/net"];
    SKIP.iter().any(|s| path == Path::new(s))
}

// ---------------------------------------------------------------- running programs

/// Lower-case names of the programs that are running now (the program file and its folder).
pub fn running_names() -> HashSet<String> {
    let mut ids = HashSet::new();
    let Ok(mut sys) = sysinfo::System::new() else { return ids };
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    for proc_ in sys.processes().values() {
        ids.insert(proc_.name().to_string_lossy().to_lowercase());
        if let Some(exe) = proc_.exe() {
            ids.insert(file_name(exe).to_lowercase());
            if let Some(dir) = exe.parent() {
                ids.insert(file_name(dir).to_lowercase());
            }
        }
    }
    ids.retain(|n| n.len() >= 3);
    ids
}

/// Cache folders whose name differs from the program that owns them.
const CACHE_OWNERS: &[(&str, &str)] = &[("mozilla", "firefox"), ("bravesoftware", "brave"), ("microsoft-edge", "msedge"), ("vivaldi", "vivaldi-bin")];

/// A cache folder belongs to a running program when it carries the program's name.
pub fn cache_in_use(folder_name: &str, running: &HashSet<String>) -> bool {
    let n = folder_name.to_lowercase();
    if running.contains(&n) {
        return true;
    }
    if CACHE_OWNERS.iter().any(|(folder, prog)| n == *folder && running.contains(*prog)) {
        return true;
    }
    n.split(['-', '_', '.']).any(|part| part.len() >= 4 && running.contains(part))
}

/// Caches that are slow or costly to rebuild (downloaded models, virtual environments,
/// browsers for testing). They are listed but not ticked.
const KEEP_BY_DEFAULT: &[&str] = &[
    "huggingface", "torch", "whisper", "ollama", "pypoetry", "pre-commit", "ms-playwright", "puppeteer", "cypress", "jetbrains", "bazel", "wine",
    "winetricks", "lutris", "virtualenvs", "uv",
];
pub fn keep_cache_by_default(folder_name: &str) -> bool {
    KEEP_BY_DEFAULT.contains(&folder_name.to_lowercase().as_str())
}

/// Session programs that keep the desktop running; never offered for quitting.
const SESSION: &[&str] = &[
    "systemd", "dbus-daemon", "dbus-broker", "gnome-shell", "gnome-session", "plasmashell", "kwin_wayland", "kwin_x11", "xorg", "xwayland", "pipewire",
    "pipewire-pulse", "wireplumber", "pulseaudio", "gjs", "ibus-daemon", "fcitx5", "ssh-agent", "gpg-agent", "sshd", "login", "bash", "zsh", "fish", "sh",
    "dconf-service", "xfce4-session", "xfwm4", "xfce4-panel", "xfdesktop", "cinnamon", "mate-session", "sway", "hyprland", "gdm-wayland-session",
    "gdm-x-session", "sddm", "lightdm", "snapd", "(sd-pam)", "nautilus", "polkitd", "ksmserver", "kded5", "kded6", "mutter",
];

/// The programs the person opened that use the most memory: (id, name, memory, processor share).
pub fn heavy_apps() -> Vec<(sysinfo::Pid, String, u64, f32)> {
    let Ok(mut sys) = sysinfo::System::new() else { return vec![] };
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let me = std::fs::metadata(home()).map(|m| crate::scan::owner_id(&m)).ok();
    let mut v: Vec<_> = sys
        .processes()
        .iter()
        .filter(|(pid, p)| {
            let name = p.name().to_string_lossy().to_lowercase();
            let exe = p.exe().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
            pid.as_u32() != std::process::id()
                && !exe.is_empty()
                && std::fs::metadata(format!("/proc/{}", pid.as_u32())).map(|m| crate::scan::owner_id(&m)).ok() == me
                && !SESSION.contains(&name.as_str())
                && !exe.starts_with("/usr/libexec/")
                && !exe.starts_with("/usr/lib/systemd/")
                && !["gvfs", "xdg-", "at-spi", "tracker-", "localsearch", "evolution-", "gsd-", "goa-", "ibus-", "kio", "baloo"].iter().any(|s| name.starts_with(s))
        })
        .map(|(pid, p)| (*pid, p.name().to_string_lossy().to_string(), p.memory(), p.cpu_usage()))
        .collect();
    v.sort_by(|a, b| b.2.cmp(&a.2));
    v.truncate(8);
    v
}

// ---------------------------------------------------------------- desktop entries

/// The keys of the "[Desktop Entry]" (or "[Service]"/"[Unit]") part of a .desktop or unit file.
fn read_entry(path: &Path, sections: &[&str]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Ok(text) = std::fs::read_to_string(path) else { return out };
    let mut inside = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = sections.contains(&line);
            continue;
        }
        if !inside || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            out.entry(k.trim().to_string()).or_insert_with(|| v.trim().to_string());
        }
    }
    out
}

/// The program a command line starts, without `env VAR=…` in front, quotes or switches.
pub fn exec_program(exec: &str) -> String {
    // split into words, keeping a quoted path with spaces in one piece
    let mut words: Vec<String> = vec![];
    let (mut cur, mut quote) = (String::new(), None::<char>);
    for c in exec.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => cur.push(c),
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c.is_whitespace() => {
                if !cur.is_empty() {
                    words.push(std::mem::take(&mut cur));
                }
            }
            None => cur.push(c),
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    let first = words.into_iter().find(|w| w != "env" && !(w.contains('=') && !w.starts_with('/')));
    // systemd marks such as "-/usr/bin/x" or "@/usr/bin/x"
    first.unwrap_or_default().trim_start_matches(['-', '@', '+', '!', ':']).to_string()
}

/// Finds a program on the PATH when only its name is given.
fn resolve(program: &str) -> Option<PathBuf> {
    if program.is_empty() {
        return None;
    }
    let p = Path::new(program);
    if p.is_absolute() {
        return p.exists().then(|| p.to_path_buf());
    }
    let path = std::env::var("PATH").unwrap_or_else(|_| "/usr/local/bin:/usr/bin:/bin".into());
    path.split(':').map(|d| Path::new(d).join(program)).find(|c| c.is_file())
}

/// (label, program) of a startup entry: an autostart .desktop file or a systemd user unit.
pub fn entry_program(path: &Path) -> (String, String) {
    if path.extension().is_some_and(|e| e == "desktop") {
        let d = read_entry(path, &["[Desktop Entry]"]);
        let label = d.get("Name").cloned().unwrap_or_else(|| file_name(path));
        return (label, exec_program(d.get("Exec").map(|s| s.as_str()).unwrap_or("")));
    }
    let d = read_entry(path, &["[Service]"]);
    (file_name(path), exec_program(d.get("ExecStart").map(|s| s.as_str()).unwrap_or("")))
}

/// The folders that hold things that start by themselves, with what the app calls them.
pub fn startup_places() -> Vec<(PathBuf, &'static str)> {
    vec![
        (config_home().join("autostart"), "Your startup apps"),
        (PathBuf::from("/etc/xdg/autostart"), "All users' startup apps"),
        (config_home().join("systemd/user"), "Your background services"),
    ]
}
pub fn is_startup_file(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "desktop" || e == "service" || e == "timer")
}

/// The systemd unit behind a file in the person's own unit folder.
pub fn agent_label(path: &Path) -> Option<String> {
    let in_units = path.parent().is_some_and(|p| p.ends_with("systemd/user"));
    (in_units && path.extension().is_some_and(|e| e == "service" || e == "timer")).then(|| file_name(path))
}
/// Stops and disables a user service, so removing its file takes effect now.
pub fn unload_agent(unit: &str) {
    let _ = std::process::Command::new("systemctl").args(["--user", "disable", "--now", unit]).output();
}

/// The apps that open by themselves at login: (name, file).
fn autostart_entries() -> Vec<(String, PathBuf)> {
    children(&config_home().join("autostart"))
        .into_iter()
        .filter(|f| f.extension().is_some_and(|e| e == "desktop"))
        .filter_map(|f| {
            let d = read_entry(&f, &["[Desktop Entry]"]);
            if d.get("Hidden").is_some_and(|v| v == "true") {
                return None;
            }
            Some((d.get("Name").cloned().unwrap_or_else(|| file_name(&f).trim_end_matches(".desktop").to_string()), f))
        })
        .collect()
}
pub fn login_items() -> Result<Vec<String>, String> {
    let mut v: Vec<String> = autostart_entries().into_iter().map(|e| e.0).collect();
    v.sort_by_key(|n| n.to_lowercase());
    Ok(v)
}
/// Moves the startup entry to the Trash, so it can be put back.
pub fn remove_login_item(name: &str) -> Result<(), String> {
    let (_, file) = autostart_entries().into_iter().find(|e| e.0 == name).ok_or("That startup entry is no longer there.")?;
    trash::delete(&file).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- applications

pub const LEFTOVER_DIRS: &[&str] = &[".config", ".cache", ".local/share", ".local/state", ".var/app"];

fn desktop_dirs() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        PathBuf::from("/var/lib/snapd/desktop/applications"),
        data_home().join("flatpak/exports/share/applications"),
        data_home().join("applications"),
    ]
}

struct Entry {
    file: PathBuf,
    id: String,
    name: String,
    program: Option<PathBuf>,
}

/// Every app the desktop shows in its menu. A later folder wins over an earlier one, the way
/// the desktop itself lets the person's own entry replace the system's.
fn desktop_entries() -> Vec<Entry> {
    let mut by_id: HashMap<String, Entry> = HashMap::new();
    for dir in desktop_dirs() {
        for f in children(&dir) {
            if f.extension().is_none_or(|e| e != "desktop") {
                continue;
            }
            let d = read_entry(&f, &["[Desktop Entry]"]);
            let id = file_name(&f).trim_end_matches(".desktop").to_string();
            let hidden = ["NoDisplay", "Hidden"].iter().any(|k| d.get(*k).is_some_and(|v| v == "true"));
            if hidden || d.get("Type").is_some_and(|t| t != "Application") || d.get("Exec").is_none() {
                by_id.remove(&id);
                continue;
            }
            let name = d.get("Name").cloned().unwrap_or_else(|| id.clone());
            let program = resolve(&exec_program(&d["Exec"]));
            by_id.insert(id.clone(), Entry { file: f, id, name, program });
        }
    }
    by_id.into_values().collect()
}

/// True when access times on the home disk are switched off, so "last opened" can't be known.
fn access_times_off() -> bool {
    std::fs::read_to_string("/proc/mounts").is_ok_and(|m| {
        m.lines().any(|l| {
            let mut it = l.split_whitespace();
            let (_, at, _, opts) = (it.next(), it.next(), it.next(), it.next().unwrap_or(""));
            at == Some("/") && opts.split(',').any(|o| o == "noatime")
        })
    })
}

/// Days since the program was last started, by its access time. 0 when that can't be known.
fn program_idle_days(program: Option<&PathBuf>, no_atime: bool) -> u64 {
    if no_atime {
        return 0;
    }
    program.and_then(|p| std::fs::metadata(p).ok()).and_then(|m| m.accessed().ok().max(m.modified().ok())).map(crate::scan::days_since).unwrap_or(0)
}

/// Folders an app can be dropped into and removed from by hand.
fn loose_app_dirs() -> Vec<PathBuf> {
    let h = home();
    vec![h.join("Applications"), h.join("AppImages"), h.join(".local/bin"), h.join("bin")]
}
fn is_appimage(p: &Path) -> bool {
    p.extension().is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("appimage"))
}

/// Package name, version and installed size for files owned by the system's package manager.
fn package_info(files: &[PathBuf]) -> HashMap<PathBuf, (String, String, u64)> {
    let mut out = HashMap::new();
    if files.is_empty() {
        return out;
    }
    let run = |cmd: &str, args: Vec<String>| std::process::Command::new(cmd).args(args).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).to_string());
    // Debian, Ubuntu and relatives
    let mut owner: HashMap<String, Vec<PathBuf>> = HashMap::new();
    if let Some(text) = run("dpkg", std::iter::once("-S".to_string()).chain(files.iter().map(|f| f.display().to_string())).collect()) {
        for line in text.lines() {
            if let Some((pkg, file)) = line.split_once(": ") {
                let pkg = pkg.split([',', ':']).next().unwrap_or(pkg).trim().to_string();
                owner.entry(pkg).or_default().push(PathBuf::from(file.trim()));
            }
        }
    }
    if !owner.is_empty() {
        let mut args = vec!["-W".to_string(), "-f=${Package}\\t${Version}\\t${Installed-Size}\\n".to_string()];
        args.extend(owner.keys().cloned());
        if let Some(text) = run("dpkg-query", args) {
            for line in text.lines() {
                let mut it = line.split('\t');
                let (Some(pkg), Some(ver), Some(kb)) = (it.next(), it.next(), it.next()) else { continue };
                for f in owner.get(pkg).into_iter().flatten() {
                    out.insert(f.clone(), (pkg.to_string(), ver.to_string(), kb.trim().parse::<u64>().unwrap_or(0) * 1024));
                }
            }
        }
        return out;
    }
    // Fedora, openSUSE and relatives
    let mut args = vec!["-qf".to_string(), "--qf".to_string(), "%{NAME}\\t%{VERSION}\\t%{SIZE}\\n".to_string()];
    args.extend(files.iter().map(|f| f.display().to_string()));
    if let Some(text) = run("rpm", args) {
        // one line per file, in the order asked
        for (f, line) in files.iter().zip(text.lines()) {
            let mut it = line.split('\t');
            if let (Some(pkg), Some(ver), Some(size)) = (it.next(), it.next(), it.next()) {
                if let Ok(size) = size.trim().parse::<u64>() {
                    out.insert(f.clone(), (pkg.to_string(), ver.to_string(), size));
                }
            }
        }
    }
    out
}

pub fn scan_apps(p: &Progress) -> Vec<AppInfo> {
    let h = home();
    let lib: Vec<PathBuf> = LEFTOVER_DIRS.iter().flat_map(|d| children(&h.join(d))).filter(|c| !is_protected(c)).collect();
    let entries = desktop_entries();
    let no_atime = access_times_off();
    let system: Vec<PathBuf> = entries.iter().map(|e| e.file.clone()).filter(|f| f.starts_with("/usr")).collect();
    p.set("Asking the package manager…");
    let packages = package_info(&system);
    let loose = loose_app_dirs();
    let mut out = vec![];
    let mut seen: HashSet<PathBuf> = HashSet::new();

    for e in entries {
        if p.cancelled() {
            break;
        }
        p.set(format!("Measuring {}", e.name));
        let file = e.file.to_string_lossy().to_string();
        let exe_name = e.program.as_ref().map(|x| file_name(x)).unwrap_or_default();
        let mut leftovers: Vec<PathBuf> = crate::scan::find_leftovers(&e.name, &e.id, &lib);
        for extra in crate::scan::find_leftovers(&exe_name, "", &lib) {
            if !leftovers.contains(&extra) {
                leftovers.push(extra);
            }
        }
        let (path, size, version, managed, idle);
        if file.contains("/flatpak/exports/") {
            let base = e.file.ancestors().nth(4).map(|b| b.join("app").join(&e.id)).unwrap_or_default();
            path = e.file.clone();
            size = dir_size(&base, Some(p));
            version = String::new();
            managed = Some(format!("Installed with Flatpak. To remove it, run: flatpak uninstall {}", e.id));
            idle = 0;
        } else if file.contains("/snapd/desktop/") {
            let snap = e.id.split('_').next().unwrap_or(&e.id).to_string();
            path = e.file.clone();
            size = children(Path::new("/var/lib/snapd/snaps"))
                .iter()
                .filter(|f| file_name(f).starts_with(&format!("{snap}_")))
                .filter_map(|f| std::fs::metadata(f).ok().map(|m| m.len()))
                .max()
                .unwrap_or(0);
            version = String::new();
            managed = Some(format!("Installed as a snap. To remove it, run: snap remove {snap}"));
            idle = 0;
            for extra in children(&h.join("snap")).into_iter().filter(|c| file_name(c) == snap) {
                leftovers.push(extra);
            }
        } else if let Some(prog) = e.program.as_ref().filter(|x| x.starts_with(&h) && (is_appimage(x) || loose.iter().any(|d| x.parent() == Some(d)))) {
            // an app the person put in their home folder: it can go to the Trash
            path = prog.clone();
            size = std::fs::metadata(prog).map(|m| m.len()).unwrap_or(0);
            version = String::new();
            managed = None;
            idle = program_idle_days(Some(prog), no_atime);
            if e.file.starts_with(&h) && !leftovers.contains(&e.file) {
                leftovers.push(e.file.clone());
            }
            seen.insert(prog.clone());
        } else {
            let pkg = packages.get(&e.file);
            path = e.program.clone().unwrap_or_else(|| e.file.clone());
            size = pkg.map(|k| k.2).unwrap_or(0);
            version = pkg.map(|k| k.1.clone()).unwrap_or_default();
            managed = Some(match pkg {
                Some(k) => format!("Installed as the system package “{}”. Remove it with your software manager.", k.0),
                None if path.starts_with(&h) => "Installed by hand in your home folder. Remove its folder yourself.".to_string(),
                None => "Installed for the whole computer. Remove it with your software manager.".to_string(),
            });
            idle = program_idle_days(e.program.as_ref(), no_atime);
        }
        let leftovers: Vec<(PathBuf, u64)> = leftovers.into_iter().map(|l| {
            let s = dir_size(&l, None);
            (l, s)
        }).collect();
        // the app's own menu entry goes with the app; other name-only matches start unticked
        let mut skip = crate::scan::name_only_leftovers(&e.id, &leftovers);
        skip.remove(&e.file);
        out.push(AppInfo { name: e.name, path, bundle_id: e.id, version, size, leftovers, skip, last_used_days: idle, managed });
    }

    // AppImages that have no menu entry
    for dir in loose {
        for f in children(&dir).into_iter().filter(|f| is_appimage(f) && !seen.contains(f)) {
            let name = file_name(&f).trim_end_matches(".AppImage").trim_end_matches(".appimage").to_string();
            let leftovers: Vec<(PathBuf, u64)> = crate::scan::find_leftovers(&name, "", &lib).into_iter().map(|l| {
                let s = dir_size(&l, None);
                (l, s)
            }).collect();
            let skip = leftovers.iter().map(|l| l.0.clone()).collect();
            let size = std::fs::metadata(&f).map(|m| m.len()).unwrap_or(0);
            let idle = program_idle_days(Some(&f), no_atime);
            out.push(AppInfo { name, path: f, bundle_id: String::new(), version: String::new(), size, leftovers, skip, last_used_days: idle, managed: None });
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// How many apps there are and which were not started for a long time (for the Smart Scan).
pub fn apps_summary() -> (usize, Vec<String>) {
    let no_atime = access_times_off();
    let entries = desktop_entries();
    let unused = entries
        .iter()
        .filter(|e| program_idle_days(e.program.as_ref(), no_atime) > crate::pages::UNUSED_APP_DAYS)
        .map(|e| e.name.clone())
        .collect();
    (entries.len(), unused)
}

// ---------------------------------------------------------------- trash

/// Empties the desktop's Trash for good. Returns the error text when it fails.
#[cfg(target_os = "linux")]
pub fn empty_trash() -> Result<(), String> {
    let items = trash::os_limited::list().map_err(|e| e.to_string())?;
    trash::os_limited::purge_all(items).map_err(|e| e.to_string())?;
    // files that sit in the Trash without a record of where they came from
    for left in children(&trash_dir()) {
        let is_dir = std::fs::symlink_metadata(&left).is_ok_and(|m| m.is_dir());
        let gone = if is_dir { std::fs::remove_dir_all(&left) } else { std::fs::remove_file(&left) };
        gone.map_err(|e| format!("{}: {e}", file_name(&left)))?;
    }
    Ok(())
}
#[cfg(not(target_os = "linux"))]
pub fn empty_trash() -> Result<(), String> {
    Err("not available on this system".into())
}

// ---------------------------------------------------------------- maintenance

pub fn tasks() -> Vec<Task> {
    vec![
        Task { title: "Free up memory", desc: "Rarely needed: Linux manages memory well on its own.", icon: "🧠", admin: true, cmd: "sync; echo 3 > /proc/sys/vm/drop_caches", selected: false, last: None },
        Task { title: "Flush DNS cache", desc: "Fixes websites that won't load after network changes.", icon: "🌐", admin: true, cmd: "resolvectl flush-caches", selected: false, last: None },
        Task { title: "Trim the system journal", desc: "Keeps the last two weeks of system logs and erases older entries for good.", icon: "📄", admin: true, cmd: "journalctl --vacuum-time=2weeks", selected: false, last: None },
        Task { title: "Rebuild the font cache", desc: "Fixes fonts that are missing or show as boxes after installing new ones.", icon: "🔤", admin: false, cmd: "fc-cache -f", selected: false, last: None },
        Task { title: "Refresh the app menu", desc: "Fixes missing or duplicate entries in the app menu and in “Open With”.", icon: "📋", admin: false, cmd: "update-desktop-database \"${XDG_DATA_HOME:-$HOME/.local/share}/applications\"", selected: true, last: None },
    ]
}

/// Runs commands as administrator through the desktop's own password window.
pub fn run_admin(joined: &str) -> String {
    match std::process::Command::new("pkexec").args(["sh", "-c", joined]).output() {
        Ok(o) if o.status.success() => "Done".to_string(),
        Ok(o) if o.status.code() == Some(126) => "Cancelled".to_string(),
        Ok(o) if o.status.code() == Some(127) => "Failed: the password window isn't available (pkexec)".to_string(),
        Ok(o) => format!("Failed: {}", String::from_utf8_lossy(&o.stderr).lines().next().unwrap_or("").trim()),
        Err(_) => "Failed: pkexec is not installed".to_string(),
    }
}

// ---------------------------------------------------------------- cloud folders

/// Folders that desktop cloud apps keep in the home folder.
pub fn cloud_roots() -> Vec<(String, PathBuf)> {
    const NAMES: &[&str] = &[
        "Dropbox", "OneDrive", "Nextcloud", "ownCloud", "Google Drive", "GoogleDrive", "pCloudDrive", "MEGA", "MEGAsync", "Insync", "Proton Drive",
        "Seafile", "SynologyDrive", "Sync", "Yandex.Disk",
    ];
    children(&home())
        .into_iter()
        .filter(|c| c.is_dir())
        .filter(|c| {
            let n = file_name(c);
            NAMES.contains(&n.as_str()) || n.starts_with("Dropbox (") || n.starts_with("OneDrive - ")
        })
        .map(|c| (file_name(&c), c))
        .collect()
}

// ---------------------------------------------------------------- battery, network, windows

fn battery_dir() -> Option<PathBuf> {
    children(Path::new("/sys/class/power_supply")).into_iter().find(|d| {
        std::fs::read_to_string(d.join("type")).is_ok_and(|t| t.trim() == "Battery") && d.join("capacity").exists() && !d.join("scope").exists()
    })
}
fn read_num(path: PathBuf) -> Option<f64> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}
pub fn battery() -> Option<(u8, String)> {
    let d = battery_dir()?;
    let pct = read_num(d.join("capacity"))?.clamp(0.0, 100.0) as u8;
    let state = match std::fs::read_to_string(d.join("status")).unwrap_or_default().trim() {
        "Discharging" => "On battery",
        "Charging" => "Charging",
        _ => "Charged",
    };
    Some((pct, state.to_string()))
}
/// "Normal", or "Service Recommended" when the battery holds under 60% of what it did new.
pub fn battery_condition() -> Option<String> {
    let d = battery_dir()?;
    let full = read_num(d.join("energy_full")).or_else(|| read_num(d.join("charge_full")))?;
    let design = read_num(d.join("energy_full_design")).or_else(|| read_num(d.join("charge_full_design")))?;
    if design <= 0.0 {
        return None;
    }
    Some(if full / design < 0.6 { "Service Recommended" } else { "Normal" }.to_string())
}
pub fn wifi_name() -> Option<String> {
    let out = std::process::Command::new("iwgetid").arg("-r").output().ok();
    let name = out.filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).filter(|s| !s.is_empty());
    name.or_else(|| {
        let o = std::process::Command::new("nmcli").args(["-t", "-f", "active,ssid", "dev", "wifi"]).output().ok()?;
        String::from_utf8_lossy(&o.stdout).lines().find_map(|l| l.strip_prefix("yes:").map(|s| s.to_string())).filter(|s| !s.is_empty())
    })
}

/// Shows a file in the desktop's file manager, selected when the file manager supports it.
pub fn reveal(p: &Path) {
    let mut uri = String::from("file://");
    for b in p.to_string_lossy().bytes() {
        if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) {
            uri.push(b as char);
        } else {
            uri.push_str(&format!("%{b:02X}"));
        }
    }
    let shown = std::process::Command::new("dbus-send")
        .args(["--session", "--print-reply", "--dest=org.freedesktop.FileManager1", "--type=method_call", "/org/freedesktop/FileManager1", "org.freedesktop.FileManager1.ShowItems"])
        .arg(format!("array:string:{uri}"))
        .arg("string:")
        .output()
        .is_ok_and(|o| o.status.success());
    if !shown {
        let folder = if p.is_dir() { p } else { p.parent().unwrap_or(p) };
        let _ = std::process::Command::new("xdg-open").arg(folder).spawn();
    }
}

/// Opens the desktop's disk usage tool. False when none is installed.
pub fn open_disk_usage() -> bool {
    ["baobab", "filelight", "qdirstat", "gnome-disks"].iter().any(|tool| std::process::Command::new(tool).spawn().is_ok())
}

/// Asks for a file with the desktop's own file window.
pub fn pick_file() -> Option<PathBuf> {
    let try_tool = |cmd: &str, args: &[&str]| {
        let out = std::process::Command::new(cmd).args(args).output().ok()?;
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        (out.status.success() && !s.is_empty()).then(|| PathBuf::from(s))
    };
    try_tool("zenity", &["--file-selection", "--title=Choose a threat database (.json)", "--file-filter=*.json"])
        .or_else(|| try_tool("kdialog", &["--getopenfilename", ".", "*.json"]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_of_a_command_line() {
        assert_eq!(exec_program("/usr/bin/firefox %u"), "/usr/bin/firefox");
        assert_eq!(exec_program("env GDK_BACKEND=x11 \"/opt/My App/run\" --flag"), "/opt/My App/run");
        assert_eq!(exec_program("env A=1 B=2 code --new-window %F"), "code");
        assert_eq!(exec_program("-/usr/bin/sync-daemon --quiet"), "/usr/bin/sync-daemon");
        assert_eq!(exec_program(""), "");
    }

    #[test]
    fn running_program_owns_its_cache() {
        let running: HashSet<String> = ["firefox", "chrome", "code"].iter().map(|s| s.to_string()).collect();
        assert!(cache_in_use("mozilla", &running));
        assert!(cache_in_use("google-chrome", &running));
        assert!(cache_in_use("Code", &running));
        assert!(!cache_in_use("thumbnails", &running));
        assert!(!cache_in_use("fontconfig", &running));
    }

    #[test]
    fn linux_protected_folders() {
        let h = home();
        for p in [PathBuf::from("/usr/share/applications/x.desktop"), PathBuf::from("/etc"), PathBuf::from("/home"), h.join(".config"), h.join(".cache"), h.join(".ssh/id_ed25519"), h.join(".local/share/keyrings/login.keyring"), h.join(".local/share/Trash")] {
            assert!(is_protected(&p), "{} should be protected", p.display());
        }
        for p in [h.join(".cache/mozilla"), h.join(".config/Code"), h.join("Downloads/old.deb"), h.join("Applications/Paint.AppImage"), h.join(".config/autostart/x.desktop")] {
            assert!(!is_protected(&p), "{} should not be protected", p.display());
        }
    }

    #[test]
    fn startup_entries() {
        assert_eq!(agent_label(Path::new("/h/.config/systemd/user/sync.service")).as_deref(), Some("sync.service"));
        assert!(agent_label(Path::new("/h/.config/autostart/x.desktop")).is_none());
        assert!(is_startup_file(Path::new("/h/.config/autostart/x.desktop")));
        assert!(!is_startup_file(Path::new("/h/.config/autostart/readme.txt")));
    }
}
