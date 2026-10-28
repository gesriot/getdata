//! Modal dialogs.

use eframe::egui::{self, Color32, Id, Modal, RichText, Sense, Ui, pos2, vec2};

use crate::app::{After, App, Mode};
use crate::digitize::LineCenter;
use crate::export::{FileFormat, NumberFormat, Separator, TextExportSettings, c_g};
use crate::i18n::{tr, trf};
use crate::model::*;

pub struct AxisValueDlg {
    pub axis: AxisId,
    pub pos: Pt,
    pub text: String,
    /// Continue with the next axis point (Set the scale sequence).
    pub sequence: bool,
    pub focus: bool,
}

pub struct AdjustScaleDlg {
    pub vals: [String; 4],
    pub x_log: bool,
    pub y_log: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OptionsPage {
    Digitize,
    AutoTrace,
    DigitizeArea,
    ExportFile,
    ExportClipboard,
}

pub struct OptionsDlg {
    pub page: OptionsPage,
    pub digitize: DigitizeSettings,
    pub auto_trace: AutoTraceSettings,
    pub at_boundary: bool,
    pub file: TextExportSettings,
    pub clip: TextExportSettings,
}

pub struct DigitizeAreaDlg {
    pub s: DigitizeAreaSettings,
    pub dx: String,
    pub dy: String,
    pub gx: String,
    pub gy: String,
}

pub struct LinePropsDlg {
    /// `None`: add a new line.
    pub index: Option<usize>,
    pub caption: String,
    pub line_type: LineType,
    pub color: [u8; 3],
}

pub struct GridDlg {
    pub s: GridSettings,
    pub dx: String,
    pub dy: String,
    pub gx: String,
    pub gy: String,
    pub shift_x: bool,
    pub shift_y: bool,
}

pub struct DirectionDlg {
    pub start: LineCenter,
    pub dirs: Vec<f64>,
    /// Left and right boundaries of the sector, radians (y up).
    pub left: f64,
    pub right: f64,
    pub tex: Option<egui::TextureHandle>,
}

pub struct ExportDlg {
    pub format: FileFormat,
    pub txt: TextExportSettings,
}

#[derive(Default)]
pub struct Dialogs {
    pub message: Option<String>,
    pub save_changes: Option<After>,
    pub axis_value: Option<AxisValueDlg>,
    pub adjust_scale: Option<AdjustScaleDlg>,
    pub options: Option<OptionsDlg>,
    pub digitize_area: Option<DigitizeAreaDlg>,
    pub line_props: Option<LinePropsDlg>,
    pub grid: Option<GridDlg>,
    pub direction: Option<DirectionDlg>,
    pub export: Option<ExportDlg>,
    pub confirm_remove_line: Option<usize>,
    pub confirm_remove_all: bool,
}

impl Dialogs {
    pub fn any_open(&self) -> bool {
        self.message.is_some()
            || self.save_changes.is_some()
            || self.axis_value.is_some()
            || self.adjust_scale.is_some()
            || self.options.is_some()
            || self.digitize_area.is_some()
            || self.line_props.is_some()
            || self.grid.is_some()
            || self.direction.is_some()
            || self.export.is_some()
            || self.confirm_remove_line.is_some()
            || self.confirm_remove_all
    }
}

/// Parse a number typed by the user (a decimal comma is accepted).
pub fn parse_num(s: &str) -> Option<f64> {
    s.trim().replace(',', ".").parse::<f64>().ok().filter(|v| v.is_finite())
}

pub fn num_text(v: f64) -> String {
    c_g(v, 15)
}

fn ok_cancel(ui: &mut Ui) -> (bool, bool) {
    let mut ok = false;
    let mut cancel = false;
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ok = ui.add_sized([80.0, 24.0], egui::Button::new(tr("OK"))).clicked();
        cancel = ui.add_sized([80.0, 24.0], egui::Button::new(tr("Cancel"))).clicked();
    });
    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
    (ok || enter, cancel)
}

fn title(ui: &mut Ui, t: &str) {
    ui.heading(t);
    ui.separator();
}

fn swatch(ui: &mut Ui, c: Color32, size: egui::Vec2) -> egui::Response {
    let (r, p) = ui.allocate_painter(size, Sense::click());
    p.rect_filled(r.rect, 2.0, c);
    p.rect_stroke(r.rect, 2.0, (1.0, ui.visuals().widgets.noninteractive.fg_stroke.color), egui::StrokeKind::Inside);
    r
}

impl App {
    pub fn show_dialogs(&mut self, ctx: &egui::Context) {
        self.dlg_message(ctx);
        self.dlg_save_changes(ctx);
        self.dlg_axis_value(ctx);
        self.dlg_adjust_scale(ctx);
        self.dlg_options(ctx);
        self.dlg_digitize_area(ctx);
        self.dlg_line_props(ctx);
        self.dlg_grid(ctx);
        self.dlg_direction(ctx);
        self.dlg_export(ctx);
        self.dlg_confirms(ctx);
    }

    fn dlg_message(&mut self, ctx: &egui::Context) {
        let Some(msg) = self.dialogs.message.clone() else { return };
        let r = Modal::new(Id::unique("dlg_message")).show(ctx, |ui| {
            ui.set_max_width(420.0);
            ui.label(RichText::new("⚠").size(22.0));
            ui.label(msg);
            ui.add_space(6.0);
            ui.add_sized([80.0, 24.0], egui::Button::new(tr("OK"))).clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter))
        });
        if r.inner || r.should_close() {
            self.dialogs.message = None;
        }
    }

    fn dlg_save_changes(&mut self, ctx: &egui::Context) {
        let Some(after) = self.dialogs.save_changes.clone() else { return };
        let mut choice = None;
        Modal::new(Id::unique("dlg_save")).show(ctx, |ui| {
            ui.label(tr("Save changes to current workspace?"));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.add_sized([80.0, 24.0], egui::Button::new(tr("Yes"))).clicked() {
                    choice = Some(1);
                }
                if ui.add_sized([80.0, 24.0], egui::Button::new(tr("No"))).clicked() {
                    choice = Some(2);
                }
                if ui.add_sized([80.0, 24.0], egui::Button::new(tr("Cancel"))).clicked() {
                    choice = Some(0);
                }
            });
        });
        match choice {
            Some(1) => {
                self.dialogs.save_changes = None;
                if self.save_workspace(false) {
                    self.run_after(after);
                }
            }
            Some(2) => {
                self.dialogs.save_changes = None;
                self.modified = false;
                self.run_after(after);
            }
            Some(_) => self.dialogs.save_changes = None,
            None => {}
        }
    }

    fn dlg_axis_value(&mut self, ctx: &egui::Context) {
        let Some(d) = &mut self.dialogs.axis_value else { return };
        let label = match d.axis {
            AxisId::Xmin => tr("Xmin value"),
            AxisId::Xmax => tr("Xmax value"),
            AxisId::Ymin => tr("Ymin value"),
            AxisId::Ymax => tr("Ymax value"),
        };
        let mut result: Option<bool> = None;
        let r = Modal::new(Id::unique("dlg_axis")).show(ctx, |ui| {
            ui.label(format!("{label}:"));
            let te = ui.add(egui::TextEdit::singleline(&mut d.text).desired_width(200.0));
            if d.focus {
                te.request_focus();
                d.focus = false;
            }
            let (ok, cancel) = ok_cancel(ui);
            if ok {
                result = Some(true);
            } else if cancel {
                result = Some(false);
            }
        });
        if r.should_close() && result.is_none() {
            result = Some(false);
        }
        match result {
            Some(true) => {
                let Some(v) = parse_num(&d.text) else {
                    self.dialogs.message = Some(tr("Please enter a number.").into());
                    return;
                };
                let (axis, pos, seq) = (d.axis, d.pos, d.sequence);
                self.dialogs.axis_value = None;
                self.set_axis(axis, pos, v);
                self.advance_scale(axis, seq);
            }
            Some(false) => {
                self.dialogs.axis_value = None;
                if matches!(self.mode, Mode::SetScale(_)) {
                    self.set_mode(Mode::Normal);
                }
            }
            None => {}
        }
    }

    /// Continue the Set-the-scale sequence after `axis`.
    pub fn advance_scale(&mut self, axis: AxisId, seq: bool) {
        if !seq {
            return;
        }
        let next = match axis {
            AxisId::Xmin => Some(AxisId::Xmax),
            AxisId::Xmax => Some(AxisId::Ymin),
            AxisId::Ymin => Some(AxisId::Ymax),
            AxisId::Ymax => None,
        };
        match next {
            Some(n) => self.mode = Mode::SetScale(n),
            None => self.set_mode(Mode::Normal),
        }
    }

    pub fn open_axis_value(&mut self, axis: AxisId, pos: Pt, sequence: bool) {
        let v = self.ws.axis(axis).map(|a| a.value).unwrap_or_else(|| {
            // Propose the pixel coordinate as for default axes.
            let (x, y) = pos.pixel();
            match axis {
                AxisId::Xmin | AxisId::Xmax => x as f64,
                AxisId::Ymin | AxisId::Ymax => (self.ws.height as i32 - 1 - y) as f64,
            }
        });
        self.dialogs.axis_value = Some(AxisValueDlg { axis, pos, text: num_text(v), sequence, focus: true });
    }

    pub fn open_adjust_scale(&mut self) {
        let v = |id: AxisId| self.ws.axis(id).map(|a| num_text(a.value)).unwrap_or_default();
        self.dialogs.adjust_scale = Some(AdjustScaleDlg {
            vals: [v(AxisId::Xmin), v(AxisId::Xmax), v(AxisId::Ymin), v(AxisId::Ymax)],
            x_log: self.ws.x_log,
            y_log: self.ws.y_log,
        });
    }

    fn dlg_adjust_scale(&mut self, ctx: &egui::Context) {
        let Some(d) = &mut self.dialogs.adjust_scale else { return };
        let mut result = None;
        let r = Modal::new(Id::unique("dlg_adjust")).show(ctx, |ui| {
            title(ui, tr("Adjust the scale"));
            let names = [tr("Xmin value"), tr("Xmax value"), tr("Ymin value"), tr("Ymax value")];
            for (axis, (range, log)) in [(tr("X axis"), (0..2, &mut d.x_log)), (tr("Y axis"), (2..4, &mut d.y_log))] {
                ui.group(|ui| {
                    ui.label(RichText::new(axis).strong());
                    egui::Grid::new(axis).num_columns(2).show(ui, |ui| {
                        for i in range {
                            ui.label(format!("{}:", names[i]));
                            ui.add(egui::TextEdit::singleline(&mut d.vals[i]).desired_width(160.0));
                            ui.end_row();
                        }
                    });
                    let text = if axis == tr("X axis") { tr("Log scale along X axis") } else { tr("Log scale along Y axis") };
                    ui.checkbox(log, text);
                });
            }
            let (ok, cancel) = ok_cancel(ui);
            if ok {
                result = Some(true);
            } else if cancel {
                result = Some(false);
            }
        });
        if r.should_close() && result.is_none() {
            result = Some(false);
        }
        match result {
            Some(true) => {
                let mut vals = [None; 4];
                for i in 0..4 {
                    if !d.vals[i].trim().is_empty() {
                        match parse_num(&d.vals[i]) {
                            Some(v) => vals[i] = Some(v),
                            None => {
                                self.dialogs.message = Some(tr("Please enter a number.").into());
                                return;
                            }
                        }
                    }
                }
                let pos_ok = |a: Option<f64>, b: Option<f64>| a.is_none_or(|v| v > 0.0) && b.is_none_or(|v| v > 0.0);
                if d.x_log && !pos_ok(vals[0], vals[1]) {
                    self.dialogs.message = Some(tr("X axis: log scale not allowed (both Xmin and Xmax should be > 0)").into());
                    return;
                }
                if d.y_log && !pos_ok(vals[2], vals[3]) {
                    self.dialogs.message = Some(tr("Y axis: log scale not allowed (both Ymin and Ymax should be > 0)").into());
                    return;
                }
                let (xl, yl) = (d.x_log, d.y_log);
                self.dialogs.adjust_scale = None;
                self.checkpoint();
                for (i, v) in vals.into_iter().enumerate() {
                    if let (Some(v), Some(a)) = (v, self.ws.axes[i].as_mut()) {
                        a.value = v;
                    }
                }
                self.ws.x_log = xl;
                self.ws.y_log = yl;
            }
            Some(false) => self.dialogs.adjust_scale = None,
            None => {}
        }
    }

    pub fn open_options(&mut self) {
        self.dialogs.options = Some(OptionsDlg {
            page: OptionsPage::Digitize,
            digitize: self.ws.digitize.clone(),
            auto_trace: self.ws.auto_trace.clone(),
            at_boundary: self.ws.digitize_area.at_boundary,
            file: self.settings.export_file.clone(),
            clip: self.settings.export_clipboard.clone(),
        });
    }

    fn text_export_page(ui: &mut Ui, s: &mut TextExportSettings) {
        ui.horizontal_top(|ui| {
            ui.group(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(tr("Lines")).strong());
                    ui.radio_value(&mut s.all_lines, false, tr("Current line only"));
                    ui.radio_value(&mut s.all_lines, true, tr("All lines"));
                    ui.checkbox(&mut s.print_caption, tr("Print line caption"));
                });
            });
            ui.group(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(tr("Output format")).strong());
                    ui.radio_value(&mut s.format, NumberFormat::Fixed, "0.001");
                    ui.radio_value(&mut s.format, NumberFormat::Exponent, "1e-003");
                    ui.radio_value(&mut s.format, NumberFormat::Mathematica, "Mathematica {1*10^(-3)}");
                });
            });
        });
        ui.checkbox(&mut s.sort_by_x, tr("X-value based sorting"));
        ui.checkbox(&mut s.header, tr("Print header"));
        ui.group(|ui| {
            ui.label(RichText::new(tr("Separator")).strong());
            ui.radio_value(&mut s.separator, Separator::Space, tr("Space separated"));
            ui.radio_value(&mut s.separator, Separator::Tab, tr("Tab separated"));
        });
    }

    fn dlg_options(&mut self, ctx: &egui::Context) {
        let Some(d) = &mut self.dialogs.options else { return };
        let mut result = None;
        let r = Modal::new(Id::unique("dlg_options")).show(ctx, |ui| {
            title(ui, tr("Options"));
            // Fixed-size columns: a separator or growing column inside a modal makes
            // its auto-sizing never settle.
            let column = egui::Layout::top_down(egui::Align::Min);
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(vec2(170.0, 270.0), column, |ui| {
                    ui.set_min_size(vec2(170.0, 270.0));
                    for (p, name) in [
                        (OptionsPage::Digitize, tr("Digitize")),
                        (OptionsPage::AutoTrace, tr("Auto trace")),
                        (OptionsPage::DigitizeArea, tr("Digitize area")),
                        (OptionsPage::ExportFile, tr("Export to text file")),
                        (OptionsPage::ExportClipboard, tr("Export to clipboard")),
                    ] {
                        let indent = matches!(p, OptionsPage::AutoTrace | OptionsPage::DigitizeArea);
                        ui.horizontal(|ui| {
                            if indent {
                                ui.add_space(12.0);
                            }
                            ui.selectable_value(&mut d.page, p, name);
                        });
                    }
                });
                ui.allocate_ui_with_layout(vec2(400.0, 270.0), column, |ui| {
                    ui.set_min_size(vec2(400.0, 270.0));
                    match d.page {
                        OptionsPage::Digitize => {
                            ui.label(RichText::new(tr("Digitize")).strong());
                            ui.add_space(4.0);
                            ui.label(tr("Line/background identification method:"));
                            ui.radio_value(&mut d.digitize.line_id_method, LineIdMethod::ByColor, tr("by color"));
                            ui.radio_value(&mut d.digitize.line_id_method, LineIdMethod::ByIntensity, tr("by intensity"));
                            ui.radio_value(&mut d.digitize.line_id_method, LineIdMethod::Combined, tr("combined"));
                            ui.add_space(4.0);
                            ui.checkbox(&mut d.digitize.ignore_pixels, tr("Ignore single pixels/very thin lines"));
                        }
                        OptionsPage::AutoTrace => {
                            ui.label(RichText::new(tr("Auto trace")).strong());
                            ui.add_space(4.0);
                            ui.label(tr("Distance between data points:"));
                            ui.add(egui::Slider::new(&mut d.auto_trace.trace_distance, 3..=50));
                            ui.checkbox(&mut d.auto_trace.smooth_lines, tr("Smooth lines"));
                        }
                        OptionsPage::DigitizeArea => {
                            ui.label(RichText::new(tr("Digitize area")).strong());
                            ui.add_space(4.0);
                            ui.label(tr("Data points are placed"));
                            ui.radio_value(&mut d.at_boundary, false, tr("in the middle of a line"));
                            ui.radio_value(&mut d.at_boundary, true, tr("at the area boundary"));
                        }
                        OptionsPage::ExportFile => Self::text_export_page(ui, &mut d.file),
                        OptionsPage::ExportClipboard => Self::text_export_page(ui, &mut d.clip),
                    }
                });
            });
            ui.separator();
            let (ok, cancel) = ok_cancel(ui);
            if ok {
                result = Some(true);
            } else if cancel {
                result = Some(false);
            }
        });
        if r.should_close() && result.is_none() {
            result = Some(false);
        }
        match result {
            Some(true) => {
                let d = self.dialogs.options.take().expect("open");
                self.ws.digitize = d.digitize;
                self.ws.auto_trace = d.auto_trace;
                self.ws.digitize_area.at_boundary = d.at_boundary;
                self.settings.ignore_pixels = self.ws.digitize.ignore_pixels;
                self.settings.line_id_method = self.ws.digitize.line_id_method as i32;
                self.settings.trace_distance = self.ws.auto_trace.trace_distance;
                self.settings.smooth_lines = self.ws.auto_trace.smooth_lines;
                self.settings.at_boundary = d.at_boundary;
                self.settings.export_file = d.file;
                self.settings.export_clipboard = d.clip;
                self.modified = true;
            }
            Some(false) => self.dialogs.options = None,
            None => {}
        }
    }

    pub fn open_digitize_area(&mut self) {
        let s = self.ws.digitize_area.clone();
        let rect = s.area_type == AreaType::Rectangular;
        self.dialogs.digitize_area = Some(DigitizeAreaDlg {
            dx: num_text(if rect { s.dx } else { s.delta_x }),
            dy: num_text(s.delta_y),
            gx: num_text(s.go_through_x),
            gy: num_text(s.go_through_y),
            s,
        });
    }

    fn units_combo(ui: &mut Ui, id: &str, u: &mut GridUnits, axis_values: &'static str) {
        egui::ComboBox::from_id_salt(id)
            .selected_text(if *u == GridUnits::Points { tr("points") } else { tr(axis_values) })
            .show_ui(ui, |ui| {
                ui.selectable_value(u, GridUnits::Points, tr("points"));
                ui.selectable_value(u, GridUnits::AxisValues, tr(axis_values));
            });
    }

    fn step_label(log: bool, units: GridUnits, x: bool) -> &'static str {
        match (x, log && units == GridUnits::AxisValues) {
            (true, false) => "dx =",
            (true, true) => "x(n+1)/x(n) =",
            (false, false) => "dy =",
            (false, true) => "y(n+1)/y(n) =",
        }
    }

    fn dlg_digitize_area(&mut self, ctx: &egui::Context) {
        let has_scale = self.transform().is_ok();
        let (xl, yl) = (self.ws.x_log, self.ws.y_log);
        let Some(d) = &mut self.dialogs.digitize_area else { return };
        let mut result = None;
        let r = Modal::new(Id::unique("dlg_area")).show(ctx, |ui| {
            title(ui, tr("Digitize area"));
            ui.horizontal_top(|ui| {
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(tr("Area")).strong());
                        ui.radio_value(&mut d.s.area_type, AreaType::Rectangular, tr("Rectangular area"));
                        ui.add_enabled_ui(has_scale, |ui| {
                            ui.radio_value(&mut d.s.area_type, AreaType::RelatedToAxes, tr("Area is related to axes"));
                        });
                    });
                });
                ui.add_enabled_ui(d.s.area_type == AreaType::RelatedToAxes, |ui| {
                    ui.group(|ui| {
                        ui.vertical(|ui| {
                            ui.label(RichText::new(tr("Grids")).strong());
                            ui.radio_value(&mut d.s.grid_type, 0, tr("X grid"));
                            ui.radio_value(&mut d.s.grid_type, 1, tr("Y grid"));
                        });
                    });
                });
            });
            ui.group(|ui| {
                ui.label(RichText::new(tr("Grid settings")).strong());
                let rect = d.s.area_type == AreaType::Rectangular;
                egui::Grid::new("area_grid").num_columns(3).show(ui, |ui| {
                    let xg = rect || d.s.grid_type == 0;
                    ui.add_enabled_ui(xg, |ui| {
                        ui.label(if rect { "dx =" } else { Self::step_label(xl, d.s.delta_x_units, true) });
                    });
                    ui.add_enabled(xg, egui::TextEdit::singleline(&mut d.dx).desired_width(100.0));
                    if rect {
                        ui.label(tr("points"));
                    } else {
                        ui.add_enabled_ui(xg, |ui| Self::units_combo(ui, "ux", &mut d.s.delta_x_units, "x axis values"));
                    }
                    ui.end_row();
                    ui.add_enabled_ui(!rect && xg, |ui| {
                        ui.checkbox(&mut d.s.shift_x, tr("Shift X grid to make it go through x ="));
                    });
                    ui.add_enabled(!rect && xg && d.s.shift_x, egui::TextEdit::singleline(&mut d.gx).desired_width(100.0));
                    ui.end_row();
                    let yg = !rect && d.s.grid_type == 1;
                    ui.add_enabled_ui(yg, |ui| ui.label(Self::step_label(yl, d.s.delta_y_units, false)));
                    ui.add_enabled(yg, egui::TextEdit::singleline(&mut d.dy).desired_width(100.0));
                    ui.add_enabled_ui(yg, |ui| Self::units_combo(ui, "uy", &mut d.s.delta_y_units, "y axis values"));
                    ui.end_row();
                    ui.add_enabled_ui(yg, |ui| {
                        ui.checkbox(&mut d.s.shift_y, tr("Shift Y grid to make it go through y ="));
                    });
                    ui.add_enabled(yg && d.s.shift_y, egui::TextEdit::singleline(&mut d.gy).desired_width(100.0));
                    ui.end_row();
                });
            });
            let (ok, cancel) = ok_cancel(ui);
            if ok {
                result = Some(true);
            } else if cancel {
                result = Some(false);
            }
        });
        if r.should_close() && result.is_none() {
            result = Some(false);
        }
        match result {
            Some(true) => {
                let rect = d.s.area_type == AreaType::Rectangular;
                let need = |s: &str| parse_num(s);
                let (Some(dx), Some(dy)) = (need(&d.dx), need(&d.dy).or(Some(d.s.delta_y))) else {
                    self.dialogs.message = Some(tr("Please enter a number.").into());
                    return;
                };
                let mut s = d.s.clone();
                if rect {
                    if dx < 1.0 {
                        self.dialogs.message = Some(trf("dx should not be less than {}", &[&"1"]));
                        return;
                    }
                    s.dx = dx;
                } else {
                    let log_x = xl && s.delta_x_units == GridUnits::AxisValues;
                    let log_y = yl && s.delta_y_units == GridUnits::AxisValues;
                    if s.grid_type == 0 && (if log_x { dx <= 1.0 } else { dx <= 0.0 }) {
                        let m = if log_x { "x(n+1)/x(n) should not be less than {}" } else { "dx should not be less than {}" };
                        self.dialogs.message = Some(trf(m, &[&if log_x { "1" } else { "0" }]));
                        return;
                    }
                    if s.grid_type == 1 && (if log_y { dy <= 1.0 } else { dy <= 0.0 }) {
                        let m = if log_y { "y(n+1)/y(n) should not be less than {}" } else { "dy should not be less than {}" };
                        self.dialogs.message = Some(trf(m, &[&if log_y { "1" } else { "0" }]));
                        return;
                    }
                    s.delta_x = dx;
                    s.delta_y = dy;
                    if s.shift_x {
                        s.go_through_x = parse_num(&d.gx).unwrap_or(s.go_through_x);
                    }
                    if s.shift_y {
                        s.go_through_y = parse_num(&d.gy).unwrap_or(s.go_through_y);
                    }
                }
                self.dialogs.digitize_area = None;
                self.ws.digitize_area = s;
                self.modified = true;
                self.set_mode(Mode::DigitizeArea);
            }
            Some(false) => self.dialogs.digitize_area = None,
            None => {}
        }
    }

    pub fn open_line_props(&mut self, index: Option<usize>) {
        self.dialogs.line_props = Some(match index {
            Some(i) => {
                let l = &self.ws.lines[i];
                LinePropsDlg { index, caption: l.caption.clone(), line_type: l.line_type, color: [l.pt_color.0, l.pt_color.1, l.pt_color.2] }
            }
            None => {
                let c = self.next_point_color();
                LinePropsDlg {
                    index,
                    caption: trf("Line #{}", &[&(self.ws.lines.len() + 1)]),
                    line_type: LineType::Polyline,
                    color: [c.0, c.1, c.2],
                }
            }
        });
    }

    fn dlg_line_props(&mut self, ctx: &egui::Context) {
        let n = self.ws.lines.len();
        let Some(d) = &mut self.dialogs.line_props else { return };
        let mut result = None;
        let r = Modal::new(Id::unique("dlg_line")).show(ctx, |ui| {
            match d.index {
                Some(i) => {
                    title(ui, tr("Line properties"));
                    ui.label(trf("Line #{} caption:", &[&(i + 1)]));
                }
                None => {
                    title(ui, tr("Add line"));
                    ui.label(trf("Caption for the new line (#{}):", &[&(n + 1)]));
                }
            }
            ui.add(egui::TextEdit::singleline(&mut d.caption).desired_width(340.0));
            ui.horizontal_top(|ui| {
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(tr("Line type")).strong());
                        ui.radio_value(&mut d.line_type, LineType::Polyline, tr("polyline"));
                        ui.radio_value(&mut d.line_type, LineType::ClosedPolyline, tr("closed polyline"));
                        ui.radio_value(&mut d.line_type, LineType::SeparatePoints, tr("separate points"));
                    });
                });
                ui.vertical(|ui| {
                    ui.label(tr("Points color:"));
                    ui.color_edit_button_srgb(&mut d.color);
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.add_sized([80.0, 24.0], egui::Button::new(tr("OK"))).clicked() {
                    result = Some(1);
                }
                if ui.add_sized([80.0, 24.0], egui::Button::new(tr("Cancel"))).clicked() {
                    result = Some(0);
                }
                if d.index.is_some() && ui.add_sized([100.0, 24.0], egui::Button::new(tr("Remove line"))).clicked() {
                    result = Some(2);
                }
            });
        });
        if r.should_close() && result.is_none() {
            result = Some(0);
        }
        match result {
            Some(1) => {
                let d = self.dialogs.line_props.take().expect("open");
                let color = Rgb(d.color[0], d.color[1], d.color[2]);
                match d.index {
                    Some(i) => {
                        self.checkpoint();
                        let l = &mut self.ws.lines[i];
                        l.caption = d.caption;
                        l.line_type = d.line_type;
                        l.pt_color = color;
                    }
                    None => self.add_line(d.caption, d.line_type, color),
                }
            }
            Some(2) => {
                let i = self.dialogs.line_props.take().and_then(|d| d.index);
                self.dialogs.confirm_remove_line = i;
            }
            Some(_) => self.dialogs.line_props = None,
            None => {}
        }
    }

    pub fn open_grid_settings(&mut self) {
        let s = self.ws.grid.clone();
        self.dialogs.grid = Some(GridDlg {
            dx: num_text(if s.area_type == AreaType::Rectangular { s.dx } else { s.delta_x }),
            dy: num_text(if s.area_type == AreaType::Rectangular { s.dy } else { s.delta_y }),
            gx: num_text(s.go_through_x),
            gy: num_text(s.go_through_y),
            shift_x: s.go_through_x != 0.0,
            shift_y: s.go_through_y != 0.0,
            s,
        });
    }

    fn dlg_grid(&mut self, ctx: &egui::Context) {
        let has_scale = self.transform().is_ok();
        let (xl, yl) = (self.ws.x_log, self.ws.y_log);
        let Some(d) = &mut self.dialogs.grid else { return };
        let mut result = None;
        let r = Modal::new(Id::unique("dlg_grid")).show(ctx, |ui| {
            title(ui, tr("Grid Settings"));
            let before = d.s.area_type;
            ui.radio_value(&mut d.s.area_type, AreaType::Rectangular, tr("Rectangular area"));
            ui.add_enabled_ui(has_scale, |ui| {
                ui.radio_value(&mut d.s.area_type, AreaType::RelatedToAxes, tr("Area is related to axes"));
            });
            if before != d.s.area_type {
                let rect = d.s.area_type == AreaType::Rectangular;
                d.dx = num_text(if rect { d.s.dx } else { d.s.delta_x });
                d.dy = num_text(if rect { d.s.dy } else { d.s.delta_y });
            }
            let rect = d.s.area_type == AreaType::Rectangular;
            ui.group(|ui| {
                ui.label(RichText::new(tr("Grid settings")).strong());
                egui::Grid::new("grid_grid").num_columns(3).show(ui, |ui| {
                    ui.label(if rect { "dx =" } else { Self::step_label(xl, d.s.delta_x_units, true) });
                    ui.add(egui::TextEdit::singleline(&mut d.dx).desired_width(100.0));
                    if rect {
                        ui.label(tr("points"));
                    } else {
                        Self::units_combo(ui, "gux", &mut d.s.delta_x_units, "x axis values");
                    }
                    ui.end_row();
                    ui.add_enabled_ui(!rect, |ui| ui.checkbox(&mut d.shift_x, tr("Shift X grid to make it go through x =")));
                    ui.add_enabled(!rect && d.shift_x, egui::TextEdit::singleline(&mut d.gx).desired_width(100.0));
                    ui.end_row();
                    ui.label(if rect { "dy =" } else { Self::step_label(yl, d.s.delta_y_units, false) });
                    ui.add(egui::TextEdit::singleline(&mut d.dy).desired_width(100.0));
                    if rect {
                        ui.label(tr("points"));
                    } else {
                        Self::units_combo(ui, "guy", &mut d.s.delta_y_units, "y axis values");
                    }
                    ui.end_row();
                    ui.add_enabled_ui(!rect, |ui| ui.checkbox(&mut d.shift_y, tr("Shift Y grid to make it go through y =")));
                    ui.add_enabled(!rect && d.shift_y, egui::TextEdit::singleline(&mut d.gy).desired_width(100.0));
                    ui.end_row();
                });
            });
            let (ok, cancel) = ok_cancel(ui);
            if ok {
                result = Some(true);
            } else if cancel {
                result = Some(false);
            }
        });
        if r.should_close() && result.is_none() {
            result = Some(false);
        }
        match result {
            Some(true) => {
                let (Some(dx), Some(dy)) = (parse_num(&d.dx), parse_num(&d.dy)) else {
                    self.dialogs.message = Some(tr("Please enter a number.").into());
                    return;
                };
                if dx <= 0.0 || dy <= 0.0 {
                    self.dialogs.message = Some(trf("dx should not be less than {}", &[&"0"]));
                    return;
                }
                let mut s = d.s.clone();
                if s.area_type == AreaType::Rectangular {
                    s.dx = dx.max(2.0);
                    s.dy = dy.max(2.0);
                } else {
                    s.delta_x = dx;
                    s.delta_y = dy;
                    s.go_through_x = if d.shift_x { parse_num(&d.gx).unwrap_or(0.0) } else { 0.0 };
                    s.go_through_y = if d.shift_y { parse_num(&d.gy).unwrap_or(0.0) } else { 0.0 };
                }
                s.show_grid = true;
                self.dialogs.grid = None;
                self.ws.grid = s;
                self.modified = true;
            }
            Some(false) => self.dialogs.grid = None,
            None => {}
        }
    }

    pub fn open_direction(&mut self, p: egui::Pos2) {
        let Some((start, dirs)) = self.trace_start(p) else { return };
        let first = dirs.first().copied().unwrap_or(0.0);
        let q = std::f64::consts::FRAC_PI_4;
        self.dialogs.direction = Some(DirectionDlg { start, dirs, left: first + q, right: first - q, tex: None });
    }

    fn dlg_direction(&mut self, ctx: &egui::Context) {
        let Some(img) = &self.image else {
            self.dialogs.direction = None;
            return;
        };
        let Some(d) = &mut self.dialogs.direction else { return };
        // 101x101 pixels around the start point, shown 3x.
        let (cx, cy) = d.start.pos.pixel();
        if d.tex.is_none() {
            let mut px = Vec::with_capacity(101 * 101);
            for y in cy - 50..=cy + 50 {
                for x in cx - 50..=cx + 50 {
                    let c = if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
                        let p = img.get_pixel(x as u32, y as u32);
                        Color32::from_rgb(p[0], p[1], p[2])
                    } else {
                        Color32::GRAY
                    };
                    px.push(c);
                }
            }
            d.tex = Some(ctx.load_texture("direction", egui::ColorImage::new([101, 101], px), egui::TextureOptions::NEAREST));
        }
        let mut result = None;
        let r = Modal::new(Id::unique("dlg_dir")).show(ctx, |ui| {
            title(ui, tr("Auto trace: choose direction"));
            ui.label(tr("Left/right mouse button to set left/right boundary"));
            let size = 303.0;
            let (resp, painter) = ui.allocate_painter(vec2(size, size), Sense::click());
            let rect = resp.rect;
            if let Some(t) = &d.tex {
                painter.image(t.id(), rect, egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            // Centre of the start point inside the 101 px window.
            let (sx, sy) = d.start.pos.to_image();
            let c = rect.min + vec2(((sx - (cx - 50) as f64) * 3.0) as f32, ((sy - (cy - 50) as f64) * 3.0) as f32);
            let angle_of = |p: egui::Pos2| {
                let a = (-(p.y - c.y) as f64).atan2((p.x - c.x) as f64);
                if a < 0.0 { a + std::f64::consts::TAU } else { a }
            };
            if let Some(p) = resp.interact_pointer_pos() {
                if resp.clicked() {
                    d.left = angle_of(p);
                } else if resp.secondary_clicked() {
                    d.right = angle_of(p);
                }
            }
            // Sector from the right boundary counter-clockwise to the left boundary.
            let rad = size * 0.45;
            let mut a0 = d.right;
            let mut a1 = d.left;
            while a1 <= a0 {
                a1 += std::f64::consts::TAU;
            }
            let steps = 48;
            let mut pts = vec![c];
            for k in 0..=steps {
                let a = a0 + (a1 - a0) * k as f64 / steps as f64;
                pts.push(c + vec2((a.cos() * rad as f64) as f32, -(a.sin() * rad as f64) as f32));
            }
            painter.add(egui::Shape::convex_polygon(pts.clone(), Color32::from_rgba_unmultiplied(80, 160, 255, 50), egui::Stroke::NONE));
            for (a, col) in [(d.left, Color32::from_rgb(0, 160, 0)), (d.right, Color32::from_rgb(220, 0, 0))] {
                painter.line_segment([c, c + vec2((a.cos() * rad as f64) as f32, -(a.sin() * rad as f64) as f32)], (2.0, col));
            }
            for &a in &d.dirs {
                painter.circle_filled(c + vec2((a.cos() * 40.0) as f32, -(a.sin() * 40.0) as f32), 3.0, Color32::from_rgb(255, 128, 0));
            }
            painter.circle_stroke(c, 4.0, (1.5, Color32::RED));
            a0 = a0.rem_euclid(std::f64::consts::TAU);
            let _ = a0;
            let (ok, cancel) = ok_cancel(ui);
            if ok {
                result = Some(true);
            } else if cancel {
                result = Some(false);
            }
        });
        if r.should_close() && result.is_none() {
            result = Some(false);
        }
        match result {
            Some(true) => {
                let d = self.dialogs.direction.take().expect("open");
                self.auto_trace_from(d.start, d.right, d.left);
            }
            Some(false) => self.dialogs.direction = None,
            None => {}
        }
    }

    pub fn open_export(&mut self) {
        if self.transform().is_err() {
            self.dialogs.message = Some(tr("Export not possible: Xmin, Xmax, Ymin and Ymax should be set").into());
            return;
        }
        let format = FileFormat::ALL.get(self.settings.export_filter).copied().unwrap_or(FileFormat::Txt);
        self.dialogs.export = Some(ExportDlg { format, txt: self.settings.export_file.clone() });
    }

    fn dlg_export(&mut self, ctx: &egui::Context) {
        let Some(d) = &mut self.dialogs.export else { return };
        let mut result = None;
        let r = Modal::new(Id::unique("dlg_export")).show(ctx, |ui| {
            title(ui, tr("Export data"));
            for f in FileFormat::ALL {
                ui.radio_value(&mut d.format, f, format!("{}  (*.{})", f.filter_name(), f.ext()));
            }
            if d.format == FileFormat::Txt {
                ui.separator();
                ui.label(RichText::new(tr("Export to text file")).strong());
                Self::text_export_page(ui, &mut d.txt);
            }
            let (ok, cancel) = ok_cancel(ui);
            if ok {
                result = Some(true);
            } else if cancel {
                result = Some(false);
            }
        });
        if r.should_close() && result.is_none() {
            result = Some(false);
        }
        match result {
            Some(true) => {
                let d = self.dialogs.export.take().expect("open");
                self.settings.export_filter = FileFormat::ALL.iter().position(|f| *f == d.format).unwrap_or(0);
                self.settings.export_file = d.txt;
                self.export_to(d.format);
            }
            Some(false) => self.dialogs.export = None,
            None => {}
        }
    }

    fn dlg_confirms(&mut self, ctx: &egui::Context) {
        let yes_no = |id: &str, text: &str| -> Option<bool> {
            let mut res = None;
            let r = Modal::new(Id::unique(id)).show(ctx, |ui| {
                ui.set_max_width(420.0);
                ui.label(text);
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.add_sized([80.0, 24.0], egui::Button::new(tr("Yes"))).clicked() {
                        res = Some(true);
                    }
                    if ui.add_sized([80.0, 24.0], egui::Button::new(tr("No"))).clicked() {
                        res = Some(false);
                    }
                });
            });
            if r.should_close() && res.is_none() { Some(false) } else { res }
        };
        if let Some(i) = self.dialogs.confirm_remove_line {
            match yes_no(
                "dlg_rmline",
                tr("Are you sure you want to completely remove this line and corresponding data points?"),
            ) {
                Some(true) => {
                    self.dialogs.confirm_remove_line = None;
                    self.remove_line(i);
                }
                Some(false) => self.dialogs.confirm_remove_line = None,
                None => {}
            }
        }
        if self.dialogs.confirm_remove_all {
            match yes_no(
                "dlg_rmall",
                tr("Are you sure you want to completely remove all lines and data points?"),
            ) {
                Some(true) => {
                    self.dialogs.confirm_remove_all = false;
                    self.remove_all_data();
                }
                Some(false) => self.dialogs.confirm_remove_all = false,
                None => {}
            }
        }
    }

    pub fn color_swatch(ui: &mut Ui, c: Rgb) -> egui::Response {
        swatch(ui, c.to_color32(), vec2(18.0, 18.0))
    }
}
