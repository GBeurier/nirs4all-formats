use std::error::Error;
use std::path::{Path, PathBuf};

use nirs4all_io::{open_path, SpectralRecord};
use serde_json::{json, Value};

const CASES: &[(&str, &str)] = &[
    ("asd_legacy_float", "samples/asd/3L9257.000"),
    ("asd_v6_double", "samples/asd/v6sample00000.asd"),
    ("asd_v7_field_double", "samples/asd/v7_field_44231B009.asd"),
    ("asd_v8_double", "samples/asd/v8sample00001.asd"),
    ("csv_synthetic", "samples/csv_tsv/synthetic_nirs.csv"),
    ("bruker_dpt_synthetic", "samples/bruker_dpt/synthetic.dpt"),
    (
        "bruker_opus_absorbance_multi",
        "samples/bruker_opus/617262_1TP_C-1_A5.0",
    ),
    (
        "bruker_opus_reflectance",
        "samples/bruker_opus/test_spectra.0",
    ),
    (
        "nicolet_omnic_spa_baso4",
        "samples/nicolet_omnic/2-BaSO4_0.SPA",
    ),
    (
        "nicolet_omnic_spg_wodger",
        "samples/nicolet_omnic/wodger.spg",
    ),
    (
        "nicolet_omnic_spa_not_opus",
        "samples/nicolet_omnic/not_opus.spa",
    ),
    (
        "nicolet_omnic_srs_gc_demo",
        "samples/nicolet_omnic/GC_Demo.srs",
    ),
    ("nicolet_omnic_srs_tgair", "samples/nicolet_omnic/TGAIR.srs"),
    ("perkin_elmer_sp_spectra", "samples/perkin_elmer/spectra.sp"),
    (
        "buchi_nircal_foliar_transfer",
        "samples/buchi_nircal/muestras-tejido-foliar_transfer.nir",
    ),
    ("jasco_jws_243", "samples/jasco/243.jws"),
    (
        "jasco_jws_fluorescence",
        "samples/jasco/sample_fluorescence.jws",
    ),
    ("jasco_jws_cd_ht_abs", "samples/jasco/sample_CD_HT_Abs.jws"),
    (
        "horiba_jobinyvon_spec",
        "samples/raman_horiba/jobinyvon_test_spec.xml",
    ),
    (
        "horiba_jobinyvon_spec_cm1",
        "samples/raman_horiba/jobinyvon_test_spec_3s_cm-1.xml",
    ),
    (
        "horiba_jobinyvon_spec_ev",
        "samples/raman_horiba/jobinyvon_test_spec_3s_eV.xml",
    ),
    (
        "horiba_jobinyvon_spec_range",
        "samples/raman_horiba/jobinyvon_test_spec_range.xml",
    ),
    (
        "horiba_jobinyvon_linescan",
        "samples/raman_horiba/jobinyvon_test_linescan.xml",
    ),
    (
        "horiba_jobinyvon_map_x3_y2",
        "samples/raman_horiba/jobinyvon_test_map_x3-y2.xml",
    ),
    (
        "horiba_labspec_532nm_si",
        "samples/raman_horiba/labspec_532nm_Si.txt",
    ),
    (
        "horiba_labspec_activation",
        "samples/raman_horiba/labspec_Activation.txt",
    ),
    (
        "horiba_labspec_smc1_initial",
        "samples/raman_horiba/labspec_SMC1_Initial.txt",
    ),
    (
        "horiba_labspec_lasertest1",
        "samples/raman_horiba/labspec_lasertest1.txt",
    ),
    (
        "horiba_labspec_serie190214",
        "samples/raman_horiba/labspec_serie190214.txt",
    ),
    (
        "horiba_labspec_linbwo6_pol",
        "samples/raman_horiba/labspec_LiNbWO6_pol.txt",
    ),
    (
        "horiba_labspec_gd2o3_aln_map",
        "samples/raman_horiba/labspec6_Gd2O3_AlN_map.txt",
    ),
    (
        "renishaw_wdf_test_spectrum",
        "samples/raman_renishaw/renishaw_test_spectrum.wdf",
    ),
    (
        "renishaw_wdf_test_linescan",
        "samples/raman_renishaw/renishaw_test_linescan.wdf",
    ),
    (
        "renishaw_wdf_interrupted_acquisition",
        "samples/raman_renishaw/interrupted_acquisition.wdf",
    ),
    ("renishaw_wdf_wire_sp", "samples/raman_renishaw/wire_sp.wdf"),
    (
        "trivista_tvf_single",
        "samples/raman_trivista/spec_1s_1acc_1frame_average.tvf",
    ),
    (
        "trivista_tvf_two_frames",
        "samples/raman_trivista/spec_3s_1acc_2frames_average.tvf",
    ),
    (
        "trivista_tvf_two_acc_average",
        "samples/raman_trivista/spec_3s_2acc_1frame_average.tvf",
    ),
    (
        "trivista_tvf_two_acc_no_average",
        "samples/raman_trivista/spec_3s_2acc_1frame_no_average.tvf",
    ),
    (
        "trivista_tvf_multiple_spectrometers",
        "samples/raman_trivista/spec_multiple_spectrometers.tvf",
    ),
    (
        "trivista_tvf_step_and_glue",
        "samples/raman_trivista/spec_step_and_glue.tvf",
    ),
    (
        "trivista_tvf_timeseries",
        "samples/raman_trivista/spec_timeseries_2x1s_delta3s.tvf",
    ),
    (
        "trivista_tvf_linescan",
        "samples/raman_trivista/linescan.tvf",
    ),
    ("trivista_tvf_map", "samples/raman_trivista/map.tvf"),
    (
        "digitalsurf_spectrum",
        "samples/digitalsurf/test_spectrum.pro",
    ),
    (
        "digitalsurf_spectra",
        "samples/digitalsurf/test_spectra.pro",
    ),
    (
        "digitalsurf_spectral_map",
        "samples/digitalsurf/test_spectral_map.sur",
    ),
    (
        "digitalsurf_spectral_map_compressed",
        "samples/digitalsurf/test_spectral_map_compressed.sur",
    ),
    (
        "digitalsurf_surface",
        "samples/digitalsurf/test_surface.sur",
    ),
    ("hamamatsu_focus_mode", "samples/hamamatsu/focus_mode.img"),
    (
        "hamamatsu_operate_mode",
        "samples/hamamatsu/operate_mode.img",
    ),
    (
        "hamamatsu_photon_counting",
        "samples/hamamatsu/photon_counting.img",
    ),
    (
        "hamamatsu_shading_file",
        "samples/hamamatsu/shading_file.img",
    ),
    ("hamamatsu_xaxis_other", "samples/hamamatsu/xaxis_other.img"),
    ("avantes_wave_table", "samples/avantes/avantes_export.ttt"),
    ("avantes_irradiance", "samples/avantes/irr_820_1941.IRR"),
    ("avantes_legacy_trm", "samples/avantes/avantes2.TRM"),
    (
        "avantes_legacy_trm_alt",
        "samples/avantes/avantes_trans.TRM",
    ),
    (
        "avantes_legacy_scope",
        "samples/avantes/avantes_reflect.ROH",
    ),
    ("avantes_legacy_dark", "samples/avantes/1305084U1.DRK"),
    ("avantes_legacy_white", "samples/avantes/1305084U1.REF"),
    (
        "avantes_avasoft8_raw",
        "samples/avantes/1904090M1_0003.Raw8",
    ),
    ("avantes_avasoft8_irr8", "samples/avantes/eg.IRR8"),
    ("envi_sli_synthetic", "samples/envi_sli/synthetic_lib.hdr"),
    (
        "ocean_optics_spectrasuite",
        "samples/ocean_optics/OOusb4000.txt",
    ),
    (
        "ocean_optics_oceanview",
        "samples/ocean_optics/OceanView.txt",
    ),
    (
        "ocean_optics_craic",
        "samples/ocean_optics/CRAIC_export.txt",
    ),
    (
        "ocean_optics_master_transmission",
        "samples/ocean_optics/FMNH6834.00000001.Master.Transmission",
    ),
    ("ocean_optics_csv", "samples/ocean_optics/spec.csv"),
    ("ocean_optics_jaz", "samples/ocean_optics/jazspec.jaz"),
    (
        "ocean_optics_jaz_irradiance",
        "samples/ocean_optics/irrad.JazIrrad",
    ),
    (
        "ocean_optics_procspec_linux",
        "samples/ocean_optics/OceanOptics_Linux.ProcSpec",
    ),
    (
        "ocean_optics_procspec_windows",
        "samples/ocean_optics/OceanOptics_Windows.ProcSpec",
    ),
    (
        "ocean_optics_procspec_whiteref",
        "samples/ocean_optics/whiteref.ProcSpec",
    ),
    ("jcamp_nist_water", "samples/jcamp_dx/nist_water_ir.jdx"),
    ("jcamp_bruker_sqz", "samples/jcamp_dx/BRUKSQZ.DX"),
    ("jcamp_bruker_dif", "samples/jcamp_dx/BRUKDIF.DX"),
    ("jcamp_specfile_packed", "samples/jcamp_dx/SPECFILE.DX"),
    ("jcamp_bruker_ntuples", "samples/jcamp_dx/BRUKNTUP.DX"),
    ("jcamp_fid_ntuples", "samples/jcamp_dx/TESTFID.DX"),
    (
        "jcamp_ocean_optics_link",
        "samples/ocean_optics/OceanOptics_period.jdx",
    ),
    (
        "msa_iso22029_xy",
        "samples/msa_iso22029/ISO_22029_2022_compliance.msa",
    ),
    (
        "msa_iso22029_xy_ncolumns2",
        "samples/msa_iso22029/ISO_22029_2022_compliance_XY_NCOLUMNS2.msa",
    ),
    (
        "msa_iso22029_y_ncolumns5",
        "samples/msa_iso22029/example2_NCOLUMNS5.msa",
    ),
    (
        "msa_iso22029_minimum",
        "samples/msa_iso22029/minimum_metadata.msa",
    ),
    (
        "row_spectral_table_siware_neospectra",
        "samples/siware_neospectra/synthetic_neospectra.csv",
    ),
    (
        "row_spectral_table_modtran_albedo",
        "samples/modtran/synthetic_albedo.dat",
    ),
    (
        "row_spectral_table_pp_systems_spt",
        "samples/pp_systems/synthetic_unispec.SPT",
    ),
    (
        "row_spectral_table_pp_systems_spu",
        "samples/pp_systems/synthetic_unispec_dc.SPU",
    ),
    (
        "row_spectral_table_envi_ecostress",
        "samples/envi_sli/ecostress_b.spectrum.txt",
    ),
    (
        "row_spectral_table_shimadzu_uvprobe",
        "samples/shimadzu/synthetic_uvprobe.txt",
    ),
    (
        "row_spectral_table_usgs_specpr_ascii",
        "samples/specpr/asphalt_gds366.27407.asc",
    ),
    (
        "row_spectral_table_witec_ascii",
        "samples/raman_witec/Si-wafer-Raman-Spectrum-1.txt",
    ),
    (
        "row_spectral_table_jasco_ascii",
        "samples/jasco/synthetic_jws_export.txt",
    ),
    (
        "row_spectral_table_idl_envi_output",
        "samples/csv_tsv/idl_envi_output.txt",
    ),
    (
        "siware_api_json",
        "samples/siware_api/synthetic_siware_api.json",
    ),
    ("netcdf_synthetic_nirs", "samples/netcdf/synthetic_nirs.nc"),
    ("hdf5_synthetic_nirs", "samples/hdf5/synthetic_nirs.h5"),
    ("hdf5_synthetic_fgi", "samples/fgi/synthetic_fgi.h5"),
    (
        "numpy_npy_synthetic_nirs",
        "samples/numpy/synthetic_nirs_X.npy",
    ),
    (
        "numpy_npz_synthetic_nirs",
        "samples/numpy/synthetic_nirs.npz",
    ),
    (
        "parquet_synthetic_nirs",
        "samples/parquet/synthetic_nirs.parquet",
    ),
    (
        "matlab_synthetic_v5",
        "samples/matlab/synthetic_nirs_v5.mat",
    ),
    (
        "matlab_synthetic_v73",
        "samples/matlab/synthetic_nirs_v73.mat",
    ),
    (
        "matlab_eigenvector_corn",
        "samples/matlab/eigenvector_corn.mat",
    ),
    (
        "matlab_eigenvector_nir_shootout_2002",
        "samples/matlab/eigenvector_nir_shootout_2002.mat",
    ),
    ("matlab_spectrochempy_dso", "samples/matlab/scpdata_dso.mat"),
    (
        "matlab_spectrochempy_als2004",
        "samples/matlab/scpdata_als2004dataset.MAT",
    ),
    (
        "rdata_prospectr_nirsoil",
        "samples/matlab/prospectr_NIRsoil.RData",
    ),
    ("excel_synthetic_nirs", "samples/excel/synthetic_nirs.xlsx"),
    (
        "excel_multisheet_nirs",
        "samples/excel/synthetic_multisheet_nirs.xlsx",
    ),
    (
        "spectral_matrix_foss_winisi",
        "samples/foss_winisi/synthetic_winisi_export.txt",
    ),
    (
        "spectral_matrix_metrohm_visionair",
        "samples/metrohm/synthetic_visionair.csv",
    ),
    (
        "spectral_matrix_viavi_micronir",
        "samples/viavi_micronir/synthetic_micronir.csv",
    ),
    ("mfr_sun_photometer", "samples/mfr/synthetic_mfr.OUT"),
    (
        "microtops_sun_photometer",
        "samples/microtops/synthetic_microtops.TXT",
    ),
    ("animl_synthetic_nirs", "samples/animl/synthetic_nirs.animl"),
    (
        "allotrope_asm_absorbance_spectrum",
        "samples/allotrope_asm/ACSINS_absorbance_spectrum.json",
    ),
    (
        "allotrope_asm_emission_spectrum",
        "samples/allotrope_asm/spectrum_emission_data.json",
    ),
    (
        "allotrope_asm_endpoint_absorbance",
        "samples/allotrope_asm/MD_SMP_absorbance_example.json",
    ),
    ("galactic_spc_benzene", "samples/galactic_spc/BENZENE.SPC"),
    ("galactic_spc_s_xy", "samples/galactic_spc/s_xy.spc"),
    (
        "galactic_spc_ocean_optics",
        "samples/ocean_optics/OceanOptics.spc",
    ),
    ("galactic_spc_nir_multi", "samples/galactic_spc/nir.spc"),
    ("galactic_spc_m_xyxy", "samples/galactic_spc/m_xyxy.spc"),
    (
        "spectral_evolution_sed",
        "samples/spectral_evolution/1566060_09506_working.sed",
    ),
    ("svc_sig_moc", "samples/svc_ger/BNL13001_000_moc.sig"),
];

#[test]
fn reader_outputs_match_golden_summaries() -> Result<(), Box<dyn Error>> {
    for (name, relative_path) in CASES {
        let records = open_path(workspace_file(relative_path))?;
        let actual = serde_json::to_string_pretty(&summarize_records(&records))? + "\n";
        let golden_path = golden_file(name);

        if std::env::var("NIRS4ALL_IO_ACCEPT_GOLDENS").as_deref() == Ok("1") {
            std::fs::create_dir_all(golden_path.parent().expect("golden parent"))?;
            std::fs::write(&golden_path, actual)?;
            continue;
        }

        let expected = std::fs::read_to_string(&golden_path)?;
        assert_eq!(actual, expected, "golden mismatch for {name}");
    }
    Ok(())
}

fn summarize_records(records: &[SpectralRecord]) -> Value {
    json!({
        "record_count": records.len(),
        "records": records.iter().map(summarize_record).collect::<Vec<_>>(),
    })
}

fn summarize_record(record: &SpectralRecord) -> Value {
    let signals = record
        .signals
        .iter()
        .map(|(name, signal)| {
            json!({
                "name": name,
                "axis_kind": &signal.axis.kind,
                "axis_order": &signal.axis.order,
                "axis_unit": signal.axis.unit,
                "axis_len": signal.axis.values.len(),
                "axis_first": round6(signal.axis.values[0]),
                "axis_last": round6(signal.axis.values[signal.axis.values.len() - 1]),
                "signal_type": &signal.signal_type,
                "unit": signal.unit,
                "role": signal.role,
                "values_len": signal.values.len(),
                "value_first": round6(signal.values[0]),
                "value_last": round6(signal.values[signal.values.len() - 1]),
                "value_sum": round6(signal.values.iter().sum::<f64>()),
            })
        })
        .collect::<Vec<_>>();

    json!({
        "format": record.provenance.format,
        "reader": record.provenance.reader,
        "record_signal_type": &record.signal_type,
        "signal_count": record.signals.len(),
        "signals": signals,
        "target_keys": record.targets.keys().collect::<Vec<_>>(),
        "metadata_keys": record.metadata.keys().collect::<Vec<_>>(),
        "quality_flags": record.quality_flags,
        "warnings": record.provenance.warnings,
        "source_sha256": record.provenance.sources.first().map(|source| &source.sha256),
    })
}

fn round6(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

fn workspace_file(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn golden_file(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/goldens")
        .join(format!("{name}.summary.json"))
}
