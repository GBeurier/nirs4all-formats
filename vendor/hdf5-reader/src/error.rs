/// Errors produced by the HDF5 reader.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid HDF5 magic bytes")]
    InvalidMagic,

    #[error("unsupported superblock version {0}")]
    UnsupportedSuperblockVersion(u8),

    #[error("unsupported object header version {0}")]
    UnsupportedObjectHeaderVersion(u8),

    #[error("unsupported B-tree version {0}")]
    UnsupportedBTreeVersion(u8),

    #[error("unsupported symbol table node version {0}")]
    UnsupportedSymbolTableNodeVersion(u8),

    #[error("unsupported local heap version {0}")]
    UnsupportedLocalHeapVersion(u8),

    #[error("unsupported global heap version {0}")]
    UnsupportedGlobalHeapVersion(u8),

    #[error("unsupported fractal heap version {0}")]
    UnsupportedFractalHeapVersion(u8),

    #[error("unsupported dataspace version {0}")]
    UnsupportedDataspaceVersion(u8),

    #[error("unsupported datatype class {0}")]
    UnsupportedDatatypeClass(u8),

    #[error("unsupported layout class {0}")]
    UnsupportedLayoutClass(u8),

    #[error("unsupported layout version {0}")]
    UnsupportedLayoutVersion(u8),

    #[error("unsupported filter pipeline version {0}")]
    UnsupportedFilterPipelineVersion(u8),

    #[error("unsupported fill value version {0}")]
    UnsupportedFillValueVersion(u8),

    #[error("unsupported link message version {0}")]
    UnsupportedLinkVersion(u8),

    #[error("unsupported link type {0}")]
    UnsupportedLinkType(u8),

    #[error("unsupported attribute message version {0}")]
    UnsupportedAttributeVersion(u8),

    #[error("unsupported B-tree v2 record type {0}")]
    UnsupportedBTreeV2RecordType(u8),

    #[error("unsupported chunk indexing type {0}")]
    UnsupportedChunkIndexType(u8),

    #[error("unsupported size of offsets: {0}")]
    UnsupportedOffsetSize(u8),

    #[error("unsupported size of lengths: {0}")]
    UnsupportedLengthSize(u8),

    #[error("invalid B-tree signature")]
    InvalidBTreeSignature,

    #[error("invalid symbol table node signature")]
    InvalidSymbolTableNodeSignature,

    #[error("invalid local heap signature")]
    InvalidLocalHeapSignature,

    #[error("invalid global heap signature")]
    InvalidGlobalHeapSignature,

    #[error("invalid fractal heap signature")]
    InvalidFractalHeapSignature,

    #[error("invalid object header signature")]
    InvalidObjectHeaderSignature,

    #[error("invalid B-tree v2 signature: {context}")]
    InvalidBTreeV2Signature { context: &'static str },

    #[error("invalid fixed array signature: {context}")]
    InvalidFixedArraySignature { context: &'static str },

    #[error("invalid extensible array signature: {context}")]
    InvalidExtensibleArraySignature { context: &'static str },

    #[error("checksum mismatch: expected {expected:#010x}, got {actual:#010x}")]
    ChecksumMismatch { expected: u32, actual: u32 },

    #[error("unexpected end of data at offset {offset} (need {needed} bytes, have {available})")]
    UnexpectedEof {
        offset: u64,
        needed: u64,
        available: u64,
    },

    #[error("offset {0:#x} is out of bounds")]
    OffsetOutOfBounds(u64),

    #[error("group not found: {0}")]
    GroupNotFound(String),

    #[error("dataset not found: {0}")]
    DatasetNotFound(String),

    #[error("attribute not found: {0}")]
    AttributeNotFound(String),

    #[error("type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },

    #[error("unsupported filter: {0}")]
    UnsupportedFilter(String),

    #[error("decompression error: {0}")]
    DecompressionError(String),

    #[error("filter pipeline error: {0}")]
    FilterError(String),

    #[error("invalid data: {0}")]
    InvalidData(String),

    #[error("slice out of bounds: dimension {dim}, index {index}, size {size}")]
    SliceOutOfBounds { dim: usize, index: u64, size: u64 },

    #[error("undefined address (0xFFFFFFFFFFFFFFFF)")]
    UndefinedAddress,

    #[error("{0}")]
    Other(String),

    #[error("{path}: {source}")]
    Context { path: String, source: Box<Error> },
}

impl Error {
    /// Wrap this error with an object path for additional context.
    pub fn with_context(self, path: impl Into<String>) -> Self {
        Error::Context {
            path: path.into(),
            source: Box::new(self),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub use hdf5_core::ByteOrder;

impl From<hdf5_core::Error> for Error {
    fn from(err: hdf5_core::Error) -> Self {
        match err {
            hdf5_core::Error::InvalidData(message) => Error::InvalidData(message),
        }
    }
}
