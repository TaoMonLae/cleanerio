use crate::art::{self, Glyph, Outline};
use crate::scan::{self, human, short, ActionReport, AgentItem, AppInfo, AppsSummary, BigFile, CloudFolder, DupGroup, Job, JunkGroup, Node, Task};
use crate::ui_kit::*;
use eframe::egui::{self, Align, Align2, Color32, FontId, Layout, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::{Disks, System};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Smart,
    Cleanup,
    Protection,
    Performance,
    Apps,
    Clutter,
    Space,
    Cloud,
    Tools,
    Activity,
}

pub struct Intro {
    pub title: &'static str,
    pub text: &'static str,
    pub bullets: &'static [(Glyph, &'static str)],
}

impl Page {
    pub const MAIN: [Page; 8] = [Page::Smart, Page::Cleanup, Page::Protection, Page::Performance, Page::Apps, Page::Clutter, Page::Space, Page::Cloud];
    pub const BOTTOM: [Page; 2] = [Page::Tools, Page::Activity];

    pub fn name(self) -> &'static str {
        match self {
            Page::Smart => "Smart Scan",
            Page::Cleanup => "Cleanup",
            Page::Protection => "Protection",
            Page::Performance => "Performance",
            Page::Apps => "Applications",
            Page::Clutter => "My Files",
            Page::Space => "Space Map",
            Page::Cloud => "Cloud Files",
            Page::Tools => "Quick Tools",
            Page::Activity => "Activity",
        }
    }
    pub fn glyph(self) -> Glyph {
        match self {
            Page::Smart => Glyph::Sparkles,
            Page::Cleanup => Glyph::Bubble,
            Page::Protection => Glyph::Shield,
            Page::Performance => Glyph::Bolt,
            Page::Apps => Glyph::Grid,
            Page::Clutter => Glyph::Folder,
            Page::Space => Glyph::Lens,
            Page::Cloud => Glyph::Cloud,
            Page::Tools => Glyph::Sliders,
            Page::Activity => Glyph::Chart,
        }
    }
    pub fn outline(self) -> Outline {
        match self {
            Page::Smart => Outline::Squircle,
            Page::Cleanup => Outline::Circle,
            Page::Protection => Outline::Octagon,
            Page::Performance => Outline::Chevron,
            Page::Apps => Outline::Hexagon,
            Page::Clutter => Outline::Blob,
            Page::Space => Outline::Squircle,
            Page::Cloud => Outline::Cloud,
            Page::Tools | Page::Activity => Outline::Squircle,
        }
    }
    /// Main colour for buttons, bars and pictures.
    pub fn accent(self) -> Color32 {
        match self {
            Page::Smart => Color32::from_rgb(122, 86, 240),
            Page::Cleanup => Color32::from_rgb(34, 160, 107),
            Page::Protection => Color32::from_rgb(217, 72, 138),
            Page::Performance => Color32::from_rgb(217, 122, 30),
            Page::Apps => Color32::from_rgb(63, 120, 224),
            Page::Clutter => Color32::from_rgb(31, 152, 146),
            Page::Space => Color32::from_rgb(110, 80, 224),
            Page::Cloud => Color32::from_rgb(60, 138, 219),
            Page::Tools => Color32::from_rgb(108, 103, 145),
            Page::Activity => Color32::from_rgb(76, 104, 181),
        }
    }
    /// The soft tint behind a screen's 3D object.
    pub fn tint(self) -> Color32 {
        mix(Color32::WHITE, self.accent(), 0.13)
    }
    /// Window background corners of the old look (unused).
    #[allow(dead_code)]
    pub fn bg(self) -> [Color32; 4] {
        let c = Color32::from_rgb;
        match self {
            Page::Smart => [c(125, 35, 160), c(30, 12, 80), c(95, 30, 150), c(60, 20, 140)],
            Page::Cleanup => [c(85, 125, 45), c(18, 55, 25), c(55, 115, 55), c(22, 70, 35)],
            Page::Protection => [c(145, 30, 135), c(135, 45, 120), c(105, 22, 70), c(85, 15, 45)],
            Page::Performance => [c(190, 100, 45), c(110, 35, 20), c(170, 75, 35), c(140, 48, 25)],
            Page::Apps => [c(22, 42, 105), c(75, 35, 160), c(40, 85, 175), c(40, 60, 165)],
            Page::Clutter => [c(40, 100, 112), c(62, 135, 128), c(36, 98, 92), c(28, 78, 76)],
            Page::Space => [c(75, 18, 175), c(24, 10, 66), c(62, 16, 150), c(52, 16, 135)],
            Page::Cloud => [c(30, 85, 165), c(18, 42, 100), c(50, 115, 185), c(28, 65, 135)],
            Page::Tools => [c(70, 55, 120), c(30, 26, 60), c(80, 55, 130), c(40, 32, 85)],
            Page::Activity => [c(30, 45, 95), c(15, 20, 48), c(42, 65, 120), c(25, 35, 80)],
        }
    }
    pub fn intro(self) -> Intro {
        match self {
            Page::Smart => Intro {
                title: "Smart Scan",
                text: "A quick checkup that tidies, protects and speeds up your Mac in one go.",
                bullets: &[(Glyph::Bubble, "Junk Cleanup"), (Glyph::Shield, "Threat Check"), (Glyph::Bolt, "Speed Tune-up")],
            },
            Page::Cleanup => Intro {
                title: "Cleanup",
                text: "Clear out caches, logs and leftovers to win back free space.",
                bullets: &[(Glyph::Trash, "System Junk"), (Glyph::Mail, "Mail Downloads"), (Glyph::Hammer, "Developer Junk"), (Glyph::Trash, "Trash")],
            },
            Page::Protection => Intro {
                title: "Protection",
                text: "Check your Mac against a database of Mac threats: trojans, adware, spyware and crypto miners.",
                bullets: &[(Glyph::Bug, "Malware Removal"), (Glyph::Shield, "Suspicious Helpers"), (Glyph::Bolt, "Running Threats")],
            },
            Page::Performance => Intro {
                title: "Performance",
                text: "Keep your Mac quick with regular maintenance and a tidy startup.",
                bullets: &[(Glyph::Wrench, "Maintenance Tasks"), (Glyph::Power, "Login Items"), (Glyph::Rocket, "Background Items")],
            },
            Page::Apps => Intro {
                title: "Applications",
                text: "See every app and how much room it takes, then remove it cleanly.",
                bullets: &[(Glyph::Grid, "Uninstaller"), (Glyph::Leftovers, "Leftover Files"), (Glyph::Sliders, "Reset Apps")],
            },
            Page::Clutter => Intro {
                title: "My Files",
                text: "Sort through big, old and duplicate files in a few clicks.",
                bullets: &[(Glyph::Folder, "Large & Old Files"), (Glyph::Copy, "Duplicates"), (Glyph::Download, "Downloads")],
            },
            Page::Space => Intro {
                title: "Space Map",
                text: "See what's taking up the most room and tidy your storage fast.",
                bullets: &[(Glyph::Disk, "Visual Storage Map"), (Glyph::Lens, "Hidden Space Found"), (Glyph::Folder, "Biggest Folders")],
            },
            Page::Cloud => Intro {
                title: "Cloud Files",
                text: "See how much room your cloud folders take on this Mac and free it up.",
                bullets: &[(Glyph::Cloud, "iCloud Drive"), (Glyph::Folder, "Other Cloud Folders"), (Glyph::Download, "Free Local Copies")],
            },
            Page::Tools | Page::Activity => Intro { title: "", text: "", bullets: &[] },
        }
    }
}

pub struct SmartResult {
    pub junk: Vec<JunkGroup>,
    pub agents: Vec<AgentItem>,
    pub apps: AppsSummary,
    pub big: Vec<BigFile>,
}

pub struct ClutterResult {
    pub big: Vec<BigFile>,
    pub dups: Vec<DupGroup>,
    pub downloads: Vec<BigFile>,
}

pub enum Pending {
    Trash { title: String, paths: Vec<(PathBuf, u64)> },
    EmptyTrash { size: u64 },
    Evict { paths: Vec<(PathBuf, u64)> },
    Quit { pid: sysinfo::Pid, name: String },
}

pub struct CleanerApp {
    pub page: Page,
    // screenshot mode
    shot: Option<PathBuf>,
    pub frames: u32,
    shot_requested: bool,
    shot_done: Arc<AtomicBool>,
    // system
    pub sys: System,
    last_refresh: Option<Instant>,
    pub last_procs: Option<Instant>,
    pub cpu: f32,
    pub mem_used: u64,
    pub mem_total: u64,
    pub disk_total: u64,
    pub disk_free: u64,
    pub procs: Vec<(sysinfo::Pid, String, u64, f32)>,
    procs_job: Option<Job<Vec<(sysinfo::Pid, String, u64, f32)>>>,
    /// Running threats to stop once the person confirms the removal.
    pub pending_stop: Vec<u32>,
    net: Option<sysinfo::Networks>,
    net_at: Instant,
    /// Upload and download, bytes per second.
    pub net_rate: (f64, f64),
    pub wifi: Option<String>,
    /// Battery percent and state ("Charging", "On battery", "Charged").
    pub battery: Option<(u8, String)>,
    last_slow: Option<Instant>,
    /// External drives: name, free bytes, total bytes.
    pub externals: Vec<(String, u64, u64)>,
    /// Battery condition from System Information ("Normal", "Service Recommended"...), read once.
    pub battery_condition: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    /// Number of apps not opened for six months, with when it was counted.
    pub unused_apps: Option<(usize, Instant)>,
    // smart scan
    pub smart_job: Option<Job<SmartResult>>,
    pub smart: Option<AppsSummary>,
    pub smart_done: bool,
    // cleanup / protection
    pub junk: Option<Vec<JunkGroup>>,
    pub junk_job: Option<Job<Vec<JunkGroup>>>,
    pub agents: Option<Vec<AgentItem>>,
    pub agents_job: Option<Job<Vec<AgentItem>>>,
    pub show_ok_agents: bool,
    /// Threat database: families, signs, date.
    pub db_info: (usize, usize, String),
    // performance
    pub tasks: Vec<Task>,
    pub task_job: Option<Job<Vec<(usize, String)>>>,
    pub perf_open: bool,
    pub perf_tab: usize,
    pub login: Option<Result<Vec<String>, String>>,
    pub login_job: Option<Job<Result<Vec<String>, String>>>,
    // apps
    pub apps: Option<Vec<AppInfo>>,
    pub apps_job: Option<Job<Vec<AppInfo>>>,
    pub app_filter: String,
    pub app_sel: Option<usize>,
    pub app_sort_size: bool,
    pub apps_unused_only: bool,
    // my files
    pub clutter_root: PathBuf,
    pub clutter_job: Option<Job<ClutterResult>>,
    pub clutter_tab: usize,
    pub clutter_scanned: bool,
    pub trash_size: Option<(u64, Instant)>,
    pub big: Option<Vec<BigFile>>,
    pub dups: Option<Vec<DupGroup>>,
    pub downloads: Option<Vec<BigFile>>,
    pub big_min_mb: u64,
    pub big_old_only: bool,
    pub big_size_group: usize,
    pub big_age_group: usize,
    pub big_kind: String,
    // space map
    pub space: Option<Node>,
    pub space_job: Option<Job<Node>>,
    pub space_root: PathBuf,
    pub space_stack: Vec<usize>,
    pub space_sel: Option<usize>,
    // cloud
    pub cloud: Option<Vec<CloudFolder>>,
    pub cloud_job: Option<Job<Vec<CloudFolder>>>,
    pub cloud_sel: usize,
    // activity
    pub history: Vec<(u64, u64, String)>,
    // actions
    pub pending: Option<Pending>,
    pub action_job: Option<Job<ActionReport>>,
    pub action_label: String,
    toast: Option<(String, Instant)>,
}

impl CleanerApp {
    pub fn new(cc: &eframe::CreationContext<'_>, shot: Option<PathBuf>) -> Self {
        let ctx = &cc.egui_ctx;
        ctx.set_visuals(egui::Visuals::light());
        ctx.all_styles_mut(|s| {
            s.spacing.item_spacing = Vec2::new(8.0, 8.0);
            s.spacing.button_padding = Vec2::new(12.0, 5.0);
            let v = &mut s.visuals;
            v.override_text_color = Some(TEXT);
            v.selection.bg_fill = TINT;
            v.selection.stroke = Stroke::new(1.0, ACCENT_DARK);
            v.hyperlink_color = ACCENT_DARK;
            for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
                w.weak_bg_fill = Color32::WHITE;
                w.bg_fill = Color32::WHITE;
                w.bg_stroke = Stroke::new(1.0, EDGE);
                w.fg_stroke = Stroke::new(1.6, TEXT);
                w.corner_radius = egui::CornerRadius::same(8);
            }
            v.widgets.inactive.bg_stroke = Stroke::new(1.2, Color32::from_rgb(168, 174, 190));
            v.widgets.hovered.weak_bg_fill = GROUND;
            v.widgets.hovered.bg_fill = GROUND;
            v.widgets.hovered.bg_stroke = Stroke::new(1.0, MUTED);
            v.widgets.active.weak_bg_fill = TINT;
            v.widgets.active.bg_fill = TINT;
            v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
            v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
            v.extreme_bg_color = Color32::WHITE;
            v.window_fill = Color32::WHITE;
            v.window_stroke = Stroke::new(1.0, LINE);
            v.panel_fill = Color32::TRANSPARENT;
        });
        let start_page = match std::env::var("CLEANERIO_PAGE").unwrap_or_default().as_str() {
            "cleanup" => Page::Cleanup,
            "protection" => Page::Protection,
            "performance" => Page::Performance,
            "apps" => Page::Apps,
            "files" | "big" => Page::Clutter,
            "space" => Page::Space,
            "cloud" => Page::Cloud,
            "tools" => Page::Tools,
            "activity" => Page::Activity,
            _ => Page::Smart,
        };
        let mut app = Self {
            page: start_page,
            shot,
            frames: 0,
            shot_requested: false,
            shot_done: Arc::new(AtomicBool::new(false)),
            sys: System::new().expect("system information"),
            last_refresh: None,
            last_procs: None,
            cpu: 0.0,
            mem_used: 0,
            mem_total: 1,
            disk_total: 1,
            disk_free: 0,
            procs: vec![],
            net: sysinfo::Networks::new_with_refreshed_list().ok(),
            net_at: Instant::now(),
            net_rate: (0.0, 0.0),
            wifi: None,
            battery: None,
            last_slow: None,
            externals: vec![],
            battery_condition: {
                let c = std::sync::Arc::new(std::sync::Mutex::new(None));
                let c2 = c.clone();
                std::thread::spawn(move || *c2.lock().unwrap() = scan::battery_condition());
                c
            },
            unused_apps: None,
            smart_job: None,
            smart: None,
            smart_done: false,
            junk: None,
            junk_job: None,
            agents: None,
            agents_job: None,
            show_ok_agents: false,
            db_info: {
                let db = crate::threats::load();
                (db.families.len(), db.signatures(), db.updated.clone())
            },
            tasks: scan::tasks(),
            task_job: None,
            perf_open: false,
            perf_tab: 0,
            login: None,
            login_job: None,
            apps: None,
            apps_job: None,
            app_filter: String::new(),
            app_sel: None,
            app_sort_size: false,
            apps_unused_only: false,
            clutter_root: scan::home(),
            clutter_job: None,
            clutter_tab: 0,
            clutter_scanned: false,
            trash_size: None,
            big: None,
            dups: None,
            downloads: None,
            big_min_mb: 50,
            big_old_only: false,
            big_size_group: 0,
            big_age_group: 0,
            big_kind: "All".into(),
            space: None,
            space_job: None,
            space_root: scan::home(),
            space_stack: vec![],
            space_sel: None,
            cloud: None,
            cloud_job: None,
            cloud_sel: 0,
            history: scan::read_activity(),
            pending: None,
            pending_stop: vec![],
            procs_job: None,
            action_job: None,
            action_label: String::new(),
            toast: None,
        };
        app.refresh_system(true);
        app
    }

    pub fn refresh_system(&mut self, force: bool) {
        if !force && self.last_refresh.is_some_and(|t| t.elapsed() < Duration::from_secs(2)) {
            return;
        }
        self.last_refresh = Some(Instant::now());
        self.sys.refresh_memory();
        self.sys.refresh_cpu_usage();
        self.cpu = self.sys.global_cpu_usage();
        self.mem_used = self.sys.used_memory();
        self.mem_total = self.sys.total_memory().max(1);
        if let Ok(disks) = Disks::new_with_refreshed_list() {
            let d = disks
                .iter()
                .find(|d| d.mount_point() == std::path::Path::new("/System/Volumes/Data"))
                .or_else(|| disks.iter().find(|d| d.mount_point() == std::path::Path::new("/")));
            if let Some(d) = d {
                self.disk_total = d.total_space().max(1);
                self.disk_free = d.available_space();
            }
            self.externals = disks
                .iter()
                .filter(|d| d.mount_point().starts_with("/Volumes") && (d.is_removable() || d.total_space() > 0))
                .map(|d| (d.name().to_string_lossy().to_string(), d.available_space(), d.total_space()))
                .collect();
        }
        if let Some(n) = &mut self.net {
            let dt = self.net_at.elapsed().as_secs_f64().max(0.5);
            n.refresh(true);
            let (mut rx, mut tx) = (0u64, 0u64);
            for (name, d) in n.iter() {
                if name.to_string_lossy().starts_with("lo") {
                    continue;
                }
                rx += d.received();
                tx += d.transmitted();
            }
            self.net_rate = (tx as f64 / dt, rx as f64 / dt);
            self.net_at = Instant::now();
        }
        if force || self.last_slow.is_none_or(|t| t.elapsed() > Duration::from_secs(30)) {
            self.last_slow = Some(Instant::now());
            self.battery = scan::battery();
            self.wifi = scan::wifi_name();
        }
    }

    /// Keeps the list of heavy apps fresh without making the window wait for it.
    pub fn refresh_procs(&mut self) {
        if let Some(v) = self.procs_job.as_ref().and_then(|j| j.take()) {
            self.procs = v;
            self.procs_job = None;
        }
        if self.procs_job.as_ref().is_some_and(|j| j.failed()) {
            self.procs_job = None;
        }
        if self.procs_job.is_some() || self.last_procs.is_some_and(|t| t.elapsed() < Duration::from_secs(3)) {
            return;
        }
        self.last_procs = Some(Instant::now());
        self.procs_job = Some(Job::spawn(|_| scan::heavy_apps()));
    }

    fn busy(&self) -> bool {
        self.smart_job.is_some()
            || self.junk_job.is_some()
            || self.agents_job.is_some()
            || self.task_job.is_some()
            || self.login_job.is_some()
            || self.apps_job.is_some()
            || self.clutter_job.is_some()
            || self.space_job.is_some()
            || self.cloud_job.is_some()
            || self.action_job.is_some()
    }

    pub fn toast(&mut self, s: impl Into<String>) {
        self.toast = Some((s.into(), Instant::now()));
    }

    /// Collect finished background work.
    fn poll(&mut self) {
        // work that crashed: stop waiting and say so
        macro_rules! drop_failed {
            ($s:ident; $($f:ident),*) => { $(
                if $s.$f.as_ref().is_some_and(|j| j.failed()) {
                    $s.$f = None;
                    $s.toast("Something went wrong and that step stopped. Please try again.");
                }
            )* };
        }
        drop_failed!(self; smart_job, junk_job, agents_job, login_job, task_job, apps_job, clutter_job, space_job, cloud_job, action_job);
        // a scan the person stopped: its half-finished result is not shown as if complete
        macro_rules! drop_stopped {
            ($s:ident; $($f:ident),*) => { $(
                if $s.$f.as_ref().is_some_and(|j| j.cancelled() && j.finished()) {
                    $s.$f = None;
                    $s.toast("Scan stopped.");
                }
            )* };
        }
        drop_stopped!(self; smart_job, junk_job, agents_job, apps_job, clutter_job, space_job, cloud_job);
        if let Some(r) = self.smart_job.as_ref().and_then(|j| j.take()) {
            self.smart_job = None;
            self.junk = Some(r.junk);
            self.agents = Some(r.agents);
            self.smart = Some(r.apps);
            if self.big.is_none() {
                self.big = Some(r.big);
            }
            self.smart_done = true;
            scan::log_activity(scan::SMART_SCAN_LOG, 0);
            self.history = scan::read_activity();
        }
        if let Some(r) = self.junk_job.as_ref().and_then(|j| j.take()) {
            self.junk_job = None;
            self.junk = Some(r);
        }
        if let Some(r) = self.agents_job.as_ref().and_then(|j| j.take()) {
            self.agents_job = None;
            self.agents = Some(r);
            scan::log_activity(scan::THREAT_SCAN_LOG, 0);
            self.history = scan::read_activity();
        }
        if let Some(r) = self.login_job.as_ref().and_then(|j| j.take()) {
            self.login_job = None;
            self.login = Some(r);
        }
        if let Some(r) = self.task_job.as_ref().and_then(|j| j.take()) {
            self.task_job = None;
            let mut names = vec![];
            for (i, msg) in r {
                if let Some(t) = self.tasks.get_mut(i) {
                    if msg == "Done" {
                        names.push(t.title);
                    }
                    t.last = Some(msg);
                }
            }
            if !names.is_empty() {
                scan::log_activity(&format!("Ran {}", names.join(", ")), 0);
                self.history = scan::read_activity();
            }
            self.toast("Maintenance finished.");
        }
        if let Some(r) = self.apps_job.as_ref().and_then(|j| j.take()) {
            self.apps_job = None;
            self.apps = Some(r);
        }
        if let Some(r) = self.clutter_job.as_ref().and_then(|j| j.take()) {
            self.clutter_job = None;
            self.big = Some(r.big);
            self.dups = Some(r.dups);
            self.downloads = Some(r.downloads);
            self.clutter_scanned = true;
        }
        if let Some(r) = self.space_job.as_ref().and_then(|j| j.take()) {
            self.space_job = None;
            self.space = Some(r);
            self.space_stack.clear();
            self.space_sel = None;
        }
        if let Some(r) = self.cloud_job.as_ref().and_then(|j| j.take()) {
            self.cloud_job = None;
            self.cloud = Some(r);
        }
        if let Some(r) = self.action_job.as_ref().and_then(|j| j.take()) {
            self.action_job = None;
            self.apply_report(r);
        }
    }

    fn apply_report(&mut self, r: ActionReport) {
        let gone: HashSet<PathBuf> = r.removed.iter().cloned().collect();
        if let Some(j) = &mut self.junk {
            for g in j.iter_mut() {
                g.items.retain(|i| !gone.contains(&i.path));
            }
        }
        if let Some(a) = &mut self.agents {
            a.retain(|i| !gone.contains(&i.path));
        }
        if let Some(a) = &mut self.apps {
            a.retain(|i| !gone.contains(&i.path));
            for app in a.iter_mut() {
                app.leftovers.retain(|l| !gone.contains(&l.0));
            }
            self.app_sel = None;
        }
        for list in [&mut self.big, &mut self.downloads].into_iter().flatten() {
            list.retain(|i| !gone.contains(&i.path));
        }
        if let Some(d) = &mut self.dups {
            for g in d.iter_mut() {
                g.files.retain(|f| !gone.contains(&f.path));
            }
            d.retain(|g| g.files.len() > 1);
        }
        if let Some(c) = &mut self.cloud {
            for f in c.iter_mut() {
                f.files.retain(|x| !gone.contains(&x.path));
            }
        }
        if !r.removed.is_empty() {
            if let Some(root) = &mut self.space {
                // remember which folder is open by its path: positions change below
                let mut trail = vec![];
                let mut node: &Node = root;
                for &i in &self.space_stack {
                    match node.children.get(i) {
                        Some(c) => {
                            trail.push(c.path.clone());
                            node = c;
                        }
                        None => break,
                    }
                }
                prune(root, &gone);
                self.space_stack.clear();
                let mut node: &Node = root;
                for p in trail {
                    match node.children.iter().position(|c| c.is_dir && c.path == p) {
                        Some(i) => {
                            self.space_stack.push(i);
                            node = &node.children[i];
                        }
                        None => break,
                    }
                }
            }
            self.space_sel = None;
        }
        self.refresh_system(true);
        let mut msg = if !r.message.is_empty() {
            r.message.clone()
        } else {
            format!("Moved {} item(s) ({}) to the Trash · empty the Trash to free the space", r.removed.len(), human(r.freed))
        };
        if !r.failed.is_empty() {
            msg += &format!(" · {} couldn't be moved (they may need an administrator or be in use)", r.failed.len());
        }
        if !r.removed.is_empty() {
            let what = if self.action_label.is_empty() { "Cleaned up".to_string() } else { self.action_label.clone() };
            scan::log_activity(&what, r.freed);
            self.history = scan::read_activity();
        }
        self.toast(msg);
    }

    // ------------------------------------------------------------------ sidebar

    /// The sidebar always shows its labels.
    #[allow(dead_code)]
    pub fn sidebar_expanded(&self) -> bool {
        true
    }

    fn nav_item(&self, ui: &mut Ui, rect: Rect, page: Page, y: f32, row_h: f32) -> bool {
        let selected = self.page == page;
        let r = Rect::from_min_size(Pos2::new(rect.left() + 10.0, y), Vec2::new(rect.width() - 20.0, row_h - 2.0));
        let resp = ui.interact(r, ui.scope_id().with(("nav", page as usize)), Sense::click());
        let p = ui.painter();
        if selected {
            p.rect_filled(r, 10.0, TINT);
        } else if resp.hovered() {
            p.rect_filled(r, 10.0, ink(10));
        }
        let col = if selected { ACCENT_DARK } else { BODY };
        let ir = Rect::from_center_size(Pos2::new(r.left() + 22.0, r.center().y), Vec2::splat(20.0));
        art::glyph(p, page.glyph(), ir, col);
        p.text(Pos2::new(r.left() + 44.0, r.center().y), Align2::LEFT_CENTER, page.name(), FontId::proportional(14.5), if selected { ACCENT_DARK } else { TEXT });
        let dot = match page {
            Page::Cleanup => self.junk.as_ref().is_some_and(|j| j.iter().any(|g| !g.items.is_empty())),
            Page::Protection => self.agents.as_ref().is_some_and(|a| a.iter().any(|i| i.verdict == scan::Verdict::Threat)),
            _ => false,
        };
        if dot {
            p.circle_filled(Pos2::new(r.right() - 14.0, r.center().y), 4.0, if page == Page::Protection { BAD } else { ACCENT });
        }
        resp.clicked()
    }

    fn sidebar(&mut self, ui: &mut Ui) {
        let rect = ui.max_rect();
        let p = ui.painter().clone();
        p.rect_filled(rect, 0.0, Color32::WHITE);
        p.line_segment([Pos2::new(rect.right() - 0.5, rect.top()), Pos2::new(rect.right() - 0.5, rect.bottom())], Stroke::new(1.0, LINE));
        // the app's mark and name, below the window buttons
        let mark = Rect::from_min_size(Pos2::new(rect.left() + 20.0, rect.top() + 46.0), Vec2::splat(28.0));
        art::brand_mark(&p, mark);
        p.text(Pos2::new(mark.right() + 10.0, mark.center().y + 1.0), Align2::LEFT_CENTER, "Cleanerio", FontId::proportional(19.0), TEXT);
        let row_h = 42.0;
        let mut clicked = None;
        let mut y = rect.top() + 94.0;
        for page in Page::MAIN.into_iter().chain(Page::BOTTOM) {
            if self.nav_item(ui, rect, page, y, row_h) {
                clicked = Some(page);
            }
            y += row_h;
        }
        // the startup disk, always in view
        if rect.bottom() - 84.0 > y {
            let used = self.disk_total.saturating_sub(self.disk_free);
            let frac = (used as f32 / self.disk_total as f32).clamp(0.0, 1.0);
            let x = rect.left() + 20.0;
            let w = rect.width() - 40.0;
            let top = rect.bottom() - 76.0;
            p.line_segment([Pos2::new(rect.left() + 12.0, top - 12.0), Pos2::new(rect.right() - 12.0, top - 12.0)], Stroke::new(1.0, LINE));
            p.text(Pos2::new(x, top), Align2::LEFT_TOP, "Startup disk", FontId::proportional(13.0), TEXT);
            p.text(Pos2::new(x + w, top), Align2::RIGHT_TOP, format!("{:.0}% used", frac * 100.0), FontId::proportional(13.0), MUTED);
            let track = Rect::from_min_size(Pos2::new(x, top + 24.0), Vec2::new(w, 6.0));
            p.rect_filled(track, 3.0, Color32::from_rgb(230, 232, 239));
            let mut fill = track;
            fill.set_width(track.width() * frac);
            p.rect_filled(fill, 3.0, ACCENT);
            p.text(Pos2::new(x, top + 38.0), Align2::LEFT_TOP, format!("{} free of {}", human(self.disk_free), human(self.disk_total)), FontId::proportional(12.5), MUTED);
        }
        if let Some(p) = clicked {
            if p == Page::Activity {
                self.history = scan::read_activity();
            }
            self.page = p;
        }
    }

    // ------------------------------------------------------------------ shared screen parts

    pub fn header(&self, ui: &mut Ui, sub: &str) {
        ui.label(RichText::new(self.page.name()).size(28.0).color(TEXT));
        ui.label(RichText::new(sub).size(14.0).color(MUTED));
        ui.add_space(12.0);
    }

    /// The section's welcome screen: a tinted band with the title, a line of text and the
    /// button on the left and the section's 3D object on the right, then what the section
    /// covers in a plain row below. Returns true when the button was pressed.
    pub fn intro_screen(&mut self, ui: &mut Ui, button: &str, extra: Option<&mut dyn FnMut(&mut Ui, &mut Self)>) -> bool {
        let rect = ui.max_rect();
        let page = self.page;
        let info = page.intro();
        let p = ui.painter().clone();
        let band = Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.top() + (rect.height() - 84.0).max(260.0).min(rect.height())));
        p.rect_filled(band, 20.0, page.tint());
        let art_r = (band.height() * 0.27).min(band.width() * 0.15);
        let art_c = Pos2::new(band.right() - band.width() * 0.23, band.center().y - art_r * 0.08);
        art::hero(&p.with_clip_rect(band), page.outline(), page.glyph(), art_c, art_r, page.accent());

        let x = band.left() + 40.0;
        let col_w = (band.width() * 0.50).min(480.0);
        let title = p.layout(info.title.to_string(), FontId::proportional(44.0), TEXT, col_w);
        let sub = p.layout(info.text.to_string(), FontId::proportional(16.5), BODY, col_w.min(430.0));
        let total = title.size().y + 12.0 + sub.size().y + 26.0 + 48.0;
        let mut y = band.center().y - total / 2.0;
        let th = title.size().y;
        p.galley(Pos2::new(x, y), title, TEXT);
        y += th + 12.0;
        let sh = sub.size().y;
        p.galley(Pos2::new(x, y), sub, BODY);
        y += sh + 26.0;
        let brect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(primary_width(button), 48.0));
        let go = primary_at(ui, brect, button);
        if let Some(extra) = extra {
            let r = Rect::from_min_size(Pos2::new(brect.right() + 14.0, y + 2.0), Vec2::new(250.0, 44.0));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r).layout(Layout::left_to_right(Align::Center)));
            extra(&mut child, self);
        }

        // what this section covers
        let top = band.bottom() + 16.0;
        let panel = Rect::from_min_max(Pos2::new(rect.left(), top), Pos2::new(rect.right(), (top + 68.0).min(rect.bottom())));
        if !info.bullets.is_empty() && panel.height() > 40.0 {
            p.rect_filled(panel, 14.0, Color32::WHITE);
            p.rect_stroke(panel, 14.0, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
            let cw = panel.width() / info.bullets.len() as f32;
            for (i, (g, label)) in info.bullets.iter().enumerate() {
                let cx = panel.left() + cw * i as f32 + 22.0;
                art::glyph(&p, *g, Rect::from_center_size(Pos2::new(cx + 10.0, panel.center().y), Vec2::splat(20.0)), ACCENT);
                p.text(Pos2::new(cx + 32.0, panel.center().y), Align2::LEFT_CENTER, *label, FontId::proportional(15.0), TEXT);
            }
        }
        go
    }

    pub fn working(&self, ui: &mut Ui, title: &str, status: &str) -> bool {
        let rect = ui.max_rect();
        let page = self.page;
        let t = ui.input(|i| i.time) as f32;
        let p = ui.painter().clone();
        let c = Pos2::new(rect.center().x, rect.top() + rect.height() * 0.42);
        let r = (rect.width() * 0.14).min(rect.height() * 0.2);
        art::hero(&p, page.outline(), page.glyph(), c + Vec2::new(0.0, (t * 2.0).sin() * 6.0), r, page.accent());
        for i in 0..3 {
            let a = t * 2.2 + i as f32 * 2.1;
            p.circle_filled(c + Vec2::angled(a) * r * 1.45, 5.0, alpha(page.accent(), 170));
        }
        p.text(Pos2::new(rect.center().x, rect.top() + 40.0), Align2::CENTER_TOP, title, FontId::proportional(36.0), TEXT);
        p.text(Pos2::new(rect.center().x, c.y + r * 1.9), Align2::CENTER_CENTER, elide(status, 90), FontId::proportional(15.0), MUTED);
        ui.ctx().request_repaint();
        round_button(ui, "Stop", page.accent())
    }

    pub fn quick_stats(&mut self, ui: &mut Ui) {
        let used = self.disk_total.saturating_sub(self.disk_free);
        let disk = used as f32 / self.disk_total as f32;
        let mem = self.mem_used as f32 / self.mem_total as f32;
        ui.horizontal(|ui| {
            let total = 3.0 * 120.0 + 2.0 * 40.0;
            ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
            ring(ui, disk, "Storage used", &format!("{:.0}%", disk * 100.0), Color32::from_rgb(180, 140, 255));
            ui.add_space(40.0);
            ring(ui, mem, "Memory", &format!("{:.0}%", mem * 100.0), Color32::from_rgb(255, 170, 80));
            ui.add_space(40.0);
            ring(ui, self.cpu / 100.0, "Processor", &format!("{:.0}%", self.cpu), Color32::from_rgb(110, 220, 160));
        });
    }

    fn confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(pending) = &self.pending else { return };
        let mut answer: Option<bool> = None;
        let accent = self.page.accent();
        egui::Modal::new(egui::Id::unique("confirm")).show(ctx, |ui| {
            ui.set_width(440.0);
            let list = |ui: &mut Ui, paths: &Vec<(PathBuf, u64)>| {
                egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                    for (p, _) in paths.iter().take(50) {
                        ui.label(RichText::new(short(p)).size(11.5).color(MUTED));
                    }
                    if paths.len() > 50 {
                        ui.label(RichText::new(format!("…and {} more", paths.len() - 50)).size(11.5).color(MUTED));
                    }
                });
            };
            match pending {
                Pending::Trash { title, paths } => {
                    ui.label(RichText::new(title).size(18.0).strong());
                    let total: u64 = paths.iter().map(|p| p.1).sum();
                    ui.label(
                        RichText::new(format!(
                            "{} item(s){} will be moved to the Trash. You can put them back from the Trash if you change your mind.",
                            paths.len(),
                            if total > 0 { format!(" ({})", human(total)) } else { String::new() }
                        ))
                        .color(MUTED),
                    );
                    list(ui, paths);
                }
                Pending::EmptyTrash { size } => {
                    ui.label(RichText::new("Empty the Trash?").size(18.0).strong());
                    ui.label(RichText::new(format!("This permanently erases {} and can't be undone.", human(*size))).color(MUTED));
                }
                Pending::Evict { paths } => {
                    ui.label(RichText::new("Free up local copies?").size(18.0).strong());
                    ui.label(RichText::new("The files stay in iCloud and download again when you open them.").color(MUTED));
                    list(ui, paths);
                }
                Pending::Quit { name, .. } => {
                    ui.label(RichText::new(format!("Quit {name}?")).size(18.0).strong());
                    ui.label(RichText::new("The app is asked to close. Save your work in it first.").color(MUTED));
                }
            }
            ui.add_space(10.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if pill(ui, "Continue", accent).clicked() {
                    answer = Some(true);
                }
                if ghost(ui, "Cancel").clicked() {
                    answer = Some(false);
                }
            });
        });
        if let Some(a) = answer {
            let p = self.pending.take().unwrap();
            let stop = std::mem::take(&mut self.pending_stop);
            if a {
                match p {
                    Pending::Quit { pid, .. } => {
                        scan::ask_to_quit(pid.as_u32());
                        self.last_procs = None;
                    }
                    Pending::Trash { paths, title } => {
                        // stop running threats only now that the person has agreed
                        for pid in stop {
                            scan::ask_to_quit(pid);
                        }
                        self.action_label = title.trim_end_matches('?').to_string();
                        self.action_job = Some(Job::spawn(move |p| scan::trash_paths(paths, p)));
                    }
                    Pending::EmptyTrash { size } => {
                        self.action_label = "Emptied the Trash".into();
                        self.action_job = Some(Job::spawn(move |p| scan::empty_trash(p, size)));
                    }
                    Pending::Evict { paths } => {
                        self.action_label = "Freed local copies of cloud files".into();
                        self.action_job = Some(Job::spawn(move |p| scan::evict(paths, p)));
                    }
                }
            }
        }
    }

    /// For trying result screens on a sample folder: CLEANERIO_AUTOSCAN=1 starts the
    /// current page's scan on the first frame (only pages that look inside the home folder).
    fn autoscan_for_tests(&mut self) {
        if self.frames != 1 || std::env::var("CLEANERIO_AUTOSCAN").is_err() {
            return;
        }
        let home = scan::home();
        match self.page {
            Page::Smart => self.smart_job = Some(crate::pages::start_smart()),
            Page::Apps => self.apps_job = Some(Job::spawn(scan::scan_apps)),
            Page::Cleanup => self.junk_job = Some(Job::spawn(scan::scan_junk)),
            Page::Clutter => self.start_clutter(),
            Page::Space => self.space_job = Some(Job::spawn(move |p| scan::scan_space(home, p))),
            Page::Cloud => self.cloud_job = Some(Job::spawn(scan::scan_cloud)),
            Page::Protection => self.agents_job = Some(Job::spawn(scan::scan_agents)),
            Page::Performance => self.perf_open = true,
            _ => {}
        }
    }

    pub fn start_clutter(&mut self) {
        let root = self.clutter_root.clone();
        let min = crate::pages::LARGE_FILE_BYTES;
        self.clutter_job = Some(Job::spawn(move |p| {
            p.stage(0);
            let big = scan::scan_big(root.clone(), min, p);
            p.stage(1);
            let dups = scan::scan_duplicates(root, p);
            p.stage(2);
            let downloads = scan::scan_downloads(p);
            ClutterResult { big, dups, downloads }
        }));
    }

    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some(path) = self.shot.clone() else { return };
        ctx.request_repaint();
        ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaintWhileHidden);
        if self.shot_done.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        // wait for test scans to finish before taking the picture
        let waiting = std::env::var("CLEANERIO_AUTOSCAN").is_ok() && self.busy() && self.frames < 1500;
        if self.frames >= 120 && !self.shot_requested && !waiting {
            self.shot_requested = true;
            let done = self.shot_done.clone();
            ctx.request_screenshot(move |img| {
                let [w, h] = img.size;
                let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match image::RgbaImage::from_raw(w as u32, h as u32, bytes) {
                    Some(im) => {
                        if let Err(e) = im.save(&path) {
                            eprintln!("could not save screenshot: {e}");
                        }
                    }
                    None => eprintln!("screenshot had an unexpected size"),
                }
                done.store(true, Ordering::SeqCst);
            });
        }
        if self.frames > 2000 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl eframe::App for CleanerApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.frames += 1;
        self.poll();
        self.refresh_system(false);
        if self.busy() {
            ctx.request_repaint_after(Duration::from_millis(100));
        } else {
            ctx.request_repaint_after(Duration::from_secs(2));
        }

        // one calm ground for every screen
        ui.painter().rect_filled(ui.max_rect(), 0.0, GROUND);

        let side_w = 220.0;
        egui::Panel::left("sidebar")
            .exact_size(side_w)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::new())
            .show(ui, |ui| self.sidebar(ui));

        egui::CentralPanel::no_frame().show(ui, |ui| {
            let rect = ui.max_rect();
            let inner = Rect::from_min_max(Pos2::new(rect.left() + 28.0, rect.top() + 40.0), Pos2::new(rect.right() - 28.0, rect.bottom() - 18.0));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(Layout::top_down(Align::Min)));
            let ui = &mut child;
            match self.page {
                Page::Smart => self.page_smart(ui),
                Page::Cleanup => self.page_cleanup(ui),
                Page::Protection => self.page_protection(ui),
                Page::Performance => self.page_performance(ui),
                Page::Apps => self.page_apps(ui),
                Page::Clutter => self.page_clutter(ui),
                Page::Space => self.page_space(ui),
                Page::Cloud => self.page_cloud(ui),
                Page::Tools => self.page_tools(ui),
                Page::Activity => self.page_activity(ui),
            }
            if let Some(j) = &self.action_job {
                let r = Rect::from_center_size(Pos2::new(rect.center().x, rect.bottom() - 30.0), Vec2::new(rect.width() * 0.6, 32.0));
                ui.painter().rect_filled(r, 16.0, alpha(TEXT, 230));
                ui.painter().text(r.center(), Align2::CENTER_CENTER, elide(&j.status(), 80), FontId::proportional(12.5), Color32::WHITE);
            }
            if let Some((msg, t)) = &self.toast {
                if t.elapsed() < Duration::from_secs(6) {
                    let galley = ui.painter().layout_no_wrap(msg.clone(), FontId::proportional(13.0), Color32::WHITE);
                    let w = galley.size().x + 32.0;
                    let r = Rect::from_center_size(Pos2::new(rect.center().x, rect.bottom() - 30.0), Vec2::new(w, 32.0));
                    ui.painter().rect_filled(r, 16.0, TEXT);
                    ui.painter().galley(Pos2::new(r.left() + 16.0, r.center().y - galley.size().y / 2.0), galley, Color32::WHITE);
                    ctx.request_repaint_after(Duration::from_millis(500));
                } else {
                    self.toast = None;
                }
            }
        });

        self.confirm_dialog(&ctx);
        self.autoscan_for_tests();
        self.handle_screenshot(&ctx);
    }
}

/// Removes trashed paths from the space map, then fixes sizes, counts and order all the way up.
/// Returns true when something under this folder changed.
fn prune(n: &mut Node, gone: &HashSet<PathBuf>) -> bool {
    let before = n.children.len();
    n.children.retain(|c| !gone.contains(&c.path) || c.path == n.path);
    let mut changed = n.children.len() != before;
    for c in n.children.iter_mut() {
        if c.is_dir && prune(c, gone) {
            changed = true;
        }
    }
    if changed {
        n.size = n.children.iter().map(|c| c.size).sum();
        n.items = n.children.iter().map(|c| c.items).sum::<u64>() + 1;
        n.children.sort_by(|a, b| b.size.cmp(&a.size));
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(path: &str, size: u64) -> Node {
        Node { name: path.into(), path: path.into(), size, is_dir: false, children: vec![], items: 1 }
    }
    fn dir(path: &str, children: Vec<Node>) -> Node {
        let size = children.iter().map(|c| c.size).sum();
        let items = children.iter().map(|c| c.items).sum::<u64>() + 1;
        Node { name: path.into(), path: path.into(), size, is_dir: true, children, items }
    }

    #[test]
    fn prune_fixes_sizes_and_order() {
        let mut root = dir("/r", vec![dir("/r/a", vec![leaf("/r/a/big", 90), leaf("/r/a/small", 5)]), dir("/r/b", vec![leaf("/r/b/only", 40)])]);
        let gone: HashSet<PathBuf> = [PathBuf::from("/r/a/big"), PathBuf::from("/r/b/only")].into_iter().collect();
        assert!(prune(&mut root, &gone));
        assert_eq!(root.size, 5);
        assert_eq!(root.children[0].path, PathBuf::from("/r/a")); // biggest first again
        assert_eq!(root.children[0].size, 5);
        assert_eq!(root.children[1].size, 0); // an emptied folder no longer keeps its old size
        assert_eq!(root.items, 4);
        assert!(!prune(&mut root, &gone));
    }
}

pub fn reveal(p: &std::path::Path) {
    let _ = std::process::Command::new("open").arg("-R").arg(p).spawn();
}

pub fn elide(s: &str, n: usize) -> String {
    let n = n.max(4);
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n - 1).collect::<String>())
    }
}
