use nirs4all_io::open_path;

#[test]
fn refuses_committed_non_spectral_sidecars_reports_and_negative_fixtures() {
    for (relative, expected) in [
        (
            "samples/foss_winisi/synthetic_ds3_report.csv",
            "no numeric spectral headers found",
        ),
        (
            "samples/perten/synthetic_perten.csv",
            "no numeric spectral headers found",
        ),
        (
            "samples/netcdf/f03tst_open_mem.nc",
            "not a supported NIRS spectroscopy schema",
        ),
        (
            "samples/netcdf/air_temperature.nc",
            "NetCDF contains no spectra variable",
        ),
        (
            "samples/netcdf/pyrnet_to_l1a_output.nc",
            "no Microtops aot_* channel set",
        ),
        (
            "samples/microtops/microtops_arc_msm114_2_header.txt",
            "no numeric spectral headers found",
        ),
        (
            "samples/hdf5/vlen_string_dset.h5",
            "HDF5 contains no spectra dataset with matching wavelength axis",
        ),
        (
            "samples/animl/Example3.animl",
            "AnIML contains no supported axis series",
        ),
        (
            "samples/hyperspectral_cubes/92AV3C.spc",
            "unsupported format",
        ),
        (
            "samples/hyperspectral_cubes/92AV3GT.GIS",
            "unsupported format",
        ),
        (
            "samples/siware_neospectra/neospectra_ossl_column_names.csv",
            "no numeric spectral headers found",
        ),
        (
            "samples/csv_tsv/auroranir_handheld_barley_sensAIfood_metadata.xlsx",
            "Excel worksheet contains no numeric spectral headers",
        ),
        (
            "samples/foss_winisi/foss_xds_wheat2_sensAIfood_metadata.xlsx",
            "Excel worksheet contains no numeric spectral headers",
        ),
    ] {
        assert_refusal(relative, expected);
    }
}

#[test]
fn refuses_local_non_spectral_sidecars_and_derived_products_when_present() {
    for (relative, expected) in [
        (
            "samples_local/netcdf/arm_mar_aosmet_20180201.nc",
            "not a supported NIRS spectroscopy schema",
        ),
        (
            "samples_local/hyperspectral_cubes/indian_pines_gt.mat",
            "contains no supported structured NIRS dataset",
        ),
        (
            "samples_local/pp_systems/arc_lter_unispec_dc_2007_2019_indices.csv",
            "derived vegetation-index product",
        ),
        (
            "samples_local/pp_systems/arc_lter_unispec_dc_2007_2019_indices.xlsx",
            "raw .SPT/.SPU files or the referenced reflectance data scan table",
        ),
        (
            "samples_local/microtops/noaa_lauder_sonde_la20170315.lev2",
            "unsupported format",
        ),
    ] {
        let path = workspace_file(relative);
        if path.exists() {
            assert_refusal(relative, expected);
        }
    }
}

fn assert_refusal(relative: &str, expected: &str) {
    let err = open_path(workspace_file(relative)).expect_err("fixture should be refused");
    let message = err.to_string();
    assert!(
        message.contains(expected),
        "{relative}: expected {expected:?} in {message:?}"
    );
}

fn workspace_file(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}
