//! Application settings kept between sessions.

use serde::{Deserialize, Serialize};

use crate::export::TextExportSettings;
use crate::i18n::Lang;
use crate::model::{AutoTraceSettings, DigitizeAreaSettings, DigitizeSettings, LineIdMethod};

pub const STORAGE_KEY: &str = "getdata-settings";
pub const MAX_RECENT: usize = 8;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub lang: Lang,
    pub export_file: TextExportSettings,
    pub export_clipboard: TextExportSettings,
    pub ignore_pixels: bool,
    pub line_id_method: i32,
    pub trace_distance: i32,
    pub smooth_lines: bool,
    pub at_boundary: bool,
    pub eraser_size: f32,
    pub reorder_size: f32,
    pub show_all_lines: bool,
    pub connect_points: bool,
    pub show_info: bool,
    pub show_magnifier: bool,
    pub show_image: bool,
    pub recent: Vec<String>,
    pub dir_graphs: String,
    pub dir_workspaces: String,
    pub dir_export: String,
    /// Index into the export formats of the last export.
    pub export_filter: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            lang: Lang::default(),
            export_file: TextExportSettings::file_default(),
            export_clipboard: TextExportSettings::clipboard_default(),
            ignore_pixels: true,
            line_id_method: LineIdMethod::Combined as i32,
            trace_distance: 15,
            smooth_lines: true,
            at_boundary: false,
            eraser_size: 15.0,
            reorder_size: 15.0,
            show_all_lines: true,
            connect_points: true,
            show_info: true,
            show_magnifier: true,
            show_image: true,
            recent: Vec::new(),
            dir_graphs: String::new(),
            dir_workspaces: String::new(),
            dir_export: String::new(),
            export_filter: 0,
        }
    }
}

impl Settings {
    pub fn digitize(&self) -> DigitizeSettings {
        DigitizeSettings { ignore_pixels: self.ignore_pixels, line_id_method: LineIdMethod::from_i32(self.line_id_method) }
    }

    pub fn auto_trace(&self) -> AutoTraceSettings {
        AutoTraceSettings { trace_distance: self.trace_distance, smooth_lines: self.smooth_lines }
    }

    pub fn digitize_area(&self) -> DigitizeAreaSettings {
        DigitizeAreaSettings { at_boundary: self.at_boundary, ..Default::default() }
    }

    pub fn add_recent(&mut self, path: &str) {
        self.recent.retain(|p| !p.eq_ignore_ascii_case(path));
        self.recent.insert(0, path.to_string());
        self.recent.truncate(MAX_RECENT);
    }
}
