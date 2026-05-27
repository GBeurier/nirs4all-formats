use std::error::Error;
use std::path::{Path, PathBuf};

use nirs4all_formats::{open_path, SpectralRecord};
use serde_json::{json, Value};

const CASES: &[(&str, &str)] = &[
    ("asd_legacy_float", "samples/asd/3L9257.000"),
    ("asd_v6_double", "samples/asd/v6sample00000.asd"),
    ("asd_v7_field_double", "samples/asd/v7_field_44231B009.asd"),
    ("asd_v7_sample_double", "samples/asd/v7sample00000.asd"),
    ("asd_v8_double", "samples/asd/v8sample00001.asd"),
    ("asd_soil_real", "samples/asd/soil.asd"),
    ("csv_synthetic", "samples/csv_tsv/synthetic_nirs.csv"),
    ("csv_synthetic_tsv", "samples/csv_tsv/synthetic_nirs.tsv"),
    (
        "csv_synthetic_semicolon",
        "samples/csv_tsv/synthetic_nirs_semicolon.csv",
    ),
    (
        "csv_auroranir_handheld_sensaifood",
        "samples/csv_tsv/auroranir_handheld_barley_sensAIfood.csv",
    ),
    ("scio_app_scan", "samples/scio/scio_app_scan.csv"),
    (
        "scio_calibration_plate_polypen",
        "samples/scio/scio_calibration_plate_Polypen.csv",
    ),
    (
        "scio_tech_support",
        "samples/scio/scio_scans_from_tech_support.csv",
    ),
    ("bruker_dpt_synthetic", "samples/bruker_dpt/synthetic.dpt"),
    ("bruker_dpt_rs1_lightr", "samples/bruker_dpt/RS-1.dpt"),
    (
        "bruker_opus_absorbance_multi",
        "samples/bruker_opus/617262_1TP_C-1_A5.0",
    ),
    (
        "bruker_opus_reflectance",
        "samples/bruker_opus/test_spectra.0",
    ),
    (
        "bruker_opus_duplicate_absorbance",
        "samples/bruker_opus/BF_lo_01_soil_cal.1",
    ),
    (
        "bruker_opus_mmp_2107_test1",
        "samples/bruker_opus/MMP_2107_Test1.001",
    ),
    (
        "bruker_opus_brukeropus_file",
        "samples/bruker_opus/brukeropus_file.0",
    ),
    (
        "bruker_opus_afsis_icr_087266_b2",
        "samples/bruker_opus/icr_087266_B2.0",
    ),
    (
        "bruker_opus_afsis_icr_087273_g3",
        "samples/bruker_opus/icr_087273_G3.0",
    ),
    (
        "bruker_opus_issue82",
        "samples/bruker_opus/issue82_Opus_test.0",
    ),
    (
        "bruker_opus_pierreroudier_test_spectra",
        "samples/bruker_opus/opusreader_test_spectra.0",
    ),
    (
        "bruker_opus_scpdata_background",
        "samples/bruker_opus/scpdata_background.0",
    ),
    (
        "bruker_opus_scpdata_test",
        "samples/bruker_opus/scpdata_test.0000",
    ),
    (
        "nicolet_omnic_spa_baso4",
        "samples/nicolet_omnic/2-BaSO4_0.SPA",
    ),
    (
        "nicolet_omnic_spa_11_z25_cp",
        "samples/nicolet_omnic/11-Z25-CP_0.SPA",
    ),
    (
        "nicolet_omnic_spg_wodger",
        "samples/nicolet_omnic/wodger.spg",
    ),
    (
        "nicolet_omnic_spg_co_mo_al2o3",
        "samples/nicolet_omnic/CO_at_Mo_Al2O3.SPG",
    ),
    (
        "nicolet_omnic_spg_nh4y_activation",
        "samples/nicolet_omnic/nh4y-activation.spg",
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
        "horiba_labspec6_binary_gd2o3_aln_map",
        "samples/raman_horiba/AlN_Gd2O3_indepth.l6m",
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
        "renishaw_wdf_test_map2",
        "samples/raman_renishaw/renishaw_test_map2.wdf",
    ),
    (
        "renishaw_wdf_interrupted_acquisition",
        "samples/raman_renishaw/interrupted_acquisition.wdf",
    ),
    ("renishaw_wdf_wire_sp", "samples/raman_renishaw/wire_sp.wdf"),
    (
        "renishaw_wdf_wire_depth",
        "samples/raman_renishaw/wire_depth.wdf",
    ),
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
    (
        "avantes_wave_table_sample_counts",
        "samples/avantes/avantes_export2.trt",
    ),
    (
        "avantes_wave_table_long_multi",
        "samples/avantes/avantes_export_long.ttt",
    ),
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
    ("avantes_avasoft8_ascii", "samples/avantes/avasoft8.txt"),
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
    (
        "jcamp_nist_sucrose_multiblock",
        "samples/jcamp_dx/nist_sucrose_ir.jdx",
    ),
    ("jcamp_nist_acetone", "samples/jcamp_dx/acetone_ir.jdx"),
    (
        "jcamp_nist_carbon_dioxide",
        "samples/jcamp_dx/carbon_dioxide_ir.jdx",
    ),
    ("jcamp_nist_ethanol", "samples/jcamp_dx/ethanol_ir.jdx"),
    (
        "jcamp_nist_ethanol_webbook",
        "samples/jcamp_dx/nist_ethanol_nist_ir.jdx",
    ),
    (
        "jcamp_nist_glycerol",
        "samples/jcamp_dx/nist_glycerol_ir.jdx",
    ),
    ("jcamp_nist_methane", "samples/jcamp_dx/nist_methane_ir.jdx"),
    (
        "jcamp_nist_methanol",
        "samples/jcamp_dx/nist_methanol_ir.jdx",
    ),
    ("jcamp_bruker_affn", "samples/jcamp_dx/BRUKAFFN.DX"),
    ("jcamp_bruker_pac", "samples/jcamp_dx/BRUKPAC.DX"),
    ("jcamp_bruker_jcm", "samples/jcamp_dx/BRUKER1.JCM"),
    ("jcamp_bruker_sqz", "samples/jcamp_dx/BRUKSQZ.DX"),
    ("jcamp_bruker_dif", "samples/jcamp_dx/BRUKDIF.DX"),
    ("jcamp_labcalc", "samples/jcamp_dx/LABCALC.DX"),
    ("jcamp_pe1800", "samples/jcamp_dx/PE1800.DX"),
    ("jcamp_specfile_packed", "samples/jcamp_dx/SPECFILE.DX"),
    ("jcamp_test32", "samples/jcamp_dx/TEST32.DX"),
    ("jcamp_testspec", "samples/jcamp_dx/TESTSPEC.DX"),
    ("jcamp_bruker_ntuples", "samples/jcamp_dx/BRUKNTUP.DX"),
    ("jcamp_fid_ntuples", "samples/jcamp_dx/TESTFID.DX"),
    (
        "jcamp_ocean_optics_link",
        "samples/ocean_optics/OceanOptics_period.jdx",
    ),
    (
        "jcamp_synthetic_peak_assignments",
        "samples/jcamp_dx/synthetic_peak_assignments.jdx",
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
        "msa_iso22029_scientific_notation",
        "samples/msa_iso22029/ISO_22029_2022_compliance_scientific_notation.msa",
    ),
    (
        "msa_iso22029_multiline_title",
        "samples/msa_iso22029/ISO_22029_2022_compliance_title_multiple_line.msa",
    ),
    ("msa_example1", "samples/msa_iso22029/example1.msa"),
    (
        "msa_example1_with_seconds",
        "samples/msa_iso22029/example1_with_seconds.msa",
    ),
    (
        "msa_example1_wrong_date",
        "samples/msa_iso22029/example1_wrong_date.msa",
    ),
    (
        "msa_example1_wrong_date_empty_field",
        "samples/msa_iso22029/example1_wrong_date_empty_field.msa",
    ),
    ("msa_example2", "samples/msa_iso22029/example2.msa"),
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
        "row_spectral_table_envi_ecostress_a",
        "samples/envi_sli/ecostress_a.spectrum.txt",
    ),
    (
        "row_spectral_table_aster_granite",
        "samples/envi_sli/aster_granite.spectrum.txt",
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
        "usgs_aref_single_column",
        "samples/envi_sli/usgs_liba_AREF.txt",
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
    (
        "row_spectral_table_siware_api_csv",
        "samples/siware_api/synthetic_siware_api.csv",
    ),
    ("netcdf_synthetic_nirs", "samples/netcdf/synthetic_nirs.nc"),
    ("hdf5_synthetic_nirs", "samples/hdf5/synthetic_nirs.h5"),
    ("hdf5_synthetic_fgi", "samples/fgi/synthetic_fgi.h5"),
    ("fgi_xml_sidecar", "samples/fgi/synthetic_fgi.xml"),
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
        "excel_synthetic_nirs_xlsm",
        "samples/excel/synthetic_nirs_macro_compatible.xlsm",
    ),
    (
        "excel_multisheet_nirs",
        "samples/excel/synthetic_multisheet_nirs.xlsx",
    ),
    (
        "excel_scio_forensic_p_avg",
        "samples/excel/scio_forensic_P_avg.xlsx",
    ),
    (
        "excel_nirone_forensic_t_avg",
        "samples/excel/nirone_forensic_T_avg.xlsx",
    ),
    (
        "excel_neospectra_forensic_k_avg",
        "samples/siware_neospectra/neospectra_forensic_K_avg.xlsx",
    ),
    (
        "excel_micronir_forensic_k_avg",
        "samples/viavi_micronir/micronir_forensic_K_avg.xlsx",
    ),
    (
        "excel_micronir_forensic_t_avg",
        "samples/viavi_micronir/micronir_forensic_T_avg.xlsx",
    ),
    (
        "csv_siware_neospectra_ossl_slice",
        "samples/siware_neospectra/neospectra_ossl_50samples_slice.csv",
    ),
    (
        "csv_felix_f750_mango_slice",
        "samples/felix_f750/mango_dmc_f750_slice.csv",
    ),
    (
        "spectral_matrix_foss_winisi",
        "samples/foss_winisi/synthetic_winisi_export.txt",
    ),
    (
        "csv_foss_xds_barleyground_sensaifood",
        "samples/foss_winisi/foss_xds_barleyground_sensAIfood.csv",
    ),
    (
        "csv_foss_xds_wheat2_sensaifood",
        "samples/foss_winisi/foss_xds_wheat2_sensAIfood.csv",
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
    (
        "microtops_man_netcdf_msm114",
        "samples/microtops/microtops_arc_msm114_2.nc",
    ),
    ("animl_synthetic_nirs", "samples/animl/synthetic_nirs.animl"),
    (
        "animl_synthetic_nirs_autoincrement",
        "samples/animl/synthetic_nirs_autoincrement.animl",
    ),
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
    (
        "galactic_spc_barbituates",
        "samples/galactic_spc/BARBITUATES.SPC",
    ),
    (
        "galactic_spc_drug_sample",
        "samples/galactic_spc/DRUG_SAMPLE.SPC",
    ),
    ("galactic_spc_hcl", "samples/galactic_spc/HCL.SPC"),
    (
        "galactic_spc_lc_diode_array",
        "samples/galactic_spc/LC_DIODE_ARRAY.SPC",
    ),
    ("galactic_spc_raman", "samples/galactic_spc/RAMAN.SPC"),
    (
        "galactic_spc_resolution_pro",
        "samples/galactic_spc/resolutionPro.spc",
    ),
    (
        "galactic_spc_cell01_c1",
        "samples/galactic_spc/cell01_c1.spc",
    ),
    (
        "galactic_spc_cell01_c2",
        "samples/galactic_spc/cell01_c2.spc",
    ),
    ("galactic_spc_s_xy", "samples/galactic_spc/s_xy.spc"),
    ("galactic_spc_s_evenx", "samples/galactic_spc/s_evenx.spc"),
    ("galactic_spc_m_evenz", "samples/galactic_spc/m_evenz.spc"),
    ("galactic_spc_m_ordz", "samples/galactic_spc/m_ordz.spc"),
    ("galactic_spc_ft_ir", "samples/galactic_spc/Ft-ir.spc"),
    ("galactic_spc_ruby18", "samples/galactic_spc/RUBY18.SPC"),
    (
        "galactic_spc_single_polymer_film",
        "samples/galactic_spc/SINGLE_POLYMER_FILM.SPC",
    ),
    (
        "galactic_spc_bad_baseline",
        "samples/galactic_spc/SPECTRUM_WITH_BAD_BASELINE.SPC",
    ),
    ("galactic_spc_toluene", "samples/galactic_spc/TOLUENE.SPC"),
    ("galactic_spc_mercury", "samples/galactic_spc/MERC.SPC"),
    ("galactic_spc_ndr0002", "samples/galactic_spc/NDR0002.SPC"),
    ("galactic_spc_nmr_fid", "samples/galactic_spc/NMR_FID.SPC"),
    (
        "galactic_spc_test_input",
        "samples/galactic_spc/test_input.spc",
    ),
    (
        "galactic_spc_raman_sion",
        "samples/galactic_spc/raman-sion.spc",
    ),
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
    (
        "spectral_evolution_sed_dn_only",
        "samples/spectral_evolution/1566060_15025_not_working.sed",
    ),
    (
        "spectral_evolution_serbin_grape_leaf",
        "samples/spectral_evolution/serbinsh_cvars_grape_leaf.sed",
    ),
    (
        "svc_sig_acer_panvi_declared_bad",
        "samples/svc_ger/3_6_PANVI_2_T_1_001_BAD.sig",
    ),
    (
        "svc_sig_declared_bad",
        "samples/svc_ger/ACPL_D2_P1_B_1_000_BAD.sig",
    ),
    (
        "svc_sig_acer_bottom_1",
        "samples/svc_ger/ACPL_D2_P1_B_1_001.sig",
    ),
    (
        "svc_sig_acer_bottom_2",
        "samples/svc_ger/ACPL_D2_P1_B_2_001.sig",
    ),
    (
        "svc_sig_acer_middle_1",
        "samples/svc_ger/ACPL_D2_P1_M_1_000.sig",
    ),
    (
        "svc_sig_acer_middle_2",
        "samples/svc_ger/ACPL_D2_P1_M_2_000.sig",
    ),
    (
        "svc_sig_acer_top_1",
        "samples/svc_ger/ACPL_D2_P1_T_1_000.sig",
    ),
    (
        "svc_sig_acer_top_1_white_reference",
        "samples/svc_ger/ACPL_D2_P1_T_1_WR_000.sig",
    ),
    (
        "svc_sig_acer_top_2",
        "samples/svc_ger/ACPL_D2_P1_T_2_000.sig",
    ),
    (
        "svc_sig_acer_f3_bottom",
        "samples/svc_ger/ACPL_F3_P2_B_1_000.sig",
    ),
    (
        "svc_sig_laptop_bnl13001",
        "samples/svc_ger/BNL13001_000_laptop.sig",
    ),
    ("svc_sig_moc", "samples/svc_ger/BNL13001_000_moc.sig"),
    (
        "svc_sig_laptop_bnl13002",
        "samples/svc_ger/BNL13002_000_laptop.sig",
    ),
    (
        "svc_sig_serbin_ger3700",
        "samples/svc_ger/serbinsh_gr070214_003.sig",
    ),
    (
        "svc_sig_serbin_beo_hr1024i",
        "samples/svc_ger/serbinsh_BEO_CakeEater_Pheno_026_resamp.sig",
    ),
];

#[test]
fn reader_outputs_match_golden_summaries() -> Result<(), Box<dyn Error>> {
    for (name, relative_path) in CASES {
        let records = open_path(workspace_file(relative_path))?;
        let actual = serde_json::to_string_pretty(&summarize_records(&records))? + "\n";
        let golden_path = golden_file(name);

        if std::env::var("NIRS4ALL_FORMATS_ACCEPT_GOLDENS").as_deref() == Ok("1") {
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
            let coords = signal
                .coords
                .iter()
                .map(|(dim, coord)| {
                    json!({
                        "dim": dim,
                        "kind": &coord.kind,
                        "order": &coord.order,
                        "unit": coord.unit,
                        "len": coord.values.len(),
                        "first": round6(coord.values[0]),
                        "last": round6(coord.values[coord.values.len() - 1]),
                    })
                })
                .collect::<Vec<_>>();
            json!({
                "name": name,
                "axis_kind": &signal.axis.kind,
                "axis_order": &signal.axis.order,
                "axis_unit": signal.axis.unit,
                "axis_len": signal.axis.values.len(),
                "axis_first": round6(signal.axis.values[0]),
                "axis_last": round6(signal.axis.values[signal.axis.values.len() - 1]),
                "shape": &signal.shape,
                "dims": &signal.dims,
                "coords": coords,
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
