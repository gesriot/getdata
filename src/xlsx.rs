//! Office Open XML workbook (.xlsx) export, written as a minimal OPC package
//! the way GridCraft's `gridcraft-xlsx` writer does: content types first,
//! then package and workbook relationships, the workbook, its parts and the
//! document properties last.

use std::fmt::Write as _;
use std::io::{Cursor, Write};

const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";
const NS_MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const NS_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL_BASE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// A cell value.
pub enum Cell {
    Text(String),
    /// Number shown in scientific format.
    Number(f64),
}

/// One worksheet: sparse rows of (column, value), row/column indexes 0-based.
pub struct Sheet {
    pub name: String,
    pub rows: Vec<(u32, Vec<(u32, Cell)>)>,
    /// Column widths in characters, by column index.
    pub widths: Vec<(u32, f64)>,
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            // Characters not allowed in XML 1.0.
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => o.push(c),
        }
    }
    o
}

/// Column letters: 0 -> A, 25 -> Z, 26 -> AA.
fn col_name(mut c: u32) -> String {
    let mut s = Vec::new();
    loop {
        s.push(b'A' + (c % 26) as u8);
        if c < 26 {
            break;
        }
        c = c / 26 - 1;
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

/// xsd:double text that round-trips.
fn num(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let a = v.abs();
    if a == 0.0 || (1e-5..1e15).contains(&a) { format!("{v}") } else { format!("{v:e}") }
}

/// Sheet names may not contain `[]:*?/\` and are limited to 31 characters.
pub fn sheet_name(s: &str) -> String {
    let n: String = s.chars().filter(|c| !"[]:*?/\\".contains(*c)).take(31).collect();
    let n = n.trim_matches('\'').to_string();
    if n.is_empty() { "Sheet1".into() } else { n }
}

fn sheet_xml(sh: &Sheet) -> String {
    let mut o = String::from(XML_DECL);
    let _ = write!(o, "<worksheet xmlns=\"{NS_MAIN}\" xmlns:r=\"{NS_REL}\">");
    let max_row = sh.rows.iter().map(|r| r.0).max();
    let max_col = sh.rows.iter().flat_map(|r| r.1.iter().map(|c| c.0)).max();
    if let (Some(r), Some(c)) = (max_row, max_col) {
        let _ = write!(o, "<dimension ref=\"A1:{}{}\"/>", col_name(c), r + 1);
    }
    o.push_str("<sheetViews><sheetView workbookViewId=\"0\"><pane ySplit=\"2\" topLeftCell=\"A3\" activePane=\"bottomLeft\" state=\"frozen\"/></sheetView></sheetViews>");
    o.push_str("<sheetFormatPr defaultRowHeight=\"15\"/>");
    if !sh.widths.is_empty() {
        o.push_str("<cols>");
        for &(c, w) in &sh.widths {
            let _ = write!(o, "<col min=\"{0}\" max=\"{0}\" width=\"{w}\" customWidth=\"1\"/>", c + 1);
        }
        o.push_str("</cols>");
    }
    o.push_str("<sheetData>");
    for (r, cells) in &sh.rows {
        let _ = write!(o, "<row r=\"{}\">", r + 1);
        for (c, v) in cells {
            let rf = format!("{}{}", col_name(*c), r + 1);
            match v {
                Cell::Text(s) => {
                    let _ = write!(o, "<c r=\"{rf}\" t=\"inlineStr\" s=\"1\"><is><t xml:space=\"preserve\">{}</t></is></c>", esc(s));
                }
                Cell::Number(n) => {
                    let _ = write!(o, "<c r=\"{rf}\" s=\"2\"><v>{}</v></c>", num(*n));
                }
            }
        }
        o.push_str("</row>");
    }
    o.push_str("</sheetData>");
    o.push_str("<pageMargins left=\"0.7\" right=\"0.7\" top=\"0.75\" bottom=\"0.75\" header=\"0.3\" footer=\"0.3\"/>");
    o.push_str("</worksheet>");
    o
}

/// Cell styles: 0 default, 1 bold text (captions), 2 scientific numbers (built-in format 11, `0.00E+00`).
const STYLES: &str = concat!(
    "<styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">",
    "<fonts count=\"2\"><font><sz val=\"11\"/><name val=\"Calibri\"/><family val=\"2\"/></font>",
    "<font><b/><sz val=\"11\"/><name val=\"Calibri\"/><family val=\"2\"/></font></fonts>",
    "<fills count=\"2\"><fill><patternFill patternType=\"none\"/></fill><fill><patternFill patternType=\"gray125\"/></fill></fills>",
    "<borders count=\"1\"><border><left/><right/><top/><bottom/><diagonal/></border></borders>",
    "<cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs>",
    "<cellXfs count=\"3\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/>",
    "<xf numFmtId=\"0\" fontId=\"1\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyFont=\"1\"/>",
    "<xf numFmtId=\"11\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyNumberFormat=\"1\"/></cellXfs>",
    "<cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles>",
    "</styleSheet>"
);

/// Build the .xlsx package bytes.
pub fn write(sheets: &[Sheet], creator: &str, created_iso: &str) -> std::io::Result<Vec<u8>> {
    let mut parts: Vec<(String, Vec<u8>)> = Vec::new();

    // Package relationships.
    let rels = format!(
        "{XML_DECL}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
         <Relationship Id=\"rId1\" Type=\"{REL_BASE}/officeDocument\" Target=\"xl/workbook.xml\"/>\
         <Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/>\
         <Relationship Id=\"rId3\" Type=\"{REL_BASE}/extended-properties\" Target=\"docProps/app.xml\"/>\
         </Relationships>"
    );
    parts.push(("_rels/.rels".into(), rels.into_bytes()));

    // Workbook relationships and workbook.
    let mut wb_rels =
        format!("{XML_DECL}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">");
    let mut wb = format!("{XML_DECL}<workbook xmlns=\"{NS_MAIN}\" xmlns:r=\"{NS_REL}\"><bookViews><workbookView/></bookViews><sheets>");
    for (i, sh) in sheets.iter().enumerate() {
        let _ = write!(
            wb_rels,
            "<Relationship Id=\"rId{0}\" Type=\"{REL_BASE}/worksheet\" Target=\"worksheets/sheet{0}.xml\"/>",
            i + 1
        );
        let _ = write!(wb, "<sheet name=\"{}\" sheetId=\"{}\" r:id=\"rId{}\"/>", esc(&sheet_name(&sh.name)), i + 1, i + 1);
    }
    let _ = write!(
        wb_rels,
        "<Relationship Id=\"rId{}\" Type=\"{REL_BASE}/styles\" Target=\"styles.xml\"/></Relationships>",
        sheets.len() + 1
    );
    wb.push_str("</sheets></workbook>");
    parts.push(("xl/_rels/workbook.xml.rels".into(), wb_rels.into_bytes()));
    parts.push(("xl/workbook.xml".into(), wb.into_bytes()));
    for (i, sh) in sheets.iter().enumerate() {
        parts.push((format!("xl/worksheets/sheet{}.xml", i + 1), sheet_xml(sh).into_bytes()));
    }
    parts.push(("xl/styles.xml".into(), format!("{XML_DECL}{STYLES}").into_bytes()));

    // Document properties last.
    let core = format!(
        "{XML_DECL}<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" \
         xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" \
         xmlns:dcmitype=\"http://purl.org/dc/dcmitype/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\
         <dc:creator>{0}</dc:creator><dcterms:created xsi:type=\"dcterms:W3CDTF\">{1}</dcterms:created>\
         <dcterms:modified xsi:type=\"dcterms:W3CDTF\">{1}</dcterms:modified></cp:coreProperties>",
        esc(creator),
        esc(created_iso)
    );
    parts.push(("docProps/core.xml".into(), core.into_bytes()));
    let app = format!(
        "{XML_DECL}<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\" \
         xmlns:vt=\"http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes\"><Application>{}</Application></Properties>",
        esc(creator)
    );
    parts.push(("docProps/app.xml".into(), app.into_bytes()));

    let mut ct = format!(
        "{XML_DECL}<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
         <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
         <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
         <Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>"
    );
    for i in 0..sheets.len() {
        let _ = write!(
            ct,
            "<Override PartName=\"/xl/worksheets/sheet{}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>",
            i + 1
        );
    }
    ct.push_str(
        "<Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/>\
         <Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/>\
         <Override PartName=\"/docProps/app.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.extended-properties+xml\"/>\
         </Types>",
    );

    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(6))
        .last_modified_time(zip::DateTime::default());
    let io = |e: zip::result::ZipError| std::io::Error::other(e.to_string());
    zip.start_file("[Content_Types].xml", opts).map_err(io)?;
    zip.write_all(ct.as_bytes())?;
    for (name, data) in &parts {
        zip.start_file(name.as_str(), opts).map_err(io)?;
        zip.write_all(data)?;
    }
    Ok(zip.finish().map_err(io)?.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns() {
        assert_eq!(col_name(0), "A");
        assert_eq!(col_name(25), "Z");
        assert_eq!(col_name(26), "AA");
        assert_eq!(col_name(701), "ZZ");
        assert_eq!(col_name(702), "AAA");
    }

    #[test]
    fn package_is_zip() {
        let sh = Sheet {
            name: "a/b".into(),
            rows: vec![(0, vec![(0, Cell::Text("L&1".into()))]), (2, vec![(0, Cell::Number(1.5e-7)), (1, Cell::Number(2.0))])],
            widths: vec![(0, 22.0)],
        };
        let bytes = write(&[sh], "test", "2026-01-01T00:00:00Z").unwrap();
        assert_eq!(&bytes[..2], b"PK");
        let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert_eq!(z.by_index(0).unwrap().name(), "[Content_Types].xml");
        let mut s = String::new();
        std::io::Read::read_to_string(&mut z.by_name("xl/worksheets/sheet1.xml").unwrap(), &mut s).unwrap();
        assert!(s.contains("L&amp;1") && s.contains("<v>1.5e-7</v>") && s.contains("r=\"B3\""), "{s}");
    }
}
