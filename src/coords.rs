//! Image -> logical coordinate transform defined by the Xmin/Xmax/Ymin/Ymax points.
//!
//! The X axis is the line through the Xmin and Xmax points, the Y axis the line
//! through Ymin and Ymax.  Axes need not be orthogonal: the x value of a point
//! is found by moving it parallel to the Y axis onto the X axis, and vice versa.

use crate::model::{AxisId, Pt, Workspace};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaleError {
    NotSet,
    XminEqXmax,
    YminEqYmax,
    Collinear,
    XLogNotAllowed,
    YLogNotAllowed,
}

#[derive(Clone, Copy, Debug)]
pub struct Transform {
    // Image positions (continuous pixel coordinates) of the axis points.
    x0: (f64, f64),
    ex: (f64, f64),
    y0: (f64, f64),
    ey: (f64, f64),
    // Logical values, already in log10 space for log axes.
    xv: (f64, f64),
    yv: (f64, f64),
    x_log: bool,
    y_log: bool,
    det: f64,
}

fn cross(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.0 * b.1 - a.1 * b.0
}

fn sub(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 - b.0, a.1 - b.1)
}

impl Transform {
    pub fn new(ws: &Workspace) -> Result<Self, ScaleError> {
        let (Some(xmin), Some(xmax), Some(ymin), Some(ymax)) =
            (ws.axis(AxisId::Xmin), ws.axis(AxisId::Xmax), ws.axis(AxisId::Ymin), ws.axis(AxisId::Ymax))
        else {
            return Err(ScaleError::NotSet);
        };
        if xmin.value == xmax.value {
            return Err(ScaleError::XminEqXmax);
        }
        if ymin.value == ymax.value {
            return Err(ScaleError::YminEqYmax);
        }
        if ws.x_log && (xmin.value <= 0.0 || xmax.value <= 0.0) {
            return Err(ScaleError::XLogNotAllowed);
        }
        if ws.y_log && (ymin.value <= 0.0 || ymax.value <= 0.0) {
            return Err(ScaleError::YLogNotAllowed);
        }
        let x0 = xmin.pos.to_image();
        let y0 = ymin.pos.to_image();
        let ex = sub(xmax.pos.to_image(), x0);
        let ey = sub(ymax.pos.to_image(), y0);
        let det = cross(ex, ey);
        if xmin.pos == xmax.pos || ymin.pos == ymax.pos || det.abs() < 1e-9 {
            return Err(ScaleError::Collinear);
        }
        let lv = |v: f64, log: bool| if log { v.log10() } else { v };
        Ok(Self {
            x0,
            ex,
            y0,
            ey,
            xv: (lv(xmin.value, ws.x_log), lv(xmax.value, ws.x_log)),
            yv: (lv(ymin.value, ws.y_log), lv(ymax.value, ws.y_log)),
            x_log: ws.x_log,
            y_log: ws.y_log,
            det,
        })
    }

    /// Fractions (u along X axis, v along Y axis) of an image point.
    pub fn fractions(&self, p: (f64, f64)) -> (f64, f64) {
        // p = x0 + u*ex + s*ey  => u = cross(p - x0, ey) / cross(ex, ey)
        // p = y0 + v*ey + t*ex  => v = cross(ex, p - y0) / cross(ex, ey)
        let u = cross(sub(p, self.x0), self.ey) / self.det;
        let v = cross(self.ex, sub(p, self.y0)) / self.det;
        (u, v)
    }

    pub fn to_logical(&self, p: (f64, f64)) -> (f64, f64) {
        let (u, v) = self.fractions(p);
        let x = self.xv.0 + u * (self.xv.1 - self.xv.0);
        let y = self.yv.0 + v * (self.yv.1 - self.yv.0);
        (if self.x_log { 10f64.powf(x) } else { x }, if self.y_log { 10f64.powf(y) } else { y })
    }

    pub fn pt_to_logical(&self, p: Pt) -> (f64, f64) {
        self.to_logical(p.to_image())
    }

    /// Inverse transform: logical value -> image point.
    #[cfg(test)]
    pub fn to_image(&self, x: f64, y: f64) -> (f64, f64) {
        let lx = if self.x_log { x.log10() } else { x };
        let ly = if self.y_log { y.log10() } else { y };
        let u = (lx - self.xv.0) / (self.xv.1 - self.xv.0);
        let v = (ly - self.yv.0) / (self.yv.1 - self.yv.0);
        // Point on X axis line at fraction u: a = x0 + u*ex; on Y axis at v: b = y0 + v*ey.
        // Result = intersection of line through a parallel to ey and line through b parallel to ex.
        let a = (self.x0.0 + u * self.ex.0, self.x0.1 + u * self.ex.1);
        let b = (self.y0.0 + v * self.ey.0, self.y0.1 + v * self.ey.1);
        // a + s*ey = b + t*ex  => s = cross(b - a, ex) / cross(ey, ex)
        let s = cross(sub(b, a), self.ex) / cross(self.ey, self.ex);
        (a.0 + s * self.ey.0, a.1 + s * self.ey.1)
    }

    /// Image point with the given axis fractions (inverse of [`Self::fractions`]).
    pub fn from_fractions(&self, u: f64, v: f64) -> (f64, f64) {
        // p = x0 + u*ex + s*ey with s chosen so that the v fraction matches.
        let c = cross(self.ex, sub(self.x0, self.y0)) / self.det;
        let s = v - c;
        (self.x0.0 + u * self.ex.0 + s * self.ey.0, self.x0.1 + u * self.ex.1 + s * self.ey.1)
    }

    /// Fraction along the X axis of a logical x value.
    pub fn u_of(&self, x: f64) -> f64 {
        let lx = if self.x_log { x.log10() } else { x };
        (lx - self.xv.0) / (self.xv.1 - self.xv.0)
    }

    /// Fraction along the Y axis of a logical y value.
    pub fn v_of(&self, y: f64) -> f64 {
        let ly = if self.y_log { y.log10() } else { y };
        (ly - self.yv.0) / (self.yv.1 - self.yv.0)
    }

    /// Fraction step of a grid step along X: in axis values (`ratio` for log
    /// axes) or in image pixels.
    pub fn du_of(&self, step: f64, in_pixels: bool) -> f64 {
        if in_pixels {
            step / self.ex.0.hypot(self.ex.1)
        } else if self.x_log {
            step.log10() / (self.xv.1 - self.xv.0)
        } else {
            step / (self.xv.1 - self.xv.0)
        }
    }

    pub fn dv_of(&self, step: f64, in_pixels: bool) -> f64 {
        if in_pixels {
            step / self.ey.0.hypot(self.ey.1)
        } else if self.y_log {
            step.log10() / (self.yv.1 - self.yv.0)
        } else {
            step / (self.yv.1 - self.yv.0)
        }
    }

    pub fn x_log(&self) -> bool {
        self.x_log
    }

    pub fn y_log(&self) -> bool {
        self.y_log
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AxisPoint;

    #[test]
    fn roundtrip_non_orthogonal() {
        let mut ws = Workspace::default();
        ws.axes = [
            Some(AxisPoint { pos: Pt::new(3504, 3957), value: -2.0 }),
            Some(AxisPoint { pos: Pt::new(8082, 2246), value: 6.0 }),
            Some(AxisPoint { pos: Pt::new(2736, 4177), value: -1.0 }),
            Some(AxisPoint { pos: Pt::new(261, 2291), value: 1.0 }),
        ];
        let t = Transform::new(&ws).unwrap();
        let (x, y) = t.pt_to_logical(Pt::new(3504, 3957));
        assert!((x + 2.0).abs() < 1e-9, "{x}");
        let p = t.to_image(1.5, 0.25);
        let (x, y2) = t.to_logical(p);
        assert!((x - 1.5).abs() < 1e-9 && (y2 - 0.25).abs() < 1e-9, "{x} {y2} {y}");
    }

    #[test]
    fn log_axis() {
        let mut ws = Workspace::default();
        ws.y_log = true;
        ws.axes = [
            Some(AxisPoint { pos: Pt::new(0, 900), value: 0.0 }),
            Some(AxisPoint { pos: Pt::new(900, 900), value: 10.0 }),
            Some(AxisPoint { pos: Pt::new(0, 900), value: 1.0 }),
            Some(AxisPoint { pos: Pt::new(0, 0), value: 100.0 }),
        ];
        let t = Transform::new(&ws).unwrap();
        let (_, y) = t.to_logical((0.5 / 9.0, 450.5 / 9.0));
        assert!((y - 10.0).abs() < 1e-9, "{y}");
    }
}
