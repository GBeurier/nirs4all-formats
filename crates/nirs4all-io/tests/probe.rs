use nirs4all_io::{probe_path, Confidence};

#[test]
fn probes_committed_jcamp_fixture() {
    let probes = probe_path(workspace_file("samples/jcamp_dx/TESTSPEC.DX")).expect("probe");
    assert!(probes
        .iter()
        .any(|probe| probe.format == "jcamp-dx" && probe.confidence == Confidence::Definite));
}

#[test]
fn probes_committed_csv_fixture() {
    let probes = probe_path(workspace_file("samples/csv_tsv/synthetic_nirs.csv")).expect("probe");
    assert!(probes
        .iter()
        .any(|probe| probe.format == "delimited-text" && probe.confidence == Confidence::Likely));
}

#[test]
fn procspec_zip_does_not_probe_as_galactic_spc() {
    let probes = probe_path(workspace_file(
        "samples/ocean_optics/OceanOptics_Linux.ProcSpec",
    ))
    .expect("probe");
    assert!(probes.iter().any(|probe| {
        probe.format == "ocean-optics-procspec" && probe.confidence == Confidence::Definite
    }));
    assert!(!probes.iter().any(|probe| probe.format == "galactic-spc"));
}

#[test]
fn probes_committed_msa_fixture() {
    let probes = probe_path(workspace_file(
        "samples/msa_iso22029/ISO_22029_2022_compliance.msa",
    ))
    .expect("probe");
    assert!(probes.iter().any(|probe| {
        probe.format == "emsa-mas-msa" && probe.confidence == Confidence::Definite
    }));
}

#[test]
fn probes_row_spectral_table_fixture() {
    let probes = probe_path(workspace_file(
        "samples/siware_neospectra/synthetic_neospectra.csv",
    ))
    .expect("probe");
    assert!(probes.iter().any(|probe| {
        probe.format == "row-spectral-table" && probe.confidence == Confidence::Likely
    }));
    assert!(!probes
        .iter()
        .any(|probe| probe.format == "ocean-optics-two-column-csv"));
}

#[test]
fn probes_matrix_and_sun_photometer_exports() {
    let probes = probe_path(workspace_file(
        "samples/foss_winisi/synthetic_winisi_export.txt",
    ))
    .expect("probe");
    assert!(probes.iter().any(|probe| {
        probe.format == "spectral-matrix" && probe.confidence == Confidence::Likely
    }));
    assert!(!probes
        .iter()
        .any(|probe| probe.format == "row-spectral-table"));

    let probes =
        probe_path(workspace_file("samples/microtops/synthetic_microtops.TXT")).expect("probe");
    assert_eq!(probes[0].format, "microtops-sun-photometer");
    assert_eq!(probes[0].confidence, Confidence::Definite);
}

#[test]
fn probes_animl_and_allotrope_asm_documents() {
    let probes = probe_path(workspace_file("samples/animl/synthetic_nirs.animl")).expect("probe");
    assert!(probes
        .iter()
        .any(|probe| probe.format == "animl" && probe.confidence == Confidence::Definite));

    let probes = probe_path(workspace_file(
        "samples/allotrope_asm/ACSINS_absorbance_spectrum.json",
    ))
    .expect("probe");
    assert!(probes.iter().any(|probe| {
        probe.format == "allotrope-asm-json" && probe.confidence == Confidence::Definite
    }));
}

#[test]
fn probes_siware_json_and_comment_header_text_exports() {
    let probes = probe_path(workspace_file(
        "samples/siware_api/synthetic_siware_api.json",
    ))
    .expect("probe");
    assert!(probes.iter().any(|probe| {
        probe.format == "siware-api-json" && probe.confidence == Confidence::Definite
    }));

    let probes = probe_path(workspace_file("samples/csv_tsv/idl_envi_output.txt")).expect("probe");
    assert!(probes.iter().any(|probe| {
        probe.format == "row-spectral-table" && probe.confidence == Confidence::Likely
    }));
}

#[test]
fn probes_netcdf_containers() {
    let probes = probe_path(workspace_file("samples/netcdf/synthetic_nirs.nc")).expect("probe");
    assert!(probes.iter().any(|probe| {
        probe.format == "netcdf-container" && probe.confidence == Confidence::Likely
    }));
}

#[test]
fn probes_hdf5_containers() {
    let probes = probe_path(workspace_file("samples/hdf5/synthetic_nirs.h5")).expect("probe");
    assert!(probes.iter().any(|probe| {
        probe.format == "hdf5-nirs-container" && probe.confidence == Confidence::Likely
    }));
}

#[test]
fn probes_matlab_containers() {
    let probes = probe_path(workspace_file("samples/matlab/synthetic_nirs_v5.mat")).expect("probe");
    assert!(probes
        .iter()
        .any(|probe| probe.format == "matlab-v5" && probe.confidence == Confidence::Definite));

    let probes =
        probe_path(workspace_file("samples/matlab/synthetic_nirs_v73.mat")).expect("probe");
    assert!(probes.iter().any(|probe| {
        probe.format == "matlab-v73-hdf5" && probe.confidence == Confidence::Likely
    }));
}

#[test]
fn probes_excel_workbook() {
    let probes = probe_path(workspace_file("samples/excel/synthetic_nirs.xlsx")).expect("probe");
    assert!(probes
        .iter()
        .any(|probe| probe.format == "excel-workbook" && probe.confidence == Confidence::Likely));
}

fn workspace_file(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}
