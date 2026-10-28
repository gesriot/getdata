//! The loaded image split into GPU textures (so images larger than the
//! maximum texture size can still be shown).

use eframe::egui::{self, Color32, ColorImage, Context, Painter, Rect, TextureHandle, TextureOptions, pos2};
use image::RgbImage;

const TILE: u32 = 2048;

pub struct Tiles {
    tiles: Vec<(Rect, TextureHandle)>,
}

impl Tiles {
    pub fn new(ctx: &Context, img: &RgbImage) -> Self {
        let mut tiles = Vec::new();
        let (w, h) = img.dimensions();
        let mut y = 0;
        while y < h {
            let th = TILE.min(h - y);
            let mut x = 0;
            while x < w {
                let tw = TILE.min(w - x);
                let mut pixels = Vec::with_capacity((tw * th) as usize);
                for yy in y..y + th {
                    for xx in x..x + tw {
                        let p = img.get_pixel(xx, yy);
                        pixels.push(Color32::from_rgb(p[0], p[1], p[2]));
                    }
                }
                let ci = ColorImage::new([tw as usize, th as usize], pixels);
                let tex = ctx.load_texture(format!("img_{x}_{y}"), ci, TextureOptions::NEAREST);
                let r = Rect::from_min_max(pos2(x as f32, y as f32), pos2((x + tw) as f32, (y + th) as f32));
                tiles.push((r, tex));
                x += TILE;
            }
            y += TILE;
        }
        Self { tiles }
    }

    /// Paint all tiles; `to_screen` maps image coordinates to screen.
    pub fn paint(&self, painter: &Painter, to_screen: impl Fn(egui::Pos2) -> egui::Pos2, tint: Color32) {
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        for (r, tex) in &self.tiles {
            let sr = Rect::from_min_max(to_screen(r.min), to_screen(r.max));
            if sr.intersects(painter.clip_rect()) {
                painter.image(tex.id(), sr, uv, tint);
            }
        }
    }
}
