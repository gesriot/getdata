//! Vector toolbar icons drawn with the egui painter.

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    OpenImage,
    OpenWorkspace,
    Save,
    Copy,
    SetScale,
    AutoTrace,
    DigitizeArea,
    PointCapture,
    Eraser,
    Reorder,
    Undo,
    Fit,
    ZoomIn,
    ZoomOut,
    ShowImage,
    ShowGrid,
    ShowAllLines,
    ConnectPoints,
}

/// Paint `icon` into `rect` (about 18x18 points) with colour `c`.
pub fn paint(p: &Painter, rect: Rect, icon: Icon, c: Color32) {
    let s = rect.width().min(rect.height()) / 16.0;
    let o = rect.center() - vec2(8.0 * s, 8.0 * s);
    let at = |x: f32, y: f32| -> Pos2 { o + vec2(x * s, y * s) };
    let st = Stroke::new(1.4 * s.max(0.8), c);
    let thin = Stroke::new(1.0, c);
    let line = |pts: &[(f32, f32)], stroke: Stroke| {
        p.add(Shape::line(pts.iter().map(|&(x, y)| at(x, y)).collect(), stroke));
    };
    let r = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(at(x0, y0), at(x1, y1));
    let accent = Color32::from_rgb(230, 70, 60);
    let blue = Color32::from_rgb(70, 140, 230);
    match icon {
        Icon::OpenImage | Icon::ShowImage => {
            p.rect_stroke(r(1.5, 3.0, 14.5, 13.0), 1.0, st, eframe::egui::StrokeKind::Middle);
            p.add(Shape::convex_polygon(vec![at(3.0, 12.0), at(7.0, 6.5), at(10.0, 10.0), at(11.5, 8.5), at(13.5, 12.0)], c, Stroke::NONE));
            p.circle_filled(at(11.5, 5.5), 1.3 * s, c);
            if icon == Icon::OpenImage {
                p.circle_filled(at(13.5, 13.5), 2.6 * s, blue);
                line(&[(13.5, 12.0), (13.5, 15.0)], Stroke::new(1.2, Color32::WHITE));
                line(&[(12.0, 13.5), (15.0, 13.5)], Stroke::new(1.2, Color32::WHITE));
            }
        }
        Icon::OpenWorkspace => {
            line(&[(1.5, 13.5), (1.5, 3.0), (6.0, 3.0), (7.5, 5.0), (13.5, 5.0), (13.5, 7.0)], st);
            p.add(Shape::convex_polygon(vec![at(1.5, 13.5), at(4.0, 7.5), at(15.5, 7.5), at(13.0, 13.5)], c.gamma_multiply(0.5), st));
        }
        Icon::Save => {
            line(&[(2.0, 2.0), (12.0, 2.0), (14.0, 4.0), (14.0, 14.0), (2.0, 14.0), (2.0, 2.0)], st);
            p.rect_filled(r(4.5, 2.0, 11.0, 6.0), 0.0, c);
            p.rect_stroke(r(4.0, 9.0, 12.0, 14.0), 0.0, thin, eframe::egui::StrokeKind::Middle);
        }
        Icon::Copy => {
            p.rect_stroke(r(1.5, 1.5, 10.0, 11.0), 1.0, st, eframe::egui::StrokeKind::Middle);
            p.rect_filled(r(6.0, 5.0, 14.5, 14.5), 1.0, c.gamma_multiply(0.35));
            p.rect_stroke(r(6.0, 5.0, 14.5, 14.5), 1.0, st, eframe::egui::StrokeKind::Middle);
        }
        Icon::SetScale => {
            line(&[(3.0, 1.5), (3.0, 13.0), (14.5, 13.0)], st);
            for k in [5.5, 8.0, 10.5] {
                line(&[(k, 12.0), (k, 14.0)], thin);
                line(&[(2.0, 13.0 - (k - 3.0)), (4.0, 13.0 - (k - 3.0))], thin);
            }
            p.circle_stroke(at(11.0, 4.5), 2.2 * s, Stroke::new(1.2, accent));
            line(&[(11.0, 1.0), (11.0, 8.0)], Stroke::new(1.0, accent));
            line(&[(7.5, 4.5), (14.5, 4.5)], Stroke::new(1.0, accent));
        }
        Icon::AutoTrace => {
            let pts: Vec<(f32, f32)> = (0..=24)
                .map(|i| {
                    let x = 1.0 + i as f32 * 14.0 / 24.0;
                    (x, 8.0 - 4.5 * ((x - 1.0) / 14.0 * std::f32::consts::TAU).sin())
                })
                .collect();
            line(&pts, Stroke::new(1.0, c.gamma_multiply(0.6)));
            for &(x, y) in pts.iter().step_by(4) {
                p.circle_filled(at(x, y), 1.5 * s, accent);
            }
        }
        Icon::DigitizeArea => {
            for x in [3.0, 6.5, 10.0, 13.5] {
                p.add(Shape::dashed_line(&[at(x, 1.5), at(x, 14.5)], Stroke::new(1.0, blue), 2.0 * s, 1.5 * s));
            }
            let curve: Vec<(f32, f32)> = (0..=16).map(|i| (i as f32, 12.0 - (i as f32 / 16.0).powf(1.5) * 9.0)).collect();
            line(&curve, st);
            for x in [3.0f32, 6.5, 10.0, 13.5] {
                p.circle_filled(at(x, 12.0 - (x / 16.0).powf(1.5) * 9.0), 1.6 * s, accent);
            }
        }
        Icon::PointCapture => {
            line(&[(8.0, 1.0), (8.0, 6.0)], st);
            line(&[(8.0, 10.0), (8.0, 15.0)], st);
            line(&[(1.0, 8.0), (6.0, 8.0)], st);
            line(&[(10.0, 8.0), (15.0, 8.0)], st);
            p.circle_filled(at(8.0, 8.0), 1.8 * s, accent);
        }
        Icon::Eraser => {
            let body = [at(5.0, 14.0), at(1.5, 10.5), at(9.5, 2.5), at(14.5, 7.5), at(8.0, 14.0)];
            p.add(Shape::convex_polygon(body.to_vec(), c.gamma_multiply(0.25), st));
            p.add(Shape::convex_polygon(vec![at(1.5, 10.5), at(5.5, 6.5), at(10.5, 11.5), at(8.0, 14.0), at(5.0, 14.0)], accent, Stroke::NONE));
            line(&[(5.0, 14.0), (15.0, 14.0)], thin);
        }
        Icon::Reorder => {
            let pts = [(2.5, 12.0), (7.0, 4.0), (13.0, 10.5)];
            line(&pts, Stroke::new(1.0, c.gamma_multiply(0.6)));
            for (i, &(x, y)) in pts.iter().enumerate() {
                p.circle_filled(at(x, y), 2.4 * s, if i == 0 { accent } else { c });
            }
            p.add(Shape::convex_polygon(vec![at(10.0, 6.0), at(12.0, 7.5), at(9.6, 8.0)], c, Stroke::NONE));
        }
        Icon::Undo => {
            let arc: Vec<(f32, f32)> = (0..=16)
                .map(|i| {
                    let a = std::f32::consts::PI * (1.1 - i as f32 / 16.0 * 1.25);
                    (8.5 + 5.5 * a.cos(), 9.0 - 5.0 * a.sin())
                })
                .collect();
            line(&arc, st);
            p.add(Shape::convex_polygon(vec![at(0.5, 8.5), at(6.0, 8.0), at(3.0, 12.5)], c, Stroke::NONE));
        }
        Icon::Fit => {
            for (x, y, dx, dy) in [(1.5, 1.5, 1.0, 1.0), (14.5, 1.5, -1.0, 1.0), (1.5, 14.5, 1.0, -1.0), (14.5, 14.5, -1.0, -1.0)] {
                line(&[(x + 4.0 * dx, y), (x, y), (x, y + 4.0 * dy)], st);
            }
            p.rect_stroke(r(5.0, 5.0, 11.0, 11.0), 0.0, thin, eframe::egui::StrokeKind::Middle);
        }
        Icon::ZoomIn | Icon::ZoomOut => {
            p.circle_stroke(at(6.5, 6.5), 5.0 * s, st);
            line(&[(10.0, 10.0), (14.5, 14.5)], Stroke::new(2.2 * s, c));
            line(&[(4.0, 6.5), (9.0, 6.5)], st);
            if icon == Icon::ZoomIn {
                line(&[(6.5, 4.0), (6.5, 9.0)], st);
            }
        }
        Icon::ShowGrid => {
            for k in [1.5, 6.0, 10.5, 15.0] {
                line(&[(k, 1.5), (k, 14.5)], thin);
                line(&[(1.5, k), (14.5, k)], thin);
            }
        }
        Icon::ShowAllLines => {
            for (i, col) in [accent, blue, Color32::from_rgb(60, 170, 80)].into_iter().enumerate() {
                let y = 4.0 + i as f32 * 4.0;
                line(&[(1.5, y + 1.5), (5.0, y - 1.0), (9.0, y + 1.0), (14.5, y - 1.5)], Stroke::new(1.6 * s, col));
            }
        }
        Icon::ConnectPoints => {
            let pts = [(2.0, 13.0), (5.5, 6.0), (10.0, 9.5), (14.0, 2.5)];
            line(&pts, st);
            for &(x, y) in &pts {
                p.rect_filled(Rect::from_center_size(at(x, y), vec2(3.4 * s, 3.4 * s)), 0.0, accent);
            }
        }
    }
    let _ = pos2;
}
