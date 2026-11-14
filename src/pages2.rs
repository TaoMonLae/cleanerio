//! Screens for the features in extra.rs: similar pictures, privacy items, app
//! permissions, Wi-Fi networks, app updates, leftovers of deleted apps, browser tabs, the
//! duplicate finder shortcut and screen cleaning mode.

use crate::app::{elide, reveal, CleanerApp, Page, Pending};
use crate::art::Glyph;
use crate::extra;
use crate::pages::glyph_tile;
use crate::scan::{human, short, Job};
use crate::ui_kit::*;
use eframe::egui::{self, Align, Align2, Color32, FontId, Layout, Pos2, RichText, Sense, Ui, Vec2};
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn empty_note(ui: &mut Ui, text: &str) {
    ui.add_space(30.0);
    ui.vertical_centered(|ui| ui.label(RichText::new(text).size(16.0).color(MUTED)));
}

impl CleanerApp {
    // ------------------------------------------------------------------ my files: similar pictures

    pub fn similar_tab(&mut self, ui: &mut Ui, h: f32) -> Vec<(PathBuf, u64)> {
        let accent = Page::Clutter.accent();
        let groups = self.similar.get_or_insert_with(Vec::new);
        let mut auto = false;
        ui.horizontal(|ui| {
            let extra_size: u64 = groups.iter().map(|g| g.files.iter().skip(1).map(|f| f.size).sum::<u64>()).sum();
            ui.label(RichText::new(format!("{} of look-alike pictures · {} in the extra ones", if groups.len() == 1 { "1 group".to_string() } else { format!("{} groups", groups.len()) }, human(extra_size))).size(16.0).strong());
            if !groups.is_empty() && ghost(ui, "Keep the biggest, select the rest").clicked() {
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
                ui.label(RichText::new("No similar pictures found in Desktop, Documents, Downloads, Movies, Music or Pictures.").color(MUTED));
            }
            for g in groups.iter_mut().take(200) {
                card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        glyph_tile(ui, Glyph::Copy, accent, 28.0);
                        ui.label(RichText::new(format!("{} similar pictures", g.files.len())).size(15.0).strong());
                    });
                    for (k, f) in g.files.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.add_space(30.0);
                            ui.checkbox(&mut f.selected, "");
                            let tag = if k == 0 { "  · best to keep" } else { "" };
                            ui.label(RichText::new(format!("{}{tag}", elide(&short(&f.path), 70))).size(12.5)).on_hover_text(f.path.display().to_string());
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.small_button("Show").clicked() {
                                    let _ = std::process::Command::new("open").arg(&f.path).spawn();
                                }
                                ui.label(RichText::new(human(f.size)).size(12.0).color(DIM));
                            });
                        });
                    }
                });
                ui.add_space(6.0);
            }
        });
        // never take every picture of a group
        groups
            .iter()
            .flat_map(|g| {
                let all = g.files.iter().all(|f| f.selected);
                g.files.iter().skip(if all { 1 } else { 0 }).filter(|f| f.selected).map(|f| (f.path.clone(), f.size))
            })
            .collect()
    }

    // ------------------------------------------------------------------ protection: privacy

    pub fn privacy_tab(&mut self, ui: &mut Ui) {
        let accent = Page::Protection.accent();
        if let Some(j) = &self.privacy_job {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new());
                ui.label(RichText::new(j.status()).color(MUTED));
            });
            return;
        }
        if self.privacy.is_none() {
            self.privacy_job = Some(Job::spawn(extra::privacy_items));
            self.wifi_list = Some(extra::wifi_networks());
            return;
        }
        let busy = self.action_job.is_some();
        ui.label(RichText::new("Traces of what you did online and on your Mac. Quit the browser first. Removed items go to the Trash and can still be read there until you empty it.").color(MUTED));
        ui.add_space(6.0);
        let h = ui.available_height() - 80.0;
        let mut forget: Option<String> = None;
        let items = self.privacy.as_mut().unwrap();
        egui::ScrollArea::vertical().max_height(h).auto_shrink([false; 2]).show(ui, |ui| {
            if items.is_empty() {
                ui.label(RichText::new("No browser traces found (or macOS keeps them private until this app has Full Disk Access).").color(MUTED));
            }
            let mut last = "";
            for it in items.iter_mut() {
                if it.browser != last {
                    last = it.browser;
                    ui.add_space(6.0);
                    ui.label(RichText::new(it.browser).size(17.0).strong());
                }
                ui.horizontal(|ui| {
                    ui.checkbox(&mut it.selected, "");
                    ui.label(RichText::new(it.what).size(14.5));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(human(it.size)).color(MUTED));
                    });
                });
            }
            if let Some(w) = &self.wifi_list {
                ui.add_space(10.0);
                ui.label(RichText::new(format!("Saved Wi-Fi networks ({})", w.len())).size(17.0).strong());
                ui.label(RichText::new("Your Mac joins these on its own. Forget ones you no longer use (asks for your password).").size(12.5).color(MUTED));
                for n in w {
                    ui.horizontal(|ui| {
                        glyph_tile(ui, Glyph::Cloud, accent, 24.0);
                        ui.label(n);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.small_button("Forget").clicked() {
                                forget = Some(n.clone());
                            }
                        });
                    });
                }
            }
        });
        // the password window is waited for in the background, so this window stays usable
        if let Some(n) = forget {
            if self.wifi_job.is_none() {
                self.wifi_job = Some(Job::spawn(move |_| {
                    let r = extra::forget_wifi(&n);
                    (n, r)
                }));
            }
        }
        if let Some((n, r)) = self.wifi_job.as_ref().and_then(|j| j.take()) {
            self.wifi_job = None;
            match r {
                Ok(()) => {
                    self.toast(format!("Forgot “{n}”."));
                    self.wifi_list = Some(extra::wifi_networks());
                }
                Err(e) if e.contains("-128") => {}
                Err(e) => self.toast(format!("Couldn't forget it: {}", elide(&e, 60))),
            }
        }
        if self.wifi_job.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(200));
        }
        let chosen: Vec<extra::PrivacyItem> = self.privacy.iter().flatten().filter(|i| i.selected).cloned().collect();
        if !chosen.is_empty() && !busy && round_button(ui, "Remove", accent) {
            let open: Vec<&str> = chosen.iter().map(|i| i.browser).filter(|b| extra::browser_running(b)).collect();
            if let Some(b) = open.first() {
                self.toast(format!("Quit {b} first, then try again."));
            } else {
                let paths: Vec<(PathBuf, u64)> = chosen.iter().flat_map(|i| i.paths.iter().map(|p| (p.clone(), 0))).collect();
                self.pending = Some(Pending::Trash { title: format!("Remove {} privacy items?", chosen.len()), paths });
            }
        }
    }

    // ------------------------------------------------------------------ protection: permissions

    pub fn permissions_tab(&mut self, ui: &mut Ui) {
        let accent = Page::Protection.accent();
        if self.perms.is_none() {
            self.perms = Some(extra::granted_permissions());
        }
        ui.label(RichText::new("Which apps may use your camera, microphone, screen, files and more.").color(MUTED));
        ui.add_space(6.0);
        let mut reset: Option<(String, String)> = None;
        egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
            match self.perms.as_ref().unwrap() {
                Ok(list) if !list.is_empty() => {
                    let mut last = String::new();
                    for (cat, app, allowed) in list {
                        if *cat != last {
                            last = cat.clone();
                            ui.add_space(6.0);
                            ui.label(RichText::new(cat).size(16.0).strong());
                        }
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.label(RichText::new(app).size(13.5));
                            ui.label(RichText::new(if *allowed { "allowed" } else { "denied" }).size(12.0).color(if *allowed { GOOD } else { DIM }));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if *allowed && ui.small_button("Take back").on_hover_text("The app will ask again next time it needs this").clicked() {
                                    reset = Some((cat.clone(), app.clone()));
                                }
                            });
                        });
                    }
                }
                other => {
                    if let Err(e) = other {
                        ui.label(RichText::new(e).color(WARN));
                    }
                    ui.label(RichText::new("Open each kind in System Settings to see and change which apps have it:").color(MUTED));
                    ui.add_space(6.0);
                    let cols = 3;
                    let w = (ui.available_width() - 2.0 * 10.0) / cols as f32;
                    for row in extra::PERMISSIONS.chunks(cols) {
                        ui.horizontal(|ui| {
                            for (name, anchor, _) in row {
                                let b = egui::Button::new(RichText::new(*name).size(14.0)).fill(alpha(Color32::WHITE, 16)).corner_radius(12.0).min_size(Vec2::new(w - 4.0, 36.0));
                                if ui.add(b).clicked() {
                                    extra::open_privacy_pane(anchor);
                                }
                            }
                        });
                    }
                    ui.add_space(8.0);
                    if pill(ui, "Open Full Disk Access", accent).clicked() {
                        extra::open_privacy_pane("Privacy_AllFiles");
                    }
                }
            }
        });
        if let Some((cat, app)) = reset {
            match extra::reset_permission(&cat, &app) {
                Ok(()) => {
                    self.toast(format!("{app} will ask again for {cat}."));
                    self.perms = None;
                }
                Err(e) => self.toast(format!("Couldn't change it: {}", elide(&e, 60))),
            }
        }
    }

    // ------------------------------------------------------------------ applications: updates

    pub fn updates_tab(&mut self, ui: &mut Ui) {
        let accent = Page::Apps.accent();
        if let Some(j) = &self.updates_job {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new());
                ui.label(RichText::new(j.status()).color(MUTED));
            });
            return;
        }
        let Some(list) = &self.updates else {
            ui.label(RichText::new("Checks the App Store and each app's own update feed for newer versions (needs the internet).").color(MUTED));
            ui.add_space(8.0);
            if pill(ui, "Check for updates", accent).clicked() {
                self.updates_job = Some(Job::spawn(extra::app_updates));
            }
            return;
        };
        let mut again = false;
        ui.horizontal(|ui| {
            ui.label(RichText::new(if list.is_empty() { "All your apps are up to date.".to_string() } else { format!("{} can be updated", list.len()) }).size(18.0).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ghost(ui, "Check again").clicked() {
                    again = true;
                }
            });
        });
        ui.label(RichText::new("Only apps from the App Store or with their own update feed can be checked.").size(12.5).color(MUTED));
        ui.add_space(6.0);
        egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
            for u in list {
                card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        glyph_tile(ui, Glyph::Download, accent, 32.0);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&u.name).size(15.0).strong());
                            ui.label(RichText::new(format!("{} → {} · {}", u.current, u.latest, u.source)).size(12.5).color(MUTED));
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if extra::safe_link(&u.link) && pill(ui, "Get update", accent).clicked() {
                                let _ = std::process::Command::new("open").arg(&u.link).spawn();
                            }
                            if ghost(ui, "Open app").clicked() {
                                let _ = std::process::Command::new("open").arg(&u.path).spawn();
                            }
                        });
                    });
                });
                ui.add_space(6.0);
            }
        });
        if again {
            self.updates_job = Some(Job::spawn(extra::app_updates));
        }
    }

    // ------------------------------------------------------------------ applications: leftovers

    pub fn leftovers_tab(&mut self, ui: &mut Ui) {
        let accent = Page::Apps.accent();
        if let Some(j) = &self.orphans_job {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new());
                ui.label(RichText::new(j.status()).color(MUTED));
            });
            return;
        }
        if self.orphans.is_none() {
            self.orphans_job = Some(Job::spawn(extra::app_leftovers));
            return;
        }
        let busy = self.action_job.is_some();
        let list = self.orphans.as_mut().unwrap();
        let total: u64 = list.iter().map(|e| e.1.iter().map(|x| x.1).sum::<u64>()).sum();
        ui.label(RichText::new(format!("{} from {} apps that are no longer on this Mac", human(total), list.len())).size(18.0).strong());
        ui.label(RichText::new("Settings and caches named after apps that Spotlight can't find anywhere. Look before removing.").size(12.5).color(MUTED));
        ui.add_space(6.0);
        let h = ui.available_height() - 80.0;
        egui::ScrollArea::vertical().max_height(h).auto_shrink([false; 2]).show(ui, |ui| {
            if list.is_empty() {
                ui.label(RichText::new("No leftovers found.").color(MUTED));
            }
            for (id, paths, sel) in list.iter_mut() {
                ui.horizontal(|ui| {
                    ui.checkbox(sel, "");
                    ui.label(RichText::new(id.as_str()).size(14.0)).on_hover_text(paths.iter().map(|p| short(&p.0)).collect::<Vec<_>>().join("\n"));
                    ui.label(RichText::new(format!("{} places", paths.len())).size(12.0).color(DIM));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.small_button("Reveal").clicked() {
                            if let Some(p) = paths.first() {
                                reveal(&p.0);
                            }
                        }
                        ui.label(RichText::new(human(paths.iter().map(|x| x.1).sum())).color(MUTED));
                    });
                });
            }
        });
        let paths: Vec<(PathBuf, u64)> = list.iter().filter(|e| e.2).flat_map(|e| e.1.clone()).collect();
        if !paths.is_empty() && !busy && round_button(ui, "Remove", accent) {
            self.pending = Some(Pending::Trash { title: format!("Remove leftovers ({})?", human(paths.iter().map(|p| p.1).sum())), paths });
        }
    }

    // ------------------------------------------------------------------ tools: browser care

    pub fn browser_care(&mut self, ui: &mut Ui) {
        let accent = Page::Tools.accent();
        ui.label(RichText::new("See the tabs open in Safari and Chrome, spot duplicates and close what you don't need. Only browsers that are already open are asked; macOS may ask you to allow this once.").color(MUTED));
        ui.add_space(6.0);
        let mut load = self.tabs_list.is_none() && ui.button("Show my tabs").clicked();
        let mut close: Option<Vec<extra::Tab>> = None;
        if let Some(r) = &mut self.tabs_list {
            match r {
                Err(e) => {
                    ui.label(RichText::new(format!("Couldn't read the tabs: {}", elide(e, 120))).color(WARN));
                    load = ghost(ui, "Try again").clicked();
                }
                Ok(tabs) => {
                    let dups = tabs.iter().filter(|t| t.duplicate).count();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{} open tabs · {} duplicates", tabs.len(), dups)).size(17.0).strong());
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ghost(ui, "Refresh").clicked() {
                                load = true;
                            }
                        });
                    });
                    let h = ui.available_height() - 80.0;
                    egui::ScrollArea::vertical().max_height(h).auto_shrink([false; 2]).show(ui, |ui| {
                        if tabs.is_empty() {
                            ui.label(RichText::new("No Safari or Chrome windows are open.").color(MUTED));
                        }
                        for t in tabs.iter_mut() {
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut t.selected, "");
                                ui.label(RichText::new(&t.browser).size(12.0).color(DIM));
                                ui.label(RichText::new(elide(if t.title.is_empty() { &t.url } else { &t.title }, 70)).size(13.5)).on_hover_text(&t.url);
                                if t.duplicate {
                                    ui.label(RichText::new("duplicate").size(12.0).color(WARN));
                                }
                            });
                        }
                    });
                    let chosen: Vec<extra::Tab> = tabs.iter().filter(|t| t.selected).cloned().collect();
                    if !chosen.is_empty() && round_button(ui, "Close tabs", accent) {
                        close = Some(chosen);
                    }
                }
            }
        }
        if let Some(c) = close {
            let asked = c.len();
            match extra::close_tabs(c) {
                Ok(n) if n == asked => self.toast(format!("Closed {n} tabs.")),
                Ok(n) => self.toast(format!("Closed {n} of {asked} tabs. The others had moved since the list was made, so they were left alone.")),
                Err(e) => self.toast(format!("Couldn't close them: {}", elide(&e, 60))),
            }
            load = true;
        }
        if load {
            self.tabs_list = Some(extra::browser_tabs());
        }
    }

    // ------------------------------------------------------------------ tools: duplicate finder and screen cleaning

    pub fn duplicate_finder_card(&mut self, ui: &mut Ui) {
        let accent = Page::Clutter.accent();
        ui.label(RichText::new(format!(
            "Finds files that are exact copies of each other (same size, then the same content), {} or bigger, and look-alike pictures.",
            human(crate::scan::DUPLICATE_MIN_BYTES)
        ))
        .color(MUTED));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("Look in");
            let mut opts = vec![crate::scan::home()];
            opts.extend(crate::scan::clutter_roots());
            egui::ComboBox::from_id_salt("dup_root").width(220.0).selected_text(crate::scan::file_name(&self.clutter_root)).show_ui(ui, |ui| {
                for o in opts {
                    let l = crate::scan::file_name(&o);
                    ui.selectable_value(&mut self.clutter_root, o, l);
                }
            });
        });
        ui.add_space(8.0);
        if pill(ui, "Find duplicates", accent).clicked() {
            self.clutter_tab = 1;
            self.page = Page::Clutter;
            self.start_clutter();
        }
    }

    pub fn screen_cleaning_card(&mut self, ui: &mut Ui) {
        let accent = Page::Tools.accent();
        ui.label(RichText::new("Turns the whole screen black so you can wipe it and see every smudge. Click three times or press Esc to finish.").color(MUTED));
        ui.add_space(8.0);
        if pill(ui, "Start Screen Cleaning", accent).clicked() {
            self.screen_clean = Some((Instant::now(), 0));
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        }
    }

    /// Draws screen cleaning mode over everything. Returns true while it is on.
    pub fn screen_cleaning_overlay(&mut self, ui: &mut Ui) -> bool {
        let Some((start, clicks)) = self.screen_clean else { return false };
        let ctx = ui.ctx().clone();
        let r = ctx.content_rect();
        let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("screen_clean")));
        p.rect_filled(r, 0.0, Color32::BLACK);
        if start.elapsed().as_secs() < 4 {
            p.text(r.center(), Align2::CENTER_CENTER, "Wipe your screen. Click three times or press Esc to finish.", FontId::proportional(18.0), Color32::from_gray(90));
        }
        let resp = ui.interact(r, egui::Id::new("screen_clean_click"), Sense::click());
        let mut n = clicks;
        if resp.clicked() {
            n += 1;
        }
        if n >= 3 || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.screen_clean = None;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        } else {
            self.screen_clean = Some((start, n));
        }
        let _ = Pos2::ZERO;
        true
    }
}
