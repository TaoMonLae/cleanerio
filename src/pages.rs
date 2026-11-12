//! The ten screens. Each section opens on a welcome screen (big picture, title, bullets and a
//! round button); the button starts its scan, and the results replace the welcome screen.

use crate::app::{elide, reveal, CleanerApp, Page, Pending, SmartResult};
use crate::art::{self, Glyph};
use crate::scan::{self, human, short, Job, JunkKind, Verdict};
use crate::ui_kit::*;
use eframe::egui::{self, Align, Align2, Color32, FontId, Layout, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use std::path::PathBuf;
use std::time::Duration;

/// Files bigger than this count as "large": the smallest size group starts above 50 MB.
pub const LARGE_FILE_BYTES: u64 = 50_000_001;
/// Size groups (name, above, up to).
pub const SIZE_GROUPS: [(&str, u64, u64); 4] = [
    ("Any", 0, u64::MAX),
    ("50–500 MB", 50_000_000, 500_000_000),
    ("500 MB–1 GB", 500_000_000, 1_000_000_000),
    ("Over 1 GB", 1_000_000_000, u64::MAX),
];
/// Last-opened filters (name, days; negative = opened within that many days).
pub const AGE_GROUPS: [(&str, i64); 6] = [
    ("Any time", 0),
    ("Last week", -7),
    ("Over a week", 7),
    ("Over a month", 30),
    ("Over 6 months", 182),
    ("Over a year", 365),
];
/// Duplicates counted as "fresh" when changed within this many days.
pub const FRESH_DUPLICATE_DAYS: u64 = 90;

fn age_matches(rule: i64, days: u64) -> bool {
    match rule {
        0 => true,
        r if r < 0 => days < (-r) as u64,
        r => days > r as u64,
    }
}
/// Files not opened for this many days count as "old".
pub const OLD_FILE_DAYS: u64 = 365;
/// Downloads older than this are suggested for removal (estimated).
pub const OLD_DOWNLOAD_DAYS: u64 = 90;
/// Apps not opened for this many days count as "unused".
pub const UNUSED_APP_DAYS: u64 = 180;
/// The Trash counts as "big" above this (2 GB).
pub const TRASH_ALERT_BYTES: u64 = 2_000_000_000;
/// Memory share above which the Memory tile turns amber (estimated; not part of the score).
pub const MEMORY_HIGH: f32 = 0.85;

/// Mac Health score: 100 minus the penalty of every problem found.
pub const MAX_HEALTH_SCORE: i64 = 100;
pub const PENALTY_OUT_OF_DISK: i64 = 20;
pub const PENALTY_NO_SMART_SCAN: i64 = 10;
pub const PENALTY_NO_THREAT_SCAN: i64 = 5;
pub const PENALTY_UNUSED_APPS: i64 = 10;
pub const PENALTY_TRASH_OVERFILL: i64 = 5;
pub const PENALTY_UPTIME: i64 = 5;
pub const PENALTY_CRITICAL_BATTERY: i64 = 80;
/// Restart is suggested after this long ("It's been over a week now").
pub const UPTIME_WARN_DAYS: u64 = 7;
/// A scan counts as "recent" within this many days (our choice).
pub const RECENT_SCAN_DAYS: u64 = 7;

/// Health level from the score: >89 Excellent, >59 Good, >39 Fair,
/// >0 Requires Attention, otherwise Critical.
pub fn health_level(score: i64) -> (&'static str, Color32) {
    if score > 89 {
        ("Excellent", GOOD)
    } else if score > 59 {
        ("Good", GOOD)
    } else if score > 39 {
        ("Fair", WARN)
    } else if score > 0 {
        ("Requires Attention", Color32::from_rgb(178, 90, 18))
    } else {
        ("Critical", BAD)
    }
}

const SMART_STEPS: [(Page, &str); 5] = [
    (Page::Cleanup, "Looking for junk…"),
    (Page::Protection, "Looking for threats…"),
    (Page::Performance, "Checking for speed-ups…"),
    (Page::Apps, "Looking at your apps…"),
    (Page::Clutter, "Looking for clutter…"),
];

pub fn start_smart() -> Job<SmartResult> {
    Job::spawn(|p| {
        p.stage(0);
        let junk = scan::scan_junk(p);
        p.stage(1);
        let agents = scan::scan_agents(p);
        p.stage(2);
        p.set("Checking memory and startup items…");
        std::thread::sleep(Duration::from_millis(500));
        p.stage(3);
        p.set("Looking at your apps…");
        let apps = scan::apps_summary();
        p.stage(4);
        let mut big = vec![];
        for r in scan::clutter_roots() {
            if p.cancelled() {
                break;
            }
            big.extend(scan::scan_big(r, LARGE_FILE_BYTES, p));
        }
        big.sort_by(|a, b| b.size.cmp(&a.size));
        SmartResult { junk, agents, apps, big }
    })
}

/// A small tinted square with a symbol in the section's colour.
pub fn glyph_tile(ui: &mut Ui, g: Glyph, color: Color32, size: f32) {
    let (r, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, (size * 0.28) as u8, mix(Color32::WHITE, color, 0.14));
    art::glyph(p, g, r.shrink(size * 0.24), mix(color, Color32::BLACK, 0.28));
}

fn junk_glyph(k: JunkKind) -> Glyph {
    match k {
        JunkKind::Caches => Glyph::Disk,
        JunkKind::Logs => Glyph::Chart,
        JunkKind::Developer => Glyph::Hammer,
        JunkKind::Installers => Glyph::Download,
        JunkKind::Mail => Glyph::Mail,
        JunkKind::Trash => Glyph::Trash,
    }
}

fn task_glyph(title: &str) -> Glyph {
    let t = title.to_lowercase();
    if t.contains("memory") {
        Glyph::Bolt
    } else if t.contains("dns") {
        Glyph::Cloud
    } else if t.contains("spotlight") {
        Glyph::Lens
    } else if t.contains("launch") {
        Glyph::Grid
    } else if t.contains("quick look") {
        Glyph::Copy
    } else if t.contains("snapshot") {
        Glyph::Disk
    } else {
        Glyph::Wrench
    }
}

fn age_text(days: u64) -> String {
    if days > 365 {
        format!("{} yr ago", days / 365)
    } else if days > 60 {
        format!("{} mo ago", days / 30)
    } else if days > 1 {
        format!("{days} days ago")
    } else {
        "Recently".to_string()
    }
}

fn plural(n: usize, one: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {one}s") }
}

fn folder_label(p: &std::path::Path) -> String {
    if p == scan::home() {
        format!("{} (home)", scan::file_name(p))
    } else if p == std::path::Path::new("/") {
        "Whole disk".to_string()
    } else {
        scan::file_name(p)
    }
}

impl CleanerApp {
    // ------------------------------------------------------------------ smart scan

    pub fn page_smart(&mut self, ui: &mut Ui) {
        let running = self.smart_job.as_ref().map(|j| (j.stage(), j.status()));
        if running.is_none() && !self.smart_done {
            if self.intro_screen(ui, "Scan", None) {
                self.smart_job = Some(start_smart());
            }
            return;
        }
        let rect = ui.max_rect();
        let top_p = ui.painter().with_clip_rect(Rect::EVERYTHING);
        top_p.text(Pos2::new(rect.center().x, rect.top() - 26.0), Align2::CENTER_CENTER, Page::Smart.name(), FontId::proportional(16.0), MUTED);

        let junk_size: u64 = self
            .junk
            .as_ref()
            .map(|j| j.iter().filter(|g| g.kind != JunkKind::Trash).map(|g| g.selected_size()).sum())
            .unwrap_or(0);
        let threats = self.agents.as_ref().map(|a| a.iter().filter(|i| i.verdict == Verdict::Threat).count()).unwrap_or(0);
        let review = self.agents.as_ref().map(|a| a.iter().filter(|i| i.verdict == Verdict::Review).count()).unwrap_or(0);
        let tasks = self.tasks.iter().filter(|t| t.selected).count();
        let (apps_total, apps_unused) = self.smart.as_ref().map(|s| (s.total, s.unused.len())).unwrap_or((0, 0));
        let big_size: u64 = self.big.as_ref().map(|b| b.iter().map(|f| f.size).sum()).unwrap_or(0);
        let big_n = self.big.as_ref().map(|b| b.len()).unwrap_or(0);

        // card layout: three across the top, two across the bottom
        let gap = 18.0;
        let area = Rect::from_min_max(Pos2::new(rect.left(), rect.top() + 6.0), Pos2::new(rect.right(), rect.bottom() - 70.0));
        let top_h = area.height() * 0.64;
        let w = area.width() - 2.0 * gap;
        let x0 = area.left();
        let x1 = x0 + w * 0.2 + gap;
        let x2 = x1 + w * 0.56 + gap;
        let r_clean = Rect::from_min_max(Pos2::new(x0, area.top()), Pos2::new(x1 - gap, area.top() + top_h));
        let r_mid = Rect::from_min_max(Pos2::new(x1, area.top()), Pos2::new(x2 - gap, area.top() + top_h));
        let r_perf = Rect::from_min_max(Pos2::new(x2, area.top()), Pos2::new(area.right(), area.top() + top_h));
        let half = (area.width() - gap) / 2.0;
        let r_apps = Rect::from_min_max(Pos2::new(area.left(), area.top() + top_h + gap), Pos2::new(area.left() + half, area.bottom()));
        let r_clut = Rect::from_min_max(Pos2::new(area.left() + half + gap, area.top() + top_h + gap), Pos2::new(area.right(), area.bottom()));

        let stage = running.as_ref().map(|r| r.0);
        let done = |i: usize| match stage {
            Some(s) => s > i,
            None => true,
        };
        let finished = running.is_none();
        let mut go = None;

        let (big, small) = if finished {
            (format!("{} of junk", human(junk_size)), "to clean".to_string())
        } else if done(0) {
            (String::new(), "Checked".into())
        } else {
            (String::new(), "Searching…".into())
        };
        if self.smart_card(ui, r_clean, Page::Cleanup, (0.5, 0.40, 0.30), &big, &small) && finished {
            go = Some(Page::Cleanup);
        }
        let (big, small) = if finished {
            (plural(tasks, "task"), format!("recommended · memory {:.0}% used", self.mem_used as f32 / self.mem_total as f32 * 100.0))
        } else if done(2) {
            (String::new(), "Checked".into())
        } else {
            (String::new(), "Waiting…".into())
        };
        if self.smart_card(ui, r_perf, Page::Performance, (0.5, 0.40, 0.30), &big, &small) && finished {
            go = Some(Page::Performance);
        }
        let (big, small) = if finished {
            (plural(apps_total, "app"), format!("{} not opened for six months", apps_unused))
        } else if done(3) {
            (String::new(), "Checked".into())
        } else {
            (String::new(), String::new())
        };
        if self.smart_card(ui, r_apps, Page::Apps, (0.80, 0.52, 0.20), &big, &small) && finished {
            go = Some(Page::Apps);
        }
        let (big, small) = if finished {
            (human(big_size), format!("in {} large files", big_n))
        } else if done(4) {
            (String::new(), "Checked".into())
        } else {
            (String::new(), String::new())
        };
        if self.smart_card(ui, r_clut, Page::Clutter, (0.80, 0.52, 0.20), &big, &small) && finished {
            go = Some(Page::Clutter);
        }

        // the middle card follows the running step, then shows the protection result
        let p = ui.painter().with_clip_rect(r_mid);
        if let Some((s, status)) = &running {
            let (page, title) = SMART_STEPS[(*s).min(4)];
            let acc = page.accent();
            art::panel(&p, r_mid, 20.0, mix(Color32::WHITE, acc, 0.16), mix(Color32::WHITE, acc, 0.12), Stroke::NONE);
            let t = ui.input(|i| i.time) as f32;
            let c = Pos2::new(r_mid.center().x, r_mid.center().y + 10.0 + (t * 2.0).sin() * 5.0);
            art::hero(&p, page.outline(), page.glyph(), c, r_mid.height() * 0.24, acc);
            p.text(Pos2::new(r_mid.center().x, r_mid.top() + 64.0), Align2::CENTER_CENTER, title, FontId::proportional(34.0), TEXT);
            p.text(Pos2::new(r_mid.center().x, r_mid.bottom() - 40.0), Align2::CENTER_CENTER, elide(status, 70), FontId::proportional(15.0), BODY);
            ui.ctx().request_repaint();
        } else {
            let page = Page::Protection;
            let resp = ui.interact(r_mid, ui.scope_id().with("mid"), Sense::click());
            let acc = page.accent();
            art::panel(&p, r_mid, 20.0, mix(Color32::WHITE, acc, if resp.hovered() { 0.20 } else { 0.14 }), mix(Color32::WHITE, acc, 0.12), Stroke::NONE);
            art::hero(&p, page.outline(), page.glyph(), Pos2::new(r_mid.center().x, r_mid.center().y + 10.0), r_mid.height() * 0.22, acc);
            let title = if threats > 0 { plural(threats, "threat") + " found" } else { "No threats found".to_string() };
            p.text(Pos2::new(r_mid.center().x, r_mid.top() + 64.0), Align2::CENTER_CENTER, title, FontId::proportional(34.0), TEXT);
            let sub = if review > 0 { format!("{} worth a look", plural(review, "background item")) } else { "Your background items look clean".to_string() };
            p.text(Pos2::new(r_mid.center().x, r_mid.bottom() - 40.0), Align2::CENTER_CENTER, sub, FontId::proportional(15.0), BODY);
            if resp.clicked() {
                go = Some(Page::Protection);
            }
        }
        if let Some(pg) = go {
            if pg == Page::Performance {
                self.perf_open = true;
            }
            self.page = pg;
            return;
        }

        if running.is_some() {
            if round_button(ui, "Stop", Page::Smart.accent()) {
                if let Some(j) = &self.smart_job {
                    j.cancel();
                }
                self.smart_job = None;
            }
            return;
        }
        let again = Rect::from_min_size(Pos2::new(rect.right() - 120.0, rect.bottom() - 40.0), Vec2::new(120.0, 30.0));
        if ui.put(again, egui::Button::new(RichText::new("Start over").size(13.0)).fill(Color32::WHITE).stroke(Stroke::new(1.0, EDGE)).corner_radius(10.0)).clicked() {
            self.smart_done = false;
        }
        if self.action_job.is_none() && round_button(ui, "Run", Page::Smart.accent()) {
            let paths: Vec<(PathBuf, u64)> = self
                .junk
                .iter()
                .flatten()
                .filter(|g| g.kind != JunkKind::Trash)
                .flat_map(|g| g.items.iter().filter(|i| i.selected).map(|i| (i.path.clone(), i.size)))
                .collect();
            if paths.is_empty() {
                self.toast("Nothing to clean right now.");
            } else {
                self.pending = Some(Pending::Trash { title: format!("Clean up {} of junk?", human(junk_size)), paths });
            }
        }
    }

    /// One glassy card on the Smart Scan screen. Returns true when it was clicked.
    fn smart_card(&self, ui: &mut Ui, rect: Rect, page: Page, art_at: (f32, f32, f32), big: &str, small: &str) -> bool {
        let resp = ui.interact(rect, ui.scope_id().with(("card", page as usize)), Sense::click());
        let p = ui.painter().with_clip_rect(rect);
        let acc = page.accent();
        let k = if resp.hovered() { 28 } else { 16 };
        let fill = if k > 16 { GROUND } else { Color32::WHITE };
        art::panel(&p, rect, 16.0, fill, fill, Stroke::new(1.0, LINE));
        let (fx, fy, fr) = art_at;
        let c = Pos2::new(rect.left() + rect.width() * fx, rect.top() + rect.height() * fy);
        let r = rect.width().min(rect.height() * 1.6) * fr;
        art::hero(&p.with_clip_rect(rect.shrink(1.5)), page.outline(), page.glyph(), c, r, acc);
        p.text(rect.left_top() + Vec2::new(22.0, 24.0), Align2::LEFT_TOP, page.name(), FontId::proportional(17.0), TEXT);
        let wrap = rect.width() - 44.0;
        let sg = p.layout(small.to_string(), FontId::proportional(14.5), MUTED, wrap);
        let sy = rect.bottom() - 22.0 - sg.size().y;
        if !big.is_empty() {
            let size = if rect.width() < 260.0 { 30.0 } else { 34.0 };
            let bg = p.layout(big.to_string(), FontId::proportional(size), TEXT, wrap);
            p.galley(Pos2::new(rect.left() + 22.0, sy - bg.size().y - 4.0), bg, TEXT);
        }
        p.galley(Pos2::new(rect.left() + 22.0, sy), sg, MUTED);
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        resp.clicked()
    }

    // ------------------------------------------------------------------ cleanup

    pub fn page_cleanup(&mut self, ui: &mut Ui) {
        if let Some(j) = &self.junk_job {
            if self.working(ui, "Looking for junk…", &j.status()) {
                j.cancel();
            }
            return;
        }
        if self.junk.is_none() {
            if self.intro_screen(ui, "Scan", None) {
                self.junk_job = Some(Job::spawn(scan::scan_junk));
            }
            return;
        }
        self.header(ui, "Caches, logs and leftovers you can safely remove");
        let accent = Page::Cleanup.accent();
        let busy = self.action_job.is_some();
        let groups = self.junk.as_mut().unwrap();
        let total: u64 = groups.iter().filter(|g| g.kind != JunkKind::Trash).map(|g| g.selected_size()).sum();
        let trash_size: u64 = groups.iter().filter(|g| g.kind == JunkKind::Trash).map(|g| g.total()).sum();
        let (mut rescan, mut empty) = (false, false);
        ui.horizontal(|ui| {
            ui.label(RichText::new(human(total)).size(34.0).strong().color(TEXT));
            ui.label(RichText::new("selected to clean").size(15.0).color(MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ghost(ui, "Scan again").clicked() {
                    rescan = true;
                }
                if trash_size > 0 && ghost(ui, &format!("Empty Trash ({})", human(trash_size))).clicked() {
                    empty = true;
                }
            });
        });
        ui.add_space(8.0);
        let h = ui.available_height() - 80.0;
        egui::ScrollArea::vertical().max_height(h).auto_shrink([false; 2]).show(ui, |ui| {
            for g in groups.iter_mut() {
                card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        if g.kind != JunkKind::Trash {
                            let mut a = g.items.iter().any(|i| !i.in_use) && g.items.iter().all(|i| i.selected || i.in_use);
                            if ui.checkbox(&mut a, "").changed() {
                                for i in g.items.iter_mut() {
                                    i.selected = a && !i.in_use;
                                }
                            }
                        } else {
                            ui.add_space(26.0);
                        }
                        glyph_tile(ui, junk_glyph(g.kind), accent, 34.0);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(g.title).size(16.0).strong());
                            ui.label(RichText::new(g.desc).size(12.5).color(MUTED));
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let arrow = if g.expanded { "▾" } else { "▸" };
                            if !g.items.is_empty() && ui.add(egui::Button::new(arrow).frame(false)).clicked() {
                                g.expanded = !g.expanded;
                            }
                            ui.label(RichText::new(human(g.total())).size(15.0).strong());
                            ui.label(RichText::new(plural(g.items.len(), "item")).color(DIM));
                        });
                    });
                    if let Some(n) = &g.note {
                        ui.label(RichText::new(n).size(12.0).color(WARN));
                    }
                    if g.expanded {
                        ui.separator();
                        for it in g.items.iter_mut().take(300) {
                            ui.horizontal(|ui| {
                                ui.add_space(24.0);
                                if g.kind != JunkKind::Trash {
                                    ui.checkbox(&mut it.selected, "");
                                }
                                ui.label(RichText::new(&it.name).size(13.0)).on_hover_text(short(&it.path));
                                if it.in_use {
                                    ui.label(RichText::new("in use · quit the app to include").size(11.5).color(WARN));
                                }
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(RichText::new(human(it.size)).size(13.0).color(MUTED));
                                });
                            });
                        }
                    }
                });
                ui.add_space(8.0);
            }
        });
        let clean = !busy && round_button(ui, "Clean", accent);
        if rescan {
            self.junk_job = Some(Job::spawn(scan::scan_junk));
        }
        if clean {
            let paths: Vec<_> = self
                .junk
                .iter()
                .flatten()
                .filter(|g| g.kind != JunkKind::Trash)
                .flat_map(|g| g.items.iter().filter(|i| i.selected).map(|i| (i.path.clone(), i.size)))
                .collect();
            if paths.is_empty() {
                self.toast("Tick something to clean first.");
            } else {
                self.pending = Some(Pending::Trash { title: format!("Clean up {}?", human(total)), paths });
            }
        }
        if empty {
            self.pending = Some(Pending::EmptyTrash { size: trash_size });
        }
    }

    // ------------------------------------------------------------------ protection

    pub fn page_protection(&mut self, ui: &mut Ui) {
        if let Some(j) = &self.agents_job {
            if self.working(ui, "Looking for threats…", &j.status()) {
                j.cancel();
            }
            return;
        }
        if self.agents.is_none() {
            if self.intro_screen(ui, "Scan", None) {
                self.agents_job = Some(Job::spawn(scan::scan_agents));
            }
            return;
        }
        self.header(ui, "Background helpers and apps checked for unwanted software");
        let busy = self.action_job.is_some();
        let accent = Page::Protection.accent();
        let show_ok = &mut self.show_ok_agents;
        let items = self.agents.as_mut().unwrap();
        let threats = items.iter().filter(|i| i.verdict == Verdict::Threat).count();
        let review = items.iter().filter(|i| i.verdict == Verdict::Review).count();
        let mut rescan = false;
        ui.horizontal(|ui| {
            let (txt, col) = if threats > 0 { (plural(threats, "threat") + " found", BAD) } else { ("No threats found".to_string(), GOOD) };
            ui.label(RichText::new(txt).size(30.0).strong().color(col));
            if review > 0 {
                ui.label(RichText::new(format!("· {review} to review")).size(15.0).color(WARN));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ghost(ui, "Scan again").clicked() {
                    rescan = true;
                }
                ui.checkbox(show_ok, "Show all items");
            });
        });
        let mut by_cat: Vec<(String, usize)> = vec![];
        for it in items.iter().filter(|i| i.verdict == Verdict::Threat) {
            let c = if it.category.is_empty() { "Unwanted".to_string() } else { it.category.clone() };
            match by_cat.iter_mut().find(|x| x.0 == c) {
                Some(x) => x.1 += 1,
                None => by_cat.push((c, 1)),
            }
        }
        let mut load_db = false;
        ui.horizontal(|ui| {
            let (fams, sigs, updated) = &self.db_info;
            ui.label(
                RichText::new(format!(
                    "{} checked against {} signs of {} known threat families: trojans, adware, spyware and crypto miners · database from {}",
                    plural(items.len(), "item"), sigs, fams, updated
                ))
                .size(12.5)
                .color(MUTED),
            );
            if ui.small_button("Load newer database…").clicked() {
                load_db = true;
            }
        });
        if !by_cat.is_empty() {
            ui.horizontal(|ui| {
                for (c, n) in &by_cat {
                    category_tag(ui, c, *n);
                }
            });
        }
        ui.add_space(8.0);
        let show_ok = *show_ok;
        let h = ui.available_height() - 80.0;
        egui::ScrollArea::vertical().max_height(h).auto_shrink([false; 2]).show(ui, |ui| {
            let mut shown = 0;
            for it in items.iter_mut() {
                if it.verdict == Verdict::Ok && !show_ok {
                    continue;
                }
                shown += 1;
                agent_row(ui, it);
                ui.add_space(6.0);
            }
            if shown == 0 {
                ui.add_space(30.0);
                ui.vertical_centered(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(90.0), Sense::hover());
                    art::glyph(ui.painter(), Glyph::Shield, r, GOOD);
                    ui.label(RichText::new("Everything looks fine.").size(18.0));
                    ui.label(RichText::new("Tick “Show all items” to see every background helper.").color(MUTED));
                });
            }
        });
        let n = self.agents.iter().flatten().filter(|i| i.selected).count();
        let remove = n > 0 && !busy && round_button(ui, "Remove", accent);
        if load_db {
            if let Some(f) = crate::threats::pick_file() {
                match crate::threats::install(&f) {
                    Ok(db) => {
                        self.db_info = (db.families.len(), db.signatures(), db.updated.clone());
                        self.toast(format!("Threat database from {} loaded. Scanning again…", db.updated));
                        rescan = true;
                    }
                    Err(e) => self.toast(format!("Couldn't use that file: {e}")),
                }
            }
        }
        if rescan {
            self.agents_job = Some(Job::spawn(scan::scan_agents));
        }
        if remove {
            // running threats are stopped after the person confirms (see confirm_dialog)
            self.pending_stop = self.agents.iter().flatten().filter(|i| i.selected).filter_map(|i| i.pid).collect();
            let paths: Vec<_> = self.agents.iter().flatten().filter(|i| i.selected).map(|i| (i.path.clone(), 0)).collect();
            self.pending = Some(Pending::Trash { title: format!("Remove {}?", plural(paths.len(), "threat")), paths });
        }
    }

    // ------------------------------------------------------------------ performance

    pub fn page_performance(&mut self, ui: &mut Ui) {
        if !self.perf_open {
            if self.intro_screen(ui, "Scan", None) {
                self.perf_open = true;
            }
            return;
        }
        if self.login.is_none() && self.login_job.is_none() {
            self.login_job = Some(Job::spawn(|_| scan::login_items()));
        }
        if self.agents.is_none() && self.agents_job.is_none() {
            self.agents_job = Some(Job::spawn(scan::scan_agents));
        }
        self.refresh_procs();
        self.header(ui, "Keep your Mac quick with maintenance and a tidy startup");
        let accent = Page::Performance.accent();
        let labels = vec![
            format!("Maintenance ({})", self.tasks.len()),
            match &self.login {
                Some(Ok(v)) => format!("Login items ({})", v.len()),
                _ => "Login items".to_string(),
            },
            match &self.agents {
                Some(v) => format!("Background items ({})", v.iter().filter(|a| a.place != "Applications").count()),
                None => "Background items".to_string(),
            },
            "Memory".to_string(),
        ];
        tabs(ui, &labels, &mut self.perf_tab);
        ui.add_space(10.0);
        let h = ui.available_height() - 80.0;
        let mut run = false;
        match self.perf_tab {
            0 => {
                egui::ScrollArea::vertical().max_height(h).auto_shrink([false; 2]).show(ui, |ui| {
                    for t in self.tasks.iter_mut() {
                        card(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut t.selected, "");
                                glyph_tile(ui, task_glyph(t.title), accent, 34.0);
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(t.title).size(15.0).strong());
                                        if t.admin {
                                            ui.label(RichText::new("asks for your password").size(11.5).color(MUTED));
                                        }
                                    });
                                    ui.label(RichText::new(t.desc).size(12.5).color(MUTED));
                                });
                                if let Some(l) = &t.last {
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        let c = if l == "Done" { GOOD } else { WARN };
                                        ui.label(RichText::new(elide(l, 50)).size(12.5).color(c));
                                    });
                                }
                            });
                        });
                        ui.add_space(6.0);
                    }
                });
                if let Some(j) = &self.task_job {
                    let r = ui.max_rect();
                    ui.painter().text(Pos2::new(r.center().x, r.bottom() - 40.0), Align2::CENTER_CENTER, j.status(), FontId::proportional(14.0), TEXT);
                } else {
                    run = round_button(ui, "Run", accent);
                }
            }
            1 => {
                let mut remove: Option<String> = None;
                egui::ScrollArea::vertical().max_height(h).auto_shrink([false; 2]).show(ui, |ui| match &self.login {
                    None => {
                        ui.add(egui::Spinner::new());
                    }
                    Some(Err(e)) => {
                        ui.label(RichText::new("Couldn't read your login items.").size(16.0));
                        ui.label(RichText::new(format!("macOS may ask for permission to control System Events. ({})", elide(e, 90))).color(MUTED));
                    }
                    Some(Ok(v)) if v.is_empty() => {
                        ui.label(RichText::new("No apps open by themselves when you log in.").size(16.0).color(MUTED));
                    }
                    Some(Ok(v)) => {
                        ui.label(RichText::new("These apps open by themselves when you log in. Removing one here only stops it starting; the app stays.").color(MUTED));
                        ui.add_space(6.0);
                        for name in v {
                            card(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    glyph_tile(ui, Glyph::Power, accent, 32.0);
                                    ui.label(RichText::new(name).size(15.0));
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if ghost(ui, "Remove").clicked() {
                                            remove = Some(name.clone());
                                        }
                                    });
                                });
                            });
                            ui.add_space(6.0);
                        }
                    }
                });
                if let Some(n) = remove {
                    match scan::remove_login_item(&n) {
                        Ok(()) => {
                            scan::log_activity(&format!("Removed login item {n}"), 0);
                            self.toast(format!("{n} won't open at login anymore."));
                        }
                        Err(e) => self.toast(format!("Couldn't remove {n}: {}", elide(&e, 60))),
                    }
                    self.login = None;
                }
            }
            2 => {
                let busy = self.action_job.is_some();
                if self.agents.is_none() {
                    ui.add(egui::Spinner::new());
                } else {
                    let items = self.agents.as_mut().unwrap();
                    egui::ScrollArea::vertical().max_height(h).auto_shrink([false; 2]).show(ui, |ui| {
                        ui.label(RichText::new("Helpers that start in the background. Unticked items stay as they are.").color(MUTED));
                        ui.add_space(6.0);
                        for it in items.iter_mut().filter(|a| a.place != "Applications") {
                            agent_row(ui, it);
                            ui.add_space(6.0);
                        }
                    });
                    let n = items.iter().filter(|i| i.selected).count();
                    if n > 0 && !busy && round_button(ui, "Remove", accent) {
                        let paths: Vec<_> = items.iter().filter(|i| i.selected).map(|i| (i.path.clone(), 0)).collect();
                        self.pending = Some(Pending::Trash { title: format!("Move {} to the Trash?", plural(paths.len(), "background item")), paths });
                    }
                }
            }
            _ => {
                let mut quit = None;
                self.quick_stats(ui);
                ui.add_space(8.0);
                card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("Heavy memory users").size(17.0).strong());
                    ui.label(RichText::new("Quit apps you aren't using to free memory.").size(12.5).color(MUTED));
                    ui.add_space(4.0);
                    let maxm = self.procs.first().map(|p| p.2).unwrap_or(1).max(1);
                    for (pid, name, mem, _cpu) in &self.procs {
                        ui.horizontal(|ui| {
                            ui.add_sized([220.0, 20.0], egui::Label::new(RichText::new(elide(name, 30)).size(13.5)));
                            bar(ui, *mem as f32 / maxm as f32, 220.0, accent);
                            ui.label(RichText::new(human(*mem)).size(12.5).color(MUTED));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ghost(ui, "Quit").clicked() {
                                    quit = Some(*pid);
                                }
                            });
                        });
                    }
                });
                if let Some(pid) = quit {
                    let name = self.procs.iter().find(|p| p.0 == pid).map(|p| p.1.clone()).unwrap_or_default();
                    self.pending = Some(Pending::Quit { pid, name });
                }
            }
        }
        if run {
            let cmds: Vec<_> = self.tasks.iter().enumerate().filter(|t| t.1.selected).map(|(i, t)| (i, t.admin, t.cmd.to_string())).collect();
            if cmds.is_empty() {
                self.toast("Tick a task to run first.");
            } else {
                self.task_job = Some(Job::spawn(move |p| scan::run_tasks(cmds, p)));
            }
        }
    }

    // ------------------------------------------------------------------ applications

    pub fn page_apps(&mut self, ui: &mut Ui) {
        if let Some(j) = &self.apps_job {
            if self.working(ui, "Looking at your apps…", &j.status()) {
                j.cancel();
            }
            return;
        }
        if self.apps.is_none() {
            if self.intro_screen(ui, "Scan", None) {
                self.apps_job = Some(Job::spawn(scan::scan_apps));
            }
            return;
        }
        self.header(ui, "Remove apps completely, along with the files they leave behind");
        let accent = Page::Apps.accent();
        let busy = self.action_job.is_some();
        let apps = self.apps.as_ref().unwrap();
        let total: u64 = apps.iter().map(|a| a.total()).sum();
        let leftovers: u64 = apps.iter().map(|a| a.leftovers.iter().map(|l| l.1).sum::<u64>()).sum();
        let mut rescan = false;
        let mut toggle: Option<PathBuf> = None;
        ui.horizontal(|ui| {
            ui.label(RichText::new(plural(apps.len(), "app")).size(26.0).strong());
            ui.label(RichText::new(format!("using {} · {} in settings and caches", human(total), human(leftovers))).size(14.0).color(MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ghost(ui, "Refresh").clicked() {
                    rescan = true;
                }
                ui.checkbox(&mut self.apps_unused_only, "Unused for six months");
                ui.checkbox(&mut self.app_sort_size, "Biggest first");
                ui.add(egui::TextEdit::singleline(&mut self.app_filter).hint_text("Search apps").desired_width(150.0));
            });
        });
        ui.add_space(6.0);
        let filter = self.app_filter.to_lowercase();
        let unused_only = self.apps_unused_only;
        let mut order: Vec<usize> = (0..apps.len())
            .filter(|&i| filter.is_empty() || apps[i].name.to_lowercase().contains(&filter))
            .filter(|&i| !unused_only || apps[i].last_used_days > UNUSED_APP_DAYS)
            .collect();
        if self.app_sort_size {
            order.sort_by(|a, b| apps[*b].total().cmp(&apps[*a].total()));
        }
        let h = ui.available_height();
        let w = ui.available_width();
        let (mut uninstall, mut reset, mut pick) = (None, None, None);
        let app_sel = self.app_sel;
        ui.horizontal_top(|ui| {
            card(ui, |ui| {
                ui.set_width(w * 0.5 - 38.0);
                ui.set_height(h - 34.0);
                egui::ScrollArea::vertical().id_salt("apps").auto_shrink([false; 2]).show(ui, |ui| {
                    if order.is_empty() {
                        ui.label(RichText::new("No apps match.").color(MUTED));
                    }
                    for &i in &order {
                        let a = &apps[i];
                        let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 46.0), Sense::click());
                        if app_sel == Some(i) {
                            ui.painter().rect_filled(r, 10.0, TINT);
                        } else if resp.hovered() {
                            ui.painter().rect_filled(r, 10.0, ink(10));
                        }
                        let ic = Rect::from_center_size(Pos2::new(r.left() + 24.0, r.center().y), Vec2::splat(32.0));
                        let hue = (a.name.bytes().map(|b| b as u32).sum::<u32>() % 360) as f32 / 360.0;
                        ui.painter().rect_filled(ic, 8.0, hsv(hue, 0.55, 0.8));
                        let letter = a.name.chars().next().unwrap_or('?').to_uppercase().to_string();
                        ui.painter().text(ic.center(), Align2::CENTER_CENTER, letter, FontId::proportional(16.0), Color32::WHITE);
                        ui.painter().text(Pos2::new(r.left() + 50.0, r.center().y - 8.0), Align2::LEFT_CENTER, elide(&a.name, 30), FontId::proportional(14.0), TEXT);
                        let sub = if a.last_used_days > UNUSED_APP_DAYS {
                            format!("Last opened {}", age_text(a.last_used_days))
                        } else if a.version.is_empty() {
                            a.bundle_id.clone()
                        } else {
                            format!("Version {}", a.version)
                        };
                        ui.painter().text(Pos2::new(r.left() + 50.0, r.center().y + 9.0), Align2::LEFT_CENTER, elide(&sub, 34), FontId::proportional(11.5), DIM);
                        ui.painter().text(Pos2::new(r.right() - 10.0, r.center().y), Align2::RIGHT_CENTER, human(a.total()), FontId::proportional(13.0), MUTED);
                        if resp.clicked() {
                            pick = Some(i);
                        }
                    }
                });
            });
            card(ui, |ui| {
                ui.set_width(w * 0.5 - 38.0);
                ui.set_height(h - 34.0);
                match app_sel.and_then(|i| apps.get(i).map(|a| (i, a))) {
                    None => {
                        ui.add_space(h * 0.22);
                        ui.vertical_centered(|ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(150.0), Sense::hover());
                            art::hero(ui.painter(), Page::Apps.outline(), Page::Apps.glyph(), r.center(), 60.0, accent);
                            ui.label(RichText::new("Pick an app to see what it keeps on your Mac.").color(MUTED));
                        });
                    }
                    Some((i, a)) => {
                        ui.label(RichText::new(&a.name).size(22.0).strong());
                        ui.label(RichText::new(short(&a.path)).size(12.0).color(DIM));
                        if a.last_used_days > 0 {
                            ui.label(RichText::new(format!("Last opened {}", age_text(a.last_used_days).to_lowercase())).size(12.5).color(MUTED));
                        }
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label("The app itself");
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.label(human(a.size)));
                        });
                        ui.separator();
                        ui.label(RichText::new(format!("Leftover files ({})", a.leftovers.len())).strong());
                        if !a.leftovers.is_empty() {
                            ui.label(RichText::new("Untick anything you want to keep. Files matched only by the app's name start unticked.").size(11.5).color(MUTED));
                        }
                        egui::ScrollArea::vertical().id_salt("left").max_height((h - 280.0).max(60.0)).show(ui, |ui| {
                            if a.leftovers.is_empty() {
                                ui.label(RichText::new("No settings or caches found for this app.").color(MUTED));
                            }
                            for (p, s) in &a.leftovers {
                                ui.horizontal(|ui| {
                                    let mut on = !a.skip.contains(p);
                                    if ui.checkbox(&mut on, "").changed() {
                                        toggle = Some(p.clone());
                                    }
                                    ui.label(RichText::new(elide(&short(p), 48)).size(12.0)).on_hover_text(p.display().to_string());
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.label(RichText::new(human(*s)).size(12.0).color(MUTED)));
                                });
                            }
                        });
                        ui.add_space(10.0);
                        ui.horizontal_wrapped(|ui| {
                            if !busy && pill(ui, &format!("Uninstall · {}", human(a.chosen_total())), accent).clicked() {
                                uninstall = Some(i);
                            }
                            if !a.chosen().is_empty() && ghost(ui, "Reset (keep app)").on_hover_text("Removes settings and caches only").clicked() {
                                reset = Some(i);
                            }
                            if ghost(ui, "Show in Finder").clicked() {
                                reveal(&a.path);
                            }
                        });
                    }
                }
            });
        });
        if let (Some(p), Some(i)) = (toggle, self.app_sel) {
            if let Some(a) = self.apps.as_mut().and_then(|v| v.get_mut(i)) {
                if !a.skip.remove(&p) {
                    a.skip.insert(p);
                }
            }
        }
        if pick.is_some() {
            self.app_sel = pick;
        }
        if rescan {
            self.apps_job = Some(Job::spawn(scan::scan_apps));
        }
        if let Some(i) = uninstall {
            let a = &self.apps.as_ref().unwrap()[i];
            if scan::app_is_running(&a.path) {
                let msg = format!("{} is open. Quit it first, then uninstall.", a.name);
                self.toast(msg);
            } else {
                let mut paths = vec![(a.path.clone(), a.size)];
                paths.extend(a.chosen());
                self.pending = Some(Pending::Trash { title: format!("Uninstall {}?", a.name), paths });
            }
        }
        if let Some(i) = reset {
            let a = &self.apps.as_ref().unwrap()[i];
            self.pending = Some(Pending::Trash { title: format!("Reset {}?", a.name), paths: a.chosen() });
        }
    }

    // ------------------------------------------------------------------ my files

    pub fn page_clutter(&mut self, ui: &mut Ui) {
        if let Some(j) = &self.clutter_job {
            let title = ["Looking for large files…", "Looking for duplicates…", "Looking at downloads…"][j.stage().min(2)];
            if self.working(ui, title, &j.status()) {
                j.cancel();
            }
            return;
        }
        if self.big.is_none() || !self.clutter_scanned {
            let mut pick = |ui: &mut Ui, app: &mut CleanerApp| {
                let mut opts = vec![scan::home()];
                opts.extend(scan::clutter_roots());
                egui::ComboBox::from_id_salt("clutter_root")
                    .width(240.0)
                    .selected_text(RichText::new(folder_label(&app.clutter_root)).size(15.0))
                    .show_ui(ui, |ui| {
                        for o in opts {
                            let l = folder_label(&o);
                            ui.selectable_value(&mut app.clutter_root, o, l);
                        }
                    });
            };
            if self.intro_screen(ui, "Scan", Some(&mut pick)) {
                self.start_clutter();
            }
            return;
        }
        self.header(ui, &format!("Large, old, duplicate and downloaded files in {}", short(&self.clutter_root)));
        let accent = Page::Clutter.accent();
        let busy = self.action_job.is_some();
        let labels = vec![
            format!("Large & old ({})", self.big.as_ref().map(|b| b.len()).unwrap_or(0)),
            format!("Duplicates ({})", self.dups.as_ref().map(|b| b.len()).unwrap_or(0)),
            format!("Downloads ({})", self.downloads.as_ref().map(|b| b.len()).unwrap_or(0)),
        ];
        let mut rescan = false;
        ui.horizontal(|ui| {
            tabs(ui, &labels, &mut self.clutter_tab);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ghost(ui, "Scan again").clicked() {
                    rescan = true;
                }
            });
        });
        ui.add_space(8.0);
        let h = ui.available_height() - 80.0;
        let selected: Vec<(PathBuf, u64)>;
        match self.clutter_tab {
            0 => {
                let kind_sel = &mut self.big_kind;
                let size_sel = &mut self.big_size_group;
                let age_sel = &mut self.big_age_group;
                let files = self.big.as_mut().unwrap();
                ui.horizontal(|ui| {
                    for k in ["All", "Video", "Audio", "Picture", "Archive", "Installer", "Document", "Other"] {
                        if ui.selectable_label(kind_sel == k, k).clicked() {
                            *kind_sel = k.to_string();
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Size").color(MUTED));
                    for (i, (name, _, _)) in SIZE_GROUPS.iter().enumerate() {
                        if ui.selectable_label(*size_sel == i, *name).clicked() {
                            *size_sel = i;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Last opened").color(MUTED));
                    for (i, (name, _)) in AGE_GROUPS.iter().enumerate() {
                        if ui.selectable_label(*age_sel == i, *name).clicked() {
                            *age_sel = i;
                        }
                    }
                });
                let (_, lo, hi) = SIZE_GROUPS[*size_sel];
                let (_, age) = AGE_GROUPS[*age_sel];
                let visible: Vec<usize> = (0..files.len())
                    .filter(|&i| {
                        let f = &files[i];
                        (kind_sel == "All" || f.kind == *kind_sel) && f.size > lo && f.size <= hi && age_matches(age, f.age_days)
                    })
                    .collect();
                card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if visible.is_empty() {
                        ui.label(RichText::new(format!("No files over {} match these filters.", human(LARGE_FILE_BYTES))).color(MUTED));
                    }
                    if !visible.is_empty() {
                        file_header(ui);
                    }
                    egui::ScrollArea::vertical().max_height(h - 104.0).auto_shrink([false; 2]).show_rows(ui, 26.0, visible.len(), |ui, range| {
                        for &i in &visible[range] {
                            let f = &mut files[i];
                            file_row(ui, &mut f.selected, &f.name, &f.path, f.size, f.age_days, &f.kind);
                        }
                    });
                });
                selected = files.iter().filter(|f| f.selected).map(|f| (f.path.clone(), f.size)).collect();
            }
            1 => {
                let groups = self.dups.get_or_insert_with(Vec::new);
                let mut auto = false;
                ui.horizontal(|ui| {
                    let waste: u64 = groups.iter().map(|g| g.size * (g.files.len() as u64 - 1)).sum();
                    ui.label(RichText::new(format!("{} of extra copies", human(waste))).size(16.0).strong());
                    let fresh: u64 = groups.iter().filter(|g| g.files.iter().any(|f| f.age_days <= FRESH_DUPLICATE_DAYS)).map(|g| g.size * (g.files.len() as u64 - 1)).sum();
                    ui.label(RichText::new(format!("· {} from the last 3 months", human(fresh))).color(MUTED));
                    if !groups.is_empty() && ghost(ui, "Keep the newest, select the rest").clicked() {
                        auto = true;
                    }
                });
                if auto {
                    for g in groups.iter_mut() {
                        for (k, f) in g.files.iter_mut().enumerate() {
                            f.selected = k > 0;
                        }
                    }
                }
                egui::ScrollArea::vertical().max_height(h - 40.0).auto_shrink([false; 2]).show(ui, |ui| {
                    if groups.is_empty() {
                        ui.label(RichText::new("No duplicate files found.").color(MUTED));
                    }
                    for g in groups.iter_mut().take(300) {
                        card(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                glyph_tile(ui, Glyph::Copy, accent, 30.0);
                                ui.label(RichText::new(elide(&g.name, 50)).size(15.0).strong());
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(RichText::new(format!("{} copies · {} each", g.files.len(), human(g.size))).color(MUTED));
                                });
                            });
                            for f in g.files.iter_mut() {
                                ui.horizontal(|ui| {
                                    ui.add_space(30.0);
                                    ui.checkbox(&mut f.selected, "");
                                    ui.label(RichText::new(elide(&short(&f.path), 80)).size(12.5)).on_hover_text(f.path.display().to_string());
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if ui.small_button("Reveal").clicked() {
                                            reveal(&f.path);
                                        }
                                        ui.label(RichText::new(age_text(f.age_days)).size(12.0).color(DIM));
                                    });
                                });
                            }
                        });
                        ui.add_space(6.0);
                    }
                });
                // never take every copy of a file: the newest one stays if all are ticked
                selected = groups
                    .iter()
                    .flat_map(|g| {
                        let all = g.files.iter().all(|f| f.selected);
                        g.files.iter().skip(if all { 1 } else { 0 }).filter(|f| f.selected).map(move |f| (f.path.clone(), g.size))
                    })
                    .collect();
            }
            _ => {
                let files = self.downloads.get_or_insert_with(Vec::new);
                let mut auto = false;
                ui.horizontal(|ui| {
                    let old: u64 = files.iter().filter(|f| f.age_days > OLD_DOWNLOAD_DAYS).map(|f| f.size).sum();
                    ui.label(RichText::new(format!("{} in downloads older than {} days", human(old), OLD_DOWNLOAD_DAYS)).size(16.0).strong());
                    if ghost(ui, "Select the old ones").clicked() {
                        auto = true;
                    }
                });
                if auto {
                    for f in files.iter_mut() {
                        f.selected = f.age_days > OLD_DOWNLOAD_DAYS;
                    }
                }
                card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if files.is_empty() {
                        ui.label(RichText::new("Your Downloads folder is empty.").color(MUTED));
                    }
                    if !files.is_empty() {
                        file_header(ui);
                    }
                    egui::ScrollArea::vertical().max_height(h - 104.0).auto_shrink([false; 2]).show_rows(ui, 26.0, files.len(), |ui, range| {
                        for f in &mut files[range] {
                            file_row(ui, &mut f.selected, &f.name, &f.path, f.size, f.age_days, &f.kind);
                        }
                    });
                });
                selected = files.iter().filter(|f| f.selected).map(|f| (f.path.clone(), f.size)).collect();
            }
        }
        if !selected.is_empty() && !busy && round_button(ui, "Remove", accent) {
            let total: u64 = selected.iter().map(|s| s.1).sum();
            self.pending = Some(Pending::Trash { title: format!("Move {} ({}) to the Trash?", plural(selected.len(), "file"), human(total)), paths: selected });
        }
        if rescan {
            self.start_clutter();
        }
    }

    // ------------------------------------------------------------------ space map

    pub fn page_space(&mut self, ui: &mut Ui) {
        if let Some(j) = &self.space_job {
            if self.working(ui, "Mapping your storage…", &j.status()) {
                j.cancel();
            }
            return;
        }
        if self.space.is_none() {
            let mut pick = |ui: &mut Ui, app: &mut CleanerApp| {
                egui::ComboBox::from_id_salt("space_root")
                    .width(240.0)
                    .selected_text(RichText::new(folder_label(&app.space_root)).size(15.0))
                    .show_ui(ui, |ui| {
                        for o in [scan::home(), PathBuf::from("/")] {
                            let l = folder_label(&o);
                            ui.selectable_value(&mut app.space_root, o, l);
                        }
                    });
            };
            if self.intro_screen(ui, "Scan", Some(&mut pick)) {
                let r = self.space_root.clone();
                self.space_job = Some(Job::spawn(move |p| scan::scan_space(r, p)));
            }
            return;
        }
        let accent = Page::Space.accent();
        let busy = self.action_job.is_some();
        let root = self.space.as_ref().unwrap();
        let mut node = root;
        let mut crumbs = vec![(0usize, node.name.clone())];
        for (d, &i) in self.space_stack.iter().enumerate() {
            match node.children.get(i) {
                Some(n) => {
                    node = n;
                    crumbs.push((d + 1, n.name.clone()));
                }
                None => break,
            }
        }
        let (mut go_depth, mut enter, mut select) = (None, None, None);
        let mut rescan = false;
        let mut trash: Option<(PathBuf, u64)> = None;
        let sel = self.space_sel;
        ui.horizontal(|ui| {
            glyph_tile(ui, Glyph::Lens, accent, 34.0);
            for (d, name) in &crumbs {
                if ui.link(RichText::new(if name.is_empty() { "/" } else { name }).size(16.0).color(TEXT)).clicked() {
                    go_depth = Some(*d);
                }
                ui.label(RichText::new("›").color(DIM));
            }
            ui.label(RichText::new(format!("{} · {} items", human(node.size), node.items)).color(MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ghost(ui, "Scan again").clicked() {
                    rescan = true;
                }
            });
        });
        ui.add_space(10.0);
        let avail = ui.available_size();
        let side = 270.0;
        ui.horizontal_top(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(avail.x - side - 12.0, avail.y - 6.0), Sense::hover());
            let items: Vec<(usize, f64)> = node.children.iter().enumerate().filter(|(_, c)| c.size > 0).map(|(i, c)| (i, c.size as f64)).collect();
            let bubbles = bubble_map(&items, rect);
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 20.0, Page::Space.tint());
            for (i, c, r) in &bubbles {
                let item = &node.children[*i];
                let br = Rect::from_center_size(*c, Vec2::splat(r * 2.0));
                let resp = ui.interact(br, ui.scope_id().with(("bubble", *i)), Sense::click());
                let hover = resp.hovered();
                let hue = 0.45 + ((*i as f32) * 0.11) % 0.5;
                let base: Color32 = hsv(hue, if item.is_dir { 0.62 } else { 0.25 }, if hover { 0.58 } else { 0.5 });
                painter.circle_filled(*c + Vec2::new(0.0, r * 0.10), r * 0.97, alpha(Color32::from_rgb(40, 30, 90), 40));
                painter.circle_filled(*c, *r, mix(base, Color32::BLACK, 0.22));
                painter.circle_filled(*c - Vec2::new(r * 0.05, r * 0.07), r * 0.93, base);
                painter.circle_filled(*c - Vec2::new(r * 0.36, r * 0.42), r * 0.13, alpha(Color32::WHITE, 85));
                if sel == Some(*i) {
                    painter.circle_stroke(*c, *r + 2.0, Stroke::new(3.0, Color32::WHITE));
                    painter.circle_stroke(*c, *r + 5.0, Stroke::new(2.0, ACCENT));
                }
                if *r > 34.0 {
                    let name = elide(&item.name, (r / 4.5) as usize);
                    painter.text(*c - Vec2::new(0.0, 8.0), Align2::CENTER_CENTER, name, FontId::proportional((r / 5.0).clamp(11.0, 16.0)), Color32::WHITE);
                    painter.text(*c + Vec2::new(0.0, 10.0), Align2::CENTER_CENTER, human(item.size), FontId::proportional(12.0), alpha(Color32::WHITE, 210));
                }
                let resp = resp.on_hover_text(format!("{}\n{}", item.name, human(item.size)));
                if resp.clicked() {
                    select = Some(*i);
                }
                if resp.double_clicked() && item.is_dir && !item.children.is_empty() {
                    enter = Some(*i);
                }
            }
            if bubbles.is_empty() {
                painter.text(rect.center(), Align2::CENTER_CENTER, "This folder is empty or couldn't be read", FontId::proportional(15.0), MUTED);
            }
            card(ui, |ui| {
                ui.set_width(side - 32.0);
                ui.set_min_height(avail.y - 40.0);
                match sel.and_then(|i| node.children.get(i)) {
                    None => {
                        ui.label(RichText::new("Biggest items").strong());
                        for c in node.children.iter().take(14) {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(elide(&c.name, 20)).size(12.5));
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.label(RichText::new(human(c.size)).size(12.0).color(MUTED)));
                            });
                        }
                        ui.add_space(8.0);
                        ui.label(RichText::new("Click a bubble to select it, double-click a folder to open it.").size(11.5).color(DIM));
                    }
                    Some(c) => {
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(48.0), Sense::hover());
                        art::glyph(ui.painter(), if c.is_dir { Glyph::Folder } else { Glyph::Copy }, r, ACCENT);
                        ui.label(RichText::new(&c.name).size(16.0).strong());
                        ui.label(RichText::new(human(c.size)).size(24.0).color(ACCENT_DARK));
                        if c.is_dir {
                            ui.label(RichText::new(format!("{} items inside", c.items)).color(MUTED));
                        }
                        ui.add_space(8.0);
                        let real = c.path != node.path;
                        if c.is_dir && !c.children.is_empty() && ghost(ui, "Open folder").clicked() {
                            enter = sel;
                        }
                        if real && ghost(ui, "Show in Finder").clicked() {
                            reveal(&c.path);
                        }
                        if real && scan::is_protected(&c.path) {
                            ui.label(RichText::new("Protected folder: macOS and your apps need it. Open it to clean what's inside.").size(12.5).color(WARN));
                        } else if real && !busy && pill(ui, "Move to Trash", accent).clicked() {
                            trash = Some((c.path.clone(), c.size));
                        }
                    }
                }
            });
        });
        if let Some(s) = select {
            self.space_sel = Some(s);
        }
        if let Some(d) = go_depth {
            self.space_stack.truncate(d);
            self.space_sel = None;
        }
        if let Some(i) = enter {
            self.space_stack.push(i);
            self.space_sel = None;
        }
        if rescan {
            let r = self.space_root.clone();
            self.space = None;
            self.space_job = Some(Job::spawn(move |p| scan::scan_space(r, p)));
        }
        if let Some(t) = trash {
            self.pending = Some(Pending::Trash { title: format!("Move “{}” to the Trash?", scan::file_name(&t.0)), paths: vec![t] });
        }
    }

    // ------------------------------------------------------------------ cloud files

    pub fn page_cloud(&mut self, ui: &mut Ui) {
        if let Some(j) = &self.cloud_job {
            if self.working(ui, "Measuring cloud folders…", &j.status()) {
                j.cancel();
            }
            return;
        }
        if self.cloud.is_none() {
            if self.intro_screen(ui, "Scan", None) {
                self.cloud_job = Some(Job::spawn(scan::scan_cloud));
            }
            return;
        }
        self.header(ui, "How much room your cloud folders take on this Mac");
        let accent = Page::Cloud.accent();
        let busy = self.action_job.is_some();
        let folders = self.cloud.as_mut().unwrap();
        let mut rescan = false;
        ui.horizontal(|ui| {
            let total: u64 = folders.iter().map(|f| f.local).sum();
            ui.label(RichText::new(human(total)).size(30.0).strong());
            ui.label(RichText::new("of cloud files stored on this Mac").size(15.0).color(MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ghost(ui, "Scan again").clicked() {
                    rescan = true;
                }
            });
        });
        ui.add_space(8.0);
        if folders.is_empty() {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("No cloud folders found on this Mac.").size(18.0));
                ui.label(RichText::new("iCloud Drive and folders from other cloud apps show up here.").color(MUTED));
            });
        } else {
            let sel = self.cloud_sel.min(folders.len() - 1);
            let labels: Vec<String> = folders.iter().map(|f| format!("{}  ·  {}", f.name, human(f.local))).collect();
            let mut s = sel;
            tabs(ui, &labels, &mut s);
            self.cloud_sel = s;
            ui.add_space(8.0);
            let f = &mut folders[sel];
            ui.label(
                RichText::new(if f.icloud {
                    "Freeing a local copy keeps the file in iCloud; it downloads again when you open it."
                } else {
                    "This folder belongs to another cloud app. Use that app to free local copies, or show files in Finder."
                })
                .color(MUTED),
            );
            let h = ui.available_height() - 90.0;
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                if f.files.is_empty() {
                    ui.label(RichText::new("No large files are stored on this Mac here.").color(MUTED));
                }
                egui::ScrollArea::vertical().max_height(h).auto_shrink([false; 2]).show_rows(ui, 26.0, f.files.len(), |ui, range| {
                    for b in &mut f.files[range] {
                        file_row(ui, &mut b.selected, &b.name, &b.path, b.size, b.age_days, &b.kind);
                    }
                });
            });
            let icloud = f.icloud;
            let paths: Vec<_> = f.files.iter().filter(|b| b.selected).map(|b| (b.path.clone(), b.size)).collect();
            if icloud && !paths.is_empty() && !busy && round_button(ui, "Free up", accent) {
                self.pending = Some(Pending::Evict { paths });
            }
        }
        if rescan {
            self.cloud_job = Some(Job::spawn(scan::scan_cloud));
        }
    }

    // ------------------------------------------------------------------ quick tools

    pub fn page_tools(&mut self, ui: &mut Ui) {
        self.header(ui, "Handy one-click fixes");
        let accent = Page::Tools.accent();
        if self.trash_size.is_none_or(|(_, t)| t.elapsed() > Duration::from_secs(5)) {
            self.trash_size = Some((scan::dir_size(&scan::home().join(".Trash"), None), std::time::Instant::now()));
        }
        let trash_size = self.trash_size.map(|t| t.0).unwrap_or(0);
        let mut run: Option<usize> = None;
        let mut empty = false;
        let mut storage = false;
        let cols = 3;
        let w = (ui.available_width() - (cols as f32 - 1.0) * 14.0) / cols as f32;
        let n = self.tasks.len() + 2;
        let busy = self.task_job.is_some() || self.action_job.is_some();
        egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
            for row in 0..n.div_ceil(cols) {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    for col in 0..cols {
                        let i = row * cols + col;
                        if i >= n {
                            break;
                        }
                        card(ui, |ui| {
                            ui.set_width(w - 34.0);
                            ui.set_height(150.0);
                            let (g, title, desc, last) = if i < self.tasks.len() {
                                let t = &self.tasks[i];
                                (task_glyph(t.title), t.title.to_string(), t.desc.to_string(), t.last.clone())
                            } else if i == self.tasks.len() {
                                (Glyph::Trash, "Empty the Trash".to_string(), format!("Erase {} in the Trash for good.{}", human(trash_size), if trash_size > TRASH_ALERT_BYTES { " It's getting big." } else { "" }), None)
                            } else {
                                (Glyph::Disk, "Storage settings".to_string(), "Open macOS's own storage overview.".to_string(), None)
                            };
                            glyph_tile(ui, g, accent, 38.0);
                            ui.label(RichText::new(title).size(16.0).strong());
                            // the text takes the same room in every card, so the buttons line up
                            let top = ui.cursor().top();
                            ui.label(RichText::new(desc).size(12.5).color(MUTED));
                            ui.add_space((40.0 - (ui.cursor().top() - top)).max(0.0));
                            ui.horizontal(|ui| {
                                if !busy && pill(ui, if i == n - 1 { "Open" } else { "Run" }, accent).clicked() {
                                    if i < self.tasks.len() {
                                        run = Some(i);
                                    } else if i == self.tasks.len() {
                                        empty = true;
                                    } else {
                                        storage = true;
                                    }
                                }
                                if let Some(l) = last {
                                    ui.label(RichText::new(elide(&l, 30)).size(12.0).color(if l == "Done" { GOOD } else { WARN }));
                                }
                            });
                        });
                    }
                });
                ui.add_space(14.0);
            }
            if let Some(j) = &self.task_job {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new());
                    ui.label(RichText::new(j.status()).color(MUTED));
                });
            }
        });
        if let Some(i) = run {
            let t = &self.tasks[i];
            let cmds = vec![(i, t.admin, t.cmd.to_string())];
            self.task_job = Some(Job::spawn(move |p| scan::run_tasks(cmds, p)));
        }
        if empty {
            if trash_size == 0 {
                self.toast("The Trash is already empty.");
            } else {
                self.pending = Some(Pending::EmptyTrash { size: trash_size });
            }
        }
        if storage {
            let _ = std::process::Command::new("open").arg("x-apple.systempreferences:com.apple.settings.Storage").spawn();
        }
    }

    // ------------------------------------------------------------------ activity

    pub fn page_activity(&mut self, ui: &mut Ui) {
        self.header(ui, "How your Mac is doing, and what was cleaned");
        let accent = Page::Activity.accent();
        let free = self.disk_free;
        let limit = scan::low_disk_limit(self.disk_total);
        let mem = self.mem_used as f32 / self.mem_total as f32;
        let cpu = self.cpu / 100.0;
        if self.trash_size.is_none_or(|(_, t)| t.elapsed() > Duration::from_secs(10)) {
            self.trash_size = Some((scan::dir_size(&scan::home().join(".Trash"), None), std::time::Instant::now()));
        }
        let trash = self.trash_size.map(|t| t.0).unwrap_or(0);
        if self.unused_apps.is_none_or(|(_, t)| t.elapsed() > Duration::from_secs(60)) {
            self.unused_apps = Some((scan::apps_summary().unused.len(), std::time::Instant::now()));
        }
        let unused = self.unused_apps.map(|u| u.0).unwrap_or(0);
        let uptime_days = sysinfo::System::uptime().unwrap_or(0) / 86_400;
        let battery_cond = self.battery_condition.lock().unwrap().clone();
        let battery_bad = battery_cond.as_deref().is_some_and(|c| !c.eq_ignore_ascii_case("normal"));
        let smart_days = scan::days_since_logged(&self.history, scan::SMART_SCAN_LOG);
        let threat_days = scan::days_since_logged(&self.history, scan::THREAT_SCAN_LOG);
        // (penalty, title, text, button, where it leads), biggest penalty first
        let mut issues: Vec<(i64, String, String, &str, (Page, usize))> = vec![];
        if battery_bad {
            issues.push((PENALTY_CRITICAL_BATTERY, "Your battery needs service".into(), format!("macOS reports its condition as “{}”.", battery_cond.clone().unwrap_or_default()), "See Activity", (Page::Activity, 0)));
        }
        if free < limit {
            issues.push((PENALTY_OUT_OF_DISK, "Your disk is getting full".into(), format!("Only {} is left. Clean up junk to make room.", human(free)), "Clean Up Junk", (Page::Cleanup, 0)));
        }
        if smart_days.is_none_or(|d| d > RECENT_SCAN_DAYS) {
            issues.push((PENALTY_NO_SMART_SCAN, "Run a Smart Scan".into(), "It's been a while since your Mac had a full checkup.".into(), "Smart Scan", (Page::Smart, 0)));
        }
        if unused > 0 {
            issues.push((PENALTY_UNUSED_APPS, "Remove apps you don't use".into(), format!("{} not opened for six months.", plural(unused, "app")), "See Apps", (Page::Apps, 0)));
        }
        if trash > TRASH_ALERT_BYTES {
            issues.push((PENALTY_TRASH_OVERFILL, "Your Trash is getting big".into(), format!("It holds {}. Empty it to get the space back.", human(trash)), "Open Tools", (Page::Tools, 0)));
        }
        if uptime_days > UPTIME_WARN_DAYS {
            issues.push((PENALTY_UPTIME, "Time to restart".into(), format!("Your Mac has been running for {uptime_days} days. Restart it once in a while."), "OK", (Page::Activity, 0)));
        }
        if threat_days.is_none_or(|d| d > RECENT_SCAN_DAYS) {
            issues.push((PENALTY_NO_THREAT_SCAN, "Check for threats".into(), format!("Scan with the threat database ({} families) to stay safe.", self.db_info.0), "Scan for Threats", (Page::Protection, 0)));
        }
        issues.sort_by(|a, b| b.0.cmp(&a.0));
        let score = (MAX_HEALTH_SCORE - issues.iter().map(|i| i.0).sum::<i64>()).max(0);
        let (word, col) = health_level(score);
        let mut go: Option<(Page, usize)> = None;
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new("Mac Health:").size(24.0).strong());
                ui.label(RichText::new(word).size(24.0).strong().color(col));
                ui.label(RichText::new(format!("{score} / {MAX_HEALTH_SCORE}")).size(15.0).color(MUTED))
                    .on_hover_text("Based on disk space, Trash size, battery condition, time since restart, unused apps and how recently you scanned.");
            });
            ui.label(RichText::new("Your Mac").color(MUTED));
            ui.add_space(6.0);
            let w = (ui.available_width() - 2.0 * 12.0) / 3.0;
            let tile = |ui: &mut Ui, g: Glyph, title: &str, line: String, line_col: Color32, action: Option<&str>| -> bool {
                let mut clicked = false;
                egui::Frame::new()
                    .fill(Color32::WHITE)
                    .stroke(Stroke::new(1.0, LINE))
                    .corner_radius(14.0)
                    .inner_margin(egui::Margin::same(14))
                    .show(ui, |ui| {
                        ui.set_width(w - 30.0);
                        ui.set_height(70.0);
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
                            art::glyph(ui.painter(), g, r, ACCENT);
                            ui.vertical(|ui| {
                                ui.label(RichText::new(title).size(16.0).strong());
                                ui.label(RichText::new(line).size(13.5).color(line_col));
                            });
                        });
                        if let Some(a) = action {
                            ui.with_layout(Layout::right_to_left(Align::Max), |ui| {
                                if ui.add(egui::Button::new(RichText::new(a).size(15.0).strong().color(ACCENT_DARK)).frame(false)).clicked() {
                                    clicked = true;
                                }
                            });
                        }
                    });
                clicked
            };
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                let dc = if free < limit { WARN } else { MUTED };
                if tile(ui, Glyph::Disk, "Startup disk", format!("Available: {}", human(free)), dc, Some("Free Up")) {
                    go = Some((Page::Cleanup, 0));
                }
                if tile(ui, Glyph::Grid, "Memory", format!("Used: {:.0}%", mem * 100.0), if mem > MEMORY_HIGH { WARN } else { MUTED }, Some("Optimize")) {
                    go = Some((Page::Performance, 3));
                }
                tile(ui, Glyph::Chart, "CPU", format!("Load: {:.0}% · up {} days", cpu * 100.0, uptime_days), if uptime_days > UPTIME_WARN_DAYS { WARN } else { MUTED }, None);
            });
            ui.add_space(8.0);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                let (bt, bc) = match &self.battery {
                    Some((pct, st)) => (
                        format!("{pct}% · {st}{}", battery_cond.as_deref().map(|c| format!(" · {c}")).unwrap_or_default()),
                        if battery_bad { BAD } else if *pct < 20 && st == "On battery" { WARN } else { MUTED },
                    ),
                    None => ("No battery (power adapter)".to_string(), MUTED),
                };
                tile(ui, Glyph::Bolt, "Battery", bt, bc, None);
                let net = self.wifi.clone().unwrap_or_else(|| "Network".to_string());
                tile(ui, Glyph::Cloud, &net, format!("↑ {}   ↓ {}", scan::rate(self.net_rate.0), scan::rate(self.net_rate.1)), MUTED, None);
                let ext = if self.externals.is_empty() {
                    "No devices connected".to_string()
                } else {
                    self.externals.iter().map(|(n, f, _)| format!("{n}: {} free", human(*f))).collect::<Vec<_>>().join(" · ")
                };
                tile(ui, Glyph::Download, "External drives", elide(&ext, 40), MUTED, None);
            });
            ui.add_space(10.0);
            // today's recommendation: the most pressing item first
            let (title, text, button, target) = match issues.first() {
                Some(i) => (i.1.clone(), i.2.clone(), i.3, i.4),
                None => ("All good".to_string(), "Nothing needs your attention right now.".to_string(), "Smart Scan", (Page::Smart, 0)),
            };
            ui.label(RichText::new("Today's recommendation").size(17.0).strong());
            ui.horizontal(|ui| {
                glyph_tile(ui, Glyph::Sparkles, accent, 40.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(title).size(15.5).strong());
                    ui.label(RichText::new(text).color(MUTED));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ghost(ui, button).clicked() {
                        go = Some(target);
                    }
                });
            });
        });
        if let Some((pg, tab)) = go {
            if pg == Page::Performance {
                self.perf_open = true;
                self.perf_tab = tab;
            }
            self.page = pg;
            return;
        }
        ui.add_space(10.0);
        let freed: u64 = self.history.iter().map(|h| h.1).sum();
        ui.horizontal(|ui| {
            ui.label(RichText::new(human(freed)).size(28.0).strong());
            ui.label(RichText::new(format!("cleaned up in total · {}", plural(self.history.len(), "action"))).color(MUTED));
        });
        ui.add_space(6.0);
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            if self.history.is_empty() {
                ui.label(RichText::new("Nothing cleaned yet. What you clean, remove or run shows up here.").color(MUTED));
            }
            egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                for (ts, bytes, what) in self.history.iter().take(200) {
                    ui.horizontal(|ui| {
                        let days = now.saturating_sub(*ts) / 86_400;
                        let when = if days == 0 { "Today".to_string() } else if days == 1 { "Yesterday".to_string() } else { format!("{days} days ago") };
                        ui.add_sized([90.0, 20.0], egui::Label::new(RichText::new(when).size(12.5).color(DIM)));
                        ui.label(RichText::new(what).size(13.5));
                        if *bytes > 0 {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.label(RichText::new(human(*bytes)).size(13.0).color(GOOD)));
                        }
                    });
                }
            });
        });
    }
}

fn agent_row(ui: &mut Ui, it: &mut scan::AgentItem) {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.checkbox(&mut it.selected, "");
            let (col, tag) = match it.verdict {
                Verdict::Threat => (BAD, if it.category.is_empty() { "Threat" } else { it.category.as_str() }),
                Verdict::Review => (WARN, "Review"),
                Verdict::Ok => (GOOD, "OK"),
            };
            let (r, _) = ui.allocate_exact_size(Vec2::new(72.0, 22.0), Sense::hover());
            ui.painter().rect_filled(r, 11.0, alpha(col, 50));
            ui.painter().text(r.center(), Align2::CENTER_CENTER, tag, FontId::proportional(12.0), col);
            ui.vertical(|ui| {
                ui.label(RichText::new(&it.label).size(14.5).strong());
                ui.label(RichText::new(format!("{} · {}", it.place, it.reason)).size(12.0).color(MUTED));
                if !it.program.is_empty() {
                    ui.label(RichText::new(elide(&it.program, 90)).size(11.5).color(DIM).monospace());
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ghost(ui, "Show in Finder").clicked() {
                    reveal(&it.path);
                }
            });
        });
    });
}

fn category_tag(ui: &mut Ui, c: &str, n: usize) {
    let col = match c {
        "Trojan" => BAD,
        "Spyware" => Color32::from_rgb(176, 48, 118),
        "Miner" => WARN,
        "Adware" => Color32::from_rgb(178, 90, 18),
        _ => MUTED,
    };
    let text = format!("{n} {c}");
    let g = ui.painter().layout_no_wrap(text, FontId::proportional(13.5), col);
    let (r, _) = ui.allocate_exact_size(Vec2::new(g.size().x + 22.0, 24.0), Sense::hover());
    ui.painter().rect_filled(r, 12.0, alpha(col, 45));
    ui.painter().galley(Pos2::new(r.left() + 11.0, r.center().y - g.size().y / 2.0), g, col);
}

/// Column names above a list of `file_row`s (same widths, so they line up).
fn file_header(ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.set_height(18.0);
        ui.add_space(26.0);
        ui.label(RichText::new("Name").size(11.5).color(DIM));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(70.0);
            ui.add_sized([86.0, 16.0], egui::Label::new(RichText::new("Size").size(11.5).color(DIM)));
            ui.add_sized([86.0, 16.0], egui::Label::new(RichText::new("Last opened").size(11.5).color(DIM)));
            ui.add_sized([76.0, 16.0], egui::Label::new(RichText::new("Kind").size(11.5).color(DIM)));
        });
    });
    ui.separator();
}

fn file_row(ui: &mut Ui, selected: &mut bool, name: &str, path: &std::path::Path, size: u64, age_days: u64, kind: &str) {
    ui.horizontal(|ui| {
        ui.set_height(22.0);
        ui.checkbox(selected, "");
        ui.label(RichText::new(elide(name, 60)).size(13.0)).on_hover_text(short(path));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.small_button("Reveal").clicked() {
                reveal(path);
            }
            ui.add_space(16.0);
            ui.add_sized([86.0, 18.0], egui::Label::new(RichText::new(human(size)).size(13.0)));
            let col = if age_days > OLD_FILE_DAYS { WARN } else { MUTED };
            ui.add_sized([86.0, 18.0], egui::Label::new(RichText::new(age_text(age_days)).size(12.5).color(col)));
            ui.add_sized([76.0, 18.0], egui::Label::new(RichText::new(kind).size(12.5).color(MUTED)));
        });
    });
}

/// Packs one circle per item into `rect`, area following size, biggest in the middle.
/// Returns (item index, centre, radius).
pub fn bubble_map(items: &[(usize, f64)], rect: Rect) -> Vec<(usize, Pos2, f32)> {
    if items.is_empty() || rect.width() < 10.0 {
        return vec![];
    }
    let total: f64 = items.iter().map(|i| i.1).sum();
    // circles fill about 55% of the area
    let scale = (rect.width() * rect.height()) as f64 * 0.55 / total.max(1.0);
    let mut placed: Vec<(usize, Pos2, f32)> = vec![];
    let c0 = rect.center();
    for &(idx, size) in items {
        let r = ((size * scale / std::f64::consts::PI).sqrt() as f32).max(6.0).min(rect.height() * 0.45);
        if placed.is_empty() {
            placed.push((idx, c0, r));
            continue;
        }
        // walk a spiral outwards until the circle fits without overlapping
        let mut found = None;
        let mut a = 0.0f32;
        let mut d = 0.0f32;
        while d < rect.width() {
            let p = c0 + Vec2::angled(a) * d;
            let inside = p.x - r >= rect.left() && p.x + r <= rect.right() && p.y - r >= rect.top() && p.y + r <= rect.bottom();
            if inside && placed.iter().all(|(_, q, rq)| p.distance(*q) >= r + rq + 4.0) {
                found = Some(p);
                break;
            }
            a += 0.35;
            d += 0.9;
        }
        if let Some(p) = found {
            placed.push((idx, p, r));
        }
    }
    placed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_group(i: usize, size: u64) -> bool {
        let (_, lo, hi) = SIZE_GROUPS[i];
        size > lo && size <= hi
    }

    #[test]
    fn size_groups() {
        assert_eq!(LARGE_FILE_BYTES, 50_000_001);
        assert!(!in_group(1, 50_000_000) && in_group(1, 50_000_001) && in_group(1, 500_000_000));
        assert!(!in_group(2, 500_000_000) && in_group(2, 500_000_001) && in_group(2, 1_000_000_000));
        assert!(!in_group(3, 1_000_000_000) && in_group(3, 1_000_000_001));
        // every large file is in exactly one of the three groups
        for s in [50_000_001u64, 500_000_000, 500_000_001, 1_000_000_000, 1_000_000_001, 9_000_000_000] {
            assert_eq!((1..4).filter(|&i| in_group(i, s)).count(), 1, "size {s}");
        }
    }

    #[test]
    fn age_filters() {
        assert!(age_matches(0, 0) && age_matches(0, 9999));
        assert!(age_matches(-7, 6) && !age_matches(-7, 7));
        assert!(!age_matches(7, 7) && age_matches(7, 8));
        assert!(!age_matches(30, 30) && age_matches(30, 31));
        assert!(!age_matches(182, 182) && age_matches(182, 183));
        assert!(!age_matches(365, 365) && age_matches(365, 366));
        assert_eq!((UNUSED_APP_DAYS, OLD_FILE_DAYS, OLD_DOWNLOAD_DAYS, FRESH_DUPLICATE_DAYS), (180, 365, 90, 90));
        assert_eq!(TRASH_ALERT_BYTES, 2_000_000_000);
    }

    #[test]
    fn health_levels() {
        assert_eq!(health_level(100).0, "Excellent");
        assert_eq!(health_level(90).0, "Excellent");
        assert_eq!(health_level(89).0, "Good");
        assert_eq!(health_level(60).0, "Good");
        assert_eq!(health_level(59).0, "Fair");
        assert_eq!(health_level(40).0, "Fair");
        assert_eq!(health_level(39).0, "Requires Attention");
        assert_eq!(health_level(1).0, "Requires Attention");
        assert_eq!(health_level(0).0, "Critical");
        assert_eq!(health_level(-20).0, "Critical");
    }
}
