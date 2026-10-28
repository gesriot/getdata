//! Information pane (current status / data table) and the magnifier.

use eframe::egui::{self, Color32, RichText, Sense, Ui, pos2, vec2};

use crate::app::{App, InfoTab, Mode};
use crate::export::c_g;
use crate::i18n::{tr, trf};
use crate::model::*;

const MAG_PIXELS: i32 = 21;

impl App {
    pub fn info_pane(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.info_tab, InfoTab::Status, tr("Current status"));
            ui.selectable_value(&mut self.info_tab, InfoTab::Data, tr("Data"));
        });
        ui.separator();
        match self.info_tab {
            InfoTab::Status => self.status_tab(ui),
            InfoTab::Data => self.data_tab(ui),
        }
    }

    fn status_tab(&mut self, ui: &mut Ui) {
        let t = self.transform();
        // Current point.
        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(tr("Current point")).strong());
            let hover = self.hover.filter(|p| p.x >= 0.0 && p.y >= 0.0 && self.pixel_at(*p).is_some());
            ui.horizontal(|ui| {
                let c = hover.and_then(|p| self.pixel_at(p));
                Self::color_swatch(ui, c.unwrap_or(Rgb::WHITE));
                ui.vertical(|ui| {
                    let comp = |v: Option<u8>| v.map(|v| v.to_string()).unwrap_or_default();
                    ui.monospace(format!("R: {:>3}", comp(c.map(|c| c.0))));
                    ui.monospace(format!("G: {:>3}", comp(c.map(|c| c.1))));
                    ui.monospace(format!("B: {:>3}", comp(c.map(|c| c.2))));
                });
                if let Some(p) = hover {
                    ui.monospace(format!("({},{})", p.x.floor() as i32, p.y.floor() as i32));
                }
            });
            let scale_word = |log: bool| if log { "(log)" } else { "(linear)" };
            match (&t, hover) {
                (Ok(t), Some(p)) => {
                    let (x, y) = t.to_logical((p.x as f64, p.y as f64));
                    let f = |v: f64| if v != 0.0 && (v.abs() >= 1e5 || v.abs() < 1e-3) { format!("{v:.4e}") } else { format!("{v:.4}") };
                    ui.monospace(format!("{:<9}x: {}", scale_word(self.ws.x_log), f(x)));
                    ui.monospace(format!("{:<9}y: {}", scale_word(self.ws.y_log), f(y)));
                }
                (Ok(_), None) => {
                    ui.monospace(format!("{:<9}x: n/a", scale_word(self.ws.x_log)));
                    ui.monospace(format!("{:<9}y: n/a", scale_word(self.ws.y_log)));
                }
                (Err(e), _) => {
                    ui.label(RichText::new(Self::scale_error_text(*e)).color(ui.visuals().warn_fg_color));
                }
            }
        });
        // Current line.
        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(tr("Current line")).strong());
            ui.horizontal(|ui| {
                let cur = self.ws.current_line;
                let caption = self.ws.lines[cur].caption.clone();
                let mut sel = cur;
                egui::ComboBox::from_id_salt("cur_line").selected_text(caption).width(ui.available_width() - 30.0).show_ui(ui, |ui| {
                    for (i, l) in self.ws.lines.iter().enumerate() {
                        ui.selectable_value(&mut sel, i, &l.caption);
                    }
                });
                if sel != cur {
                    self.ws.current_line = sel;
                    self.selected.clear();
                    self.moving = None;
                    self.reorder_next = 0;
                }
                Self::color_swatch(ui, self.ws.line().pt_color).on_hover_text(tr("Points color:"));
            });
            ui.horizontal(|ui| {
                let l = self.ws.line().clone();
                for (name, c, line) in [(tr("Line"), l.line_color, true), (tr("Background"), l.bk_color, false)] {
                    ui.vertical(|ui| {
                        ui.label(name);
                        ui.horizontal(|ui| {
                            let r = Self::color_swatch(ui, c).on_hover_text(if line { tr("Set line color") } else { tr("Set background color") });
                            if r.clicked() {
                                self.set_mode(Mode::PickColor { line });
                            }
                            ui.vertical(|ui| {
                                ui.small(format!("R: {}", c.0));
                                ui.small(format!("G: {}", c.1));
                                ui.small(format!("B: {}", c.2));
                            });
                        });
                    });
                }
                ui.vertical(|ui| {
                    if ui.button(tr("Properties")).clicked() {
                        self.open_line_props(Some(self.ws.current_line));
                    }
                    if ui.button(tr("Add line")).clicked() {
                        self.open_line_props(None);
                    }
                });
            });
            egui::Grid::new("counts").num_columns(2).show(ui, |ui| {
                ui.label(tr("data points:"));
                ui.label(self.ws.line().points.len().to_string());
                ui.end_row();
                ui.label(tr("total data points:"));
                ui.label(self.ws.total_points().to_string());
                ui.end_row();
            });
        });
        // Coordinate system.
        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(tr("Coordinate system")).strong());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    for id in AxisId::ALL {
                        ui.horizontal(|ui| {
                            let set = self.ws.axis(id).is_some();
                            let (r, p) = ui.allocate_painter(vec2(10.0, 10.0), Sense::hover());
                            let c = if set { Color32::from_rgb(0, 170, 0) } else { Color32::from_rgb(200, 0, 0) };
                            p.rect_filled(r.rect.shrink(1.0), 1.0, c);
                            ui.label(id.name());
                        });
                    }
                });
                ui.vertical(|ui| {
                    if ui.add_enabled(self.has_image(), egui::Button::new(tr("Default axes"))).clicked() {
                        self.set_default_axes();
                    }
                    if ui.button(tr("Adjust the scale")).clicked() {
                        self.open_adjust_scale();
                    }
                });
            });
        });
    }

    fn data_tab(&mut self, ui: &mut Ui) {
        let t = self.transform();
        let n = self.ws.line().points.len();
        ui.label(format!("{} – {}", self.ws.line().caption, trf("data points: {}", &[&n])));
        if let Err(e) = &t {
            ui.label(RichText::new(Self::scale_error_text(*e)).color(ui.visuals().warn_fg_color));
        }
        let (ctrl_a, copy, del) = ui.input(|i| {
            (
                i.modifiers.command && i.key_pressed(egui::Key::A),
                i.events.iter().any(|e| matches!(e, egui::Event::Copy)),
                i.key_pressed(egui::Key::Delete),
            )
        });
        let focused = ui.ctx().memory(|m| m.focused().is_none());
        if focused && ctrl_a {
            self.selected = (0..n).collect();
        }
        if focused && del && !self.selected.is_empty() {
            let s = self.selected.clone();
            self.remove_points(&s);
        }
        if focused && copy && !self.selected.is_empty() && let Ok(t) = &t {
            let mut out = String::new();
            for &i in &self.selected {
                let (x, y) = t.pt_to_logical(self.ws.line().points[i]);
                out.push_str(&format!("{}\t{}\r\n", c_g(x, 15), c_g(y, 15)));
            }
            ui.ctx().copy_text(out);
        }
        let row_h = ui.text_style_height(&egui::TextStyle::Monospace) + 4.0;
        let w = ui.available_width();
        egui::Grid::new("data_head").num_columns(3).min_col_width(40.0).show(ui, |ui| {
            ui.add_sized([40.0, row_h], egui::Label::new(RichText::new("N°").strong()));
            ui.add_sized([(w - 60.0) / 2.0, row_h], egui::Label::new(RichText::new("X").strong()));
            ui.add_sized([(w - 60.0) / 2.0, row_h], egui::Label::new(RichText::new("Y").strong()));
        });
        ui.separator();
        let mods = ui.input(|i| i.modifiers);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, row_h, n, |ui, range| {
            for i in range {
                let p = self.ws.line().points[i];
                let (xs, ys) = match &t {
                    Ok(t) => {
                        let (x, y) = t.pt_to_logical(p);
                        (c_g(x, 8), c_g(y, 8))
                    }
                    Err(_) => {
                        let (x, y) = p.to_image();
                        (format!("{x:.2} px"), format!("{y:.2} px"))
                    }
                };
                let selected = self.selected.contains(&i);
                let text = format!("{:>5}  {:<16} {}", i + 1, xs, ys);
                let r = ui.add_sized([w, row_h], egui::Button::selectable(selected, RichText::new(text).monospace()));
                if r.clicked() {
                    if mods.command {
                        if !self.selected.remove(&i) {
                            self.selected.insert(i);
                        }
                    } else if mods.shift && let Some(&a) = self.selected.iter().next() {
                        self.selected = (a.min(i)..=a.max(i)).collect();
                    } else {
                        self.selected = std::iter::once(i).collect();
                    }
                }
            }
        });
    }

    /// Magnified view around the cursor; double click sets a point at sub-pixel precision.
    pub fn magnifier(&mut self, ui: &mut Ui) {
        let side = ui.available_width().clamp(120.0, 260.0);
        let (resp, painter) = ui.allocate_painter(vec2(side, side), Sense::click());
        let rect = resp.rect;
        painter.rect_filled(rect, 0.0, Color32::from_gray(90));
        let (Some(img), Some(c)) = (&self.image, self.mag_center) else { return };
        let k = side / MAG_PIXELS as f32;
        let (cx, cy) = (c.x.floor() as i32, c.y.floor() as i32);
        let half = MAG_PIXELS / 2;
        // Pixels.
        for dy in -half..=half {
            for dx in -half..=half {
                let (x, y) = (cx + dx, cy + dy);
                if x < 0 || y < 0 || x as u32 >= img.width() || y as u32 >= img.height() {
                    continue;
                }
                let p = img.get_pixel(x as u32, y as u32);
                let r = egui::Rect::from_min_size(rect.min + vec2((dx + half) as f32 * k, (dy + half) as f32 * k), vec2(k, k));
                painter.rect_filled(r, 0.0, Color32::from_rgb(p[0], p[1], p[2]));
            }
        }
        let origin = pos2((cx - half) as f32, (cy - half) as f32);
        let to_mag = |p: egui::Pos2| rect.min + (p - origin) * k;
        // Points of the current line and axis points.
        let line = self.ws.line();
        for p in &line.points {
            let (x, y) = p.to_image();
            let s = to_mag(pos2(x as f32, y as f32));
            if rect.contains(s) {
                painter.circle_filled(s, 3.0, line.pt_color.to_color32());
                painter.circle_stroke(s, 3.0, (1.0, Color32::BLACK));
            }
        }
        for id in AxisId::ALL {
            if let Some(a) = self.ws.axis(id) {
                let (x, y) = a.pos.to_image();
                let s = to_mag(pos2(x as f32, y as f32));
                if rect.contains(s) {
                    painter.circle_stroke(s, 5.0, (2.0, Color32::RED));
                }
            }
        }
        // Cross at the cursor position.
        let s = to_mag(c);
        let st = egui::Stroke::new(1.0, Color32::from_rgb(255, 0, 0));
        painter.line_segment([pos2(rect.left(), s.y), pos2(rect.right(), s.y)], st);
        painter.line_segment([pos2(s.x, rect.top()), pos2(s.x, rect.bottom())], st);
        painter.rect_stroke(rect, 0.0, (1.0, Color32::BLACK), egui::StrokeKind::Inside);

        if resp.double_clicked()
            && let Some(m) = resp.interact_pointer_pos()
        {
            let p = origin + (m - rect.min) / k;
            let ctrl = ui.input(|i| i.modifiers.ctrl);
            let pt = self.point_at(p, !ctrl);
            match self.mode.clone() {
                Mode::SetScale(axis) => self.open_axis_value(axis, pt, true),
                _ => self.add_point(pt),
            }
        }
    }
}
