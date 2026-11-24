//! Minimal SpreadsheetML workbook: numbers and inline strings.

use super::{refused, zip_bytes, WriteError};

const MAIN_NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const REL_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PACKAGE_NS: &str = "http://schemas.openxmlformats.org/package/2006";
const XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";
/// Excel refuses longer cell text.
const MAX_TEXT: usize = 32_767;

#[derive(Clone, Debug)]
pub(crate) enum Cell {
    Empty,
    Number(f64),
    Text(String),
}

pub(crate) fn workbook(sheets: &[(&str, Vec<Vec<Cell>>)]) -> Result<Vec<u8>, WriteError> {
    let mut types = format!(
        "{XML}<Types xmlns=\"{PACKAGE_NS}/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>"
    );
    let mut book = format!("{XML}<workbook xmlns=\"{MAIN_NS}\" xmlns:r=\"{REL_NS}\"><sheets>");
    let mut rels = format!("{XML}<Relationships xmlns=\"{PACKAGE_NS}/relationships\">");
    let mut parts: Vec<(String, String)> = Vec::new();
    for (index, (name, rows)) in sheets.iter().enumerate() {
        let id = index + 1;
        types.push_str(&format!("<Override PartName=\"/xl/worksheets/sheet{id}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"));
        book.push_str(&format!(
            "<sheet name=\"{name}\" sheetId=\"{id}\" r:id=\"rId{id}\"/>"
        ));
        rels.push_str(&format!("<Relationship Id=\"rId{id}\" Type=\"{REL_NS}/worksheet\" Target=\"worksheets/sheet{id}.xml\"/>"));
        parts.push((format!("xl/worksheets/sheet{id}.xml"), sheet_xml(rows)?));
    }
    types.push_str("</Types>");
    book.push_str("</sheets></workbook>");
    rels.push_str("</Relationships>");
    let root = format!("{XML}<Relationships xmlns=\"{PACKAGE_NS}/relationships\"><Relationship Id=\"rId1\" Type=\"{REL_NS}/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>");
    let mut entries = vec![
        ("[Content_Types].xml".to_string(), types),
        ("_rels/.rels".to_string(), root),
        ("xl/workbook.xml".to_string(), book),
        ("xl/_rels/workbook.xml.rels".to_string(), rels),
    ];
    entries.extend(parts);
    zip_bytes(
        entries
            .iter()
            .map(|(name, text)| (name.as_str(), text.as_bytes())),
    )
}

fn sheet_xml(rows: &[Vec<Cell>]) -> Result<String, WriteError> {
    let mut out = format!("{XML}<worksheet xmlns=\"{MAIN_NS}\"><sheetData>");
    for (row_index, row) in rows.iter().enumerate() {
        let number = row_index + 1;
        out.push_str(&format!("<row r=\"{number}\">"));
        for (column, cell) in row.iter().enumerate() {
            let reference = || format!("{}{number}", column_letters(column));
            match cell {
                Cell::Empty => {}
                Cell::Number(value) => {
                    out.push_str(&format!("<c r=\"{}\"><v>{value:?}</v></c>", reference()));
                }
                Cell::Text(text) => {
                    if text.chars().count() > MAX_TEXT {
                        return Err(refused("a text cell exceeds the Excel length limit"));
                    }
                    out.push_str(&format!(
                        "<c r=\"{}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",
                        reference(),
                        escape(text)
                    ));
                }
            }
        }
        out.push_str("</row>");
    }
    out.push_str("</sheetData></worksheet>");
    Ok(out)
}

fn column_letters(mut column: usize) -> String {
    let mut letters = Vec::new();
    loop {
        letters.push(b'A' + (column % 26) as u8);
        if column < 26 {
            break;
        }
        column = column / 26 - 1;
    }
    letters.iter().rev().map(|byte| *byte as char).collect()
}

/// XML 1.0 forbids most control characters.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\t' | '\n' | '\r' => out.push(ch),
            ch if ch < ' ' || matches!(ch, '\u{FFFE}' | '\u{FFFF}') => out.push('_'),
            ch => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_letters_follow_spreadsheet_order() {
        for (index, letters) in [
            (0, "A"),
            (25, "Z"),
            (26, "AA"),
            (701, "ZZ"),
            (702, "AAA"),
            (16383, "XFD"),
        ] {
            assert_eq!(column_letters(index), letters);
        }
    }
}
