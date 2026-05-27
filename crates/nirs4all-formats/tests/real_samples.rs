use nirs4all_formats::open_path;

#[test]
fn opens_additional_real_samples_outside_horiba_witec() {
    for (relative, expected_format, expected_records) in [
        ("samples/asd/soil.asd", "asd-fieldspec", 1),
        ("samples/asd/v7sample00000.asd", "asd-fieldspec", 1),
        ("samples/avantes/avantes_export2.trt", "avantes-ascii", 1),
        (
            "samples/avantes/avantes_export_long.ttt",
            "avantes-ascii",
            1,
        ),
        ("samples/galactic_spc/RAMAN.SPC", "galactic-spc", 1),
        ("samples/galactic_spc/raman-sion.spc", "galactic-spc", 36),
        ("samples/jcamp_dx/nist_ethanol_nist_ir.jdx", "jcamp-dx", 1),
        (
            "samples/nicolet_omnic/11-Z25-CP_0.SPA",
            "nicolet-omnic-spa",
            1,
        ),
        (
            "samples/spectral_evolution/1566060_15025_not_working.sed",
            "spectral-evolution-sed",
            1,
        ),
        ("samples/svc_ger/ACPL_D2_P1_B_1_001.sig", "svc-ger-sig", 1),
    ] {
        let records = open_path(workspace_file(relative)).unwrap_or_else(|err| {
            panic!("open {relative}: {err}");
        });

        assert_eq!(records.len(), expected_records, "{relative}");
        assert_eq!(records[0].provenance.format, expected_format, "{relative}");
        assert!(!records[0].signals.is_empty(), "{relative}");
        let signal = records[0].signals.values().next().expect("signal");
        assert!(!signal.axis.values.is_empty(), "{relative}");
        assert_eq!(signal.axis.values.len(), signal.values.len(), "{relative}");
    }
}

fn workspace_file(relative: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}
