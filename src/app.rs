//! Application state and the operations behind menu commands.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use eframe::egui::{self, Pos2, Vec2, pos2};
use image::RgbImage;

use crate::coords::{ScaleError, Transform};
use crate::digitize::{self, Classifier, LineCenter, Scorer, TraceDirections, TraceParams};
use crate::export::{self, FileFormat};
use crate::gdw;
use crate::i18n::{tr, trf};
use crate::imageio;
use crate::model::*;
use crate::settings::{STORAGE_KEY, Settings};
use crate::tiles::Tiles;
use crate::view::View;

const MAX_UNDO: usize = 100;

/// Interaction mode of the main view.
#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    Normal,
    /// Setting the axis points one after another.
    SetScale(AxisId),
    PickColor { line: bool },
    AutoTrace,
    DigitizeArea,
    PointCapture,
    Eraser,
    Reorder,
}

/// A digitize-area drag in progress (sub-pixel units).
#[derive(Clone, Copy, Debug)]
pub struct AreaDrag {
    pub start: (f64, f64),
    pub end: (f64, f64),
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    lines: Vec<DataLine>,
    current_line: usize,
    axes: [Option<AxisPoint>; 4],
    x_log: bool,
    y_log: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InfoTab {
    Status,
    Data,
}

/// What to do once the user has answered "Save changes?".
#[derive(Clone, Debug)]
pub enum After {
    OpenImage(Option<PathBuf>),
    OpenWorkspace(Option<PathBuf>),
    Close,
    Exit,
}

pub struct App {
    pub ws: Workspace,
    pub ws_path: Option<PathBuf>,
    pub image: Option<RgbImage>,
    pub image_path: Option<PathBuf>,
    pub tiles: Option<Tiles>,
    pub view: View,
    pub settings: Settings,
    pub mode: Mode,
    pub modified: bool,
    pub undo: Vec<Snapshot>,
    /// Transient message for the status bar.
    pub message: Option<String>,
    /// Image position (continuous pixel coordinates) under the cursor.
    pub hover: Option<Pos2>,
    /// Centre of the magnifier (image coordinates).
    pub mag_center: Option<Pos2>,
    /// Selected rows of the Data tab (point indexes of the current line).
    pub selected: BTreeSet<usize>,
    /// Point selected with the right button in point capture mode.
    pub moving: Option<usize>,
    pub info_tab: InfoTab,
    pub dialogs: crate::dialogs::Dialogs,
    pub area: Option<AreaDrag>,
    /// Reorder tool: number of points already in order.
    pub reorder_next: usize,
    pub show_help: bool,
    pub show_about: bool,
    pub exit_confirmed: bool,
    /// Image position of the last right click (context menu).
    pub ctx_menu_pos: Option<Pos2>,
    pub ctx: egui::Context,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, open: Option<PathBuf>) -> Self {
        let settings: Settings =
            cc.storage.and_then(|s| eframe::get_value(s, STORAGE_KEY)).unwrap_or_default();
        crate::i18n::set_lang(settings.lang);
        let mut app = Self {
            ws: Workspace::default(),
            ws_path: None,
            image: None,
            image_path: None,
            tiles: None,
            view: View::default(),
            settings,
            mode: Mode::Normal,
            modified: false,
            undo: Vec::new(),
            message: None,
            hover: None,
            mag_center: None,
            selected: BTreeSet::new(),
            moving: None,
            info_tab: InfoTab::Status,
            dialogs: Default::default(),
            area: None,
            reorder_next: 0,
            show_help: false,
            show_about: false,
            exit_confirmed: false,
            ctx_menu_pos: None,
            ctx: cc.egui_ctx.clone(),
        };
        app.reset_workspace();
        if let Some(p) = open {
            app.open_path(&p);
        }
        app
    }

    pub fn reset_workspace(&mut self) {
        let mut ws = Workspace::default();
        ws.digitize = self.settings.digitize();
        ws.auto_trace = self.settings.auto_trace();
        ws.digitize_area = self.settings.digitize_area();
        self.ws = ws;
        self.ws_path = None;
        self.undo.clear();
        self.selected.clear();
        self.moving = None;
        self.modified = false;
        self.mode = Mode::Normal;
        self.area = None;
    }

    // ---- helpers -------------------------------------------------------

    pub fn has_image(&self) -> bool {
        self.image.is_some()
    }

    pub fn image_size(&self) -> Vec2 {
        self.image.as_ref().map(|i| egui::vec2(i.width() as f32, i.height() as f32)).unwrap_or(Vec2::ZERO)
    }

    pub fn transform(&self) -> Result<Transform, ScaleError> {
        Transform::new(&self.ws)
    }

    pub fn scale_error_text(e: ScaleError) -> &'static str {
        match e {
            ScaleError::NotSet => tr("No data: set Xmin, Xmax, Ymin and Ymax"),
            ScaleError::XminEqXmax => tr("No data: Xmin = Xmax"),
            ScaleError::YminEqYmax => tr("No data: Ymin = Ymax"),
            ScaleError::Collinear => tr("No data: axes OX and OY are collinear"),
            ScaleError::XLogNotAllowed => tr("No data: log scale along X axis not allowed"),
            ScaleError::YLogNotAllowed => tr("No data: log scale along Y axis not allowed"),
        }
    }

    pub fn classifier(&self) -> Classifier {
        let l = self.ws.line();
        Classifier::new(l.line_color, l.bk_color, self.ws.digitize.line_id_method)
    }

    pub fn scorer(&self) -> Option<Scorer<'_>> {
        self.image.as_ref().map(|img| Scorer::new(img, self.classifier(), self.ws.digitize.ignore_pixels))
    }

    pub fn trace_radius(&self) -> i32 {
        self.ws.auto_trace.trace_distance.clamp(3, 50) * MF
    }

    pub fn pixel_at(&self, p: Pos2) -> Option<Rgb> {
        let img = self.image.as_ref()?;
        if p.x < 0.0 || p.y < 0.0 {
            return None;
        }
        let (x, y) = (p.x as u32, p.y as u32);
        (x < img.width() && y < img.height()).then(|| {
            let c = img.get_pixel(x, y);
            Rgb(c[0], c[1], c[2])
        })
    }

    pub fn set_message(&mut self, m: impl Into<String>) {
        self.message = Some(m.into());
    }

    pub fn error(&mut self, m: impl Into<String>) {
        self.dialogs.message = Some(m.into());
    }

    // ---- undo ------------------------------------------------------------

    pub fn checkpoint(&mut self) {
        self.undo.push(Snapshot {
            lines: self.ws.lines.clone(),
            current_line: self.ws.current_line,
            axes: self.ws.axes,
            x_log: self.ws.x_log,
            y_log: self.ws.y_log,
        });
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
        self.modified = true;
    }

    pub fn do_undo(&mut self) {
        if let Some(s) = self.undo.pop() {
            self.ws.lines = s.lines;
            self.ws.current_line = s.current_line.min(self.ws.lines.len() - 1);
            self.ws.axes = s.axes;
            self.ws.x_log = s.x_log;
            self.ws.y_log = s.y_log;
            self.selected.clear();
            self.moving = None;
            self.reorder_next = 0;
            self.modified = true;
        }
    }

    // ---- files -------------------------------------------------------------

    pub fn window_title(&self) -> String {
        let name = self
            .ws_path
            .as_ref()
            .or(self.image_path.as_ref())
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned());
        match name {
            Some(n) => format!("{}{} - GetData", n, if self.modified { "*" } else { "" }),
            None => "GetData".into(),
        }
    }

    /// Ask to save changes before `after`; runs it directly when nothing changed.
    pub fn guard(&mut self, after: After) {
        if self.modified && (self.has_image() || self.ws.total_points() > 0) {
            self.dialogs.save_changes = Some(after);
        } else {
            self.run_after(after);
        }
    }

    pub fn run_after(&mut self, after: After) {
        match after {
            After::OpenImage(p) => self.open_image_dialog(p),
            After::OpenWorkspace(p) => self.open_workspace_dialog(p),
            After::Close => self.close(),
            After::Exit => {
                self.exit_confirmed = true;
                self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn start_dir(&self, d: &str) -> Option<PathBuf> {
        (!d.is_empty()).then(|| PathBuf::from(d)).filter(|p| p.is_dir())
    }

    pub fn open_image_dialog(&mut self, path: Option<PathBuf>) {
        let path = path.or_else(|| {
            let mut d = rfd::FileDialog::new()
                .set_title(tr("Open Image"))
                .add_filter(tr("Image files"), &["jpg", "jpeg", "tif", "tiff", "bmp", "pcx", "png", "gif"])
                .add_filter("JPEG/JFIF (*.jpg)", &["jpg", "jpeg"])
                .add_filter("TIFF Bitmap (*.tif)", &["tif", "tiff"])
                .add_filter("Windows Bitmap (*.bmp)", &["bmp"])
                .add_filter("PaintBrush PCX (*.pcx)", &["pcx"])
                .add_filter("PNG (*.png)", &["png"])
                .add_filter(tr("All files"), &["*"]);
            if let Some(dir) = self.start_dir(&self.settings.dir_graphs) {
                d = d.set_directory(dir);
            }
            d.pick_file()
        });
        let Some(path) = path else { return };
        match imageio::load(&path) {
            Ok(img) => {
                self.reset_workspace();
                self.set_image(img, &path);
                let (line, bk) = digitize::acquire_colors(self.image.as_ref().expect("just set"));
                let l = self.ws.line_mut();
                l.line_color = line;
                l.bk_color = bk;
                self.ws.image_file = path.to_string_lossy().into_owned();
                if let Some(d) = path.parent() {
                    self.settings.dir_graphs = d.to_string_lossy().into_owned();
                }
                self.settings.add_recent(&path.to_string_lossy());
            }
            Err(e) => self.error(format!("{}\n{e}", trf("Unable to open file \"{}\"", &[&path.display()]))),
        }
    }

    fn set_image(&mut self, img: RgbImage, path: &Path) {
        self.tiles = Some(Tiles::new(&self.ctx, &img));
        self.ws.width = img.width();
        self.ws.height = img.height();
        self.image = Some(img);
        self.image_path = Some(path.to_path_buf());
        self.view.fit = true;
    }

    pub fn open_workspace_dialog(&mut self, path: Option<PathBuf>) {
        let path = path.or_else(|| {
            let mut d = rfd::FileDialog::new()
                .set_title(tr("Open Workspace"))
                .add_filter(tr("GetData Workspace"), &["gdw"])
                .add_filter(tr("All files"), &["*"]);
            if let Some(dir) = self.start_dir(&self.settings.dir_workspaces) {
                d = d.set_directory(dir);
            }
            d.pick_file()
        });
        let Some(path) = path else { return };
        let ws = match gdw::load(&path) {
            Ok(ws) => ws,
            Err(gdw::GdwError::Io(e)) => {
                return self.error(format!("{}\n{e}", trf("Unable to open file \"{}\"", &[&path.display()])));
            }
            Err(_) => return self.error(trf("File \"{}\" is not a valid GetData workspace.", &[&path.display()])),
        };
        let img_path = gdw::resolve_image_path(&ws.image_file, &path);
        let img = match imageio::load(&img_path) {
            Ok(i) => Some((i, img_path)),
            Err(_) => {
                // Offer to locate the image.
                let ask = rfd::MessageDialog::new()
                    .set_level(rfd::MessageLevel::Warning)
                    .set_title("GetData")
                    .set_description(trf("Unable to open file \"{}\".\nTry to locate this file?", &[&img_path.display()]))
                    .set_buttons(rfd::MessageButtons::YesNo)
                    .show();
                if ask == rfd::MessageDialogResult::Yes {
                    rfd::FileDialog::new()
                        .set_title(tr("Open Image"))
                        .add_filter(tr("Image files"), &["jpg", "jpeg", "tif", "tiff", "bmp", "pcx", "png", "gif"])
                        .pick_file()
                        .and_then(|p| imageio::load(&p).ok().map(|i| (i, p)))
                } else {
                    None
                }
            }
        };
        self.reset_workspace();
        self.ws = ws;
        if let Some((img, p)) = img {
            self.set_image(img, &p);
            self.ws.image_file = p.to_string_lossy().into_owned();
        } else {
            self.image = None;
            self.tiles = None;
            self.image_path = None;
        }
        match self.ws.zoom {
            ZoomSetting::FitToWindow => self.view.fit = true,
            ZoomSetting::Scale(s) => {
                self.view.fit = false;
                self.view.scale = (s as f32 / 100.0).clamp(crate::view::MIN_SCALE, crate::view::MAX_SCALE);
                self.view.center = pos2(self.ws.position.0 as f32, self.ws.position.1 as f32);
            }
        }
        self.ws_path = Some(path.clone());
        if let Some(d) = path.parent() {
            self.settings.dir_workspaces = d.to_string_lossy().into_owned();
        }
        self.settings.add_recent(&path.to_string_lossy());
    }

    /// Open an image or a workspace depending on the file extension.
    pub fn open_path(&mut self, p: &Path) {
        let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if ext == "gdw" {
            self.guard(After::OpenWorkspace(Some(p.to_path_buf())));
        } else {
            self.guard(After::OpenImage(Some(p.to_path_buf())));
        }
    }

    pub fn save_workspace(&mut self, save_as: bool) -> bool {
        let path = match (&self.ws_path, save_as) {
            (Some(p), false) => Some(p.clone()),
            _ => {
                let mut d = rfd::FileDialog::new()
                    .set_title(tr("Save Workspace"))
                    .add_filter(tr("GetData Workspace"), &["gdw"]);
                if let Some(dir) = self.start_dir(&self.settings.dir_workspaces) {
                    d = d.set_directory(dir);
                }
                if let Some(stem) = self.image_path.as_ref().and_then(|p| p.file_stem()) {
                    d = d.set_file_name(format!("{}.gdw", stem.to_string_lossy()));
                }
                d.save_file().map(|p| if p.extension().is_none() { p.with_extension("gdw") } else { p })
            }
        };
        let Some(path) = path else { return false };
        let mut ws = self.ws.clone();
        if let (Some(img), Some(dir)) = (&self.image_path, path.parent()) {
            ws.image_file = gdw::relative_path(img, dir);
        }
        ws.zoom = if self.view.fit { ZoomSetting::FitToWindow } else { ZoomSetting::Scale((self.view.scale * 100.0) as f64) };
        ws.position = (self.view.center.x as i32, self.view.center.y as i32);
        match gdw::save(&ws, &path) {
            Ok(()) => {
                self.ws_path = Some(path.clone());
                self.modified = false;
                self.set_message(tr("Workspace saved"));
                if let Some(d) = path.parent() {
                    self.settings.dir_workspaces = d.to_string_lossy().into_owned();
                }
                self.settings.add_recent(&path.to_string_lossy());
                true
            }
            Err(e) => {
                self.error(format!("{}\n{e}", trf("Unable to open file \"{}\"", &[&path.display()])));
                false
            }
        }
    }

    pub fn close(&mut self) {
        self.reset_workspace();
        self.image = None;
        self.tiles = None;
        self.image_path = None;
    }

    // ---- export --------------------------------------------------------------

    pub fn copy_to_clipboard(&mut self) {
        match self.transform() {
            Ok(t) => {
                let text = export::to_text(&self.ws, &t, &self.settings.export_clipboard);
                self.ctx.copy_text(text);
                self.set_message(tr("Data copied to buffer"));
            }
            Err(_) => self.error(tr("Export not possible: Xmin, Xmax, Ymin and Ymax should be set")),
        }
    }

    pub fn export_to(&mut self, fmt: FileFormat) {
        let t = match self.transform() {
            Ok(t) => t,
            Err(_) => return self.error(tr("Export not possible: Xmin, Xmax, Ymin and Ymax should be set")),
        };
        let mut d = rfd::FileDialog::new().set_title(tr("Export data")).add_filter(fmt.filter_name(), &[fmt.ext()]);
        if let Some(dir) = self.start_dir(&self.settings.dir_export) {
            d = d.set_directory(dir);
        }
        if let Some(stem) = self.image_path.as_ref().and_then(|p| p.file_stem()) {
            d = d.set_file_name(format!("{}.{}", stem.to_string_lossy(), fmt.ext()));
        }
        let Some(mut path) = d.save_file() else { return };
        if path.extension().is_none() {
            path.set_extension(fmt.ext());
        }
        if let Err(e) = export::export_file(&self.ws, &t, fmt, &path, &self.settings.export_file) {
            self.error(format!("{}\n{e}", trf("Unable to open file \"{}\"", &[&path.display()])));
        } else if let Some(dir) = path.parent() {
            self.settings.dir_export = dir.to_string_lossy().into_owned();
        }
    }

    // ---- scale -----------------------------------------------------------------

    pub fn set_default_axes(&mut self) {
        if !self.has_image() {
            return;
        }
        self.checkpoint();
        let (w, h) = (self.ws.width as i32, self.ws.height as i32);
        self.ws.axes = [
            Some(AxisPoint { pos: Pt::from_pixel(0, h - 1), value: 0.0 }),
            Some(AxisPoint { pos: Pt::from_pixel(w - 1, h - 1), value: (w - 1) as f64 }),
            Some(AxisPoint { pos: Pt::from_pixel(0, h - 1), value: 0.0 }),
            Some(AxisPoint { pos: Pt::from_pixel(0, 0), value: (h - 1) as f64 }),
        ];
        self.ws.x_log = false;
        self.ws.y_log = false;
    }

    pub fn set_axis(&mut self, id: AxisId, pos: Pt, value: f64) {
        self.checkpoint();
        self.ws.axes[id as usize] = Some(AxisPoint { pos, value });
    }

    pub fn remove_axis(&mut self, id: AxisId) {
        self.checkpoint();
        self.ws.axes[id as usize] = None;
    }

    // ---- points ----------------------------------------------------------------

    /// Add a point to the current line unless an identical one exists.
    pub fn push_point(&mut self, p: Pt) -> bool {
        let line = self.ws.line_mut();
        if line.points.contains(&p) {
            return false;
        }
        line.points.push(p);
        true
    }

    pub fn add_point(&mut self, p: Pt) {
        if self.ws.line().points.contains(&p) {
            return;
        }
        self.checkpoint();
        self.push_point(p);
    }

    pub fn add_points(&mut self, pts: &[Pt]) {
        if pts.is_empty() {
            return;
        }
        self.checkpoint();
        for &p in pts {
            self.push_point(p);
        }
    }

    pub fn remove_points(&mut self, idx: &BTreeSet<usize>) {
        if idx.is_empty() {
            return;
        }
        self.checkpoint();
        let line = self.ws.line_mut();
        let mut i = 0;
        line.points.retain(|_| {
            let keep = !idx.contains(&i);
            i += 1;
            keep
        });
        self.selected.clear();
        self.moving = None;
    }

    /// Nearest point of the current line within `max_dist` image pixels.
    pub fn nearest_point(&self, p: Pos2, max_dist: f32) -> Option<usize> {
        let target = Pt::from_image(p.x as f64, p.y as f64);
        let lim = (max_dist as f64 * MF as f64).powi(2);
        self.ws
            .line()
            .points
            .iter()
            .enumerate()
            .map(|(i, q)| (i, q.dist2(target)))
            .filter(|&(_, d)| d <= lim)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    /// Point placed at an image position: pixel centre, or exact sub-pixel when `exact`.
    pub fn point_at(&self, p: Pos2, exact: bool) -> Pt {
        if exact {
            Pt::from_image(p.x as f64, p.y as f64)
        } else {
            Pt::from_pixel(p.x.floor() as i32, p.y.floor() as i32)
        }
    }

    pub fn line_center_at(&self, p: Pos2) -> Option<LineCenter> {
        let s = self.scorer()?;
        let c = digitize::line_center(&s, self.point_at(p, false), self.trace_radius());
        (c.weight > 0).then_some(c)
    }

    // ---- automatic digitizing ----------------------------------------------------

    pub fn auto_trace_at(&mut self, p: Pos2, dirs: Option<(f64, f64)>) {
        let Some(s) = self.scorer() else { return };
        let click = self.point_at(p, false);
        if !s.is_line(click) {
            return;
        }
        let start = digitize::line_center(&s, click, self.trace_radius());
        let prm = TraceParams { radius: self.trace_radius(), smooth: self.ws.auto_trace.smooth_lines };
        let dirs = match dirs {
            Some((from, to)) => TraceDirections::Sector { from, to },
            None => TraceDirections::All,
        };
        let pts = digitize::auto_trace(&s, start, dirs, &prm, &self.ws.line().points);
        self.add_points(&pts);
    }

    /// Directions found around a trace start point (for the direction dialog).
    pub fn trace_start(&self, p: Pos2) -> Option<(LineCenter, Vec<f64>)> {
        let s = self.scorer()?;
        let click = self.point_at(p, false);
        if !s.is_line(click) {
            return None;
        }
        let start = digitize::line_center(&s, click, self.trace_radius());
        let dirs = digitize::directions_at(&s, start.pos, self.trace_radius());
        Some((start, dirs))
    }

    pub fn auto_trace_from(&mut self, start: LineCenter, from: f64, to: f64) {
        let Some(s) = self.scorer() else { return };
        let prm = TraceParams { radius: self.trace_radius(), smooth: self.ws.auto_trace.smooth_lines };
        let pts = digitize::auto_trace(&s, start, TraceDirections::Sector { from, to }, &prm, &self.ws.line().points);
        self.add_points(&pts);
    }

    /// Grid lines of a digitize-area drag, as segments in sub-pixel units.
    pub fn area_segments(&self, a: &AreaDrag) -> Vec<((f64, f64), (f64, f64))> {
        let s = &self.ws.digitize_area;
        let mut out = Vec::new();
        match s.area_type {
            AreaType::Rectangular => {
                let (x1, y1) = a.start;
                let (x2, y2) = a.end;
                let (mut c, mut sn) = (s.rotate_angle.cos(), s.rotate_angle.sin());
                if sn.abs() < 0.08 {
                    sn = 0.0;
                    c = c.signum();
                } else if c.abs() < 0.08 {
                    c = 0.0;
                    sn = sn.signum();
                }
                // Extent across the grid lines (a) and along them (len).
                let len = -(x2 - x1) * sn - (y2 - y1) * c;
                let across = (x2 - x1) * c - (y2 - y1) * sn;
                let step = (s.dx.max(0.1) * MF as f64) * if across <= 0.0 { -1.0 } else { 1.0 };
                // Grid lines pass through the centre of pixel (0, 0).
                let half = (MF / 2) as f64;
                let base = (half - x1) * c - (half - y1) * sn;
                let offset = base - (base / step).floor() * step;
                if offset.abs() <= across.abs() {
                    let n = (((across - offset) / step).abs().floor() as usize + 1).min(5000);
                    for i in 0..n {
                        let t = i as f64 * step + offset;
                        let p0 = (x1 + t * c, y1 - t * sn);
                        let p1 = (p0.0 - len * sn, p0.1 - len * c);
                        out.push((p0, p1));
                    }
                }
            }
            AreaType::RelatedToAxes => {
                let Ok(t) = self.transform() else { return out };
                let img = |p: (f64, f64)| (p.0 / MF as f64, p.1 / MF as f64);
                let (u1, v1) = t.fractions(img(a.start));
                let (u2, v2) = t.fractions(img(a.end));
                let unit = |p: (f64, f64)| (p.0 * MF as f64, p.1 * MF as f64);
                let lines = |lo: f64, hi: f64, step: f64, origin: f64, seg: &mut dyn FnMut(f64)| {
                    if !(step > 0.0) || !step.is_finite() {
                        return;
                    }
                    let first = ((lo - origin) / step).ceil() * step + origin;
                    let hi = hi + step * 1e-10;
                    let mut k = 0;
                    loop {
                        let v = first + k as f64 * step;
                        if v >= hi || k > 5000 {
                            break;
                        }
                        seg(v);
                        k += 1;
                    }
                };
                if s.grid_type == 0 {
                    let du = t.du_of(s.delta_x, s.delta_x_units == GridUnits::Points);
                    let origin = if s.shift_x { t.u_of(s.go_through_x) } else { u2 };
                    lines(u1.min(u2), u1.max(u2), du, origin, &mut |u| {
                        out.push((unit(t.from_fractions(u, v1)), unit(t.from_fractions(u, v2))));
                    });
                } else {
                    let dv = t.dv_of(s.delta_y, s.delta_y_units == GridUnits::Points);
                    let origin = if s.shift_y { t.v_of(s.go_through_y) } else { v2 };
                    lines(v1.min(v2), v1.max(v2), dv, origin, &mut |v| {
                        out.push((unit(t.from_fractions(u1, v)), unit(t.from_fractions(u2, v))));
                    });
                }
            }
        }
        out
    }

    pub fn digitize_area(&mut self, a: &AreaDrag) {
        let segs = self.area_segments(a);
        let Some(s) = self.scorer() else { return };
        let at_boundary = self.ws.digitize_area.at_boundary;
        let mut pts = Vec::new();
        for (p0, p1) in segs {
            let r = |v: f64| (v + 0.5).floor() as i32;
            digitize::digitize_segment(&s, (r(p0.0), r(p0.1)), (r(p1.0), r(p1.1)), at_boundary, &mut pts);
        }
        self.add_points(&pts);
    }

    pub fn acquire_colors(&mut self) {
        let Some(img) = &self.image else { return };
        let (line, bk) = digitize::acquire_colors(img);
        let l = self.ws.line_mut();
        l.line_color = line;
        l.bk_color = bk;
        self.modified = true;
    }

    // ---- reorder & eraser --------------------------------------------------------

    /// Indexes of current-line points within `radius` screen pixels of screen position `s`.
    pub fn points_near_screen(&self, s: Pos2, radius: f32) -> Vec<usize> {
        self.ws
            .line()
            .points
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                let (x, y) = p.to_image();
                self.view.to_screen(pos2(x as f32, y as f32)).distance(s) <= radius
            })
            .map(|(i, _)| i)
            .collect()
    }

    pub fn erase_at(&mut self, s: Pos2, first: bool) {
        let idx: BTreeSet<usize> = self.points_near_screen(s, self.settings.eraser_size).into_iter().collect();
        if idx.is_empty() {
            return;
        }
        if first {
            self.checkpoint();
        }
        let line = self.ws.line_mut();
        let mut i = 0;
        line.points.retain(|_| {
            let keep = !idx.contains(&i);
            i += 1;
            keep
        });
        self.selected.clear();
        self.modified = true;
    }

    /// Move the touched points to the next positions of the line order.
    pub fn reorder_at(&mut self, s: Pos2, skip: bool) {
        let mut touched: Vec<usize> = self
            .points_near_screen(s, self.settings.reorder_size)
            .into_iter()
            .filter(|&i| i >= self.reorder_next)
            .collect();
        if touched.is_empty() {
            return;
        }
        touched.sort_unstable();
        if skip {
            // Ctrl: continue from the middle of the line.
            self.reorder_next = touched.last().copied().unwrap_or(0) + 1;
            return;
        }
        let pts = &mut self.ws.lines[self.ws.current_line].points;
        let moved: Vec<Pt> = touched.iter().map(|&i| pts[i]).collect();
        for &i in touched.iter().rev() {
            pts.remove(i);
        }
        let at = self.reorder_next.min(pts.len());
        for (k, p) in moved.into_iter().enumerate() {
            pts.insert(at + k, p);
        }
        self.reorder_next += touched.len();
        self.modified = true;
        self.set_message(format!(" {} ", self.reorder_next));
    }

    // ---- lines -----------------------------------------------------------------

    /// Points colour proposed for a new line: the palette entry after the one
    /// closest to the last line's colour.
    pub fn next_point_color(&self) -> Rgb {
        let Some(last) = self.ws.lines.last() else { return POINT_COLORS[0] };
        let c = last.pt_color;
        let d = |p: &Rgb| {
            let (a, b, e) = (p.0 as i32 - c.0 as i32, p.1 as i32 - c.1 as i32, p.2 as i32 - c.2 as i32);
            a * a + b * b + e * e
        };
        let i = (0..POINT_COLORS.len()).min_by_key(|&i| d(&POINT_COLORS[i])).unwrap_or(0);
        POINT_COLORS[(i + 1) % POINT_COLORS.len()]
    }

    pub fn add_line(&mut self, caption: String, line_type: LineType, color: Rgb) {
        self.checkpoint();
        let cur = self.ws.line().clone();
        let mut l = DataLine::new(self.ws.lines.len(), cur.line_color, cur.bk_color);
        l.caption = caption;
        l.line_type = line_type;
        l.pt_color = color;
        self.ws.lines.push(l);
        self.ws.current_line = self.ws.lines.len() - 1;
        self.selected.clear();
        self.reorder_next = 0;
    }

    pub fn remove_line(&mut self, i: usize) {
        self.checkpoint();
        self.ws.lines.remove(i);
        if self.ws.lines.is_empty() {
            let mut l = DataLine::new(0, Rgb::BLACK, Rgb::WHITE);
            if let Some(img) = &self.image {
                (l.line_color, l.bk_color) = digitize::acquire_colors(img);
            }
            self.ws.lines.push(l);
        }
        self.ws.current_line = self.ws.current_line.min(self.ws.lines.len() - 1);
        self.selected.clear();
    }

    pub fn remove_all_data(&mut self) {
        self.checkpoint();
        let first = self.ws.lines[0].clone();
        let mut l = DataLine::new(0, first.line_color, first.bk_color);
        l.pt_color = first.pt_color;
        self.ws.lines = vec![l];
        self.ws.current_line = 0;
        self.ws.axes = [None; 4];
        self.selected.clear();
        self.reorder_next = 0;
    }

    pub fn set_mode(&mut self, m: Mode) {
        self.mode = m;
        self.moving = None;
        self.area = None;
        self.reorder_next = 0;
    }

    pub fn save_settings(&self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, STORAGE_KEY, &self.settings);
    }
}
