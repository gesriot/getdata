//! Export of digitized data, in the formats GetData has always written:
//! clipboard / TXT (three number formats), XLS (BIFF2), XML (Excel 2003 XML
//! spreadsheet), EPS and DXF.

use crate::coords::Transform;
use crate::gdw::fmt_e;
use crate::model::{DataLine, LineType, Rgb, Workspace};

pub const PROGRAM: &str = "GetData";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum NumberFormat {
    /// `0.001`
    Fixed = 0,
    /// `1e-003`
    Exponent = 1,
    /// Mathematica `{1*10^(-3)}`
    Mathematica = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Separator {
    Space = 0,
    Tab = 1,
}

/// Settings of "Export to text file" / "Export to clipboard".
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TextExportSettings {
    pub all_lines: bool,
    pub print_caption: bool,
    pub format: NumberFormat,
    pub sort_by_x: bool,
    pub header: bool,
    pub separator: Separator,
}

impl TextExportSettings {
    pub fn file_default() -> Self {
        Self {
            all_lines: true,
            print_caption: true,
            format: NumberFormat::Exponent,
            sort_by_x: false,
            header: true,
            separator: Separator::Space,
        }
    }

    pub fn clipboard_default() -> Self {
        Self {
            all_lines: false,
            print_caption: false,
            format: NumberFormat::Fixed,
            sort_by_x: false,
            header: false,
            separator: Separator::Tab,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileFormat {
    Txt,
    Xlsx,
    Xls,
    Xml,
    Eps,
    Dxf,
    DxfR12,
}

impl FileFormat {
    pub const ALL: [FileFormat; 7] =
        [FileFormat::Txt, FileFormat::Xlsx, FileFormat::Xls, FileFormat::Xml, FileFormat::Eps, FileFormat::Dxf, FileFormat::DxfR12];

    pub fn filter_name(self) -> &'static str {
        match self {
            FileFormat::Txt => "TXT - ASCII file",
            FileFormat::Xlsx => "XLSX - MS Excel 2007+",
            FileFormat::Xls => "XLS - MS Excel 2.x (legacy)",
            FileFormat::Xml => "XML - XML Table",
            FileFormat::Eps => "EPS - Encapsulated PostScript",
            FileFormat::Dxf => "DXF - AutoCAD 2000+",
            FileFormat::DxfR12 => "DXF - AutoCAD R12 (legacy)",
        }
    }

    pub fn ext(self) -> &'static str {
        match self {
            FileFormat::Txt => "txt",
            FileFormat::Xlsx => "xlsx",
            FileFormat::Xls => "xls",
            FileFormat::Xml => "xml",
            FileFormat::Eps => "eps",
            FileFormat::Dxf | FileFormat::DxfR12 => "dxf",
        }
    }
}

struct ExportLine<'a> {
    line: &'a DataLine,
    values: Vec<(f64, f64)>,
}

fn collect<'a>(ws: &'a Workspace, t: &Transform, all_lines: bool, sort_by_x: bool) -> Vec<ExportLine<'a>> {
    let lines: Vec<&DataLine> = if all_lines { ws.lines.iter().collect() } else { vec![ws.line()] };
    lines
        .into_iter()
        .map(|line| {
            let mut values: Vec<(f64, f64)> = line.points.iter().map(|&p| t.pt_to_logical(p)).collect();
            if sort_by_x {
                // Stable: equal x keep their y order.
                values.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
            }
            ExportLine { line, values }
        })
        .collect()
}

// ---- C printf-like number formatting -------------------------------------

/// `% .*f` / `%.*f`: fixed with optional leading space for non-negative numbers.
fn c_f(v: f64, prec: usize, space: bool) -> String {
    let s = format!("{v:.prec$}");
    if space && !s.starts_with('-') { format!(" {s}") } else { s }
}

/// `%.*g`
pub fn c_g(v: f64, prec: usize) -> String {
    if v == 0.0 {
        return "0".into();
    }
    if !v.is_finite() {
        return v.to_string();
    }
    let prec = prec.max(1);
    let e = format!("{v:.*e}", prec - 1);
    let (_, exp) = e.split_once('e').unwrap_or(("", "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let strip = |s: String| -> String {
        if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
    };
    if exp < -4 || exp >= prec as i32 {
        let (m, _) = e.split_once('e').unwrap_or((&e, ""));
        let m = strip(m.to_string());
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{m}e{sign}{:03}", exp.abs())
    } else {
        let decimals = (prec as i32 - 1 - exp).max(0) as usize;
        strip(format!("{v:.decimals$}"))
    }
}

/// Exponent of the number for the "0.001" format: floor(log10|v|), 0 for tiny values.
fn exp10(v: f64) -> i32 {
    let a = v.abs();
    if a >= 1e-300 { a.log10().floor() as i32 } else { 0 }
}

fn fixed_prec(v: f64) -> usize {
    (14 - exp10(v)).max(1) as usize
}

fn fixed_width(v: f64) -> usize {
    let e = exp10(v);
    fixed_prec(v) + 3 + e.max(0) as usize
}

/// Replace each `e+003` by `*10^(+003)`.
fn mathematica(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == 'e' {
            out.push_str("*10^(");
            // sign and three digits
            for _ in 0..4 {
                if let Some(d) = chars.next() {
                    out.push(d);
                }
            }
            out.push(')');
        } else {
            out.push(c);
        }
    }
    out
}

pub fn now_string() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let t = secs as i64 + local_utc_offset_secs();
    let (y, m, d) = civil_from_days(t.div_euclid(86400));
    let sod = t.rem_euclid(86400);
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    format!("{} {:02} {}, {:02}:{:02}", MONTHS[(m - 1) as usize], d, y, sod / 3600, sod % 3600 / 60)
}

#[cfg(windows)]
fn local_utc_offset_secs() -> i64 {
    #[repr(C)]
    struct TimeZoneInformation {
        bias: i32,
        standard_name: [u16; 32],
        standard_date: [u16; 8],
        standard_bias: i32,
        daylight_name: [u16; 32],
        daylight_date: [u16; 8],
        daylight_bias: i32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetTimeZoneInformation(tz: *mut TimeZoneInformation) -> u32;
    }
    // SAFETY: plain C struct filled in by the OS.
    let mut tz: TimeZoneInformation = unsafe { std::mem::zeroed() };
    let r = unsafe { GetTimeZoneInformation(&mut tz) };
    let bias = match r {
        2 => tz.bias + tz.daylight_bias,
        1 => tz.bias + tz.standard_bias,
        _ => tz.bias,
    };
    -(bias as i64) * 60
}

#[cfg(not(windows))]
fn local_utc_offset_secs() -> i64 {
    0
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn program_name() -> String {
    format!("{PROGRAM} {VERSION}")
}

// ---- Text / clipboard ------------------------------------------------------

pub fn to_text(ws: &Workspace, t: &Transform, s: &TextExportSettings) -> String {
    let lines = collect(ws, t, s.all_lines, s.sort_by_x);
    let tab = s.separator == Separator::Tab;
    let mut o = String::new();
    if s.header {
        o.push_str(&crate::i18n::trf(
            "Generated by {} on {}\nfrom file '{}'\n\n",
            &[&program_name(), &now_string(), &ws.image_file],
        ));
    }
    let mut first = true;
    for l in lines.iter().filter(|l| !l.values.is_empty()) {
        if !first {
            o.push('\n');
        }
        first = false;
        match s.format {
            NumberFormat::Fixed => {
                if s.print_caption {
                    o.push_str(&l.line.caption);
                    o.push('\n');
                }
                let wx = l.values.iter().map(|v| fixed_width(v.0)).max().unwrap_or(0);
                let wy = l.values.iter().map(|v| fixed_width(v.1)).max().unwrap_or(0);
                for &(x, y) in &l.values {
                    if tab {
                        o.push_str(&format!("{}\t{}\n", c_f(x, fixed_prec(x), false), c_f(y, fixed_prec(y), false)));
                    } else {
                        let xs = c_f(x, fixed_prec(x), true);
                        let ys = c_f(y, fixed_prec(y), true);
                        o.push_str(&format!("{xs:<wx$}   {ys:<wy$}\n"));
                    }
                }
            }
            NumberFormat::Exponent => {
                if s.print_caption {
                    o.push_str(&l.line.caption);
                    o.push('\n');
                }
                for &(x, y) in &l.values {
                    let (xs, ys) = (fmt_e(x, 14), fmt_e(y, 14));
                    if tab {
                        o.push_str(&format!("{xs}\t{ys}\n"));
                    } else {
                        let sp = |s: String| if s.starts_with('-') { s } else { format!(" {s}") };
                        o.push_str(&format!("{:<22}   {:<22}\n", sp(xs), sp(ys)));
                    }
                }
            }
            NumberFormat::Mathematica => {
                if s.print_caption {
                    o.push_str(&format!("{{{{\"{}\"}}, {{\n", l.line.caption));
                } else {
                    o.push_str("{\n");
                }
                let n = l.values.len();
                for (i, &(x, y)) in l.values.iter().enumerate() {
                    let sep = if tab { ",\t" } else { ", " };
                    let end = if i + 1 == n { "}\n" } else { "},\n" };
                    let line = format!("{{{}{sep}{}{end}", fmt_e(x, 14), fmt_e(y, 14));
                    o.push_str(&mathematica(&line));
                }
                o.push_str(if s.print_caption { "}}\n" } else { "}\n" });
            }
        }
    }
    o.replace('\n', "\r\n")
}

// ---- Excel XML ---------------------------------------------------------------

fn xml_esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn sheet_name(image_file: &str) -> String {
    let name = image_file.rsplit(['\\', '/']).next().unwrap_or(image_file);
    let name: String = name.chars().filter(|c| !"[]:*?/\\".contains(*c)).take(31).collect();
    if name.is_empty() { "Sheet1".into() } else { name }
}

pub fn to_excel_xml(ws: &Workspace, t: &Transform) -> String {
    let lines = collect(ws, t, true, false);
    let max = lines.iter().map(|l| l.values.len()).max().unwrap_or(0);
    let mut o = String::new();
    o.push_str("<?xml version=\"1.0\"?>\n");
    o.push_str("<Workbook xmlns=\"urn:schemas-microsoft-com:office:spreadsheet\"\n");
    o.push_str(" xmlns:o=\"urn:schemas-microsoft-com:office:office\"\n");
    o.push_str(" xmlns:x=\"urn:schemas-microsoft-com:office:excel\"\n");
    o.push_str(" xmlns:ss=\"urn:schemas-microsoft-com:office:spreadsheet\"\n");
    o.push_str(" xmlns:html=\"http://www.w3.org/TR/REC-html40\">\n");
    o.push_str(" <DocumentProperties xmlns=\"urn:schemas-microsoft-com:office:office\">\n");
    o.push_str(&format!("  <Author>{}</Author>\n", program_name()));
    o.push_str(&format!("  <Created>{}</Created>\n", now_string()));
    o.push_str(" </DocumentProperties>\n");
    o.push_str(" <Styles>\n  <Style ss:ID=\"s21\">\n   <NumberFormat ss:Format=\"Scientific\"/>\n  </Style>\n </Styles>\n");
    o.push_str(&format!(" <Worksheet ss:Name=\"{}\">\n", xml_esc(&sheet_name(&ws.image_file))));
    o.push_str(&format!(
        "  <Table ss:ExpandedColumnCount=\"{}\" ss:ExpandedRowCount=\"{}\">\n",
        (lines.len() * 3).saturating_sub(1).max(1),
        max + 2
    ));
    o.push_str("   <Row>\n");
    for (i, l) in lines.iter().enumerate() {
        o.push_str(&format!(
            "    <Cell ss:Index=\"{}\"><Data ss:Type=\"String\">{}</Data></Cell>\n",
            i * 3 + 1,
            xml_esc(&l.line.caption)
        ));
    }
    o.push_str("   </Row>\n");
    for r in 0..max {
        o.push_str(if r == 0 { "   <Row ss:Index=\"3\">\n" } else { "   <Row>\n" });
        for (i, l) in lines.iter().enumerate() {
            if let Some(&(x, y)) = l.values.get(r) {
                o.push_str(&format!(
                    "    <Cell ss:StyleID=\"s21\" ss:Index=\"{}\"><Data ss:Type=\"Number\">{}</Data></Cell>\n",
                    i * 3 + 1,
                    fmt_e(x, 14)
                ));
                o.push_str(&format!(
                    "    <Cell ss:StyleID=\"s21\"><Data ss:Type=\"Number\">{}</Data></Cell>\n",
                    fmt_e(y, 14)
                ));
            }
        }
        o.push_str("   </Row>\n");
    }
    o.push_str("  </Table>\n </Worksheet>\n</Workbook>\n");
    o.replace('\n', "\r\n")
}

// ---- XLSX ------------------------------------------------------------------

/// Current UTC time as `YYYY-MM-DDTHH:MM:SSZ`.
fn utc_iso() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let (y, m, d) = civil_from_days(secs.div_euclid(86400));
    let sod = secs.rem_euclid(86400);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", sod / 3600, sod % 3600 / 60, sod % 60)
}

/// Excel 2007+ workbook: same layout as the XLS export (captions in row 1,
/// two columns per line plus a spacer), with an X/Y header row and frozen headers.
pub fn to_xlsx(ws: &Workspace, t: &Transform) -> std::io::Result<Vec<u8>> {
    use crate::xlsx::{Cell, Sheet};
    let lines = collect(ws, t, true, false);
    let max = lines.iter().map(|l| l.values.len()).max().unwrap_or(0);
    let mut rows: Vec<(u32, Vec<(u32, Cell)>)> = Vec::with_capacity(max + 2);
    let col = |i: usize| (i * 3) as u32;
    rows.push((0, lines.iter().enumerate().map(|(i, l)| (col(i), Cell::Text(l.line.caption.clone()))).collect()));
    rows.push((
        1,
        lines
            .iter()
            .enumerate()
            .flat_map(|(i, _)| [(col(i), Cell::Text("X".into())), (col(i) + 1, Cell::Text("Y".into()))])
            .collect(),
    ));
    for r in 0..max {
        let cells: Vec<(u32, Cell)> = lines
            .iter()
            .enumerate()
            .filter_map(|(i, l)| l.values.get(r).map(|&(x, y)| (i, x, y)))
            .flat_map(|(i, x, y)| [(col(i), Cell::Number(x)), (col(i) + 1, Cell::Number(y))])
            .collect();
        rows.push(((r + 2) as u32, cells));
    }
    let widths = (0..lines.len())
        .flat_map(|i| [(col(i), 22.0), (col(i) + 1, 22.0), (col(i) + 2, 3.0)])
        .filter(|&(c, _)| c + 1 < (lines.len() * 3) as u32)
        .collect();
    let sheet = Sheet { name: sheet_name(&ws.image_file), rows, widths };
    crate::xlsx::write(&[sheet], &program_name(), &utc_iso())
}

// ---- XLS (BIFF2) -----------------------------------------------------------

fn biff(out: &mut Vec<u8>, id: u16, data: &[u8]) {
    out.extend_from_slice(&id.to_le_bytes());
    out.extend_from_slice(&(data.len() as u16).to_le_bytes());
    out.extend_from_slice(data);
}

/// Binary Excel 2.x worksheet: captions in row 1, data from row 3, each line
/// using two columns followed by an empty one.
pub fn to_xls(ws: &Workspace, t: &Transform) -> Vec<u8> {
    let lines = collect(ws, t, true, false);
    let max = lines.iter().map(|l| l.values.len()).max().unwrap_or(0);
    let mut o = Vec::new();
    biff(&mut o, 0x0009, &[0x02, 0x00, 0x10, 0x00]); // BOF, worksheet
    biff(&mut o, 0x0042, &0x8001u16.to_le_bytes()); // CODEPAGE: Windows ANSI
    // FORMAT records; index 11 is the scientific format used by the cells.
    const FORMATS: [&str; 12] = [
        "General",
        "0",
        "0.00",
        "#,##0",
        "#,##0.00",
        "\"$\"#,##0_);\\(\"$\"#,##0\\)",
        "\"$\"#,##0_);[Red]\\(\"$\"#,##0\\)",
        "\"$\"#,##0.00_);\\(\"$\"#,##0.00\\)",
        "\"$\"#,##0.00_);[Red]\\(\"$\"#,##0.00\\)",
        "0%",
        "0.00%",
        "0.00E+00",
    ];
    biff(&mut o, 0x001F, &(FORMATS.len() as u16).to_le_bytes());
    for f in FORMATS {
        let mut d = vec![f.len() as u8];
        d.extend_from_slice(f.as_bytes());
        biff(&mut o, 0x001E, &d);
    }
    let last_col = (lines.len() * 3).saturating_sub(1) as u16;
    let mut dim = Vec::new();
    dim.extend_from_slice(&0u16.to_le_bytes());
    dim.extend_from_slice(&((max + 2) as u16).to_le_bytes());
    dim.extend_from_slice(&0u16.to_le_bytes());
    dim.extend_from_slice(&last_col.to_le_bytes());
    biff(&mut o, 0x0000, &dim); // DIMENSIONS
    for (i, l) in lines.iter().enumerate() {
        // LABEL, ANSI text (max 255 bytes)
        let text: Vec<u8> = l.line.caption.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).take(255).collect();
        let mut d = Vec::new();
        d.extend_from_slice(&0u16.to_le_bytes());
        d.extend_from_slice(&((i * 3) as u16).to_le_bytes());
        d.extend_from_slice(&[0, 0, 0]);
        d.push(text.len() as u8);
        d.extend_from_slice(&text);
        biff(&mut o, 0x0004, &d);
    }
    for r in 0..max {
        for (i, l) in lines.iter().enumerate() {
            if let Some(&(x, y)) = l.values.get(r) {
                for (c, v) in [(i * 3, x), (i * 3 + 1, y)] {
                    let mut d = Vec::new();
                    d.extend_from_slice(&((r + 2) as u16).to_le_bytes());
                    d.extend_from_slice(&(c as u16).to_le_bytes());
                    d.extend_from_slice(&[0x00, 0x0B, 0x00]); // format #11
                    d.extend_from_slice(&v.to_le_bytes());
                    biff(&mut o, 0x0003, &d); // NUMBER
                }
            }
        }
    }
    biff(&mut o, 0x000A, &[]); // EOF
    o
}

// ---- EPS -------------------------------------------------------------------

/// Round `v` up (away from zero) to `digits` significant digits.
fn ceil_sig(digits: i32, v: f64) -> f64 {
    if v == 0.0 {
        return 0.0;
    }
    let a = v.abs();
    let e = a.log10().ceil();
    let m = 10f64.powf(digits as f64 - e);
    let r = (a * m).ceil() / m;
    if v >= 0.0 { r } else { -r }
}

pub fn to_eps(ws: &Workspace, t: &Transform, title: &str) -> String {
    let lines = collect(ws, t, true, false);
    let mut first = true;
    let (mut x0, mut x1, mut y0, mut y1) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for &(x, y) in lines.iter().flat_map(|l| l.values.iter()) {
        if first || x < x0 {
            x0 = x;
        }
        if first || x > x1 {
            x1 = x;
        }
        if first || y < y0 {
            y0 = y;
        }
        if first || y > y1 {
            y1 = y;
        }
        first = false;
    }
    if x0 != x1 {
        let m = (x1 - x0) * 0.05;
        x0 -= m;
        x1 += m;
    }
    if y0 != y1 {
        let m = (y1 - y0) * 0.05;
        y0 -= m;
        y1 += m;
    }
    let (scale, w, h, tx, ty);
    if x0 == x1 && y0 == y1 {
        scale = 1.0;
        w = 50;
        h = 50;
        tx = -x0 + 25.0;
        ty = -y0 + 25.0;
    } else {
        let s = if x0 == x1 {
            400.0 / (y1 - y0)
        } else if y0 == y1 {
            400.0 / (x1 - x0)
        } else {
            (400.0 / (x1 - x0)).min(400.0 / (y1 - y0))
        };
        scale = ceil_sig(2, s);
        let mut ww = ((x1 - x0) * scale).ceil() as i32;
        let mut hh = ((y1 - y0) * scale).ceil() as i32;
        let mut txx = -x0;
        let mut tyy = -y0;
        if ww < 50 {
            txx += (50 - ww) as f64 / (scale * 2.0);
            ww = 50;
        }
        if hh < 50 {
            tyy += (50 - hh) as f64 / (scale * 2.0);
            hh = 50;
        }
        (w, h, tx, ty) = (ww, hh, txx, tyy);
    }
    let lw = ceil_sig(4, 1.0 / scale);
    let dot = lw * 2.0;
    let rgb = |c: Rgb| format!("{:.6} {:.6} {:.6} r\n", c.0 as f64 / 255.0, c.1 as f64 / 255.0, c.2 as f64 / 255.0);

    let mut o = String::new();
    o.push_str("%!PS-Adobe-3.0 EPSF-3.0\n");
    o.push_str(&format!("%%BoundingBox: 0 0 {w} {h}\n"));
    o.push_str(&format!("%%Creator: {}\n", program_name()));
    o.push_str(&format!("%%Title: {title}\n"));
    o.push_str(&format!("%%CreationDate: {}\n", now_string()));
    o.push_str("%%EndComments\n%%BeginProlog\n");
    o.push_str("/l {lineto} bind def\n/m {moveto} bind def\n/s {stroke} bind def\n/r {setrgbcolor} bind def\n");
    o.push_str("/dot{\n  0 360 arc\n  closepath\n  fill\n} bind def\n%%EndProlog\n");
    o.push_str(&format!("{} setlinewidth\n", c_g(lw, 6)));
    o.push_str(&format!("{} {} scale\n", c_g(scale, 6), c_g(scale, 6)));
    o.push_str(&format!("{} {} translate\n", c_g(tx, 17), c_g(ty, 14)));
    for l in &lines {
        if l.values.is_empty() {
            continue;
        }
        if l.values.len() > 1 && l.line.line_type != LineType::SeparatePoints {
            o.push_str("newpath\n");
            o.push_str(&rgb(l.line.pt_color));
            for (i, &(x, y)) in l.values.iter().enumerate() {
                o.push_str(&format!("{} {} {}\n", c_g(x, 14), c_g(y, 14), if i == 0 { "m" } else { "l" }));
            }
            if l.line.line_type == LineType::ClosedPolyline {
                o.push_str("closepath\n");
            }
            o.push_str("s\n");
        } else {
            o.push_str(&rgb(l.line.pt_color));
            for &(x, y) in &l.values {
                o.push_str(&format!("newpath\n{} {} {} dot\n", c_g(x, 14), c_g(y, 14), c_g(dot, 6)));
            }
        }
    }
    o.push_str("%%EOF");
    o.replace('\n', "\r\n")
}

// ---- DXF -------------------------------------------------------------------

/// AutoCAD Color Index palette, 256 RGB triples.
const ACI: &[u8; 768] = include_bytes!("../assets/aci.bin");

/// Nearest AutoCAD colour index (the first one wins on ties).
pub fn aci(c: Rgb) -> usize {
    let d = |i: usize| {
        let p = &ACI[i * 3..i * 3 + 3];
        let (dr, dg, db) = (p[0] as i32 - c.0 as i32, p[1] as i32 - c.1 as i32, p[2] as i32 - c.2 as i32);
        dr * dr + dg * dg + db * db
    };
    (0..256).min_by_key(|&i| (d(i), i)).unwrap_or(7)
}

/// AutoCAD 2000 (AC1015) DXF: a layer per line, LWPOLYLINE or POINT entities.
pub fn to_dxf(ws: &Workspace, t: &Transform) -> String {
    let lines = collect(ws, t, true, false);
    let dl: Vec<crate::dxf::DxfLine> = lines
        .iter()
        .map(|l| crate::dxf::DxfLine {
            caption: &l.line.caption,
            color: l.line.pt_color,
            closed: l.line.line_type == LineType::ClosedPolyline,
            points_only: l.line.line_type == LineType::SeparatePoints,
            points: &l.values,
        })
        .collect();
    crate::dxf::write(&dl, aci)
}

/// R12 DXF.
pub fn to_dxf_r12(ws: &Workspace, t: &Transform) -> String {
    let lines = collect(ws, t, true, false);
    let mut o = String::new();
    o.push_str("0\nSECTION\n2\nHEADER\n");
    o.push_str("9\n$PDMODE\n70\n34\n9\n$PDSIZE\n40\n-2.0\n");
    o.push_str("0\nENDSEC\n");
    o.push_str("0\nSECTION\n2\nTABLES\n");
    o.push_str(&format!("0\nTABLE\n2\nLTYPE\n70\n{}\n", lines.len()));
    for (i, l) in lines.iter().enumerate() {
        o.push_str(&format!("0\nLTYPE\n2\nSolidLine{}\n70\n0\n3\n", i + 1));
        o.push_str(&l.line.caption);
        o.push_str("\n72\n65\n73\n0\n40\n0.0\n");
    }
    o.push_str("0\nENDTAB\n");
    o.push_str("0\nTABLE\n2\nLAYER\n70\n1\n");
    o.push_str(&format!("0\nLAYER\n2\nLayer1\n70\n0\n62\n{}\n6\nSolidLine1\n", aci(crate::model::POINT_COLORS[0])));
    o.push_str("0\nENDTAB\n");
    o.push_str("0\nENDSEC\n");
    o.push_str("0\nSECTION\n2\nBLOCKS\n0\nENDSEC\n");
    o.push_str("0\nSECTION\n2\nENTITIES\n");
    for (i, l) in lines.iter().enumerate() {
        if l.values.is_empty() {
            continue;
        }
        let style = format!("6\nSolidLine{}\n62\n{}\n", i + 1, aci(l.line.pt_color));
        if l.values.len() > 1 && l.line.line_type != LineType::SeparatePoints {
            o.push_str("0\nPOLYLINE\n8\nLayer1\n");
            o.push_str(&style);
            o.push_str(if l.line.line_type == LineType::ClosedPolyline { "70\n1\n" } else { "70\n0\n" });
            o.push_str("66\n1\n10\n0.0\n20\n0.0\n");
            for &(x, y) in &l.values {
                o.push_str("0\nVERTEX\n8\nLayer1\n");
                o.push_str(&format!("10\n{}\n20\n{}\n", c_g(x, 14), c_g(y, 14)));
            }
            o.push_str("0\nSEQEND\n");
        } else {
            for &(x, y) in &l.values {
                o.push_str("0\nPOINT\n8\nLayer1\n");
                o.push_str(&style);
                o.push_str(&format!("10\n{}\n20\n{}\n", c_g(x, 14), c_g(y, 14)));
            }
        }
    }
    o.push_str("0\nENDSEC\n0\nEOF\n");
    o.replace('\n', "\r\n")
}

/// Export to a file in the given format.
pub fn export_file(ws: &Workspace, t: &Transform, fmt: FileFormat, path: &std::path::Path, txt: &TextExportSettings) -> std::io::Result<()> {
    let bytes: Vec<u8> = match fmt {
        FileFormat::Txt => to_text(ws, t, txt).into_bytes(),
        FileFormat::Xlsx => to_xlsx(ws, t)?,
        FileFormat::Xls => to_xls(ws, t),
        FileFormat::Xml => to_excel_xml(ws, t).into_bytes(),
        FileFormat::Eps => {
            let title = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            to_eps(ws, t, &title).into_bytes()
        }
        FileFormat::Dxf => to_dxf(ws, t).into_bytes(),
        FileFormat::DxfR12 => to_dxf_r12(ws, t).into_bytes(),
    };
    std::fs::write(path, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AxisPoint, Pt};

    fn ws() -> Workspace {
        let mut ws = Workspace::default();
        ws.axes = [
            Some(AxisPoint { pos: Pt::new(0, 900), value: 0.0 }),
            Some(AxisPoint { pos: Pt::new(900, 900), value: 100.0 }),
            Some(AxisPoint { pos: Pt::new(0, 900), value: 0.0 }),
            Some(AxisPoint { pos: Pt::new(0, 0), value: 10.0 }),
        ];
        ws.lines[0].points = vec![Pt::new(90, 810), Pt::new(450, 450)];
        ws
    }

    #[test]
    fn g_format() {
        assert_eq!(c_g(0.5, 14), "0.5");
        assert_eq!(c_g(1e20, 14), "1e+020");
        assert_eq!(c_g(123.456, 14), "123.456");
        assert_eq!(c_g(0.0001, 6), "0.0001");
        assert_eq!(c_g(0.00001, 6), "1e-005");
    }

    #[test]
    fn text_formats() {
        let w = ws();
        let t = Transform::new(&w).unwrap();
        let mut s = TextExportSettings::clipboard_default();
        let txt = to_text(&w, &t, &s);
        assert_eq!(txt, "10.0000000000000\t1.00000000000000\r\n50.0000000000000\t5.00000000000000\r\n");
        s.separator = Separator::Space;
        let txt = to_text(&w, &t, &s);
        assert!(txt.starts_with(" 10.0000000000000    1.00000000000000\r\n"), "{txt:?}");
        s.format = NumberFormat::Mathematica;
        s.print_caption = true;
        let txt = to_text(&w, &t, &s);
        assert!(txt.starts_with("{{\"Line #1\"}, {\r\n{1.00000000000000*10^(+001), "), "{txt:?}");
        assert!(txt.ends_with("}\r\n}}\r\n"), "{txt:?}");
    }

    #[test]
    fn other_formats_build() {
        let w = ws();
        let t = Transform::new(&w).unwrap();
        assert!(to_dxf_r12(&w, &t).contains("VERTEX"));
        assert!(to_dxf(&w, &t).contains("LWPOLYLINE"));
        assert!(to_eps(&w, &t, "x").contains("%%BoundingBox: 0 0 "));
        assert!(to_excel_xml(&w, &t).contains("ss:ExpandedRowCount=\"4\""));
        assert_eq!(&to_xlsx(&w, &t).unwrap()[..2], b"PK");
        let xls = to_xls(&w, &t);
        assert_eq!(&xls[..4], &[0x09, 0x00, 0x04, 0x00]);
    }

    #[test]
    fn aci_basic() {
        assert_eq!(aci(Rgb(255, 0, 0)), 1);
        assert_eq!(aci(Rgb(0, 0, 0)), 0);
    }
}
