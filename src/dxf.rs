//! AutoCAD DXF export in the R2000 (AC1015) structure, modelled on CADCraft's
//! writer (`cadcraft-io/src/dxf_write.rs`): handles on every object, symbol
//! tables with owners, model/paper space block records with their layouts and
//! the OBJECTS section dictionaries.  Each data line goes on its own layer
//! (named after the caption, coloured with the points colour) as an
//! LWPOLYLINE, or as POINT entities for "separate points" lines.

use std::fmt::Write as _;

use crate::model::Rgb;

/// One data line ready for export: caption, colour, closed flag, points.
pub struct DxfLine<'a> {
    pub caption: &'a str,
    pub color: Rgb,
    pub closed: bool,
    pub points_only: bool,
    pub points: &'a [(f64, f64)],
}

enum V {
    S(String),
    F(f64),
    I(i64),
}

struct W {
    t: Vec<(i32, V)>,
    next: u64,
}

impl W {
    fn s(&mut self, c: i32, v: impl Into<String>) {
        self.t.push((c, V::S(v.into())));
    }
    fn f(&mut self, c: i32, v: f64) {
        self.t.push((c, V::F(v)));
    }
    fn i(&mut self, c: i32, v: i64) {
        self.t.push((c, V::I(v)));
    }
    fn p2(&mut self, c: i32, x: f64, y: f64) {
        self.f(c, x);
        self.f(c + 10, y);
    }
    fn p3(&mut self, c: i32, x: f64, y: f64, z: f64) {
        self.f(c, x);
        self.f(c + 10, y);
        self.f(c + 20, z);
    }
    fn h(&mut self) -> String {
        let h = format!("{:X}", self.next);
        self.next += 1;
        h
    }
    fn table_head(&mut self, name: &str, count: usize) -> String {
        let h = self.h();
        self.s(0, "TABLE");
        self.s(2, name);
        self.s(5, h.clone());
        self.s(330, "0");
        self.s(100, "AcDbSymbolTable");
        self.i(70, count as i64);
        h
    }
    fn record_head(&mut self, kind: &str, owner: &str, subclass: &str) -> String {
        let h = self.h();
        self.s(0, kind);
        self.s(if kind == "DIMSTYLE" { 105 } else { 5 }, h.clone());
        self.s(330, owner);
        self.s(100, "AcDbSymbolTableRecord");
        self.s(100, subclass);
        h
    }
    fn dict_head(&mut self, h: &str, owner: &str) {
        self.s(0, "DICTIONARY");
        self.s(5, h);
        self.s(330, owner);
        self.s(100, "AcDbDictionary");
        self.i(281, 1);
    }
}

/// Shortest round-trip real with a decimal point (as `cadcraft_dxf::format_real`).
fn real(v: f64) -> String {
    if !v.is_finite() {
        return "0.0".into();
    }
    let s = format!("{v}");
    if s.contains('.') || s.contains('e') { s } else { format!("{s}.0") }
}

/// Pre-2007 DXF text: ASCII, other characters as `\U+XXXX`, line breaks as spaces.
fn text(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '\r' | '\n' => out.push(' '),
            c if c.is_ascii() => out.push(c),
            c => {
                let mut b = [0u16; 2];
                for u in c.encode_utf16(&mut b) {
                    let _ = write!(out, "\\U+{:04X}", u);
                }
            }
        }
    }
}

fn render(tags: &[(i32, V)]) -> String {
    let mut out = String::with_capacity(tags.len() * 16);
    for (c, v) in tags {
        let _ = write!(out, "{c:>3}\r\n");
        match v {
            V::S(s) => text(s, &mut out),
            V::F(f) => out.push_str(&real(*f)),
            V::I(i) => out.push_str(&i.to_string()),
        }
        out.push_str("\r\n");
    }
    out
}

/// Layer name allowed by AutoCAD: no `<>/\":;?*|=,`` ` and at most 255 characters, unique.
fn layer_name(caption: &str, index: usize, taken: &mut Vec<String>) -> String {
    let mut n: String = caption
        .chars()
        .map(|c| if "<>/\\\":;?*|=,`".contains(c) || c.is_control() { '_' } else { c })
        .take(200)
        .collect();
    n = n.trim().to_string();
    if n.is_empty() || n == "0" || n.eq_ignore_ascii_case("defpoints") {
        n = format!("Line {}", index + 1);
    }
    let base = n.clone();
    let mut k = 2;
    while taken.iter().any(|t| t.eq_ignore_ascii_case(&n)) {
        n = format!("{base} ({k})");
        k += 1;
    }
    taken.push(n.clone());
    n
}

fn true_color(c: Rgb) -> i64 {
    ((c.0 as i64) << 16) | ((c.1 as i64) << 8) | c.2 as i64
}

/// The drawing as R2000 ASCII DXF.
pub fn write(lines: &[DxfLine], aci: impl Fn(Rgb) -> usize) -> String {
    let mut w = W { t: Vec::new(), next: 0x20 };

    // Extents of all points.
    let all = lines.iter().flat_map(|l| l.points.iter());
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(x, y) in all.filter(|p| p.0.is_finite() && p.1.is_finite()) {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    let has_ext = x0 <= x1;
    if !has_ext {
        (x0, y0, x1, y1) = (0.0, 0.0, 1.0, 1.0);
    }

    // Structural handles.
    let ms_br = w.h();
    let ps_br = w.h();
    let root_dict = w.h();
    let group_dict = w.h();
    let layout_dict = w.h();
    let pstyle_dict = w.h();
    let pstyle_normal = w.h();
    let model_layout = w.h();
    let paper_layout = w.h();

    // ---------------- HEADER ----------------
    w.s(0, "SECTION");
    w.s(2, "HEADER");
    w.s(9, "$ACADVER");
    w.s(1, "AC1015");
    w.s(9, "$DWGCODEPAGE");
    w.s(3, "ANSI_1252");
    w.s(9, "$INSBASE");
    w.p3(10, 0.0, 0.0, 0.0);
    w.s(9, "$EXTMIN");
    w.p3(10, x0, y0, 0.0);
    w.s(9, "$EXTMAX");
    w.p3(10, x1, y1, 0.0);
    w.s(9, "$LIMMIN");
    w.p2(10, x0, y0);
    w.s(9, "$LIMMAX");
    w.p2(10, x1, y1);
    w.s(9, "$INSUNITS");
    w.i(70, 0);
    w.s(9, "$MEASUREMENT");
    w.i(70, 1);
    // Points drawn as a circled cross, 2% of the viewport.
    w.s(9, "$PDMODE");
    w.i(70, 34);
    w.s(9, "$PDSIZE");
    w.f(40, -2.0);
    w.s(9, "$CLAYER");
    w.s(8, "0");
    let seed_pos = w.t.len();
    w.s(9, "$HANDSEED");
    w.s(5, "0");
    w.s(0, "ENDSEC");

    // ---------------- CLASSES ----------------
    w.s(0, "SECTION");
    w.s(2, "CLASSES");
    for (name, cpp) in [("ACDBDICTIONARYWDFLT", "AcDbDictionaryWithDefault"), ("ACDBPLACEHOLDER", "AcDbPlaceHolder")] {
        w.s(0, "CLASS");
        w.s(1, name);
        w.s(2, cpp);
        w.s(3, "ObjectDBX Classes");
        w.i(90, 0);
        w.i(280, 0);
        w.i(281, 0);
    }
    w.s(0, "ENDSEC");

    // ---------------- TABLES ----------------
    w.s(0, "SECTION");
    w.s(2, "TABLES");
    // VPORT: the active viewport shows the extents.
    let th = w.table_head("VPORT", 1);
    w.record_head("VPORT", &th, "AcDbViewportTableRecord");
    w.s(2, "*Active");
    w.i(70, 0);
    w.p2(10, 0.0, 0.0);
    w.p2(11, 1.0, 1.0);
    w.p2(12, (x0 + x1) / 2.0, (y0 + y1) / 2.0);
    w.p2(13, 0.0, 0.0);
    w.p2(14, 1.0, 1.0);
    w.p2(15, 1.0, 1.0);
    w.p3(16, 0.0, 0.0, 1.0);
    w.p3(17, 0.0, 0.0, 0.0);
    w.f(40, ((y1 - y0).max((x1 - x0) / 1.6) * 1.1).max(1e-6));
    w.f(41, 1.6);
    w.f(42, 50.0);
    w.f(50, 0.0);
    w.s(0, "ENDTAB");
    // LTYPE
    let th = w.table_head("LTYPE", 3);
    for (name, desc) in [("ByBlock", ""), ("ByLayer", ""), ("Continuous", "Solid line")] {
        w.record_head("LTYPE", &th, "AcDbLinetypeTableRecord");
        w.s(2, name);
        w.i(70, 0);
        w.s(3, desc);
        w.i(72, 65);
        w.i(73, 0);
        w.f(40, 0.0);
    }
    w.s(0, "ENDTAB");
    // LAYER: "0" plus one per data line.
    let mut taken = Vec::new();
    let names: Vec<String> = lines.iter().enumerate().map(|(i, l)| layer_name(l.caption, i, &mut taken)).collect();
    let th = w.table_head("LAYER", 1 + lines.len());
    let layer = |w: &mut W, name: &str, color: Option<Rgb>| {
        w.record_head("LAYER", &th, "AcDbLayerTableRecord");
        w.s(2, name);
        w.i(70, 0);
        match color {
            Some(c) => {
                w.i(62, aci(c).clamp(1, 255) as i64);
                w.i(420, true_color(c));
            }
            None => w.i(62, 7),
        }
        w.s(6, "Continuous");
        w.i(370, -3);
        w.s(390, pstyle_normal.clone());
    };
    layer(&mut w, "0", None);
    for (l, n) in lines.iter().zip(&names) {
        layer(&mut w, n, Some(l.color));
    }
    w.s(0, "ENDTAB");
    // STYLE
    let th = w.table_head("STYLE", 1);
    w.record_head("STYLE", &th, "AcDbTextStyleTableRecord");
    w.s(2, "Standard");
    w.i(70, 0);
    w.f(40, 0.0);
    w.f(41, 1.0);
    w.f(50, 0.0);
    w.i(71, 0);
    w.f(42, 0.2);
    w.s(3, "txt");
    w.s(4, "");
    w.s(0, "ENDTAB");
    // VIEW, UCS: empty.
    w.table_head("VIEW", 0);
    w.s(0, "ENDTAB");
    w.table_head("UCS", 0);
    w.s(0, "ENDTAB");
    // APPID
    let th = w.table_head("APPID", 1);
    w.record_head("APPID", &th, "AcDbRegAppTableRecord");
    w.s(2, "ACAD");
    w.i(70, 0);
    w.s(0, "ENDTAB");
    // DIMSTYLE
    let h = w.h();
    w.s(0, "TABLE");
    w.s(2, "DIMSTYLE");
    w.s(5, h.clone());
    w.s(330, "0");
    w.s(100, "AcDbSymbolTable");
    w.i(70, 1);
    w.s(100, "AcDbDimStyleTable");
    w.i(71, 0);
    w.record_head("DIMSTYLE", &h, "AcDbDimStyleTableRecord");
    w.s(2, "Standard");
    w.i(70, 0);
    w.s(0, "ENDTAB");
    // BLOCK_RECORD
    let th = w.table_head("BLOCK_RECORD", 2);
    for (h, name, layout) in [(&ms_br, "*Model_Space", &model_layout), (&ps_br, "*Paper_Space", &paper_layout)] {
        w.s(0, "BLOCK_RECORD");
        w.s(5, h.as_str());
        w.s(330, th.clone());
        w.s(100, "AcDbSymbolTableRecord");
        w.s(100, "AcDbBlockTableRecord");
        w.s(2, name);
        w.s(340, layout.as_str());
    }
    w.s(0, "ENDTAB");
    w.s(0, "ENDSEC");

    // ---------------- BLOCKS ----------------
    w.s(0, "SECTION");
    w.s(2, "BLOCKS");
    for (name, brh, paper) in [("*Model_Space", ms_br.clone(), false), ("*Paper_Space", ps_br.clone(), true)] {
        let bh = w.h();
        w.s(0, "BLOCK");
        w.s(5, bh);
        w.s(330, brh.clone());
        w.s(100, "AcDbEntity");
        if paper {
            w.i(67, 1);
        }
        w.s(8, "0");
        w.s(100, "AcDbBlockBegin");
        w.s(2, name);
        w.i(70, 0);
        w.p3(10, 0.0, 0.0, 0.0);
        w.s(3, name);
        w.s(1, "");
        let eh = w.h();
        w.s(0, "ENDBLK");
        w.s(5, eh);
        w.s(330, brh);
        w.s(100, "AcDbEntity");
        if paper {
            w.i(67, 1);
        }
        w.s(8, "0");
        w.s(100, "AcDbBlockEnd");
    }
    w.s(0, "ENDSEC");

    // ---------------- ENTITIES ----------------
    w.s(0, "SECTION");
    w.s(2, "ENTITIES");
    for (l, layer) in lines.iter().zip(&names) {
        let pts: Vec<(f64, f64)> = l.points.iter().copied().filter(|p| p.0.is_finite() && p.1.is_finite()).collect();
        if pts.is_empty() {
            continue;
        }
        if pts.len() > 1 && !l.points_only {
            let h = w.h();
            w.s(0, "LWPOLYLINE");
            w.s(5, h);
            w.s(330, ms_br.clone());
            w.s(100, "AcDbEntity");
            w.s(8, layer.clone());
            w.s(100, "AcDbPolyline");
            w.i(90, pts.len() as i64);
            w.i(70, i64::from(l.closed));
            for &(x, y) in &pts {
                w.p2(10, x, y);
            }
        } else {
            for &(x, y) in &pts {
                let h = w.h();
                w.s(0, "POINT");
                w.s(5, h);
                w.s(330, ms_br.clone());
                w.s(100, "AcDbEntity");
                w.s(8, layer.clone());
                w.s(100, "AcDbPoint");
                w.p3(10, x, y, 0.0);
            }
        }
    }
    w.s(0, "ENDSEC");

    // ---------------- OBJECTS ----------------
    w.s(0, "SECTION");
    w.s(2, "OBJECTS");
    w.dict_head(&root_dict, "0");
    w.s(3, "ACAD_GROUP");
    w.s(350, group_dict.clone());
    w.s(3, "ACAD_LAYOUT");
    w.s(350, layout_dict.clone());
    w.s(3, "ACAD_PLOTSTYLENAME");
    w.s(350, pstyle_dict.clone());
    w.dict_head(&group_dict, &root_dict);
    w.dict_head(&layout_dict, &root_dict);
    w.s(3, "Model");
    w.s(350, model_layout.clone());
    w.s(3, "Layout1");
    w.s(350, paper_layout.clone());
    // Plot style names: a dictionary with a default (Normal) placeholder.
    w.s(0, "ACDBDICTIONARYWDFLT");
    w.s(5, pstyle_dict.clone());
    w.s(330, root_dict.clone());
    w.s(100, "AcDbDictionary");
    w.i(281, 1);
    w.s(3, "Normal");
    w.s(350, pstyle_normal.clone());
    w.s(100, "AcDbDictionaryWithDefault");
    w.s(340, pstyle_normal.clone());
    w.s(0, "ACDBPLACEHOLDER");
    w.s(5, pstyle_normal.clone());
    w.s(330, pstyle_dict.clone());
    for (h, name, tab, brh, plot_type) in
        [(&model_layout, "Model", 0, &ms_br, 1), (&paper_layout, "Layout1", 1, &ps_br, 5)]
    {
        w.s(0, "LAYOUT");
        w.s(5, h.as_str());
        w.s(330, layout_dict.clone());
        w.s(100, "AcDbPlotSettings");
        w.s(1, "");
        w.s(2, "none_device");
        w.s(4, "");
        w.s(6, "");
        for c in 40..=49 {
            w.f(c, 0.0);
        }
        w.f(140, 0.0);
        w.f(141, 0.0);
        w.f(142, 1.0);
        w.f(143, 1.0);
        w.i(70, 512 | 32 | 16);
        w.i(72, 1);
        w.i(73, 0);
        w.i(74, plot_type);
        w.s(7, "");
        w.i(75, 0);
        w.f(147, 1.0);
        w.f(148, 0.0);
        w.f(149, 0.0);
        w.s(100, "AcDbLayout");
        w.s(1, name);
        w.i(70, 1);
        w.i(71, tab);
        w.p2(10, 0.0, 0.0);
        w.p2(11, 12.0, 9.0);
        w.p3(12, 0.0, 0.0, 0.0);
        w.p3(14, x0, y0, 0.0);
        w.p3(15, x1, y1, 0.0);
        w.f(146, 0.0);
        w.p3(13, 0.0, 0.0, 0.0);
        w.p3(16, 1.0, 0.0, 0.0);
        w.p3(17, 0.0, 1.0, 0.0);
        w.i(76, 0);
        w.s(330, brh.as_str());
    }
    w.s(0, "ENDSEC");
    w.s(0, "EOF");

    let seed = format!("{:X}", w.next);
    w.t[seed_pos + 1] = (5, V::S(seed));
    render(&w.t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structure() {
        let pts = [(0.0, 0.0), (1.5, 2.0)];
        let lines = [
            DxfLine { caption: "Road: 1", color: Rgb(255, 0, 0), closed: false, points_only: false, points: &pts },
            DxfLine { caption: "Точки", color: Rgb(0, 0, 255), closed: false, points_only: true, points: &pts },
        ];
        let s = write(&lines, |_| 1);
        assert!(s.starts_with("  0\r\nSECTION\r\n  2\r\nHEADER\r\n  9\r\n$ACADVER\r\n  1\r\nAC1015\r\n"));
        assert!(s.contains("LWPOLYLINE") && s.contains("Road_ 1") && s.contains("\\U+0422"));
        assert!(s.ends_with("  0\r\nEOF\r\n"));
        // Every handle is unique.
        let mut handles: Vec<&str> = s.split("\r\n").collect::<Vec<_>>().chunks(2).filter(|c| c[0] == "  5" || c[0] == "105").map(|c| c[1]).collect();
        let n = handles.len();
        handles.sort();
        handles.dedup();
        assert_eq!(n, handles.len());
    }
}
