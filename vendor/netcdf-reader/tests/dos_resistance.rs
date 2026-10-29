//! Malformed classic files must be rejected at `open`/read time without
//! attempting an oversized allocation. A process-wide `peak_alloc` allocator
//! lets each test assert that the peak heap stayed small while the parser
//! errored, proving the size guards fire *before* the allocation rather than
//! after.

use netcdf_reader::NcFile;
use peak_alloc::PeakAlloc;

#[global_allocator]
static PEAK: PeakAlloc = PeakAlloc;

/// Build the fixed-variable base of a tiny CDF-1 file and let the caller
/// append record data. Layout mirrors netcdf-c output.
fn cdf1_single_record_header(numrecs: u32) -> Vec<u8> {
    let mut d = Vec::new();
    d.extend_from_slice(b"CDF\x01");
    d.extend_from_slice(&numrecs.to_be_bytes());
    // dim_list: NC_DIMENSION, 1 dim "t" (unlimited, size 0).
    d.extend_from_slice(&10u32.to_be_bytes());
    d.extend_from_slice(&1u32.to_be_bytes());
    d.extend_from_slice(&1u32.to_be_bytes()); // name len
    d.extend_from_slice(b"t\0\0\0");
    d.extend_from_slice(&0u32.to_be_bytes()); // unlimited size marker
                                              // gatt_list: ABSENT.
    d.extend_from_slice(&0u32.to_be_bytes());
    d.extend_from_slice(&0u32.to_be_bytes());
    // var_list: NC_VARIABLE, 1 variable "v" i32 over dim 0.
    d.extend_from_slice(&11u32.to_be_bytes());
    d.extend_from_slice(&1u32.to_be_bytes());
    d.extend_from_slice(&1u32.to_be_bytes()); // name len
    d.extend_from_slice(b"v\0\0\0");
    d.extend_from_slice(&1u32.to_be_bytes()); // rank
    d.extend_from_slice(&0u32.to_be_bytes()); // dim id 0
    d.extend_from_slice(&0u32.to_be_bytes()); // vatt: ABSENT
    d.extend_from_slice(&0u32.to_be_bytes());
    d.extend_from_slice(&5u32.to_be_bytes()); // nc_type = NC_FLOAT? use NC_INT
                                              // fix: nc_type NC_INT = 4
    let type_pos = d.len() - 4;
    d[type_pos..type_pos + 4].copy_from_slice(&4u32.to_be_bytes());
    d.extend_from_slice(&4u32.to_be_bytes()); // vsize (padded, 1 elem * 4)
    let begin = (d.len() + 4) as u32;
    d.extend_from_slice(&begin.to_be_bytes()); // begin
    d
}

#[test]
fn classic_record_count_bomb_is_rejected_without_huge_allocation() {
    // A tiny header claiming ~1 billion records but only one record of data.
    let mut file = cdf1_single_record_header(1_000_000_000);
    file.extend_from_slice(&7i32.to_be_bytes());

    PEAK.reset_peak_usage();
    let before = PEAK.current_usage();
    let result = NcFile::from_bytes(&file);
    let peak = PEAK.peak_usage().saturating_sub(before);

    assert!(result.is_err(), "oversized record count should be rejected");
    assert!(
        peak < 4 * 1024 * 1024,
        "record-count guard allocated {peak} bytes; the fix should reject before allocating",
    );
}

/// A minimal CDF-5 file with one global `int64` attribute; the caller supplies
/// the declared value count so a bomb can be crafted.
fn cdf5_int64_attribute(nvalues: u64) -> Vec<u8> {
    let mut d = Vec::new();
    d.extend_from_slice(b"CDF\x05");
    d.extend_from_slice(&0u64.to_be_bytes()); // numrecs
    d.extend_from_slice(&0u32.to_be_bytes()); // dim_list ABSENT tag
    d.extend_from_slice(&0u64.to_be_bytes()); // dim_list count
    d.extend_from_slice(&12u32.to_be_bytes()); // gatt_list NC_ATTRIBUTE
    d.extend_from_slice(&1u64.to_be_bytes()); // 1 attribute
    d.extend_from_slice(&1u64.to_be_bytes()); // name len
    d.extend_from_slice(b"a\0\0\0");
    d.extend_from_slice(&10u32.to_be_bytes()); // nc_type NC_INT64
    d.extend_from_slice(&nvalues.to_be_bytes()); // declared value count
                                                 // Only one real value of payload follows.
    d.extend_from_slice(&1i64.to_be_bytes());
    d
}

#[test]
fn classic_attribute_count_bomb_is_rejected_without_huge_allocation() {
    // Declare 2^40 int64 values (~8 TB) in a ~60-byte file.
    let file = cdf5_int64_attribute(1u64 << 40);

    PEAK.reset_peak_usage();
    let before = PEAK.current_usage();
    let result = NcFile::from_bytes(&file);
    let peak = PEAK.peak_usage().saturating_sub(before);

    assert!(
        result.is_err(),
        "oversized attribute value count should be rejected"
    );
    assert!(
        peak < 4 * 1024 * 1024,
        "attribute guard allocated {peak} bytes; the fix should reject before allocating",
    );
}
