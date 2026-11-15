use crate::app::{CleanerApp, Page, Pending, elide};
use crate::art;
use crate::pages::{junk_glyph, plural};
use crate::scan::{self, Job, JunkKind, human, short};
use crate::ui_kit::*;
use eframe::egui::{
    self, Align, Align2, Color32, FontId, Layout, Pos2, Rect, RichText, Sense, Ui, Vec2,
};
use std::path::PathBuf;

impl CleanerApp {
    pub fn apply_ignore(&mut self) {
        if self.settings.ignore.is_empty() {
            return;
        }
        let s = self.settings.clone();
        if let Some(j) = &mut self.junk {
            for g in j.iter_mut() {
                g.items.retain(|i| !s.protects(&i.path));
            }
        }
        if let Some(b) = &mut self.big {
            b.retain(|f| !s.protects(&f.path));
        }
        if let Some(d) = &mut self.downloads {
            d.retain(|f| !s.protects(&f.path));
        }
        if let Some(d) = &mut self.dups {
            for g in d.iter_mut() {
                g.files.retain(|f| !s.protects(&f.path));
            }
            d.retain(|g| g.files.len() > 1);
        }
        if let Some(m) = &mut self.similar {
            for g in m.iter_mut() {
                g.files.retain(|f| !s.protects(&f.path));
            }
            m.retain(|g| g.files.len() > 1);
        }
    }

    pub fn cleanup_manager(&mut self, ui: &mut Ui) {
        let accent = Page::Cleanup.accent();
        let busy = self.action_job.is_some();
        let Some(mut groups) = self.junk.take() else {
            return;
        };
        let area = ui.max_rect();
        let total: u64 = groups
            .iter()
            .filter(|g| g.kind != JunkKind::Trash)
            .map(|g| g.selected_size())
            .sum();
        let count: usize = groups
            .iter()
            .filter(|g| g.kind != JunkKind::Trash)
            .map(|g| g.items.iter().filter(|i| i.selected).count())
            .sum();
        let found: u64 = groups
            .iter()
            .filter(|g| g.kind != JunkKind::Trash)
            .map(|g| g.total())
            .sum();
        let trash_size: u64 = groups
            .iter()
            .filter(|g| g.kind == JunkKind::Trash)
            .map(|g| g.total())
            .sum();
        let (mut rescan, mut empty) = (false, false);
        let mut ignore: Option<PathBuf> = None;
        self.clean_sel = self.clean_sel.min(groups.len().saturating_sub(1));

        ui.horizontal(|ui| {
            ui.label(RichText::new(human(found)).size(34.0).strong().color(TEXT));
            ui.label(
                RichText::new(format!("of junk found on your {}", scan::computer()))
                    .size(15.0)
                    .color(MUTED),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ghost(ui, "Scan again").clicked() {
                    rescan = true;
                }
            });
        });
        ui.add_space(8.0);

        let h = (ui.available_height() - 76.0).max(160.0);
        let left_w = 264.0;
        ui.horizontal_top(|ui| {
            card_frame().inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                ui.set_width(left_w);
                ui.set_height(h - 16.0);
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for (i, g) in groups.iter().enumerate() {
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(left_w, 52.0), Sense::click());
                    let p = ui.painter();
                    if i == self.clean_sel {
                        p.rect_filled(r, 10.0, Page::Cleanup.tint());
                    } else if resp.hovered() {
                        p.rect_filled(r, 10.0, ink(10));
                    }
                    let tile = Rect::from_center_size(Pos2::new(r.left() + 26.0, r.center().y), Vec2::splat(32.0));
                    p.rect_filled(tile, 9.0, mix(Color32::WHITE, accent, 0.16));
                    art::glyph(p, junk_glyph(g.kind), tile.shrink(7.5), mix(accent, Color32::BLACK, 0.28));
                    let empty_group = g.items.is_empty();
                    p.text(Pos2::new(r.left() + 52.0, r.center().y - 9.0), Align2::LEFT_CENTER, scan::words(g.title), FontId::proportional(14.5), if empty_group { MUTED } else { TEXT });
                    let picked = g.items.iter().filter(|x| x.selected).count();
                    let sub = if g.kind == JunkKind::Trash || picked == 0 { human(g.total()) } else { format!("{} · {} ticked", human(g.total()), picked) };
                    p.text(Pos2::new(r.left() + 52.0, r.center().y + 10.0), Align2::LEFT_CENTER, sub, FontId::proportional(12.0), MUTED);
                    if resp.clicked() {
                        self.clean_sel = i;
                    }
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }
                });
            });

            card_frame().show(ui, |ui| {
                ui.set_width(ui.available_width() - 4.0);
                ui.set_height(h - 32.0);
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                let g = &mut groups[self.clean_sel];
                let is_trash = g.kind == JunkKind::Trash;
                ui.label(RichText::new(scan::words(g.title)).size(18.0).strong());
                ui.label(RichText::new(scan::words(g.desc)).size(13.0).color(MUTED));
                if let Some(n) = &g.note {
                    ui.label(RichText::new(n).size(12.0).color(WARN));
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if is_trash {
                        if trash_size > 0 && !busy && ghost(ui, &scan::words("Empty Trash")).clicked() {
                            empty = true;
                        }
                    } else {
                        ui.label(RichText::new("Select:").size(13.0).color(MUTED));
                        if ui.link(RichText::new("All").size(13.0)).clicked() {
                            for it in g.items.iter_mut() {
                                it.selected = !it.in_use;
                            }
                        }
                        if ui.link(RichText::new("None").size(13.0)).clicked() {
                            for it in g.items.iter_mut() {
                                it.selected = false;
                            }
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        egui::ComboBox::from_id_salt("clean_sort").width(96.0).selected_text(if self.clean_sort == 0 { "By size" } else { "By name" }).show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.clean_sort, 0, "By size");
                            ui.selectable_value(&mut self.clean_sort, 1, "By name");
                        });
                        ui.add(egui::TextEdit::singleline(&mut self.clean_query).hint_text("Search").desired_width(150.0));
                    });
                });
                ui.separator();
                let q = self.clean_query.trim().to_lowercase();
                let mut idx: Vec<usize> = (0..g.items.len()).filter(|&i| q.is_empty() || g.items[i].name.to_lowercase().contains(&q)).collect();
                if self.clean_sort == 1 {
                    idx.sort_by_key(|&i| g.items[i].name.to_lowercase());
                } else {
                    idx.sort_by_key(|&i| std::cmp::Reverse(g.items[i].size));
                }
                if idx.is_empty() {
                    ui.add_space(18.0);
                    ui.label(RichText::new(if g.items.is_empty() { "Nothing found here." } else { "Nothing matches your search." }).color(MUTED));
                }
                egui::ScrollArea::vertical().auto_shrink([false; 2]).show_rows(ui, 26.0, idx.len(), |ui, range| {
                    for k in range {
                        let it = &mut g.items[idx[k]];
                        ui.horizontal(|ui| {
                            ui.set_height(26.0);
                            if !is_trash {
                                ui.checkbox(&mut it.selected, "");
                            }
                            ui.label(RichText::new(elide(&it.name, 52)).size(13.5)).on_hover_text(short(&it.path));
                            if it.in_use {
                                ui.label(RichText::new("in use · quit the app to include").size(11.5).color(WARN));
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.add_sized([70.0, 20.0], egui::Label::new(RichText::new(human(it.size)).size(13.0).color(MUTED)));
                                if !is_trash && ui.add(egui::Button::new(RichText::new("Ignore").size(11.5).color(DIM)).frame(false)).on_hover_text("Never list this item again. Undo it in Settings.").clicked() {
                                    ignore = Some(it.path.clone());
                                }
                            });
                        });
                    }
                });
                });
            });
        });

        let p = ui.painter();
        let line = if count == 0 {
            "No items selected".to_string()
        } else {
            format!("{} selected · {}", plural(count, "item"), human(total))
        };
        p.text(
            Pos2::new(area.left() + 4.0, area.bottom() - 16.0),
            Align2::LEFT_CENTER,
            line,
            FontId::proportional(14.0),
            if count == 0 { MUTED } else { TEXT },
        );
        let clean = !busy && round_button(ui, "Clean", accent);

        let paths: Vec<(PathBuf, u64)> = groups
            .iter()
            .filter(|g| g.kind != JunkKind::Trash)
            .flat_map(|g| {
                g.items
                    .iter()
                    .filter(|i| i.selected)
                    .map(|i| (i.path.clone(), i.size))
            })
            .collect();
        self.junk = Some(groups);
        if rescan {
            self.junk_job = Some(Job::spawn(scan::scan_junk));
        }
        if clean {
            if paths.is_empty() {
                self.toast("Tick something to clean first.");
            } else {
                self.pending = Some(Pending::Trash {
                    title: format!("Clean up {}?", human(total)),
                    paths,
                });
            }
        }
        if empty {
            self.pending = Some(Pending::EmptyTrash { size: trash_size });
        }
        if let Some(path) = ignore {
            if self.settings.add_ignore(&path.to_string_lossy()) {
                self.settings.save();
                self.apply_ignore();
                self.toast(format!(
                    "{} won't be listed again. Undo it in Settings.",
                    elide(&short(&path), 40)
                ));
            }
        }
    }

    pub fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let before = self.settings.clone();
        let mut close = false;
        let mut remove: Option<usize> = None;
        let mut add = false;
        egui::Modal::new(egui::Id::unique("settings")).show(ctx, |ui| {
            ui.set_width(540.0);
            ui.set_min_height(380.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Settings").size(22.0).strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if pill(ui, "Done", ACCENT).clicked() {
                        close = true;
                    }
                });
            });
            ui.add_space(6.0);
            let labels: Vec<String> = ["Scanning", "Alerts", "Ignore List"].iter().map(|s| s.to_string()).collect();
            self.settings_tab = self.settings_tab.min(labels.len() - 1);
            tabs(ui, &labels, &mut self.settings_tab);
            ui.add_space(12.0);
            match self.settings_tab {
                0 => {
                    ui.label(RichText::new("What Run does after a Smart Scan").size(16.0).strong());
                    ui.label(RichText::new("Smart Scan always checks everything. These choose what its Run button acts on.").size(12.5).color(MUTED));
                    ui.add_space(6.0);
                    ui.checkbox(&mut self.settings.smart_cleanup, scan::words("Cleanup: move the ticked junk to the Trash"));
                    ui.checkbox(&mut self.settings.smart_performance, "Performance: run the ticked maintenance tasks");
                    ui.add_space(10.0);
                    ui.label(RichText::new("Threats, apps and personal files are never removed by Run. Review those yourself.").size(12.5).color(MUTED));
                }
                1 => {
                    ui.label(RichText::new("When Activity warns you").size(16.0).strong());
                    ui.add_space(6.0);
                    egui::Grid::new("alerts").num_columns(2).spacing([16.0, 12.0]).show(ui, |ui| {
                        ui.label(scan::words("Trash is bigger than"));
                        egui::ComboBox::from_id_salt("set_trash").width(120.0).selected_text(format!("{} GB", self.settings.trash_alert_gb)).show_ui(ui, |ui| {
                            for v in [1u64, 2, 5, 10, 20] {
                                ui.selectable_value(&mut self.settings.trash_alert_gb, v, format!("{v} GB"));
                            }
                        });
                        ui.end_row();
                        ui.label("Free disk space is under");
                        let shown = if self.settings.low_disk_gb == 0 { format!("Automatic ({})", human(scan::low_disk_limit(self.disk_total))) } else { format!("{} GB", self.settings.low_disk_gb) };
                        egui::ComboBox::from_id_salt("set_disk").width(160.0).selected_text(shown).show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.settings.low_disk_gb, 0, "Automatic");
                            for v in [5u64, 10, 20, 50, 100] {
                                ui.selectable_value(&mut self.settings.low_disk_gb, v, format!("{v} GB"));
                            }
                        });
                        ui.end_row();
                        ui.label("Remind me to scan after");
                        egui::ComboBox::from_id_salt("set_remind").width(120.0).selected_text(plural(self.settings.scan_reminder_days as usize, "day")).show_ui(ui, |ui| {
                            for v in [7u64, 14, 30, 90] {
                                ui.selectable_value(&mut self.settings.scan_reminder_days, v, format!("{v} days"));
                            }
                        });
                        ui.end_row();
                    });
                    ui.add_space(10.0);
                    ui.label(RichText::new("Automatic warns at about a tenth of the disk.").size(12.5).color(MUTED));
                }
                _ => {
                    ui.label(RichText::new("Files and folders Cleanerio leaves alone").size(16.0).strong());
                    ui.label(RichText::new("Anything here, and everything inside it, is left out of Cleanup and My Files.").size(12.5).color(MUTED));
                    ui.add_space(6.0);
                    egui::ScrollArea::vertical().max_height(190.0).auto_shrink([false, true]).show(ui, |ui| {
                        if self.settings.ignore.is_empty() {
                            ui.label(RichText::new("Nothing is ignored yet.").color(MUTED));
                        }
                        for (i, e) in self.settings.ignore.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(elide(&short(&crate::settings::expand(e)), 56)).size(13.5)).on_hover_text(e);
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui.small_button("Remove").clicked() {
                                        remove = Some(i);
                                    }
                                });
                            });
                        }
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let r = ui.add(egui::TextEdit::singleline(&mut self.ignore_input).hint_text("~/Projects/keep-this").desired_width(380.0));
                        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if ghost(ui, "Add").clicked() || enter {
                            add = true;
                        }
                    });
                    ui.label(RichText::new("Removing an entry takes effect at the next scan.").size(12.0).color(MUTED));
                }
            }
        });
        if let Some(i) = remove {
            self.settings.ignore.remove(i);
        }
        if add {
            let entry = std::mem::take(&mut self.ignore_input);
            if !self.settings.add_ignore(&entry) && !entry.trim().is_empty() {
                self.toast("That entry is already on the list.");
            }
        }
        if self.settings != before {
            self.settings.save();
            self.apply_ignore();
        }
        if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.settings_open = false;
        }
    }
}
