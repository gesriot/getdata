//! Automatic digitizing: line/background classification, auto trace,
//! "digitize area" and automatic colour acquisition.
//!
//! All positions are in sub-pixel units (`MF` per pixel), like the data points.

use image::RgbImage;

use crate::model::{LineIdMethod, MF, Pt, Rgb};

/// Maximum number of points produced by one auto trace.
const MAX_TRACE_POINTS: usize = 2000;
/// Maximum number of points produced by one "digitize area" operation.
const MAX_AREA_POINTS: usize = 5000;
/// Maximum number of line crossings considered on one grid line.
const MAX_CROSSINGS: usize = 200;
/// cos(65°): continuation directions deviating more than this are rejected.
const MAX_TURN_COS: f64 = 0.422618;

fn round(v: f64) -> f64 {
    (v + 0.5).floor()
}

/// JPEG YCbCr with the rounding used for colour matching.
fn ycc(c: Rgb) -> (f64, f64, f64) {
    let (r, g, b) = (c.0 as f64, c.1 as f64, c.2 as f64);
    let y = round(0.299 * r + 0.587 * g + 0.114 * b);
    let cb = round(-0.1687 * r - 0.3313 * g + 0.5 * b) + 128.0;
    let cr = round(0.5 * r - 0.4187 * g - 0.0813 * b) + 128.0;
    (y, cb, cr)
}

/// Decides how much a colour looks like the line colour (positive) or the
/// background colour (negative).
#[derive(Clone, Debug)]
pub struct Classifier {
    line: (f64, f64, f64),
    bk: (f64, f64, f64),
    method: LineIdMethod,
    /// Scores within `[-threshold, threshold]` are "uncertain".
    pub threshold: i32,
}

impl Classifier {
    pub fn new(line: Rgb, bk: Rgb, method: LineIdMethod) -> Self {
        let mut c = Self { line: ycc(line), bk: ycc(bk), method, threshold: 0 };
        c.threshold = (c.score(line) as f64 * 0.005) as i32;
        c
    }

    pub fn score(&self, p: Rgb) -> i32 {
        let (yl, cbl, crl) = self.line;
        let (yb, cbb, crb) = self.bk;
        let (yp, cbp, crp) = ycc(p);

        let mut d_lb = (crl - crb).hypot(cbl - cbb);
        let mut d_pb = (crp - crb).hypot(cbp - cbb);
        let mut d_pl = (crp - crl).hypot(cbp - cbl);
        if d_lb <= 0.1 {
            d_lb += 1.0;
            d_pb += 0.5;
            d_pl += 0.5;
        }
        let t = (d_pb - d_pl) / d_lb + 1.0;
        let q = (d_pb + d_pl) / d_lb;
        let color = 2.0 * t / (q * q + 1.0) - 1.0;

        let lo = yl.min(yb);
        let hi = yl.max(yb);
        let mut intensity = if lo == hi {
            0.0
        } else if yp < lo {
            let d = (yp - lo) / (hi - lo);
            -0.5 - 0.5 / (4.0 * d * d + 1.0)
        } else if yp > hi {
            let d = (yp - hi) / (hi - lo);
            0.5 / (4.0 * d * d + 1.0) + 0.5
        } else {
            -(std::f64::consts::PI * (yp - lo) / (hi - lo)).cos()
        };
        if lo != hi && yl < yb {
            intensity = -intensity;
        }

        let total = match self.method {
            LineIdMethod::ByColor => intensity * 500.0 + color * 11500.0,
            LineIdMethod::ByIntensity => intensity * 11500.0 + color * 500.0,
            LineIdMethod::Combined => intensity * 6000.0 + color * 6000.0,
        };
        round(total) as i32
    }
}

/// Image plus classifier: scores of image pixels.
pub struct Scorer<'a> {
    pub img: &'a RgbImage,
    pub cls: Classifier,
    pub ignore_pixels: bool,
}

impl<'a> Scorer<'a> {
    pub fn new(img: &'a RgbImage, cls: Classifier, ignore_pixels: bool) -> Self {
        Self { img, cls, ignore_pixels }
    }

    fn t(&self) -> i32 {
        self.cls.threshold
    }

    pub fn width_units(&self) -> i32 {
        self.img.width() as i32 * MF
    }

    pub fn height_units(&self) -> i32 {
        self.img.height() as i32 * MF
    }

    /// Score of the pixel containing sub-pixel position (x, y); 0 outside the image.
    pub fn at(&self, x: i32, y: i32) -> i32 {
        if x < 0 || y < 0 {
            return 0;
        }
        let (px, py) = ((x / MF) as u32, (y / MF) as u32);
        if px >= self.img.width() || py >= self.img.height() {
            return 0;
        }
        let p = self.img.get_pixel(px, py);
        self.cls.score(Rgb(p[0], p[1], p[2]))
    }

    pub fn is_line(&self, p: Pt) -> bool {
        self.at(p.x, p.y) > 0
    }
}

/// Sub-pixel offsets of a closed circle of radius `r`, one unit per step,
/// starting at (0, r) and choosing at each step the move with the smallest
/// radial error.
fn circle(r: i32) -> Vec<(i32, i32)> {
    let mut pts = Vec::with_capacity((8 * r + 8) as usize);
    let (mut x, mut y, mut err) = (0i32, r, 0i32);
    loop {
        pts.push((x, y));
        let (sx, sy) = (if y >= 0 { 1 } else { -1 }, if x >= 0 { -1 } else { 1 });
        // Candidate moves: x += sx or y += sy.
        let ex = err + 1 + 2 * x * sx;
        let ey = err + 1 + 2 * y * sy;
        if ey.abs() < ex.abs() {
            err = ey;
            y += sy;
        } else {
            err = ex;
            x += sx;
        }
        if x == 0 && y == r {
            break;
        }
        if pts.len() > (16 * r + 64) as usize {
            break;
        }
    }
    pts
}

/// Points of the segment from `a` to `b` (inclusive), one unit per step.
fn segment(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    let (dx, dy) = ((b.0 - a.0) as i64, (b.1 - a.1) as i64);
    let sx = if dx < 0 { -1 } else { 1 };
    let sy = if dy < 0 { -1 } else { 1 };
    // Distance (scaled) of a point from the ideal line.
    let err = |px: i32, py: i32| ((px - a.0) as i64 * dy - (py - a.1) as i64 * dx).abs();
    let (mut x, mut y) = a;
    let mut out = Vec::with_capacity((dx.abs() + dy.abs() + 1) as usize);
    loop {
        out.push((x, y));
        if x == b.0 && y == b.1 {
            break;
        }
        if x == b.0 || (y != b.1 && err(x, y + sy) < err(x + sx, y)) {
            y += sy;
        } else {
            x += sx;
        }
    }
    out
}

/// A run of line pixels along a path (circle or segment).
#[derive(Clone, Copy, Debug)]
struct Run {
    start: usize,
    end: usize,
    /// Step that closed the run (the first background step after it), or `end`
    /// when the path ended inside the run.
    close: usize,
    weight: i64,
}

/// Find runs of line pixels along `scores` with hysteresis: uncertain values
/// keep the current state.  Returns runs and whether the path started/ended in
/// a line.
fn runs(scores: &[i32], t: i32) -> (Vec<Run>, bool, bool) {
    let mut out = Vec::new();
    let mut in_line = false;
    let mut started_in_line = false;
    let mut cur = Run { start: 0, end: 0, close: 0, weight: 0 };
    let mut seen_state = false;
    for (k, &s) in scores.iter().enumerate() {
        if s > t {
            if !in_line {
                in_line = true;
                cur = Run { start: k, end: k, close: k, weight: 0 };
                if !seen_state {
                    started_in_line = true;
                }
            }
            cur.end = k;
            cur.weight += s as i64;
            seen_state = true;
        } else if s < -t {
            if in_line {
                cur.close = k;
                out.push(cur);
            }
            in_line = false;
            seen_state = true;
        }
    }
    if in_line {
        cur.close = cur.end;
        out.push(cur);
    }
    (out, started_in_line, in_line)
}

/// Index along `scores` where the cumulative line score of `run` reaches half its weight.
fn run_middle(scores: &[i32], t: i32, run: &Run) -> usize {
    let mut acc = 0i64;
    for k in run.start..=run.end {
        if scores[k] > t {
            acc += scores[k] as i64;
            if acc * 2 >= run.weight {
                return k;
            }
        }
    }
    run.end
}

fn too_small(a: (i32, i32), b: (i32, i32)) -> bool {
    (a.0 - b.0).abs() < MF && (a.1 - b.1).abs() < MF
}

/// A direction found on the circle around a point.
#[derive(Clone, Copy, Debug)]
struct Arc {
    dx: i32,
    dy: i32,
    weight: i64,
}

impl Arc {
    fn len(&self) -> f64 {
        (self.dx as f64).hypot(self.dy as f64)
    }

    fn cos_to(&self, vx: f64, vy: f64) -> f64 {
        (self.dx as f64 * vx + self.dy as f64 * vy) / (self.len() * vx.hypot(vy))
    }

    /// Angle in [0, 2π) with the y axis pointing up.
    pub fn angle(&self) -> f64 {
        let a = (-(self.dy as f64)).atan2(self.dx as f64);
        if a < 0.0 { a + std::f64::consts::TAU } else { a }
    }
}

/// Line crossings of the circle of radius `r` around `c`.  `None` when the
/// circle contains no background pixel at all.
fn circle_arcs(s: &Scorer, c: Pt, r: i32) -> Option<Vec<Arc>> {
    let circ = circle(r);
    let n = circ.len();
    let raw: Vec<i32> = circ.iter().map(|&(dx, dy)| s.at(c.x + dx, c.y + dy)).collect();
    let t = s.t();
    // Start the scan at a background pixel so every arc is complete.
    let k0 = raw.iter().position(|&v| v < -t)?;
    let rot: Vec<i32> = (0..n).map(|k| raw[(k0 + k) % n]).collect();
    let (rs, _, _) = runs(&rot, t);
    let mut arcs = Vec::new();
    for run in rs {
        let a = circ[(k0 + run.start) % n];
        let b = circ[(k0 + run.close) % n];
        if s.ignore_pixels && too_small(a, b) {
            continue;
        }
        let (dx, dy) = circ[(k0 + run_middle(&rot, t, &run)) % n];
        arcs.push(Arc { dx, dy, weight: run.weight });
    }
    Some(arcs)
}

/// Order arcs so that arcs pointing in opposite directions follow each other.
fn pair_opposite(mut arcs: Vec<Arc>) -> Vec<Arc> {
    if arcs.len() <= 2 {
        return arcs;
    }
    let mut out = Vec::with_capacity(arcs.len());
    while arcs.len() >= 2 {
        let mut best = (-2.0, 0, 1);
        for i in 0..arcs.len() {
            for j in i + 1..arcs.len() {
                let c = -arcs[i].cos_to(arcs[j].dx as f64, arcs[j].dy as f64);
                if c > best.0 {
                    best = (c, i, j);
                }
            }
        }
        let (_, i, j) = best;
        let b = arcs.remove(j);
        let a = arcs.remove(i);
        out.push(a);
        out.push(b);
    }
    out.extend(arcs);
    out
}

/// Result of locating the centre of a line near a clicked point.
#[derive(Clone, Copy, Debug)]
pub struct LineCenter {
    pub pos: Pt,
    /// Total score across the line (its "thickness"); 0 when not found.
    pub weight: i64,
}

/// Find the centre of the line passing near `p`: a diameter of the circle of
/// radius `r` that crosses the line between two background arcs is scanned,
/// and the point at half of the accumulated line score is returned.
pub fn line_center(s: &Scorer, p: Pt, r: i32) -> LineCenter {
    let fail = LineCenter { pos: p, weight: 0 };
    if p.x < 0 || p.y < 0 || p.x >= s.width_units() || p.y >= s.height_units() {
        return fail;
    }
    let circ = circle(r);
    let n = circ.len();
    let t = s.t();
    let raw: Vec<i32> = circ.iter().map(|&(dx, dy)| s.at(p.x + dx, p.y + dy)).collect();
    // Background arcs: runs of negative scores (scores negated).
    let neg: Vec<i32> = raw.iter().map(|v| -v).collect();
    let Some(k0) = neg.iter().position(|&v| v < -t) else { return fail };
    let rot: Vec<i32> = (0..n).map(|k| neg[(k0 + k) % n]).collect();
    let (bg, _, _) = runs(&rot, t);
    if bg.is_empty() {
        return fail;
    }
    let half = n / 2;
    // Choose a diameter whose both ends lie in background, maximizing the
    // overlap of a background arc with the antipode of another one.
    let arcs: Vec<(usize, usize)> = bg.iter().map(|r| (r.start, r.end)).collect();
    let mut best: Option<(i64, usize)> = None;
    for (i, &(a0, a1)) in arcs.iter().enumerate() {
        for (j, &(b0, b1)) in arcs.iter().enumerate() {
            if i == j && arcs.len() > 1 {
                continue;
            }
            // Antipode of arc j.
            let (o0, o1) = ((b0 + half) % n, (b1 + half) % n);
            if o1 < o0 {
                continue;
            }
            let lo = a0.max(o0);
            let hi = a1.min(o1);
            if lo < hi {
                let score = ((a1 - a0) * (b1 - b0) * (hi - lo)) as i64;
                if best.is_none_or(|b| score > b.0) {
                    best = Some((score, (lo + hi) / 2));
                }
            }
        }
    }
    let Some((_, mid)) = best else { return fail };
    let ka = circ[(k0 + mid) % n];
    let kb = circ[(k0 + mid + half) % n];
    let a = (p.x + ka.0, p.y + ka.1);
    let b = (p.x + kb.0, p.y + kb.1);
    let seg = segment(a, b);
    let sc: Vec<i32> = seg.iter().map(|&(x, y)| s.at(x, y)).collect();
    let (rs, start_in, end_in) = runs(&sc, t);
    if start_in || end_in {
        return fail;
    }
    // Pick the crossing closest to the clicked point.
    let Some(run) = rs.iter().min_by_key(|r| {
        let m = seg[(r.start + r.end) / 2];
        (m.0 - p.x).pow(2) + (m.1 - p.y).pow(2)
    }) else {
        return fail;
    };
    let (x, y) = seg[run_middle(&sc, t, run)];
    LineCenter { pos: Pt::new(x, y), weight: run.weight }
}

/// Which directions to follow from the start point.
#[derive(Clone, Copy, Debug)]
pub enum TraceDirections {
    All,
    /// Counter-clockwise sector (y up) from `from` to `to`, radians.
    Sector { from: f64, to: f64 },
}

pub struct TraceParams {
    /// Distance between data points in sub-pixel units.
    pub radius: i32,
    pub smooth: bool,
}

/// Directions (angles) available from `p`, used by the "choose direction" dialog.
pub fn directions_at(s: &Scorer, p: Pt, r: i32) -> Vec<f64> {
    circle_arcs(s, p, r).unwrap_or_default().iter().map(Arc::angle).collect()
}

fn angle_in(a: f64, from: f64, to: f64) -> bool {
    let tau = std::f64::consts::TAU;
    let norm = |v: f64| v.rem_euclid(tau);
    let (a, from) = (norm(a), norm(from));
    let mut to = norm(to);
    if to <= from {
        to += tau;
    }
    (from <= a && a < to) || (from <= a + tau && a + tau < to)
}

/// Auto trace from `start` (normally the result of [`line_center`]).
///
/// `existing` are the points already in the current line: tracing stops when
/// it comes within half the trace distance of one of them.  Returns the new
/// points in the order they should be appended (start point first).
pub fn auto_trace(s: &Scorer, start: LineCenter, dirs: TraceDirections, prm: &TraceParams, existing: &[Pt]) -> Vec<Pt> {
    let r = prm.radius.max(MF);
    let mut result = vec![start.pos];
    let Some(arcs) = circle_arcs(s, start.pos, r) else { return result };
    let arcs = pair_opposite(arcs);
    let mut budget = MAX_TRACE_POINTS - 1;
    let mut branches: Vec<Vec<Pt>> = Vec::new();
    for a in arcs {
        if let TraceDirections::Sector { from, to } = dirs
            && !angle_in(a.angle(), from, to)
        {
            continue;
        }
        let mut known: Vec<Pt> = existing.to_vec();
        known.extend_from_slice(&result);
        let branch = follow(s, start.pos, a, r, &known, &mut budget);
        let branch = if prm.smooth { smooth(s, &branch) } else { branch };
        result.extend_from_slice(&branch[1..]);
        branches.push(branch[1..].to_vec());
        if budget == 0 {
            break;
        }
    }
    // A line traced both ways from its middle: the second branch is reversed and put
    // in front of the first so the polyline is continuous.
    if let [b1, b2] = &branches[..]
        && !b1.is_empty()
        && !b2.is_empty()
    {
        result = b2.iter().rev().copied().chain(std::iter::once(start.pos)).chain(b1.iter().copied()).collect();
    }
    result
}

/// Follow one direction.  The returned vector starts with `prev` (the start point).
fn follow(s: &Scorer, prev: Pt, first: Arc, r: i32, known: &[Pt], budget: &mut usize) -> Vec<Pt> {
    let mut pts = vec![prev];
    let mut prev = prev;
    let mut cur = Pt::new(prev.x + first.dx, prev.y + first.dy);
    let mut weight = first.weight;
    let near = ((r / 2) as f64).powi(2);
    let (w, h) = (s.width_units(), s.height_units());
    while cur.x >= 0 && cur.x < w && cur.y >= 0 && cur.y < h && *budget > 0 {
        // Reached existing data of the line (or came back to the start point).
        if known.iter().any(|&k| k != cur && cur.dist2(k) <= near) {
            break;
        }
        pts.push(cur);
        *budget -= 1;
        let Some(mut arcs) = circle_arcs(s, cur, r) else { break };
        let (vx, vy) = ((cur.x - prev.x) as f64, (cur.y - prev.y) as f64);
        arcs.retain(|a| a.cos_to(vx, vy) >= MAX_TURN_COS);
        if arcs.is_empty() {
            break;
        }
        if arcs.len() > 1 && weight > 0 {
            // Keep crossings of similar "thickness" to the one we are following.
            let wf = weight as f64;
            let best = arcs.iter().map(|a| (a.weight - weight).abs()).min().unwrap_or(0) as f64 / wf;
            let (lo, hi) = (best * 0.6 - 0.15, best * 1.4 + 0.15);
            arcs.retain(|a| {
                let d = (a.weight - weight).abs() as f64 / wf;
                d >= lo && d <= hi
            });
        }
        let next = arcs
            .iter()
            .copied()
            .max_by(|a, b| a.cos_to(vx, vy).total_cmp(&b.cos_to(vx, vy)))
            .expect("non-empty");
        prev = cur;
        cur = Pt::new(cur.x + next.dx, cur.y + next.dy);
        weight = next.weight;
    }
    pts
}

/// Least-squares cubic through 5 points at t = 1..5, evaluated at `t`.
fn cubic5(p: [f64; 5], t: f64) -> f64 {
    let a0 = (16.0 * p[0] - 14.0 * p[1] - 4.0 * p[2] + 11.0 * p[3] - 4.0 * p[4]) / 5.0;
    let a1 = (-127.0 * p[0] + 179.0 * p[1] + 36.0 * p[2] - 143.0 * p[3] + 55.0 * p[4]) / 42.0;
    let a2 = (25.0 * p[0] - 44.0 * p[1] - 4.0 * p[2] + 40.0 * p[3] - 17.0 * p[4]) / 28.0;
    let a3 = (-p[0] + 2.0 * p[1] - 2.0 * p[3] + p[4]) / 12.0;
    a0 + t * (a1 + t * (a2 + t * a3))
}

/// Move `orig` towards `fitted` as long as the destination stays on the line.
fn accept(s: &Scorer, orig: Pt, fitted: Pt) -> Pt {
    let t = s.t();
    if s.at(fitted.x, fitted.y) > t {
        return fitted;
    }
    let (mut good, mut bad) = (orig, fitted);
    for _ in 0..6 {
        let m = Pt::new((good.x + bad.x) / 2, (good.y + bad.y) / 2);
        if s.at(m.x, m.y) > t {
            good = m;
        } else {
            bad = m;
        }
    }
    good
}

/// Three passes of local cubic smoothing (first and last points are kept).
fn smooth(s: &Scorer, pts: &[Pt]) -> Vec<Pt> {
    let mut p = pts.to_vec();
    let n = p.len();
    if n <= 4 {
        return p;
    }
    let fit = |p: &[Pt], i0: usize, t: f64| -> Pt {
        let xs = [0, 1, 2, 3, 4].map(|k| p[i0 + k].x as f64);
        let ys = [0, 1, 2, 3, 4].map(|k| p[i0 + k].y as f64);
        Pt::new(round(cubic5(xs, t)) as i32, round(cubic5(ys, t)) as i32)
    };
    for _ in 0..3 {
        for i in 4..n {
            let f = fit(&p, i - 4, 3.0);
            p[i - 2] = accept(s, p[i - 2], f);
        }
    }
    let f = fit(&p, 0, 2.0);
    p[1] = accept(s, p[1], f);
    let f = fit(&p, n - 5, 4.0);
    p[n - 2] = accept(s, p[n - 2], f);
    p
}

/// Points placed along one grid line from `a` to `b`.
pub fn digitize_segment(s: &Scorer, a: (i32, i32), b: (i32, i32), at_boundary: bool, out: &mut Vec<Pt>) {
    if out.len() >= MAX_AREA_POINTS {
        return;
    }
    let seg = segment(a, b);
    let t = s.t();
    let sc: Vec<i32> = seg.iter().map(|&(x, y)| s.at(x, y)).collect();
    let (mut rs, start_in, end_in) = runs(&sc, t);
    if s.ignore_pixels {
        rs.retain(|r| !too_small(seg[r.start], seg[r.close]));
    }
    rs.truncate(MAX_CROSSINGS);
    let count = rs.len();
    for (i, r) in rs.iter().enumerate() {
        let first_open = i == 0 && start_in;
        let last_open = i + 1 == count && end_in;
        if at_boundary {
            // Boundaries that coincide with the ends of the grid line are not real boundaries.
            if !first_open {
                let (x, y) = seg[r.start];
                out.push(Pt::new(x, y));
            }
            if !last_open {
                let (x, y) = seg[r.end];
                out.push(Pt::new(x, y));
            }
        } else if !first_open && !last_open {
            let (x, y) = seg[run_middle(&sc, t, r)];
            out.push(Pt::new(x, y));
        }
        if out.len() >= MAX_AREA_POINTS {
            return;
        }
    }
}

/// Automatically acquire line and background colours from the image:
/// the background is the most frequent colour, the line the most frequent
/// colour clearly different from it.
pub fn acquire_colors(img: &RgbImage) -> (Rgb, Rgb) {
    let (w, h) = img.dimensions();
    let total = (w as u64 * h as u64).min(62500);
    let mut bins = vec![[0u64; 4]; 4096];
    let mut seed: u64 = 0x2545F4914F6CDD1D;
    for _ in 0..total {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let x = ((seed >> 33) % w as u64) as u32;
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let y = ((seed >> 33) % h as u64) as u32;
        let p = img.get_pixel(x, y);
        let idx = ((p[0] >> 4) as usize) << 8 | ((p[1] >> 4) as usize) << 4 | (p[2] >> 4) as usize;
        let b = &mut bins[idx];
        b[0] += 1;
        b[1] += p[0] as u64;
        b[2] += p[1] as u64;
        b[3] += p[2] as u64;
    }
    let coords = |i: usize| ((i >> 8) as i32, ((i >> 4) & 15) as i32, (i & 15) as i32);
    let mean = |b: &[u64; 4]| {
        if b[0] == 0 { Rgb::BLACK } else { Rgb((b[1] / b[0]) as u8, (b[2] / b[0]) as u8, (b[3] / b[0]) as u8) }
    };
    let bk_idx = (0..4096).max_by_key(|&i| (bins[i][0], std::cmp::Reverse(i))).unwrap_or(0);
    let (br, bg, bb) = coords(bk_idx);
    let line_idx = (0..4096)
        .filter(|&i| {
            let (r, g, b) = coords(i);
            (r - br).pow(2) + (g - bg).pow(2) + (b - bb).pow(2) > 35
        })
        .max_by_key(|&i| (bins[i][0], std::cmp::Reverse(i)));
    let line = line_idx.map(|i| mean(&bins[i])).unwrap_or(Rgb::BLACK);
    (line, mean(&bins[bk_idx]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img_with_hline(w: u32, h: u32, y0: u32, thick: u32) -> RgbImage {
        RgbImage::from_fn(w, h, |_, y| if y >= y0 && y < y0 + thick { image::Rgb([0, 0, 0]) } else { image::Rgb([255, 255, 255]) })
    }

    #[test]
    fn classifier_extremes() {
        // Black and white have equal chroma, so only the intensity term counts.
        let c = Classifier::new(Rgb::BLACK, Rgb::WHITE, LineIdMethod::Combined);
        assert_eq!(c.score(Rgb::BLACK), 6000);
        assert_eq!(c.score(Rgb::WHITE), -6000);
        assert_eq!(c.threshold, 30);
        assert!(c.score(Rgb(100, 100, 100)) > 0);
        let c = Classifier::new(Rgb(255, 0, 0), Rgb::WHITE, LineIdMethod::ByColor);
        assert!(c.score(Rgb(255, 0, 0)) > 11000);
        assert!(c.score(Rgb(0, 0, 255)) < 0);
    }

    #[test]
    fn circle_closes() {
        let c = circle(27);
        assert_eq!(c[0], (0, 27));
        for &(x, y) in &c {
            let r = ((x * x + y * y) as f64).sqrt();
            assert!((r - 27.0).abs() < 1.0, "{x},{y}");
        }
        assert!(c.len() > 100);
    }

    #[test]
    fn segment_endpoints() {
        let s = segment((0, 0), (10, -4));
        assert_eq!(s[0], (0, 0));
        assert_eq!(*s.last().unwrap(), (10, -4));
    }

    #[test]
    fn center_and_trace_horizontal_line() {
        let img = img_with_hline(200, 60, 30, 3);
        let s = Scorer::new(&img, Classifier::new(Rgb::BLACK, Rgb::WHITE, LineIdMethod::Combined), true);
        let c = line_center(&s, Pt::from_pixel(100, 29), 10 * MF);
        assert!(c.weight > 0);
        assert_eq!(c.pos.y / MF, 31);
        let pts = auto_trace(&s, c, TraceDirections::All, &TraceParams { radius: 10 * MF, smooth: true }, &[]);
        assert!(pts.len() > 15, "{}", pts.len());
        for p in &pts {
            assert_eq!(p.y / MF, 31, "{p:?}");
        }
        let xs: Vec<i32> = pts.iter().map(|p| p.x / MF).collect();
        assert!(*xs.iter().min().unwrap() < 15 && *xs.iter().max().unwrap() > 185, "{xs:?}");
    }

    #[test]
    fn area_vertical_grid() {
        let img = img_with_hline(100, 50, 20, 2);
        let s = Scorer::new(&img, Classifier::new(Rgb::BLACK, Rgb::WHITE, LineIdMethod::Combined), true);
        let mut out = Vec::new();
        digitize_segment(&s, (50 * MF + 4, 4), (50 * MF + 4, 49 * MF + 4), false, &mut out);
        assert_eq!(out.len(), 1);
        assert!((out[0].y / MF - 20).abs() <= 1);
    }

    #[test]
    fn acquire_black_on_white() {
        let img = img_with_hline(100, 100, 40, 10);
        let (l, b) = acquire_colors(&img);
        assert_eq!(b, Rgb::WHITE);
        assert_eq!(l, Rgb::BLACK);
    }
}
