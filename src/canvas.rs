//! The main image view: painting and mouse interaction for every mode.

use eframe::egui::{self, Color32, CursorIcon, PointerButton, Pos2, Rect, Sense, Shape, Stroke, Ui, pos2, vec2};

use crate::app::{App, AreaDrag, Mode};
use crate::i18n::tr;
use crate::model::*;

const AXIS_COLOR: Color32 = Color32::from_rgb(230, 30, 30);
const GRID_COLOR: Color32 = Color32::from_rgba_premultiplied(40, 110, 200, 150);

fn to_pos(p: Pt) -> Pos2 {
    let (x, y) = p.to_image();
    pos2(x as f32, y as f32)
}

/// Contrasting outline for a colour.
fn outline(c: Color32) -> Color32 {
    let l = 0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32;
    if l > 140.0 { Color32::from_gray(40) } else { Color32::WHITE }
}

impl App {
    pub fn canvas(&mut self, ui: &mut Ui) {
        let rect = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);

        let size = self.image_size();
        self.view.rect = rect;
        if self.view.fit && size.x > 0.0 {
            self.view.fit_to(rect, size);
        }

        let hover_screen = resp.hover_pos();
        self.hover = hover_screen.map(|s| self.view.to_image(s));
        if self.hover.is_some() {
            self.mag_center = self.hover;
        }

        if !self.has_image() {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{}  (O)", tr("Open Image")),
                egui::FontId::proportional(18.0),
                ui.visuals().weak_text_color(),
            );
            return;
        }

        self.handle_zoom_and_pan(ui, &resp);
        self.handle_mode_input(ui, &resp);

        // ---- painting ----
        let img_rect = self.view.image_rect_on_screen(size);
        if self.settings.show_image {
            if let Some(t) = &self.tiles {
                t.paint(&painter, |p| self.view.to_screen(p), Color32::WHITE);
            }
        } else {
            painter.rect_filled(img_rect, 0.0, Color32::WHITE);
        }
        painter.rect_stroke(img_rect, 0.0, (1.0, Color32::from_gray(120)), egui::StrokeKind::Outside);
        if self.ws.grid.show_grid {
            self.paint_grid(&painter.with_clip_rect(img_rect.intersect(painter.clip_rect())));
        }
        self.paint_axes(&painter);
        self.paint_points(&painter);
        self.paint_overlays(&painter, hover_screen);
    }

    fn handle_zoom_and_pan(&mut self, ui: &mut Ui, resp: &egui::Response) {
        if resp.hovered() {
            let (scroll, zoom) = ui.input(|i| {
                let wheel: f32 = i
                    .events
                    .iter()
                    .filter_map(|e| match e {
                        egui::Event::MouseWheel { delta, .. } => Some(delta.y),
                        _ => None,
                    })
                    .sum();
                (wheel, i.zoom_delta())
            });
            if let Some(p) = resp.hover_pos() {
                if scroll > 0.0 {
                    let s = self.view.zoom_in_step();
                    self.view.zoom_at(p, s);
                } else if scroll < 0.0 {
                    let s = self.view.zoom_out_step();
                    self.view.zoom_at(p, s);
                } else if zoom != 1.0 {
                    self.view.zoom_at(p, self.view.scale * zoom);
                }
            }
        }
        let normal_drag = self.mode == Mode::Normal && !ui.input(|i| i.modifiers.shift);
        if resp.dragged_by(PointerButton::Middle) || (normal_drag && resp.dragged_by(PointerButton::Primary)) {
            self.view.pan(resp.drag_delta());
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        }
        self.view.clamp(self.image_size());
    }

    fn handle_mode_input(&mut self, ui: &mut Ui, resp: &egui::Response) {
        let mods = ui.input(|i| i.modifiers);
        let pos = resp.interact_pointer_pos().map(|s| (s, self.view.to_image(s)));
        if !matches!(self.mode, Mode::Normal) && resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
        }
        match self.mode.clone() {
            Mode::Normal => {
                if let Some((_, p)) = pos {
                    if resp.clicked() && mods.shift {
                        match self.nearest_point(p, 4.0 / self.view.scale) {
                            Some(i) => {
                                self.remove_points(&std::iter::once(i).collect());
                            }
                            None => self.add_point(self.point_at(p, mods.ctrl)),
                        }
                    }
                    if resp.secondary_clicked() {
                        self.ctx_menu_pos = Some(p);
                    }
                }
                let mpos = self.ctx_menu_pos;
                resp.context_menu(|ui| {
                    if let Some(p) = mpos {
                        self.context_menu(ui, p);
                    }
                });
            }
            Mode::SetScale(axis) => {
                if let Some((_, p)) = pos
                    && resp.clicked()
                {
                    let pt = self.point_at(p, mods.ctrl);
                    self.open_axis_value(axis, pt, true);
                }
            }
            Mode::PickColor { line } => {
                if let Some((_, p)) = pos
                    && resp.clicked()
                    && let Some(c) = self.pixel_at(p)
                {
                    self.checkpoint();
                    let l = self.ws.line_mut();
                    if line {
                        l.line_color = c;
                    } else {
                        l.bk_color = c;
                    }
                    self.set_mode(Mode::Normal);
                }
            }
            Mode::AutoTrace => {
                if let Some((_, p)) = pos {
                    if resp.clicked() {
                        self.auto_trace_at(p, None);
                    } else if resp.secondary_clicked() {
                        self.open_direction(p);
                    }
                }
            }
            Mode::DigitizeArea => {
                let units = |p: Pos2| ((p.x * MF as f32) as f64, (p.y * MF as f32) as f64);
                if resp.drag_started_by(PointerButton::Primary)
                    && let Some((_, p)) = pos
                {
                    let u = units(p);
                    self.area = Some(AreaDrag { start: u, end: u });
                } else if resp.dragged_by(PointerButton::Primary)
                    && let (Some(a), Some((_, p))) = (self.area.as_mut(), pos)
                {
                    let d = resp.drag_delta() / self.view.scale * MF as f32;
                    if mods.ctrl {
                        a.start.0 += d.x as f64;
                        a.start.1 += d.y as f64;
                        a.end.0 += d.x as f64;
                        a.end.1 += d.y as f64;
                    } else if mods.shift && self.ws.digitize_area.area_type == AreaType::Rectangular {
                        self.ws.digitize_area.rotate_angle += resp.drag_delta().x as f64 * 0.01;
                    } else {
                        a.end = units(p);
                    }
                }
                if resp.drag_stopped_by(PointerButton::Primary)
                    && let Some(a) = self.area.take()
                {
                    self.digitize_area(&a);
                }
            }
            Mode::PointCapture => {
                if let Some((_, p)) = pos {
                    if resp.clicked() {
                        let pt = if mods.shift {
                            self.line_center_at(p).map(|c| c.pos)
                        } else {
                            Some(self.point_at(p, mods.ctrl))
                        };
                        if let Some(pt) = pt {
                            match self.moving.take() {
                                Some(i) if i < self.ws.line().points.len() => {
                                    self.checkpoint();
                                    self.ws.line_mut().points[i] = pt;
                                }
                                _ => self.add_point(pt),
                            }
                        }
                    } else if resp.secondary_clicked() {
                        self.moving = self.nearest_point(p, 6.0 / self.view.scale);
                    }
                }
            }
            Mode::Eraser | Mode::Reorder => {
                let eraser = self.mode == Mode::Eraser;
                if mods.shift && resp.dragged_by(PointerButton::Primary) {
                    let size = if eraser { &mut self.settings.eraser_size } else { &mut self.settings.reorder_size };
                    *size = (*size - resp.drag_delta().y).clamp(3.0, 200.0);
                } else if let Some((s, _)) = pos
                    && (resp.drag_started_by(PointerButton::Primary)
                        || resp.dragged_by(PointerButton::Primary)
                        || resp.clicked())
                {
                    if eraser {
                        self.erase_at(s, resp.drag_started() || resp.clicked());
                    } else {
                        if resp.drag_started() || resp.clicked() {
                            self.checkpoint();
                        }
                        self.reorder_at(s, mods.ctrl);
                    }
                }
            }
        }
    }

    fn context_menu(&mut self, ui: &mut Ui, p: Pos2) {
        let pt = self.point_at(p, false);
        for (id, set, rm) in [
            (AxisId::Xmin, "Set Xmin", "Remove Xmin"),
            (AxisId::Xmax, "Set Xmax", "Remove Xmax"),
            (AxisId::Ymin, "Set Ymin", "Remove Ymin"),
            (AxisId::Ymax, "Set Ymax", "Remove Ymax"),
        ] {
            ui.horizontal(|ui| {
                if ui.button(tr(set)).clicked() {
                    self.open_axis_value(id, pt, false);
                    ui.close();
                }
                if self.ws.axis(id).is_some() && ui.button(tr(rm)).clicked() {
                    self.remove_axis(id);
                    ui.close();
                }
            });
        }
        ui.separator();
        if ui.button(tr("Zoom in")).clicked() {
            let s = self.view.zoom_in_step();
            self.view.zoom_at(self.view.to_screen(p), s);
            ui.close();
        }
        if ui.button(tr("Zoom out")).clicked() {
            let s = self.view.zoom_out_step();
            self.view.zoom_at(self.view.to_screen(p), s);
            ui.close();
        }
        ui.separator();
        if ui.button(tr("Set point")).clicked() {
            self.add_point(pt);
            ui.close();
        }
        if let Some(i) = self.nearest_point(p, 6.0 / self.view.scale)
            && ui.button(tr("Remove point")).clicked()
        {
            self.remove_points(&std::iter::once(i).collect());
            ui.close();
        }
        if ui.button(tr("Set point in the middle of a line")).clicked() {
            if let Some(c) = self.line_center_at(p) {
                self.add_point(c.pos);
            }
            ui.close();
        }
        ui.separator();
        if ui.button(tr("Set line color")).clicked() {
            if let Some(c) = self.pixel_at(p) {
                self.checkpoint();
                self.ws.line_mut().line_color = c;
            }
            ui.close();
        }
        if ui.button(tr("Set background color")).clicked() {
            if let Some(c) = self.pixel_at(p) {
                self.checkpoint();
                self.ws.line_mut().bk_color = c;
            }
            ui.close();
        }
        ui.separator();
        ui.menu_button(tr("Auto trace line"), |ui| {
            if ui.button(tr("All directions")).clicked() {
                self.auto_trace_at(p, None);
                ui.close();
            }
            if ui.button(tr("Choose direction")).clicked() {
                self.open_direction(p);
                ui.close();
            }
        });
    }

    fn paint_grid(&self, painter: &egui::Painter) {
        let g = &self.ws.grid;
        let size = self.image_size();
        let stroke = Stroke::new(1.0, GRID_COLOR);
        let s = |p: Pos2| self.view.to_screen(p);
        match g.area_type {
            AreaType::Rectangular => {
                let (dx, dy) = (g.dx.max(2.0) as f32, g.dy.max(2.0) as f32);
                if dx * self.view.scale >= 3.0 {
                    let mut x = 0.5;
                    while x <= size.x {
                        painter.line_segment([s(pos2(x, 0.0)), s(pos2(x, size.y))], stroke);
                        x += dx;
                    }
                }
                if dy * self.view.scale >= 3.0 {
                    let mut y = 0.5;
                    while y <= size.y {
                        painter.line_segment([s(pos2(0.0, y)), s(pos2(size.x, y))], stroke);
                        y += dy;
                    }
                }
            }
            AreaType::RelatedToAxes => {
                let Ok(t) = self.transform() else { return };
                let corners = [(0.0, 0.0), (size.x as f64, 0.0), (0.0, size.y as f64), (size.x as f64, size.y as f64)];
                let fr: Vec<(f64, f64)> = corners.iter().map(|&c| t.fractions(c)).collect();
                let (umin, umax) = fr.iter().fold((f64::MAX, f64::MIN), |a, f| (a.0.min(f.0), a.1.max(f.0)));
                let (vmin, vmax) = fr.iter().fold((f64::MAX, f64::MIN), |a, f| (a.0.min(f.1), a.1.max(f.1)));
                let du = t.du_of(g.delta_x, g.delta_x_units == GridUnits::Points);
                let dv = t.dv_of(g.delta_y, g.delta_y_units == GridUnits::Points);
                let ou = if g.go_through_x != 0.0 || !t.x_log() { t.u_of(if t.x_log() && g.go_through_x <= 0.0 { 1.0 } else { g.go_through_x }) } else { 0.0 };
                let ov = if g.go_through_y != 0.0 || !t.y_log() { t.v_of(if t.y_log() && g.go_through_y <= 0.0 { 1.0 } else { g.go_through_y }) } else { 0.0 };
                let pt = |p: (f64, f64)| s(pos2(p.0 as f32, p.1 as f32));
                let draw = |lo: f64, hi: f64, step: f64, origin: f64, f: &dyn Fn(f64) -> ((f64, f64), (f64, f64))| {
                    if !(step > 0.0) || !step.is_finite() || !origin.is_finite() || (hi - lo) / step > 2000.0 {
                        return;
                    }
                    let mut v = ((lo - origin) / step).ceil() * step + origin;
                    while v <= hi {
                        let (a, b) = f(v);
                        painter.line_segment([pt(a), pt(b)], stroke);
                        v += step;
                    }
                };
                draw(umin, umax, du, ou, &|u| (t.from_fractions(u, vmin), t.from_fractions(u, vmax)));
                draw(vmin, vmax, dv, ov, &|v| (t.from_fractions(umin, v), t.from_fractions(umax, v)));
            }
        }
    }

    fn paint_axes(&self, painter: &egui::Painter) {
        let s = |p: Pt| self.view.to_screen(to_pos(p));
        // Axes OX and OY through the axis points.
        let axis_line = |a: Option<AxisPoint>, b: Option<AxisPoint>| {
            if let (Some(a), Some(b)) = (a, b)
                && a.pos != b.pos
            {
                let (pa, pb) = (s(a.pos), s(b.pos));
                let d = (pb - pa).normalized() * 4000.0;
                painter.add(Shape::dashed_line(&[pa - d, pb + d], Stroke::new(1.0, AXIS_COLOR.gamma_multiply(0.6)), 6.0, 4.0));
            }
        };
        axis_line(self.ws.axis(AxisId::Xmin), self.ws.axis(AxisId::Xmax));
        axis_line(self.ws.axis(AxisId::Ymin), self.ws.axis(AxisId::Ymax));
        for id in AxisId::ALL {
            let Some(a) = self.ws.axis(id) else { continue };
            let c = s(a.pos);
            let st = Stroke::new(2.0, AXIS_COLOR);
            painter.line_segment([c - vec2(8.0, 0.0), c + vec2(8.0, 0.0)], st);
            painter.line_segment([c - vec2(0.0, 8.0), c + vec2(0.0, 8.0)], st);
            painter.circle_stroke(c, 5.0, st);
            let (dx, dy) = match id {
                AxisId::Xmin => (6.0, 8.0),
                AxisId::Xmax => (6.0, 8.0),
                AxisId::Ymin => (-6.0, -20.0),
                AxisId::Ymax => (6.0, -20.0),
            };
            let align = if dx < 0.0 { egui::Align2::RIGHT_TOP } else { egui::Align2::LEFT_TOP };
            let text = format!("{} = {}", id.name(), crate::export::c_g(a.value, 6));
            let font = egui::FontId::proportional(12.0);
            let galley = painter.layout_no_wrap(text, font, AXIS_COLOR);
            let r = align.anchor_size(c + vec2(dx, dy), galley.size()).expand(2.0);
            painter.rect_filled(r, 2.0, Color32::from_rgba_unmultiplied(255, 255, 255, 200));
            painter.galley(r.min + vec2(2.0, 2.0), galley, AXIS_COLOR);
        }
    }

    fn paint_points(&self, painter: &egui::Painter) {
        let clip = painter.clip_rect().expand(20.0);
        for (li, line) in self.ws.lines.iter().enumerate() {
            let current = li == self.ws.current_line;
            if !current && !self.settings.show_all_lines {
                continue;
            }
            let color = line.pt_color.to_color32();
            let col = if current { color } else { color.gamma_multiply(0.55) };
            let pts: Vec<Pos2> = line.points.iter().map(|&p| self.view.to_screen(to_pos(p))).collect();
            if self.settings.connect_points && line.line_type != LineType::SeparatePoints && pts.len() > 1 {
                let mut path = pts.clone();
                if line.line_type == LineType::ClosedPolyline {
                    path.push(pts[0]);
                }
                painter.add(Shape::line(path, Stroke::new(if current { 1.5 } else { 1.0 }, col)));
            }
            let r = if current { 3.5 } else { 2.5 };
            for (i, &p) in pts.iter().enumerate() {
                if !clip.contains(p) {
                    continue;
                }
                let rect = Rect::from_center_size(p, vec2(r * 2.0, r * 2.0));
                painter.rect_filled(rect, 0.0, col);
                painter.rect_stroke(rect, 0.0, (1.0, outline(col)), egui::StrokeKind::Outside);
                if current && (self.selected.contains(&i) || self.moving == Some(i)) {
                    painter.circle_stroke(p, r + 4.0, (2.0, Color32::from_rgb(0, 120, 255)));
                }
                if current && self.mode == Mode::Reorder && i == self.reorder_next.saturating_sub(1) {
                    painter.circle_stroke(p, r + 5.0, (2.0, Color32::from_rgb(0, 180, 0)));
                }
            }
            // The first point of the line is enlarged in the reorder tool.
            if current && self.mode == Mode::Reorder && let Some(&p) = pts.first() {
                painter.circle_stroke(p, r + 7.0, (1.5, col));
            }
        }
    }

    fn paint_overlays(&self, painter: &egui::Painter, hover: Option<Pos2>) {
        match self.mode {
            Mode::Eraser | Mode::Reorder => {
                if let Some(h) = hover {
                    let r = if self.mode == Mode::Eraser { self.settings.eraser_size } else { self.settings.reorder_size };
                    let c = self.pixel_at(self.view.to_image(h)).map(|c| c.to_color32()).unwrap_or(Color32::WHITE);
                    painter.circle_stroke(h, r, (1.5, outline(c)));
                }
            }
            Mode::DigitizeArea => {
                if let Some(a) = &self.area {
                    let st = Stroke::new(1.0, Color32::from_rgb(0, 140, 255));
                    for (p0, p1) in self.area_segments(a) {
                        let q = |p: (f64, f64)| self.view.to_screen(pos2((p.0 / MF as f64) as f32, (p.1 / MF as f64) as f32));
                        painter.line_segment([q(p0), q(p1)], st);
                    }
                }
            }
            Mode::AutoTrace | Mode::PointCapture | Mode::SetScale(_) | Mode::PickColor { .. } => {
                if let Some(h) = hover {
                    let c = self.pixel_at(self.view.to_image(h)).map(|c| c.to_color32()).unwrap_or(Color32::WHITE);
                    let st = Stroke::new(1.0, outline(c));
                    painter.line_segment([h - vec2(12.0, 0.0), h - vec2(3.0, 0.0)], st);
                    painter.line_segment([h + vec2(3.0, 0.0), h + vec2(12.0, 0.0)], st);
                    painter.line_segment([h - vec2(0.0, 12.0), h - vec2(0.0, 3.0)], st);
                    painter.line_segment([h + vec2(0.0, 3.0), h + vec2(0.0, 12.0)], st);
                }
            }
            Mode::Normal => {}
        }
    }
}
