//! Mapping between image coordinates and screen coordinates of the main view.

use eframe::egui::{Pos2, Rect, Vec2, pos2, vec2};

#[derive(Clone, Debug)]
pub struct View {
    /// Screen pixels per image pixel.
    pub scale: f32,
    /// Image coordinate shown at the centre of the canvas.
    pub center: Pos2,
    pub fit: bool,
    /// Canvas rectangle from the last frame.
    pub rect: Rect,
}

impl Default for View {
    fn default() -> Self {
        Self { scale: 1.0, center: Pos2::ZERO, fit: true, rect: Rect::from_min_size(Pos2::ZERO, vec2(1.0, 1.0)) }
    }
}

pub const MIN_SCALE: f32 = 0.1;
pub const MAX_SCALE: f32 = 15.0;

/// Zoom levels (percent), used by zoom in/out and the View => Zoom menu.
pub const ZOOM_PRESETS: [f32; 28] = [
    10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0, 150.0, 200.0, 250.0, 300.0, 350.0, 400.0, 450.0, 500.0,
    600.0, 700.0, 800.0, 900.0, 1000.0, 1100.0, 1200.0, 1300.0, 1400.0, 1500.0,
];

impl View {
    pub fn to_screen(&self, p: Pos2) -> Pos2 {
        self.rect.center() + (p - self.center) * self.scale
    }

    pub fn to_image(&self, s: Pos2) -> Pos2 {
        self.center + (s - self.rect.center()) / self.scale
    }

    pub fn image_rect_on_screen(&self, size: Vec2) -> Rect {
        Rect::from_min_max(self.to_screen(Pos2::ZERO), self.to_screen(size.to_pos2()))
    }

    pub fn fit_to(&mut self, rect: Rect, size: Vec2) {
        self.rect = rect;
        if size.x <= 0.0 || size.y <= 0.0 {
            return;
        }
        self.scale = (rect.width() / size.x).min(rect.height() / size.y).clamp(MIN_SCALE, MAX_SCALE);
        self.center = (size / 2.0).to_pos2();
    }

    /// Change zoom keeping the image point under `anchor` (screen) fixed.
    pub fn zoom_at(&mut self, anchor: Pos2, new_scale: f32) {
        let new_scale = new_scale.clamp(MIN_SCALE, MAX_SCALE);
        let img = self.to_image(anchor);
        self.scale = new_scale;
        self.center = img - (anchor - self.rect.center()) / self.scale;
        self.fit = false;
    }

    pub fn pan(&mut self, screen_delta: Vec2) {
        self.center -= screen_delta / self.scale;
        self.fit = false;
    }

    /// Keep the image at least partly visible.
    pub fn clamp(&mut self, size: Vec2) {
        self.center = pos2(self.center.x.clamp(0.0, size.x), self.center.y.clamp(0.0, size.y));
    }

    pub fn zoom_in_step(&self) -> f32 {
        ZOOM_PRESETS.iter().map(|p| p / 100.0).find(|&z| z > self.scale * 1.001).unwrap_or(MAX_SCALE)
    }

    pub fn zoom_out_step(&self) -> f32 {
        ZOOM_PRESETS.iter().rev().map(|p| p / 100.0).find(|&z| z < self.scale / 1.001).unwrap_or(MIN_SCALE)
    }
}
