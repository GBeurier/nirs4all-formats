//! Integration tests for the CF conventions pipeline (unpack, mask, combined).
//!
//! All tests build CDF-1 files in memory using NcFile::from_bytes so no
//! external fixtures are required.

use netcdf_reader::{NcFile, NcFormat, NcSliceInfo, NcSliceInfoElem};

// ---------------------------------------------------------------------------
// Helper: build a minimal CDF-1 binary with one dimension, one NC_SHORT
// variable, and the given variable-level attributes.
//
// The variable always has dimension "x" of the given size and stores i16
// values in big-endian format.
// ---------------------------------------------------------------------------

/// Attribute specification for the builder.
struct AttrSpec {
    name: &'static str,
    /// NC type code for the attribute value.
    nc_type: u32,
    /// Raw big-endian bytes for the attribute value (already padded to 4-byte boundary).
    payload: Vec<u8>,
    /// Number of elements in the attribute.
    nelems: u32,
}

/// Build an NC_DOUBLE scalar attribute.
fn double_attr(name: &'static str, value: f64) -> AttrSpec {
    AttrSpec {
        name,
        nc_type: 6, // NC_DOUBLE
        nelems: 1,
        payload: value.to_be_bytes().to_vec(),
    }
}

/// Build an NC_SHORT scalar attribute.
fn short_attr(name: &'static str, value: i16) -> AttrSpec {
    let mut payload = value.to_be_bytes().to_vec();
    // NC_SHORT values are padded to 4-byte boundary
    payload.extend_from_slice(&[0, 0]);
    AttrSpec {
        name,
        nc_type: 3, // NC_SHORT
        nelems: 1,
        payload,
    }
}

/// Build a complete CDF-1 byte buffer containing one NC_SHORT variable with
/// the supplied data values and variable-level attributes.
fn build_cdf1_short_var(var_name: &str, values: &[i16], attrs: Vec<AttrSpec>) -> Vec<u8> {
    let dim_size = values.len() as u32;
    let mut buf = Vec::new();

    // --- Magic + numrecs ---
    buf.extend_from_slice(b"CDF\x01");
    buf.extend_from_slice(&0u32.to_be_bytes()); // numrecs = 0

    // --- dim_list: 1 dimension "x" ---
    buf.extend_from_slice(&0x0000_000Au32.to_be_bytes()); // NC_DIMENSION tag
    buf.extend_from_slice(&1u32.to_be_bytes()); // nelems = 1
                                                // name "x"
    buf.extend_from_slice(&1u32.to_be_bytes());
    buf.push(b'x');
    buf.extend_from_slice(&[0, 0, 0]); // pad to 4
                                       // dim size
    buf.extend_from_slice(&dim_size.to_be_bytes());

    // --- global att_list: ABSENT ---
    buf.extend_from_slice(&0u32.to_be_bytes());
    buf.extend_from_slice(&0u32.to_be_bytes());

    // --- var_list: 1 variable ---
    buf.extend_from_slice(&0x0000_000Bu32.to_be_bytes()); // NC_VARIABLE tag
    buf.extend_from_slice(&1u32.to_be_bytes()); // nelems = 1

    // var name (padded to 4-byte boundary)
    let name_bytes = var_name.as_bytes();
    buf.extend_from_slice(&(name_bytes.len() as u32).to_be_bytes());
    buf.extend_from_slice(name_bytes);
    let name_pad = (4 - (name_bytes.len() % 4)) % 4;
    buf.extend_from_slice(&vec![0u8; name_pad]);

    // ndims = 1, dimid = 0
    buf.extend_from_slice(&1u32.to_be_bytes());
    buf.extend_from_slice(&0u32.to_be_bytes());

    // variable att_list
    if attrs.is_empty() {
        buf.extend_from_slice(&0u32.to_be_bytes()); // ABSENT
        buf.extend_from_slice(&0u32.to_be_bytes());
    } else {
        buf.extend_from_slice(&0x0000_000Cu32.to_be_bytes()); // NC_ATTRIBUTE tag
        buf.extend_from_slice(&(attrs.len() as u32).to_be_bytes());
        for attr in &attrs {
            let aname = attr.name.as_bytes();
            buf.extend_from_slice(&(aname.len() as u32).to_be_bytes());
            buf.extend_from_slice(aname);
            let aname_pad = (4 - (aname.len() % 4)) % 4;
            buf.extend_from_slice(&vec![0u8; aname_pad]);
            buf.extend_from_slice(&attr.nc_type.to_be_bytes());
            buf.extend_from_slice(&attr.nelems.to_be_bytes());
            buf.extend_from_slice(&attr.payload);
        }
    }

    // nc_type = NC_SHORT = 3
    buf.extend_from_slice(&3u32.to_be_bytes());

    // vsize: number of bytes of data (including row padding to 4).
    // For non-record variables: total bytes rounded up to 4-byte multiple.
    let raw_bytes = dim_size as usize * 2; // i16 = 2 bytes each
    let vsize = raw_bytes.div_ceil(4) * 4;
    buf.extend_from_slice(&(vsize as u32).to_be_bytes());

    // begin (data offset): right after this 4-byte field
    let data_offset = buf.len() as u32 + 4;
    buf.extend_from_slice(&data_offset.to_be_bytes());

    // --- Variable data: i16 big-endian, padded to 4 bytes ---
    for &v in values {
        buf.extend_from_slice(&v.to_be_bytes());
    }
    // pad to 4-byte boundary
    let data_pad = (4 - (raw_bytes % 4)) % 4;
    buf.extend_from_slice(&vec![0u8; data_pad]);

    buf
}

/// Build a CDF-1 byte buffer containing one NC_FLOAT variable.
fn build_cdf1_float_var(var_name: &str, values: &[f32], attrs: Vec<AttrSpec>) -> Vec<u8> {
    let dim_size = values.len() as u32;
    let mut buf = Vec::new();

    // --- Magic + numrecs ---
    buf.extend_from_slice(b"CDF\x01");
    buf.extend_from_slice(&0u32.to_be_bytes()); // numrecs = 0

    // --- dim_list: 1 dimension "x" ---
    buf.extend_from_slice(&0x0000_000Au32.to_be_bytes()); // NC_DIMENSION tag
    buf.extend_from_slice(&1u32.to_be_bytes()); // nelems = 1
    buf.extend_from_slice(&1u32.to_be_bytes()); // name length = 1
    buf.push(b'x');
    buf.extend_from_slice(&[0, 0, 0]); // pad to 4
    buf.extend_from_slice(&dim_size.to_be_bytes());

    // --- global att_list: ABSENT ---
    buf.extend_from_slice(&0u32.to_be_bytes());
    buf.extend_from_slice(&0u32.to_be_bytes());

    // --- var_list: 1 variable ---
    buf.extend_from_slice(&0x0000_000Bu32.to_be_bytes()); // NC_VARIABLE tag
    buf.extend_from_slice(&1u32.to_be_bytes());

    let name_bytes = var_name.as_bytes();
    buf.extend_from_slice(&(name_bytes.len() as u32).to_be_bytes());
    buf.extend_from_slice(name_bytes);
    let name_pad = (4 - (name_bytes.len() % 4)) % 4;
    buf.extend_from_slice(&vec![0u8; name_pad]);

    buf.extend_from_slice(&1u32.to_be_bytes()); // ndims = 1
    buf.extend_from_slice(&0u32.to_be_bytes()); // dimid = 0

    // variable att_list
    if attrs.is_empty() {
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
    } else {
        buf.extend_from_slice(&0x0000_000Cu32.to_be_bytes()); // NC_ATTRIBUTE tag
        buf.extend_from_slice(&(attrs.len() as u32).to_be_bytes());
        for attr in &attrs {
            let aname = attr.name.as_bytes();
            buf.extend_from_slice(&(aname.len() as u32).to_be_bytes());
            buf.extend_from_slice(aname);
            let aname_pad = (4 - (aname.len() % 4)) % 4;
            buf.extend_from_slice(&vec![0u8; aname_pad]);
            buf.extend_from_slice(&attr.nc_type.to_be_bytes());
            buf.extend_from_slice(&attr.nelems.to_be_bytes());
            buf.extend_from_slice(&attr.payload);
        }
    }

    // nc_type = NC_FLOAT = 5
    buf.extend_from_slice(&5u32.to_be_bytes());

    let vsize = dim_size as usize * 4;
    buf.extend_from_slice(&(vsize as u32).to_be_bytes());

    let data_offset = buf.len() as u32 + 4;
    buf.extend_from_slice(&data_offset.to_be_bytes());

    for &v in values {
        buf.extend_from_slice(&v.to_be_bytes());
    }

    buf
}

// ===========================================================================
// Test 1: read_variable_as_f64 works on i16-stored data
// ===========================================================================

#[test]
fn read_i16_variable_as_f64() {
    let values: Vec<i16> = vec![100, 200, -300, 0, 32767];
    let data = build_cdf1_short_var("temp", &values, vec![]);

    let file = NcFile::from_bytes(&data).unwrap();
    assert_eq!(file.format(), NcFormat::Classic);

    let arr = file.read_variable_as_f64("temp").unwrap();
    assert_eq!(arr.shape(), &[5]);
    assert_eq!(arr[[0]], 100.0);
    assert_eq!(arr[[1]], 200.0);
    assert_eq!(arr[[2]], -300.0);
    assert_eq!(arr[[3]], 0.0);
    assert_eq!(arr[[4]], 32767.0);
}

// ===========================================================================
// Test 2: read_variable_unpacked with scale_factor and add_offset
// ===========================================================================

#[test]
fn read_variable_unpacked_scale_and_offset() {
    // Stored i16 values; actual = stored * 0.01 + 273.15
    let values: Vec<i16> = vec![0, 100, 1000, -500, 2000];
    let attrs = vec![
        double_attr("scale_factor", 0.01),
        double_attr("add_offset", 273.15),
    ];
    let data = build_cdf1_short_var("temperature", &values, attrs);

    let file = NcFile::from_bytes(&data).unwrap();
    let arr = file.read_variable_unpacked("temperature").unwrap();
    assert_eq!(arr.shape(), &[5]);

    // actual = stored * 0.01 + 273.15
    let expected = [
        0.0 * 0.01 + 273.15,    // 273.15
        100.0 * 0.01 + 273.15,  // 274.15
        1000.0 * 0.01 + 273.15, // 283.15
        -500.0 * 0.01 + 273.15, // 268.15
        2000.0 * 0.01 + 273.15, // 293.15
    ];
    for (i, &exp) in expected.iter().enumerate() {
        assert!(
            (arr[[i]] - exp).abs() < 1e-10,
            "index {}: got {}, expected {}",
            i,
            arr[[i]],
            exp
        );
    }
}

// ===========================================================================
// Test 3: read_variable_masked replaces _FillValue with NaN
// ===========================================================================

#[test]
fn read_variable_masked_fill_value() {
    // Use -9999 as the fill value for i16 data.
    let fill: i16 = -9999;
    let values: Vec<i16> = vec![10, -9999, 30, -9999, 50];
    let attrs = vec![short_attr("_FillValue", fill)];
    let data = build_cdf1_short_var("obs", &values, attrs);

    let file = NcFile::from_bytes(&data).unwrap();
    let arr = file.read_variable_masked("obs").unwrap();
    assert_eq!(arr.shape(), &[5]);

    assert_eq!(arr[[0]], 10.0);
    assert!(arr[[1]].is_nan(), "index 1 should be NaN (fill value)");
    assert_eq!(arr[[2]], 30.0);
    assert!(arr[[3]].is_nan(), "index 3 should be NaN (fill value)");
    assert_eq!(arr[[4]], 50.0);
}

// ===========================================================================
// Test 4: read_variable_unpacked_masked with packed i16 data + fill value
//
// CF order: mask first (in packed space), then unpack.
// ===========================================================================

#[test]
fn read_variable_unpacked_masked_combined() {
    let fill: i16 = -32768; // i16::MIN as fill sentinel
                            // stored values: 1000, fill, 2000, 3000, fill
    let values: Vec<i16> = vec![1000, -32768, 2000, 3000, -32768];
    let attrs = vec![
        short_attr("_FillValue", fill),
        double_attr("scale_factor", 0.1),
        double_attr("add_offset", 20.0),
    ];
    let data = build_cdf1_short_var("packed", &values, attrs);

    let file = NcFile::from_bytes(&data).unwrap();
    let arr = file.read_variable_unpacked_masked("packed").unwrap();
    assert_eq!(arr.shape(), &[5]);

    // actual = stored * 0.1 + 20.0 (for non-fill values)
    let expected_0 = 1000.0 * 0.1 + 20.0; // 120.0
    let expected_2 = 2000.0 * 0.1 + 20.0; // 220.0
    let expected_3 = 3000.0 * 0.1 + 20.0; // 320.0

    assert!(
        (arr[[0]] - expected_0).abs() < 1e-10,
        "index 0: got {}, expected {}",
        arr[[0]],
        expected_0
    );
    assert!(arr[[1]].is_nan(), "index 1 should be NaN (fill value)");
    assert!(
        (arr[[2]] - expected_2).abs() < 1e-10,
        "index 2: got {}, expected {}",
        arr[[2]],
        expected_2
    );
    assert!(
        (arr[[3]] - expected_3).abs() < 1e-10,
        "index 3: got {}, expected {}",
        arr[[3]],
        expected_3
    );
    assert!(arr[[4]].is_nan(), "index 4 should be NaN (fill value)");

    // Verify that NaN survives unpacking (NaN * scale + offset == NaN).
    assert!(arr[[1]].is_nan());
}

// ===========================================================================
// Test 5: read_variable_slice on classic data
// ===========================================================================

#[test]
fn read_variable_slice_classic() {
    let values: Vec<f32> = vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0];
    let data = build_cdf1_float_var("data", &values, vec![]);

    let file = NcFile::from_bytes(&data).unwrap();

    // Slice: elements [1..4) with step 1 => [20.0, 30.0, 40.0]
    let selection = NcSliceInfo {
        selections: vec![NcSliceInfoElem::Slice {
            start: 1,
            end: 4,
            step: 1,
        }],
    };
    let arr: ndarray::ArrayD<f32> = file.read_variable_slice("data", &selection).unwrap();
    assert_eq!(arr.shape(), &[3]);
    assert_eq!(arr[[0]], 20.0);
    assert_eq!(arr[[1]], 30.0);
    assert_eq!(arr[[2]], 40.0);

    // Slice with stride 2: elements [0, 2, 4] => [10.0, 30.0, 50.0]
    let selection_stride = NcSliceInfo {
        selections: vec![NcSliceInfoElem::Slice {
            start: 0,
            end: 6,
            step: 2,
        }],
    };
    let arr2: ndarray::ArrayD<f32> = file.read_variable_slice("data", &selection_stride).unwrap();
    assert_eq!(arr2.shape(), &[3]);
    assert_eq!(arr2[[0]], 10.0);
    assert_eq!(arr2[[1]], 30.0);
    assert_eq!(arr2[[2]], 50.0);

    // Single-index selection: Index(3) => scalar-like 1D collapsed to 0D
    let selection_idx = NcSliceInfo {
        selections: vec![NcSliceInfoElem::Index(3)],
    };
    let arr3: ndarray::ArrayD<f32> = file.read_variable_slice("data", &selection_idx).unwrap();
    assert_eq!(arr3.shape(), &[] as &[usize]);
    assert_eq!(arr3[[]], 40.0);
}

// ===========================================================================
// Additional: masked with missing_value attribute (not just _FillValue)
// ===========================================================================

#[test]
fn read_variable_masked_missing_value() {
    let missing: i16 = -1;
    let values: Vec<i16> = vec![5, -1, 15, 25, -1];
    let attrs = vec![short_attr("missing_value", missing)];
    let data = build_cdf1_short_var("sensor", &values, attrs);

    let file = NcFile::from_bytes(&data).unwrap();
    let arr = file.read_variable_masked("sensor").unwrap();
    assert_eq!(arr.shape(), &[5]);

    assert_eq!(arr[[0]], 5.0);
    assert!(arr[[1]].is_nan());
    assert_eq!(arr[[2]], 15.0);
    assert_eq!(arr[[3]], 25.0);
    assert!(arr[[4]].is_nan());
}

// ===========================================================================
// Additional: unpacking with scale_factor only (no add_offset)
// ===========================================================================

#[test]
fn read_variable_unpacked_scale_only() {
    let values: Vec<i16> = vec![10, 20, 30];
    let attrs = vec![double_attr("scale_factor", 0.5)];
    let data = build_cdf1_short_var("scaled", &values, attrs);

    let file = NcFile::from_bytes(&data).unwrap();
    let arr = file.read_variable_unpacked("scaled").unwrap();
    assert_eq!(arr.shape(), &[3]);

    // actual = stored * 0.5 + 0.0
    assert!((arr[[0]] - 5.0).abs() < 1e-10);
    assert!((arr[[1]] - 10.0).abs() < 1e-10);
    assert!((arr[[2]] - 15.0).abs() < 1e-10);
}

// ===========================================================================
// Additional: unpacking with add_offset only (no scale_factor)
// ===========================================================================

#[test]
fn read_variable_unpacked_offset_only() {
    let values: Vec<i16> = vec![0, 100, -50];
    let attrs = vec![double_attr("add_offset", 1000.0)];
    let data = build_cdf1_short_var("offset_var", &values, attrs);

    let file = NcFile::from_bytes(&data).unwrap();
    let arr = file.read_variable_unpacked("offset_var").unwrap();
    assert_eq!(arr.shape(), &[3]);

    // actual = stored * 1.0 + 1000.0
    assert!((arr[[0]] - 1000.0).abs() < 1e-10);
    assert!((arr[[1]] - 1100.0).abs() < 1e-10);
    assert!((arr[[2]] - 950.0).abs() < 1e-10);
}

// ===========================================================================
// Additional: no unpack/mask attributes => raw data returned unchanged
// ===========================================================================

#[test]
fn read_variable_unpacked_masked_no_attrs_returns_raw() {
    let values: Vec<i16> = vec![1, 2, 3, 4];
    let data = build_cdf1_short_var("plain", &values, vec![]);

    let file = NcFile::from_bytes(&data).unwrap();

    // unpacked with no attributes should return raw data as f64
    let arr = file.read_variable_unpacked("plain").unwrap();
    assert_eq!(arr[[0]], 1.0);
    assert_eq!(arr[[1]], 2.0);
    assert_eq!(arr[[2]], 3.0);
    assert_eq!(arr[[3]], 4.0);

    // masked with no attributes should return raw data as f64
    let arr2 = file.read_variable_masked("plain").unwrap();
    assert_eq!(arr2[[0]], 1.0);
    assert_eq!(arr2[[3]], 4.0);

    // combined with no attributes should return raw data as f64
    let arr3 = file.read_variable_unpacked_masked("plain").unwrap();
    assert_eq!(arr3[[0]], 1.0);
    assert_eq!(arr3[[3]], 4.0);
}
