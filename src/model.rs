//! Core data model: lines of data points, axes calibration and digitizer settings.
//!
//! Point coordinates are stored as integer sub-pixel units, `MF` units per image pixel (MF = 9).  The centre of image
//! pixel `p` is `p * MF + MF / 2`.

pub const MF: i32 = 9;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Pt {
    pub x: i32,
    pub y: i32,
}

impl Pt {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Centre of image pixel (px, py).
    pub const fn from_pixel(px: i32, py: i32) -> Self {
        Self { x: px * MF + MF / 2, y: py * MF + MF / 2 }
    }

    /// Continuous image coordinates (pixel `p` spans `[p, p+1)`).
    pub fn to_image(self) -> (f64, f64) {
        ((self.x as f64 + 0.5) / MF as f64, (self.y as f64 + 0.5) / MF as f64)
    }

    pub fn from_image(x: f64, y: f64) -> Self {
        Self { x: (x * MF as f64).floor() as i32, y: (y * MF as f64).floor() as i32 }
    }

    /// Image pixel that contains this point.
    pub fn pixel(self) -> (i32, i32) {
        (self.x.div_euclid(MF), self.y.div_euclid(MF))
    }

    pub fn dist2(self, o: Pt) -> f64 {
        let dx = (self.x - o.x) as f64;
        let dy = (self.y - o.y) as f64;
        dx * dx + dy * dy
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const WHITE: Rgb = Rgb(255, 255, 255);
    pub const BLACK: Rgb = Rgb(0, 0, 0);

    pub fn to_color32(self) -> eframe::egui::Color32 {
        eframe::egui::Color32::from_rgb(self.0, self.1, self.2)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LineType {
    #[default]
    Polyline = 0,
    ClosedPolyline = 1,
    SeparatePoints = 2,
}

impl LineType {
    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => LineType::ClosedPolyline,
            2 => LineType::SeparatePoints,
            _ => LineType::Polyline,
        }
    }
}

/// Default point colours for new lines, cycled by line index.
pub const POINT_COLORS: [Rgb; 8] = [
    Rgb(220, 70, 220),
    Rgb(255, 20, 160),
    Rgb(255, 128, 64),
    Rgb(128, 0, 255),
    Rgb(230, 220, 20),
    Rgb(0, 160, 255),
    Rgb(0, 200, 80),
    Rgb(255, 60, 60),
];

#[derive(Clone, Debug, PartialEq)]
pub struct DataLine {
    pub caption: String,
    pub line_color: Rgb,
    pub bk_color: Rgb,
    pub pt_color: Rgb,
    pub line_type: LineType,
    pub points: Vec<Pt>,
}

impl DataLine {
    pub fn new(index: usize, line_color: Rgb, bk_color: Rgb) -> Self {
        Self {
            caption: format!("Line #{}", index + 1),
            line_color,
            bk_color,
            pt_color: POINT_COLORS[index % POINT_COLORS.len()],
            line_type: LineType::Polyline,
            points: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisId {
    Xmin = 0,
    Xmax = 1,
    Ymin = 2,
    Ymax = 3,
}

impl AxisId {
    pub const ALL: [AxisId; 4] = [AxisId::Xmin, AxisId::Xmax, AxisId::Ymin, AxisId::Ymax];

    pub fn name(self) -> &'static str {
        match self {
            AxisId::Xmin => "Xmin",
            AxisId::Xmax => "Xmax",
            AxisId::Ymin => "Ymin",
            AxisId::Ymax => "Ymax",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisPoint {
    pub pos: Pt,
    pub value: f64,
}

/// How the program decides whether a pixel belongs to the line or the background.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LineIdMethod {
    ByColor = 0,
    ByIntensity = 1,
    #[default]
    Combined = 2,
}

impl LineIdMethod {
    pub fn from_i32(v: i32) -> Self {
        match v {
            0 => LineIdMethod::ByColor,
            1 => LineIdMethod::ByIntensity,
            _ => LineIdMethod::Combined,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DigitizeSettings {
    pub ignore_pixels: bool,
    pub line_id_method: LineIdMethod,
}

impl Default for DigitizeSettings {
    fn default() -> Self {
        Self { ignore_pixels: true, line_id_method: LineIdMethod::Combined }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AutoTraceSettings {
    /// Distance between data points, in screen pixels (3..=50).
    pub trace_distance: i32,
    pub smooth_lines: bool,
}

impl Default for AutoTraceSettings {
    fn default() -> Self {
        Self { trace_distance: 15, smooth_lines: true }
    }
}

/// Area grid kind for "Digitize area" and the displayed grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AreaType {
    /// Rectangular area in image space (may be rotated).
    #[default]
    Rectangular = 0,
    /// Area whose sides follow the axes.
    RelatedToAxes = 1,
}

/// Units of the grid step: in pixels or in axis values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GridUnits {
    Points = 0,
    #[default]
    AxisValues = 1,
}

impl GridUnits {
    pub fn from_i32(v: i32) -> Self {
        if v == 0 { GridUnits::Points } else { GridUnits::AxisValues }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DigitizeAreaSettings {
    /// false: points in the middle of a line, true: at the area boundary.
    pub at_boundary: bool,
    pub area_type: AreaType,
    /// 0 = X grid, 1 = Y grid.
    pub grid_type: i32,
    pub go_through_x: f64,
    pub go_through_y: f64,
    pub shift_x: bool,
    pub shift_y: bool,
    /// Rotation angle of the rectangular grid in radians.
    pub rotate_angle: f64,
    /// Grid step for the rectangular area, in pixels.
    pub dx: f64,
    /// Grid steps for the axes-related area.
    pub delta_x: f64,
    pub delta_y: f64,
    pub delta_x_units: GridUnits,
    pub delta_y_units: GridUnits,
}

impl Default for DigitizeAreaSettings {
    fn default() -> Self {
        Self {
            at_boundary: false,
            area_type: AreaType::Rectangular,
            grid_type: 0,
            go_through_x: 0.0,
            go_through_y: 0.0,
            shift_x: false,
            shift_y: false,
            rotate_angle: 0.0,
            dx: 8.0,
            delta_x: 1.0,
            delta_y: 1.0,
            delta_x_units: GridUnits::AxisValues,
            delta_y_units: GridUnits::AxisValues,
        }
    }
}

/// "View => Show grid" settings.
#[derive(Clone, Debug, PartialEq)]
pub struct GridSettings {
    pub show_grid: bool,
    pub area_type: AreaType,
    pub delta_x: f64,
    pub delta_y: f64,
    pub delta_x_units: GridUnits,
    pub delta_y_units: GridUnits,
    pub go_through_x: f64,
    pub go_through_y: f64,
    /// Grid step in pixels when the grid is not related to the axes.
    pub dx: f64,
    pub dy: f64,
}

impl Default for GridSettings {
    fn default() -> Self {
        Self {
            show_grid: false,
            area_type: AreaType::Rectangular,
            delta_x: 1.0,
            delta_y: 1.0,
            delta_x_units: GridUnits::AxisValues,
            delta_y_units: GridUnits::AxisValues,
            go_through_x: 0.0,
            go_through_y: 0.0,
            dx: 15.0,
            dy: 15.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ZoomSetting {
    FitToWindow,
    /// Zoom in percent.
    Scale(f64),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Workspace {
    pub image_file: String,
    pub width: u32,
    pub height: u32,
    pub lines: Vec<DataLine>,
    pub current_line: usize,
    pub axes: [Option<AxisPoint>; 4],
    pub x_log: bool,
    pub y_log: bool,
    /// Image point shown in the centre of the view.
    pub position: (i32, i32),
    pub zoom: ZoomSetting,
    pub digitize: DigitizeSettings,
    pub auto_trace: AutoTraceSettings,
    pub digitize_area: DigitizeAreaSettings,
    pub grid: GridSettings,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            image_file: String::new(),
            width: 0,
            height: 0,
            lines: vec![DataLine::new(0, Rgb::BLACK, Rgb::WHITE)],
            current_line: 0,
            axes: [None; 4],
            x_log: false,
            y_log: false,
            position: (0, 0),
            zoom: ZoomSetting::FitToWindow,
            digitize: DigitizeSettings::default(),
            auto_trace: AutoTraceSettings::default(),
            digitize_area: DigitizeAreaSettings::default(),
            grid: GridSettings::default(),
        }
    }
}

impl Workspace {
    pub fn axis(&self, id: AxisId) -> Option<AxisPoint> {
        self.axes[id as usize]
    }

    pub fn line(&self) -> &DataLine {
        &self.lines[self.current_line]
    }

    pub fn line_mut(&mut self) -> &mut DataLine {
        &mut self.lines[self.current_line]
    }

    pub fn total_points(&self) -> usize {
        self.lines.iter().map(|l| l.points.len()).sum()
    }
}
