//! Verify that `open_bytes` and `read_bytes` produce the same records as the
//! filesystem-backed entry points for representative format families.

use std::path::{Path, PathBuf};

use nirs4all_io::{open_bytes, open_path};

fn workspace_file(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn assert_bytes_match(relative: &str) {
    let path = workspace_file(relative);
    let bytes = std::fs::read(&path).expect("read");
    let from_path = open_path(&path).expect("open_path");
    let from_bytes = open_bytes(&path, &bytes).expect("open_bytes");
    assert_eq!(
        from_bytes.len(),
        from_path.len(),
        "{relative}: record count mismatch"
    );
    for (a, b) in from_bytes.iter().zip(from_path.iter()) {
        assert_eq!(a.signal_type, b.signal_type, "{relative}: signal_type");
        assert_eq!(
            a.signals.keys().collect::<Vec<_>>(),
            b.signals.keys().collect::<Vec<_>>(),
            "{relative}: signal keys"
        );
        for (key, signal_bytes) in &a.signals {
            let signal_path = &b.signals[key];
            assert_eq!(
                signal_bytes.values, signal_path.values,
                "{relative}: signal '{key}' values"
            );
            assert_eq!(
                signal_bytes.axis.values, signal_path.axis.values,
                "{relative}: signal '{key}' axis"
            );
        }
        assert_eq!(a.metadata, b.metadata, "{relative}: metadata");
        assert_eq!(a.targets, b.targets, "{relative}: targets");
    }
}

#[test]
fn open_bytes_matches_open_path_for_text_formats() {
    for relative in [
        "samples/csv_tsv/synthetic_nirs.csv",
        "samples/jcamp_dx/TESTSPEC.DX",
        "samples/spectral_evolution/1566060_15025_not_working.sed",
        "samples/svc_ger/ACPL_D2_P1_B_1_001.sig",
        "samples/bruker_dpt/RS-1.dpt",
        "samples/avantes/avantes_export2.trt",
        "samples/scio/sample_scio_bands.csv",
        "samples/usgs_spectral_library/coal-bituminous-tx-cm12_aref.spectrum.txt",
    ] {
        if workspace_file(relative).exists() {
            assert_bytes_match(relative);
        }
    }
}

#[test]
fn open_bytes_matches_open_path_for_binary_formats() {
    for relative in [
        "samples/asd/soil.asd",
        "samples/asd/v7sample00000.asd",
        "samples/galactic_spc/RAMAN.SPC",
        "samples/bruker_opus/icr_087266_B2.0",
        "samples/nicolet_omnic/11-Z25-CP_0.SPA",
        "samples/perkin_elmer/synthetic_pesample.sp",
        "samples/avantes/1305084U1.REF",
    ] {
        if workspace_file(relative).exists() {
            assert_bytes_match(relative);
        }
    }
}

#[test]
fn open_bytes_matches_open_path_for_compound_documents() {
    let relative = "samples/jasco/synthetic_jasco_jws.jws";
    if workspace_file(relative).exists() {
        assert_bytes_match(relative);
    }
}

#[test]
fn open_bytes_matches_open_path_for_zip_archives() {
    let relative = "samples/ocean_optics/OceanOptics_Linux.ProcSpec";
    if workspace_file(relative).exists() {
        assert_bytes_match(relative);
    }
}

#[test]
fn open_bytes_matches_open_path_for_excel_workbooks() {
    let relative = "samples/excel/synthetic_lab_template.xlsx";
    if workspace_file(relative).exists() {
        assert_bytes_match(relative);
    }
}

#[cfg(feature = "fmt-parquet")]
#[test]
fn open_bytes_matches_open_path_for_parquet() {
    // ParquetReader gained a bytes-mode entry point in the M4 follow-up
    // (commit P1), so the committed NIRS parquet table now decodes from
    // RAM as well as from disk.
    let relative = "samples/parquet/synthetic_nirs.parquet";
    if workspace_file(relative).exists() {
        assert_bytes_match(relative);
    }
}

#[test]
fn open_bytes_refuses_envi_standard_cube_without_sidecar() {
    // ENVI Standard cubes need the `.hdr` companion; `open_bytes` uses the
    // NoSidecars resolver and therefore fails with UnsupportedSidecar.
    let path = workspace_file("samples/envi_sli/cubescope-mini-cube.img");
    if !path.exists() {
        return;
    }
    let bytes = std::fs::read(&path).expect("read");
    let err = open_bytes(&path, &bytes).expect_err("envi cube must require sidecar");
    let message = err.to_string();
    assert!(
        message.contains("sidecar") && message.contains("no sidecar resolver was supplied"),
        "unexpected error: {message}"
    );
}

#[test]
fn open_bytes_refuses_erdas_lan_without_sidecar() {
    let path = workspace_file("samples/hyperspectral_cubes/92AV3C.lan");
    if !path.exists() {
        return;
    }
    let bytes = std::fs::read(&path).expect("read");
    let err = open_bytes(&path, &bytes).expect_err("ERDAS LAN must require sidecar");
    let message = err.to_string();
    assert!(
        message.contains("sidecar") && message.contains("no sidecar resolver was supplied"),
        "unexpected error: {message}"
    );
}

#[cfg(feature = "fmt-hdf5")]
#[test]
fn open_bytes_refuses_fgi_xml_without_hdf5_sidecar() {
    // The FGI XML+HDF5 reader points at its HDF5 payload through
    // `<DataReference path="...">`. Without a resolver, the lookup goes
    // through NoSidecars and surfaces UnsupportedSidecar instead of a
    // generic "missing companion file" message.
    let path = workspace_file("samples/fgi/synthetic_fgi.xml");
    if !path.exists() {
        return;
    }
    let bytes = std::fs::read(&path).expect("read");
    let err = open_bytes(&path, &bytes).expect_err("FGI XML must require HDF5 sidecar");
    let message = err.to_string();
    assert!(
        message.contains("sidecar") && message.contains("no sidecar resolver was supplied"),
        "unexpected error: {message}"
    );
}
