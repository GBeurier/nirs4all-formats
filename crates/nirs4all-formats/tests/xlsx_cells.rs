//! Independent literal packets and original retained OOXML bytes; no golden rewrite.

use std::io::{Cursor, Read, Write};

use nirs4all_formats::{
    decode_xlsx_cells, DecodedCell, DecodedWorkbook, XlsxCellOrigin, XlsxCellValue,
    XlsxDateKind, XlsxDecodeError, XlsxDecodeLimits, XlsxDecodeUsage, XlsxUsedExtent,
    XLSX_CELLS_OPERATION, XLSX_CELLS_VERSION,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

const NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/xlsx_cells_v1.json")).unwrap()
}

fn unhex(hex: &str) -> Vec<u8> {
    assert_eq!(hex.len() % 2, 0);
    hex.as_bytes().chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn packet(cell: &DecodedCell) -> Value {
    let mut value = match &cell.value {
        XlsxCellValue::IntegerLexeme { token } => json!({"kind":"IntegerLexeme", "token":token}),
        XlsxCellValue::Float64 { bits, token } => {
            json!({"kind":"Float64", "token":token, "bits":format!("{bits:016x}")})
        }
        XlsxCellValue::Text(text) => json!({"kind":"Text", "text":text}),
        XlsxCellValue::Bool(value) => json!({"kind":"Bool", "value":value}),
        XlsxCellValue::Empty => json!({"kind":"Empty"}),
        XlsxCellValue::Error(text) => json!({"kind":"Error", "text":text}),
        XlsxCellValue::Date { kind, serial_bits, token, epoch_1904, rendered } => json!({
            "kind":"Date", "date_kind":format!("{kind:?}"),
            "serial_bits":format!("{serial_bits:016x}"), "token":token,
            "epoch_1904":epoch_1904, "rendered":rendered
        }),
    };
    value["position"] = json!([cell.position.0, cell.position.1]);
    value["origin"] = json!(match cell.origin {
        XlsxCellOrigin::Literal => "Literal",
        XlsxCellOrigin::FormulaCache => "FormulaCache",
    });
    value
}

#[test]
fn retained_original_bytes_and_independent_packets() {
    let fixture = fixture();
    assert_eq!(fixture["operation"], XLSX_CELLS_OPERATION);
    assert_eq!(fixture["version"], XLSX_CELLS_VERSION);
    assert_eq!(fixture["join_cases"].as_array().unwrap().len(), 11);
    assert_eq!(fixture["numeric_cases"].as_array().unwrap().len(), 7);
    // Literal expectations are independent of the retained workbook XML and decoder.
    for (case, sheet_name) in fixture["join_cases"].as_array().unwrap().iter()
        .map(|case| (case, "S"))
        .chain(fixture["numeric_cases"].as_array().unwrap().iter()
            .map(|case| (case, "Sheet")))
    {
        let name = case["name"].as_str().unwrap();
        let bytes = unhex(case["archive_hex"].as_str().unwrap());
        assert_eq!(sha(&bytes), case["archive_sha256"], "{name}");
        // Preserve raw evidence, without treating the lossy BEFORE result as expected output.
        for channel in ["stdout", "stderr"] {
            assert_eq!(
                sha(case[format!("before_{channel}")].as_str().unwrap().as_bytes()),
                case[format!("before_{channel}_sha256")], "{name}"
            );
        }
        let mut archive = ZipArchive::new(Cursor::new(&bytes)).unwrap();
        for (path, original) in case["parts"].as_object().unwrap() {
            let mut raw = String::new();
            archive.by_name(path).unwrap().read_to_string(&mut raw).unwrap();
            assert_eq!(&raw, original.as_str().unwrap(), "{name}: {path}");
        }
        let decoded = decode_xlsx_cells(&bytes, name, XlsxDecodeLimits::unlimited());
        if let Some(error) = case["error_contains"].as_str() {
            let error_text = decoded.unwrap_err().to_string();
            assert!(error_text.contains(error), "{name}: {error_text}");
            continue;
        }
        let decoded = decoded.unwrap();
        assert_eq!(decoded.operation, "nirs4all.formats.xlsx-cells/v1");
        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.source_name, name);
        assert_eq!(decoded.source_sha256, case["archive_sha256"]);
        assert_eq!(decoded.sheet_names, [sheet_name], "{name}");
        assert_eq!(decoded.sheets[0].name, sheet_name, "{name}");
        let cells: Vec<_> = decoded.sheets[0].cells.iter().map(packet).collect();
        assert_eq!(json!(cells), case["cells"], "{name}");
        let extent = decoded.sheets[0].used_extent.unwrap();
        assert_eq!(json!([extent.height, extent.width]), case["used_extent"], "{name}");
        if name == "no-r-attributes" {
            assert!(decoded.sheets[0].cells.iter().all(|cell| cell.reference.is_none()));
            assert_eq!(decoded.sheets[0].cells[3].position, (1, 1));
        }
    }
}

/// These are new synthetic source literals, distinct from the retained archives.
fn worksheet(rows: &str) -> String {
    format!("<worksheet xmlns=\"{NS}\"><sheetData>{rows}</sheetData></worksheet>")
}

fn zip_parts(parts: &[(&str, &str)]) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, text) in parts {
        zip.start_file(*name, SimpleFileOptions::default().compression_method(CompressionMethod::Stored)).unwrap();
        zip.write_all(text.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn book_at(
    sheet: &str,
    base: &str,
    document: &str,
    target: &str,
    part: &str,
    workbook_properties: &str,
    extra: &[(&str, &str)],
) -> Vec<u8> {
    let package = format!("<Relationships><Relationship Id=\"doc\" Type=\"{REL}/officeDocument\" Target=\"{document}\"/></Relationships>");
    let workbook = format!("<workbook xmlns=\"{NS}\" xmlns:r=\"{REL}\">{workbook_properties}<sheets><sheet name=\"S\" sheetId=\"1\" r:id=\"s\"/></sheets></workbook>");
    let relationships = format!("<Relationships><Relationship Id=\"s\" Type=\"{REL}/worksheet\" Target=\"{target}\"/></Relationships>");
    let workbook_path = format!("{base}workbook.xml");
    let rels_path = format!("{base}_rels/workbook.xml.rels");
    let mut parts = vec![
        ("_rels/.rels", package.as_str()),
        (workbook_path.as_str(), workbook.as_str()),
        (rels_path.as_str(), relationships.as_str()),
        (part, sheet),
    ];
    parts.extend_from_slice(extra);
    zip_parts(&parts)
}

fn book(rows: &str, extra: &[(&str, &str)]) -> Vec<u8> {
    book_at(&worksheet(rows), "xl/", "xl/workbook.xml", "worksheets/s.xml", "xl/worksheets/s.xml", "", extra)
}

fn decode(bytes: &[u8]) -> DecodedWorkbook {
    decode_xlsx_cells(bytes, "synthetic.xlsx", XlsxDecodeLimits::unlimited()).unwrap()
}

#[test]
fn exact_tokens_kinds_and_signed_zero_survive_without_projection() {
    let bytes = book(concat!(
        "<row>",
        "<c><v>007</v></c><c><v>+007</v></c><c><v>+5</v></c>",
        "<c><v>123456789012345678901234567890</v></c>",
        "<c><v>12345678901234567890123456789012345678901234567890</v></c>",
        "<c t=\"inlineStr\"><is><t>123</t></is></c><c><v>18446744073709551615</v></c>",
        "<c><v>-0</v></c><c t=\"n\"><v>-0.0</v></c><c t=\"str\"><v>-0</v></c>",
        "<c><v>1E+30</v></c><c><v>1000000000000000000000000000000</v></c>",
        "<c><v>1000000000000000019884624838656</v></c>",
        "<c t=\"str\"><v> NA </v></c><c t=\"str\"><v>NaN</v></c>",
        "<c t=\"str\"><v> </v></c><c t=\"b\"><v>1</v></c>",
        "<c t=\"e\"><v>#N/A</v></c></row>"
    ), &[]);
    let decoded = decode(&bytes);
    let values: Vec<_> = decoded.sheets[0].cells.iter().map(|cell| cell.value.clone()).collect();
    let int = |token: &str| XlsxCellValue::IntegerLexeme { token: token.to_owned() };
    let text = |text: &str| XlsxCellValue::Text(text.to_owned());
    assert_eq!(values, vec![
        int("007"), int("+007"), int("+5"), int("123456789012345678901234567890"),
        int("12345678901234567890123456789012345678901234567890"),
        text("123"), int("18446744073709551615"), int("-0"),
        XlsxCellValue::Float64 { bits: 0x8000000000000000, token: "-0.0".to_owned() },
        text("-0"),
        XlsxCellValue::Float64 { bits: 0x46293e5939a08cea, token: "1E+30".to_owned() },
        int("1000000000000000000000000000000"), int("1000000000000000019884624838656"),
        text(" NA "), text("NaN"), text(" "), XlsxCellValue::Bool(true),
        XlsxCellValue::Error("#N/A".to_owned()),
    ]);
    assert_eq!(decoded.sheets[0].cells[8].cell_type.as_deref(), Some("n"));
}

#[test]
fn fragments_refuse_only_for_absent_or_numeric_type() {
    for kind in ["", " t=\"n\""] {
        for content in ["1&#50;", "&#50;", "<![CDATA[2]]>", "1<!--split-->2", "a&amp;b", "<!--empty-->"] {
            let bytes = book(&format!("<row><c{kind}><v>{content}</v></c></row>"), &[]);
            let error = decode_xlsx_cells(&bytes, "fragment.xlsx", XlsxDecodeLimits::unlimited()).unwrap_err();
            assert!(error.to_string().contains("ambiguous numeric XML content"), "{kind}: {content}: {error}");
        }
    }
    // Empty v is structurally empty, including an empty formula cache.
    let decoded = decode(&book("<row><c><v/></c><c><f>1+1</f><v/></c></row>", &[]));
    assert_eq!(decoded.sheets[0].used_extent, None);
    assert_eq!(decoded.sheets[0].cells.len(), 2);
    assert_eq!(decoded.sheets[0].cells[1].origin, XlsxCellOrigin::FormulaCache);

    let shared = "<sst uniqueCount=\"1\"><si><t>shared</t></si></sst>";
    let bytes = book(concat!(
        "<row><c t=\"str\"><v>a&amp;b</v></c>",
        "<c t=\"inlineStr\"><is><t>a&amp;b</t></is></c>",
        "<c t=\"d\"><v>2020-01-02T03:04:05</v></c>",
        "<c t=\"s\"><v>0<!--split--></v></c>",
        "<c t=\"b\"><v>1<!--split--></v></c>",
        "<c t=\"e\"><v>#N/A<!--split--></v></c></row>"
    ), &[("xl/sharedStrings.xml", shared)]);
    let decoded = decode(&bytes);
    assert_eq!(decoded.sheets[0].cells[0].value, XlsxCellValue::Text("a&b".to_owned()));
    assert_eq!(decoded.sheets[0].cells[1].value, XlsxCellValue::Text("a&b".to_owned()));
    assert_eq!(decoded.sheets[0].cells[2].value, XlsxCellValue::Text("2020-01-02T03:04:05".to_owned()));
    assert_eq!(decoded.sheets[0].cells[3].value, XlsxCellValue::Text("shared".to_owned()));
    assert_eq!(decoded.sheets[0].cells[4].value, XlsxCellValue::Bool(true));
    assert_eq!(decoded.sheets[0].cells[5].value, XlsxCellValue::Error("#N/A".to_owned()));
}

#[test]
fn duplicate_coordinates_refuse_for_every_kind_and_order() {
    let kinds = ["<v>7</v>", "<is><t>text</t></is>", ""];
    for first in kinds {
        for second in kinds {
            let bytes = book(&format!("<row><c r=\"B2\">{first}</c><c r=\"b02\">{second}</c></row>"), &[]);
            let error = decode_xlsx_cells(&bytes, "duplicate.xlsx", XlsxDecodeLimits::unlimited()).unwrap_err();
            assert!(error.to_string().contains("duplicate cell coordinate"), "{first}: {second}: {error}");
        }
    }
}

#[test]
fn occupancy_uses_nonempty_values_including_errors_and_infers_only_with_calamine() {
    let bytes = book(concat!(
        "<row r=\"3\"><c r=\"B3\"/><c><v>9</v></c><c t=\"e\"><v>#N/A</v></c></row>",
        "<row r=\"99\"><c r=\"Z99\" s=\"0\"/><c><f>1+1</f></c></row>"
    ), &[]);
    let decoded = decode(&bytes);
    let sheet = &decoded.sheets[0];
    assert_eq!(sheet.cells.iter().map(|cell| cell.position).collect::<Vec<_>>(), [(2,1), (2,2), (2,3), (98,25), (98,26)]);
    assert_eq!(sheet.used_extent, Some(XlsxUsedExtent { height:3, width:4 }));
    let all_blank = decode(&book("<row r=\"99\"><c r=\"Z99\"/></row>", &[]));
    assert_eq!(all_blank.sheets[0].used_extent, None);
}

#[test]
fn paths_follow_calamine_case_backslash_absolute_and_empty_base_rules() {
    let sheet = worksheet("<row><c><v>9007199254740993</v></c></row>");
    for (base, document, target, part) in [
        ("data/", "/data/not-the-workbook-name.xml", "/data/s.xml", "DATA/S.XML"),
        ("", "workbook.xml", "s.xml", "S.XML"),
        ("xl/", "xl/workbook.xml", "worksheets/s.xml", "XL\\WORKSHEETS\\S.XML"),
        // Lookup misses the normalized cache key and falls back to the exact raw path.
        ("xl/", "xl/workbook.xml", "worksheets\\s.xml", "xl/worksheets\\s.xml"),
    ] {
        let bytes = book_at(&sheet, base, document, target, part, "", &[]);
        let decoded = decode(&bytes);
        assert_eq!(decoded.sheets[0].cells[0].value, XlsxCellValue::IntegerLexeme { token:"9007199254740993".to_owned() });
    }
    // Normalizing a lookup backslash would incorrectly admit this workbook.
    let bytes = book_at(&sheet, "xl/", "xl/workbook.xml", "worksheets\\s.xml", "xl/worksheets/s.xml", "", &[]);
    assert!(decode_xlsx_cells(&bytes, "lookup.xlsx", XlsxDecodeLimits::unlimited()).is_err());
}

#[test]
fn date_kind_precedes_integer_spelling_and_preserves_rendered_none() {
    let styles = concat!(
        "<styleSheet><numFmts count=\"1\"><numFmt numFmtId=\"164\" formatCode=\"[hh]:mm:ss\"/></numFmts>",
        "<cellXfs count=\"3\"><xf numFmtId=\"0\"/><xf numFmtId=\"14\"/><xf numFmtId=\"164\"/></cellXfs></styleSheet>"
    );
    let sheet = worksheet("<row><c s=\"1\"><v>1</v></c><c s=\"1\"><v>1E+30</v></c><c s=\"2\"><v>1</v></c></row>");
    for (properties, epoch, rendered) in [
        ("", false, "1900-01-01 00:00:00"),
        ("<workbookPr date1904=\"1\"/>", true, "1904-01-02 00:00:00"),
    ] {
        let bytes = book_at(&sheet, "xl/", "xl/workbook.xml", "worksheets/s.xml", "xl/worksheets/s.xml", properties, &[("xl/styles.xml",styles)]);
        let decoded = decode(&bytes);
        assert_eq!(decoded.sheets[0].cells[0].value, XlsxCellValue::Date {
            kind:XlsxDateKind::DateTime, serial_bits:0x3ff0000000000000,
            token:"1".to_owned(), epoch_1904:epoch, rendered:Some(rendered.to_owned()),
        });
        assert_eq!(decoded.sheets[0].cells[1].value, XlsxCellValue::Date {
            kind:XlsxDateKind::DateTime, serial_bits:0x46293e5939a08cea,
            token:"1E+30".to_owned(), epoch_1904:epoch, rendered:None,
        });
        assert!(matches!(decoded.sheets[0].cells[2].value, XlsxCellValue::Date { kind:XlsxDateKind::TimeDelta, .. }));
    }
}

fn assert_limit(bytes: &[u8], limits: XlsxDecodeLimits, label: &'static str, maximum: u64) {
    assert_eq!(decode_xlsx_cells(bytes, "budget.xlsx", limits).unwrap_err(), XlsxDecodeError::Limit { label, maximum });
}

#[test]
fn archive_and_shared_copy_boundaries_charge_once() {
    let shared = "<sst uniqueCount=\"1\"><si><t>abcdefghij</t></si></sst>";
    let bytes = book("<row><c t=\"s\"><v>0</v></c><c t=\"s\"><v>0</v></c></row>", &[("xl/sharedStrings.xml",shared)]);
    let mut archive = ZipArchive::new(Cursor::new(&bytes)).unwrap();
    let archive_bytes: u64 = (0..archive.len()).map(|i| archive.by_index_raw(i).unwrap().size()).sum();
    let decoded = decode(&bytes);
    assert_eq!(decoded.usage, XlsxDecodeUsage { archive_decoded_bytes:archive_bytes, shared_copy_bytes:20 });
    let total = archive_bytes + 20;
    assert_eq!(decoded.usage.total_decoded_bytes(), Some(total));
    for limits in [
        XlsxDecodeLimits { max_decoded_file_bytes:total, decoded_allowance:u64::MAX },
        XlsxDecodeLimits { max_decoded_file_bytes:u64::MAX, decoded_allowance:total },
        XlsxDecodeLimits { max_decoded_file_bytes:total, decoded_allowance:total },
    ] {
        assert_eq!(decode_xlsx_cells(&bytes, "budget.xlsx", limits).unwrap().usage, decoded.usage);
    }
    assert_limit(&bytes, XlsxDecodeLimits { max_decoded_file_bytes:total-1, decoded_allowance:u64::MAX }, "decoded file bytes", total-1);
    assert_limit(&bytes, XlsxDecodeLimits { max_decoded_file_bytes:u64::MAX, decoded_allowance:total-1 }, "decoded file/total bytes", total-1);
    for limits in [
        XlsxDecodeLimits { max_decoded_file_bytes:archive_bytes-1, decoded_allowance:u64::MAX },
        XlsxDecodeLimits { max_decoded_file_bytes:u64::MAX, decoded_allowance:archive_bytes-1 },
    ] {
        assert_limit(&bytes, limits, "decoded file/total bytes", archive_bytes-1);
    }
    let no_shared = book("<row><c><v>1</v></c></row>", &[]);
    let archive_only = decode(&no_shared).usage.archive_decoded_bytes;
    assert_eq!(decode_xlsx_cells(&no_shared, "budget.xlsx", XlsxDecodeLimits { max_decoded_file_bytes:archive_only, decoded_allowance:archive_only }).unwrap().usage.shared_copy_bytes, 0);
    assert_eq!(XlsxDecodeUsage { archive_decoded_bytes:u64::MAX, shared_copy_bytes:1 }.total_decoded_bytes(), None);
}

#[test]
fn shared_string_reservation_and_understated_zip_refuse() {
    let bytes = book("<row><c t=\"s\"><v>0</v></c></row>", &[("xl/sharedStrings.xml", "<sst uniqueCount=\"999999999\"><si><t>x</t></si></sst>")]);
    let error = decode_xlsx_cells(&bytes, "reserve.xlsx", XlsxDecodeLimits::unlimited()).unwrap_err();
    assert!(error.to_string().contains("declares more strings than"), "{error}");

    // Corrupt the declared uncompressed size in both headers, leaving actual bytes.
    let mut bytes = book("<row><c><v>1</v></c></row>", &[]);
    let local = bytes.windows(4).position(|w| w == b"PK\x03\x04").unwrap();
    let central = bytes.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
    bytes[local+22..local+26].copy_from_slice(&1_u32.to_le_bytes());
    bytes[central+24..central+28].copy_from_slice(&1_u32.to_le_bytes());
    let error = decode_xlsx_cells(&bytes, "understated.xlsx", XlsxDecodeLimits::unlimited()).unwrap_err();
    assert!(error.to_string().contains("larger than its declared uncompressed size"), "{error}");
}

#[test]
fn shared_string_declared_reservation_boundary() {
    for (count, allowed) in [(100, true), (101, false)] {
        let prefix = format!("<sst uniqueCount=\"{count}\"><si><t>x</t></si>");
        let shared = format!("{prefix}{}</sst>", " ".repeat(100 - prefix.len() - 6));
        assert_eq!(shared.len(), 100);
        let bytes = book("<row><c t=\"s\"><v>0</v></c></row>", &[("xl/sharedStrings.xml", &shared)]);
        let decoded = decode_xlsx_cells(&bytes, "reservation.xlsx", XlsxDecodeLimits::unlimited());
        if allowed {
            assert_eq!(decoded.unwrap().sheets[0].cells[0].value, XlsxCellValue::Text("x".to_owned()));
        } else {
            assert!(decoded.unwrap_err().to_string().contains("declares more strings than"));
        }
    }
}

#[test]
fn long_integer_token_is_preserved_for_selected_sheet_field_accounting() {
    let token = format!("{}7", "0".repeat(99_999));
    let bytes = book(&format!("<row><c><v>{token}</v></c></row>"), &[]);
    let decoded = decode(&bytes);
    assert_eq!(decoded.sheets[0].cells[0].value, XlsxCellValue::IntegerLexeme { token });
    // C3: the selected-sheet max(8, token.len()) field check belongs to IO.
    assert!(decoded.usage.archive_decoded_bytes > 100_000);
}

#[test]
fn sheet_order_and_source_provenance_are_stable() {
    let package = format!("<Relationships><Relationship Type=\"{REL}/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>");
    let workbook = format!("<workbook xmlns:r=\"{REL}\"><sheets><sheet name=\"Z &amp; A\" r:id=\"z\"/><sheet name=\"A\" r:id=\"a\"/></sheets></workbook>");
    let relationships = format!("<Relationships><Relationship Id=\"z\" Type=\"{REL}/worksheet\" Target=\"z.xml\"/><Relationship Id=\"a\" Type=\"{REL}/worksheet\" Target=\"a.xml\"/></Relationships>");
    let bytes = zip_parts(&[
        ("_rels/.rels", &package), ("xl/workbook.xml", &workbook),
        ("xl/_rels/workbook.xml.rels", &relationships),
        ("xl/z.xml", &worksheet("<row><c><v>1</v></c></row>")),
        ("xl/a.xml", &worksheet("<row><c><v>2</v></c></row>")),
    ]);
    let decoded = decode_xlsx_cells(&bytes, "original-name.xlsm", XlsxDecodeLimits::unlimited()).unwrap();
    assert_eq!(decoded.source_name, "original-name.xlsm");
    assert_eq!(decoded.source_sha256, sha(&bytes));
    assert_eq!(decoded.sheet_names, ["Z & A", "A"]);
    assert_eq!(decoded.sheets.iter().map(|sheet| sheet.name.as_str()).collect::<Vec<_>>(), ["Z & A", "A"]);
}
