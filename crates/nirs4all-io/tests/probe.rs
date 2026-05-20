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

    let probes = probe_path(workspace_file("samples/matlab/scpdata_als2004dataset.MAT"))
        .expect("probe uppercase mat");
    assert!(probes
        .iter()
        .any(|probe| probe.format == "matlab-v5" && probe.confidence == Confidence::Definite));

    let probes =
        probe_path(workspace_file("samples/matlab/prospectr_NIRsoil.RData")).expect("probe RData");
    assert!(probes
        .iter()
        .any(|probe| probe.format == "rdata-rdx3-xz" && probe.confidence == Confidence::Likely));
}

#[test]
fn probes_excel_workbook() {
    let probes = probe_path(workspace_file("samples/excel/synthetic_nirs.xlsx")).expect("probe");
    assert!(probes
        .iter()
        .any(|probe| probe.format == "excel-workbook" && probe.confidence == Confidence::Likely));
}

#[test]
fn probes_nicolet_omnic_files() {
    let probes =
        probe_path(workspace_file("samples/nicolet_omnic/2-BaSO4_0.SPA")).expect("probe spa");
    assert!(probes.iter().any(|probe| {
        probe.format == "nicolet-omnic" && probe.confidence == Confidence::Definite
    }));

    let probes = probe_path(workspace_file("samples/nicolet_omnic/TGAIR.srs")).expect("probe srs");
    assert!(probes.iter().any(|probe| {
        probe.format == "nicolet-omnic-srs" && probe.confidence == Confidence::Possible
    }));
}

#[test]
fn probes_perkin_elmer_sp_files() {
    let probes = probe_path(workspace_file("samples/perkin_elmer/spectra.sp")).expect("probe sp");
    assert!(probes.iter().any(|probe| {
        probe.format == "perkin-elmer-sp" && probe.confidence == Confidence::Definite
    }));
}

#[test]
fn probes_buchi_nircal_files() {
    let probes = probe_path(workspace_file(
        "samples/buchi_nircal/muestras-tejido-foliar_transfer.nir",
    ))
    .expect("probe nircal");
    assert!(probes
        .iter()
        .any(|probe| probe.format == "buchi-nircal" && probe.confidence == Confidence::Definite));
}

#[test]
fn probes_jasco_jws_files() {
    let probes = probe_path(workspace_file("samples/jasco/243.jws")).expect("probe jws");
    assert!(probes
        .iter()
        .any(|probe| probe.format == "jasco-jws" && probe.confidence == Confidence::Likely));
}

#[test]
fn probes_horiba_labspec_files() {
    let probes = probe_path(workspace_file(
        "samples/raman_horiba/jobinyvon_test_spec.xml",
    ))
    .expect("probe");
    assert!(probes.iter().any(|probe| {
        probe.format == "horiba-jobinyvon-xml" && probe.confidence == Confidence::Definite
    }));

    let probes =
        probe_path(workspace_file("samples/raman_horiba/labspec_532nm_Si.txt")).expect("probe");
    assert!(probes.iter().any(|probe| {
        probe.format == "horiba-labspec-text" && probe.confidence == Confidence::Definite
    }));
}

#[test]
fn probes_renishaw_wdf_files() {
    let probes = probe_path(workspace_file(
        "samples/raman_renishaw/renishaw_test_spectrum.wdf",
    ))
    .expect("probe wdf");
    assert!(probes
        .iter()
        .any(|probe| probe.format == "renishaw-wdf" && probe.confidence == Confidence::Definite));
}

#[test]
fn probes_trivista_tvf_files() {
    let probes = probe_path(workspace_file(
        "samples/raman_trivista/spec_1s_1acc_1frame_average.tvf",
    ))
    .expect("probe TriVista TVF");
    assert!(probes
        .iter()
        .any(|probe| probe.format == "trivista-tvf" && probe.confidence == Confidence::Definite));
}

#[test]
fn probes_digitalsurf_sur_pro_files() {
    let probes = probe_path(workspace_file("samples/digitalsurf/test_spectrum.pro"))
        .expect("probe DigitalSurf PRO");
    assert!(probes.iter().any(|probe| {
        probe.format == "digitalsurf-sur-pro" && probe.confidence == Confidence::Definite
    }));

    let probes = probe_path(workspace_file("samples/digitalsurf/test_spectral_map.sur"))
        .expect("probe DigitalSurf SUR");
    assert!(probes.iter().any(|probe| {
        probe.format == "digitalsurf-sur-pro" && probe.confidence == Confidence::Definite
    }));
}

#[test]
fn probes_hamamatsu_img_files() {
    let probes = probe_path(workspace_file("samples/hamamatsu/operate_mode.img"))
        .expect("probe Hamamatsu IMG");
    assert!(probes
        .iter()
        .any(|probe| probe.format == "hamamatsu-img" && probe.confidence == Confidence::Definite));
}

#[test]
fn probes_mzml_ms_files() {
    let probes = probe_path(workspace_file("samples/mzml/example.mzML")).expect("probe mzML");
    assert!(probes
        .iter()
        .any(|probe| probe.format == "mzml-ms" && probe.confidence == Confidence::Definite));
}

fn workspace_file(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}
