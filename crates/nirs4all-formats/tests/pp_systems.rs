use nirs4all_formats::{open_path, probe_path};

#[test]
fn refuses_arc_lter_unispec_indices_as_derived_products_when_present() {
    for relative in [
        "samples_local/pp_systems/arc_lter_unispec_dc_2007_2019_indices.csv",
        "samples_local/pp_systems/arc_lter_unispec_dc_2007_2019_indices.xlsx",
    ] {
        let path = workspace_file(relative);
        if !path.exists() {
            continue;
        }

        let probes = probe_path(&path).expect("probe PP Systems derived product");
        assert!(
            probes
                .iter()
                .any(|probe| probe.format == "pp-systems-unispec-derived-indices"),
            "{relative}: expected PP Systems derived-indices probe in {probes:?}"
        );

        let err = open_path(&path).expect_err("derived indices should be refused");
        let message = err.to_string();
        assert!(
            message.contains("derived vegetation-index product"),
            "{relative}: {message}"
        );
        assert!(
            message.contains("not wavelength-indexed spectra"),
            "{relative}: {message}"
        );
        assert!(
            message.contains("raw .SPT/.SPU files"),
            "{relative}: {message}"
        );
    }
}

fn workspace_file(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}
