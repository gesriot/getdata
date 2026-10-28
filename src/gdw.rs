//! Reading and writing GetData workspace files (`*.gdw`, XML).

use std::fmt::Write as _;
use std::path::{Component, Path, PathBuf};

use crate::model::*;

/// Workspace format version written to files.
pub const FORMAT_VERSION: &str = "2.22";

#[derive(Debug)]
pub enum GdwError {
    Io(std::io::Error),
    Xml(roxmltree::Error),
    NotWorkspace,
}

impl std::fmt::Display for GdwError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GdwError::Io(e) => write!(f, "{e}"),
            GdwError::Xml(e) => write!(f, "XML: {e}"),
            GdwError::NotWorkspace => write!(f, "not a GetData workspace"),
        }
    }
}

fn child<'a, 'i>(n: roxmltree::Node<'a, 'i>, name: &str) -> Option<roxmltree::Node<'a, 'i>> {
    n.children().find(|c| c.is_element() && c.has_tag_name(name))
}

fn text<'a>(n: roxmltree::Node<'a, '_>, name: &str) -> Option<&'a str> {
    child(n, name).and_then(|c| c.text()).map(str::trim)
}

fn num<T: std::str::FromStr>(n: roxmltree::Node, name: &str) -> Option<T> {
    text(n, name).and_then(|s| s.parse().ok())
}

fn flag(n: roxmltree::Node, name: &str) -> Option<bool> {
    num::<i32>(n, name).map(|v| v != 0)
}

fn rgb(s: &str) -> Option<Rgb> {
    let mut it = s.split(',').map(|p| p.trim().parse::<u8>());
    Some(Rgb(it.next()?.ok()?, it.next()?.ok()?, it.next()?.ok()?))
}

/// Scale factor of stored coordinates relative to our internal MF units.
fn mf_of(n: roxmltree::Node) -> f64 {
    n.attribute("MF").and_then(|s| s.trim().parse::<f64>().ok()).filter(|&v| v > 0.0).unwrap_or(1.0)
}

fn conv(v: f64, mf: f64) -> i32 {
    if mf == MF as f64 { v as i32 } else { ((v + 0.5) / mf * MF as f64).floor() as i32 }
}

pub fn parse(xml: &str) -> Result<Workspace, GdwError> {
    let doc = roxmltree::Document::parse(xml).map_err(GdwError::Xml)?;
    let root = doc.root_element();
    if !root.has_tag_name("GetDataWorkspace") {
        return Err(GdwError::NotWorkspace);
    }
    let mut ws = Workspace { lines: Vec::new(), ..Default::default() };

    if let Some(f) = child(root, "File") {
        ws.image_file = text(f, "Name").unwrap_or_default().to_string();
        ws.width = num(f, "Width").unwrap_or(0);
        ws.height = num(f, "Height").unwrap_or(0);
    }

    for (i, ln) in root.children().filter(|c| c.has_tag_name("Line")).enumerate() {
        let mut line = DataLine::new(i, Rgb::BLACK, Rgb::WHITE);
        if let Some(c) = child(ln, "Caption") {
            line.caption = c.text().unwrap_or_default().to_string();
        }
        if let Some(c) = text(ln, "LineColor").and_then(rgb) {
            line.line_color = c;
        }
        if let Some(c) = text(ln, "BkColor").and_then(rgb) {
            line.bk_color = c;
        }
        if let Some(c) = text(ln, "PtColor").and_then(rgb) {
            line.pt_color = c;
        }
        line.line_type = LineType::from_i32(num(ln, "LineType").unwrap_or(0));
        for p in ln.children().filter(|c| c.has_tag_name("Points")) {
            let mf = mf_of(p);
            let vals: Vec<f64> =
                p.text().unwrap_or_default().split(',').filter_map(|s| s.trim().parse().ok()).collect();
            line.points.extend(vals.chunks_exact(2).map(|c| Pt::new(conv(c[0], mf), conv(c[1], mf))));
        }
        ws.lines.push(line);
    }
    if ws.lines.is_empty() {
        ws.lines.push(DataLine::new(0, Rgb::BLACK, Rgb::WHITE));
    }
    ws.current_line = num::<usize>(root, "CurrentLine").unwrap_or(0).min(ws.lines.len() - 1);

    for id in AxisId::ALL {
        if let Some(n) = child(root, id.name()) {
            let mf = mf_of(n);
            let v: Vec<f64> =
                n.text().unwrap_or_default().split(',').filter_map(|s| s.trim().parse().ok()).collect();
            if v.len() >= 3 {
                ws.axes[id as usize] = Some(AxisPoint { pos: Pt::new(conv(v[0], mf), conv(v[1], mf)), value: v[2] });
            }
        }
    }
    ws.x_log = text(root, "XScale") == Some("logarithmic");
    ws.y_log = text(root, "YScale") == Some("logarithmic");

    if let Some((x, y)) = text(root, "Position").and_then(|s| s.split_once(',')) {
        ws.position = (x.trim().parse().unwrap_or(0), y.trim().parse().unwrap_or(0));
    }
    if let Some(z) = child(root, "Zoom") {
        ws.zoom = match num::<f64>(z, "Scale") {
            Some(s) if child(z, "FitToWindow").is_none() => ZoomSetting::Scale(s),
            _ => ZoomSetting::FitToWindow,
        };
    }
    if let Some(d) = child(root, "Digitize") {
        if let Some(v) = flag(d, "IgnorePixels") {
            ws.digitize.ignore_pixels = v;
        }
        if let Some(v) = num(d, "LineIDMethod") {
            ws.digitize.line_id_method = LineIdMethod::from_i32(v);
        }
    }
    if let Some(a) = child(root, "AutoTrace") {
        if let Some(v) = num(a, "TraceDistance") {
            ws.auto_trace.trace_distance = v;
        }
        if let Some(v) = flag(a, "SmoothLines") {
            ws.auto_trace.smooth_lines = v;
        }
    }
    if let Some(a) = child(root, "DigitizeArea") {
        let s = &mut ws.digitize_area;
        if let Some(v) = flag(a, "LineOrArea") {
            s.at_boundary = v;
        }
        if let Some(v) = num::<i32>(a, "AreaType") {
            s.area_type = if v == 1 { AreaType::RelatedToAxes } else { AreaType::Rectangular };
        }
        s.grid_type = num(a, "GridType").unwrap_or(s.grid_type);
        s.go_through_x = num(a, "GoThroughX").unwrap_or(s.go_through_x);
        s.go_through_y = num(a, "GoThroughY").unwrap_or(s.go_through_y);
        s.shift_x = flag(a, "ShiftX").unwrap_or(s.shift_x);
        s.shift_y = flag(a, "ShiftY").unwrap_or(s.shift_y);
        s.rotate_angle = num(a, "RotateAngle").unwrap_or(s.rotate_angle);
        s.dx = num(a, "dx").unwrap_or(s.dx);
        s.delta_x = num(a, "deltaX").unwrap_or(s.delta_x);
        s.delta_y = num(a, "deltaY").unwrap_or(s.delta_y);
        s.delta_x_units = num(a, "deltaXUnits").map(GridUnits::from_i32).unwrap_or(s.delta_x_units);
        s.delta_y_units = num(a, "deltaYUnits").map(GridUnits::from_i32).unwrap_or(s.delta_y_units);
    }
    if let Some(g) = child(root, "GridSettings") {
        let s = &mut ws.grid;
        s.show_grid = flag(g, "ShowGrid").unwrap_or(s.show_grid);
        if let Some(v) = num::<i32>(g, "AreaType") {
            s.area_type = if v == 1 { AreaType::RelatedToAxes } else { AreaType::Rectangular };
        }
        s.delta_x = num(g, "deltaX").unwrap_or(s.delta_x);
        s.delta_y = num(g, "deltaY").unwrap_or(s.delta_y);
        s.delta_x_units = num(g, "deltaXUnits").map(GridUnits::from_i32).unwrap_or(s.delta_x_units);
        s.delta_y_units = num(g, "deltaYUnits").map(GridUnits::from_i32).unwrap_or(s.delta_y_units);
        s.go_through_x = num(g, "GoThroughX").unwrap_or(s.go_through_x);
        s.go_through_y = num(g, "GoThroughY").unwrap_or(s.go_through_y);
        s.dx = num(g, "dx").unwrap_or(s.dx);
        s.dy = num(g, "dy").unwrap_or(s.dy);
    }
    Ok(ws)
}

pub fn load(path: &Path) -> Result<Workspace, GdwError> {
    let bytes = std::fs::read(path).map_err(GdwError::Io)?;
    let xml = String::from_utf8_lossy(&bytes);
    parse(xml.trim_start_matches('\u{feff}'))
}

/// C-runtime style `%.*e` formatting with a 3-digit exponent (`1.000000e+003`).
pub fn fmt_e(v: f64, prec: usize) -> String {
    let s = format!("{v:.prec$e}");
    match s.split_once('e') {
        Some((m, e)) => {
            let (sign, digits) = match e.strip_prefix('-') {
                Some(d) => ('-', d),
                None => ('+', e),
            };
            format!("{m}e{sign}{:0>3}", digits)
        }
        None => s,
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn to_xml(ws: &Workspace) -> String {
    let mut o = String::new();
    let nl = "\r\n";
    let _ = write!(o, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>{nl}<GetDataWorkspace>{nl}");
    let _ = write!(o, "  <version>{FORMAT_VERSION}</version>{nl}");
    let _ = write!(
        o,
        "  <File>{nl}    <Name>{}</Name>{nl}    <Width>{}</Width>{nl}    <Height>{}</Height>{nl}  </File>{nl}",
        esc(&ws.image_file),
        ws.width,
        ws.height
    );
    for l in &ws.lines {
        let c = |c: Rgb| format!("{},{},{}", c.0, c.1, c.2);
        let _ = write!(o, "  <Line>{nl}    <Caption>{}</Caption>{nl}", esc(&l.caption));
        let _ = write!(o, "    <LineColor>{}</LineColor>{nl}", c(l.line_color));
        let _ = write!(o, "    <BkColor>{}</BkColor>{nl}", c(l.bk_color));
        let _ = write!(o, "    <PtColor>{}</PtColor>{nl}", c(l.pt_color));
        let _ = write!(o, "    <LineType>{}</LineType>{nl}", l.line_type as i32);
        for chunk in l.points.chunks(10) {
            let s: Vec<String> = chunk.iter().map(|p| format!("{},{}", p.x, p.y)).collect();
            let _ = write!(o, "    <Points MF=\"{MF}\">{}</Points>{nl}", s.join(","));
        }
        let _ = write!(o, "  </Line>{nl}");
    }
    let _ = write!(o, "  <CurrentLine>{}</CurrentLine>{nl}", ws.current_line);
    for id in AxisId::ALL {
        if let Some(a) = ws.axis(id) {
            let _ = write!(o, "  <{0} MF=\"{MF}\">{1},{2},{3}</{0}>{nl}", id.name(), a.pos.x, a.pos.y, fmt_e(a.value, 6));
        }
    }
    let scale = |log| if log { "logarithmic" } else { "linear" };
    let _ = write!(o, "  <XScale>{}</XScale>{nl}  <YScale>{}</YScale>{nl}", scale(ws.x_log), scale(ws.y_log));
    let _ = write!(o, "  <Position>{},{}</Position>{nl}", ws.position.0, ws.position.1);
    match ws.zoom {
        ZoomSetting::FitToWindow => {
            let _ = write!(o, "  <Zoom>{nl}    <FitToWindow/>{nl}  </Zoom>{nl}");
        }
        ZoomSetting::Scale(s) => {
            let _ = write!(o, "  <Zoom>{nl}    <Scale>{s:.1}</Scale>{nl}  </Zoom>{nl}");
        }
    }
    let b = |v: bool| v as i32;
    let d = &ws.digitize;
    let _ = write!(
        o,
        "  <Digitize>{nl}    <IgnorePixels>{}</IgnorePixels>{nl}    <LineIDMethod>{}</LineIDMethod>{nl}  </Digitize>{nl}",
        b(d.ignore_pixels),
        d.line_id_method as i32
    );
    let a = &ws.auto_trace;
    let _ = write!(
        o,
        "  <AutoTrace>{nl}    <TraceDistance>{}</TraceDistance>{nl}    <SmoothLines>{}</SmoothLines>{nl}  </AutoTrace>{nl}",
        a.trace_distance,
        b(a.smooth_lines)
    );
    let s = &ws.digitize_area;
    let _ = write!(o, "  <DigitizeArea>{nl}");
    let _ = write!(o, "    <LineOrArea>{}</LineOrArea>{nl}", b(s.at_boundary));
    let _ = write!(o, "    <AreaType>{}</AreaType>{nl}", s.area_type as i32);
    let _ = write!(o, "    <GridType>{}</GridType>{nl}", s.grid_type);
    let _ = write!(o, "    <GoThroughX>{}</GoThroughX>{nl}", fmt_e(s.go_through_x, 14));
    let _ = write!(o, "    <GoThroughY>{}</GoThroughY>{nl}", fmt_e(s.go_through_y, 14));
    let _ = write!(o, "    <ShiftX>{}</ShiftX>{nl}    <ShiftY>{}</ShiftY>{nl}", b(s.shift_x), b(s.shift_y));
    let _ = write!(o, "    <RotateAngle>{:.6}</RotateAngle>{nl}", s.rotate_angle);
    let _ = write!(o, "    <dx>{:.6}</dx>{nl}", s.dx);
    let _ = write!(o, "    <deltaX>{}</deltaX>{nl}    <deltaY>{}</deltaY>{nl}", fmt_e(s.delta_x, 14), fmt_e(s.delta_y, 14));
    let _ = write!(
        o,
        "    <deltaXUnits>{}</deltaXUnits>{nl}    <deltaYUnits>{}</deltaYUnits>{nl}",
        s.delta_x_units as i32,
        s.delta_y_units as i32
    );
    let _ = write!(o, "  </DigitizeArea>{nl}");
    let g = &ws.grid;
    let _ = write!(o, "  <GridSettings>{nl}");
    let _ = write!(o, "    <ShowGrid>{}</ShowGrid>{nl}", b(g.show_grid));
    let _ = write!(o, "    <AreaType>{}</AreaType>{nl}", g.area_type as i32);
    let _ = write!(o, "    <deltaX>{}</deltaX>{nl}    <deltaY>{}</deltaY>{nl}", fmt_e(g.delta_x, 14), fmt_e(g.delta_y, 14));
    let _ = write!(
        o,
        "    <deltaXUnits>{}</deltaXUnits>{nl}    <deltaYUnits>{}</deltaYUnits>{nl}",
        g.delta_x_units as i32,
        g.delta_y_units as i32
    );
    let _ = write!(o, "    <GoThroughX>{}</GoThroughX>{nl}", fmt_e(g.go_through_x, 14));
    let _ = write!(o, "    <GoThroughY>{}</GoThroughY>{nl}", fmt_e(g.go_through_y, 14));
    let _ = write!(o, "    <dx>{:.6}</dx>{nl}    <dy>{:.6}</dy>{nl}", g.dx, g.dy);
    let _ = write!(o, "  </GridSettings>{nl}</GetDataWorkspace>{nl}");
    o
}

pub fn save(ws: &Workspace, path: &Path) -> std::io::Result<()> {
    std::fs::write(path, to_xml(ws))
}

/// Path of `target` relative to directory `base`, using `\` separators.
/// Falls back to the absolute path when the two are on different drives.
pub fn relative_path(target: &Path, base: &Path) -> String {
    let norm = |p: &Path| -> Vec<String> {
        p.components()
            .filter(|c| !matches!(c, Component::CurDir))
            .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
            .collect()
    };
    let t = norm(target);
    let b = norm(base);
    if t.first() != b.first() {
        return target.to_string_lossy().into_owned();
    }
    let common = t.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let target_parts: Vec<String> = target
        .components()
        .filter(|c| !matches!(c, Component::CurDir))
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let mut parts: Vec<String> = std::iter::repeat_n("..".to_string(), b.len() - common).collect();
    parts.extend(target_parts[common..].iter().cloned());
    parts.join("\\")
}

/// Resolve an image path stored in a workspace relative to the workspace's directory.
pub fn resolve_image_path(stored: &str, workspace: &Path) -> PathBuf {
    let p = PathBuf::from(stored.replace('\\', std::path::MAIN_SEPARATOR_STR));
    if p.is_absolute() {
        return p;
    }
    workspace.parent().map(|d| d.join(&p)).unwrap_or(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exponent_format() {
        assert_eq!(fmt_e(0.0, 6), "0.000000e+000");
        assert_eq!(fmt_e(1e10, 6), "1.000000e+010");
        assert_eq!(fmt_e(-0.5, 14), "-5.00000000000000e-001");
    }

    #[test]
    fn rel_path() {
        let r = relative_path(Path::new(r"C:\a\graphs\x.jpg"), Path::new(r"C:\a\workspaces"));
        assert_eq!(r, r"..\graphs\x.jpg");
    }
}
