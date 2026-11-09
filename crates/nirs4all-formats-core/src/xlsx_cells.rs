//! Physical OOXML cells, independent of tabular roles and host bindings.

/// Operation identifier for the Rust typed-cell contract.
pub const XLSX_CELLS_OPERATION: &str = "nirs4all.formats.xlsx-cells/v1";
/// Version of the Rust typed-cell contract (not a package or wire version).
pub const XLSX_CELLS_VERSION: u32 = 1;

/// Decode-time byte ceilings. IO retains selected-sheet shape/field/record limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XlsxDecodeLimits {
    pub max_decoded_file_bytes: u64,
    /// Remaining decoded allowance in the caller's accounting scope.
    pub decoded_allowance: u64,
}

impl XlsxDecodeLimits {
    /// Explicit opt-out for trusted bytes; arithmetic remains checked.
    pub const fn unlimited() -> Self {
        Self {
            max_decoded_file_bytes: u64::MAX,
            decoded_allowance: u64::MAX,
        }
    }
}

/// Charge the checked sum once. Re-reading worksheet XML is not a second charge.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct XlsxDecodeUsage {
    pub archive_decoded_bytes: u64,
    pub shared_copy_bytes: u64,
}

impl XlsxDecodeUsage {
    pub fn total_decoded_bytes(self) -> Option<u64> {
        self.archive_decoded_bytes
            .checked_add(self.shared_copy_bytes)
    }
}

/// Typed limits can be mapped to the caller's existing limit labels.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum XlsxDecodeError {
    #[error("load limit exceeded: {label} (maximum {maximum}); raise host limits explicitly for trusted larger inputs")]
    Limit { label: &'static str, maximum: u64 },
    #[error("{0}")]
    Decode(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XlsxCellOrigin {
    Literal,
    /// An `f` child was present. No formula is evaluated, including empty caches.
    FormulaCache,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XlsxDateKind {
    DateTime,
    TimeDelta,
}

/// Physical kind, before NA policy, role selection or numerical admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XlsxCellValue {
    /// Complete ASCII `[+-]?[0-9]+`, without an integer storage-domain limit.
    IntegerLexeme {
        token: String,
    },
    /// Bits decoded by calamine, with the original decimal/exponent spelling.
    Float64 {
        bits: u64,
        token: String,
    },
    Text(String),
    Bool(bool),
    Empty,
    Error(String),
    /// Date classification takes precedence over an integer-spelled token.
    Date {
        kind: XlsxDateKind,
        serial_bits: u64,
        token: String,
        epoch_1904: bool,
        rendered: Option<String>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCell {
    /// Absolute zero-based position returned by calamine, including inferred positions.
    pub position: (u32, u32),
    pub value: XlsxCellValue,
    pub origin: XlsxCellOrigin,
    /// Raw optional `r` and `t` attributes, without invented references/types.
    pub reference: Option<String>,
    pub cell_type: Option<String>,
}

/// Grid anchored at A1, ending at the last non-Empty row and column.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XlsxUsedExtent {
    pub height: u64,
    pub width: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedSheet {
    pub name: String,
    /// XML order, including Empty cells; all positions are unique.
    pub cells: Vec<DecodedCell>,
    pub used_extent: Option<XlsxUsedExtent>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedWorkbook {
    pub operation: String,
    pub version: u32,
    pub source_name: String,
    /// Lowercase hexadecimal SHA-256 of the original archive bytes.
    pub source_sha256: String,
    pub sheet_names: Vec<String>,
    pub sheets: Vec<DecodedSheet>,
    pub usage: XlsxDecodeUsage,
}
