// Broad fixture matrix for the WASM binding.
//
// Run with: node bindings/wasm/tests/fixture_matrix.js
//
// Requires the `pkg-node` directory built via
// `wasm-pack build --target nodejs --release --out-dir pkg-node`.

const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');

const wasm = require('../pkg-node/nirs4all_formats_wasm.js');

function repoRoot() {
  return path.resolve(__dirname, '..', '..', '..');
}

function readBytes(relative) {
  return fs.readFileSync(path.join(repoRoot(), relative));
}

function basename(relative) {
  return path.basename(relative);
}

function sidecars(entries) {
  return Object.fromEntries(entries.map(([key, relative]) => [key, readBytes(relative)]));
}

const openCases = [
  ['csv_like', 'samples/csv_tsv/synthetic_nirs.csv', 'delimited-text'],
  ['jcamp', 'samples/jcamp_dx/TESTSPEC.DX', 'jcamp-dx'],
  ['asd', 'samples/asd/soil.asd', 'asd-fieldspec'],
  ['bruker_dpt', 'samples/bruker_dpt/synthetic.dpt', 'bruker-dpt'],
  ['bruker_opus', 'samples/bruker_opus/test_spectra.0', 'bruker-opus'],
  ['nicolet_omnic', 'samples/nicolet_omnic/2-BaSO4_0.SPA', 'nicolet-omnic-spa'],
  ['perkin_elmer', 'samples/perkin_elmer/spectra.sp', 'perkin-elmer-sp'],
  ['buchi_nircal', 'samples/buchi_nircal/muestras-tejido-foliar_transfer.nir', 'buchi-nircal'],
  ['jasco_jws', 'samples/jasco/243.jws', 'jasco-jws'],
  ['horiba_labspec_xml', 'samples/raman_horiba/jobinyvon_test_spec.xml', 'horiba-jobinyvon-xml'],
  ['horiba_labspec_txt', 'samples/raman_horiba/labspec_532nm_Si.txt', 'horiba-labspec-text'],
  ['renishaw_wdf', 'samples/raman_renishaw/renishaw_test_spectrum.wdf', 'renishaw-wdf'],
  ['trivista_tvf', 'samples/raman_trivista/spec_1s_1acc_1frame_average.tvf', 'trivista-tvf'],
  ['digitalsurf', 'samples/digitalsurf/test_spectrum.pro', 'digitalsurf-sur-pro'],
  ['hamamatsu_img', 'samples/hamamatsu/focus_mode.img', 'hamamatsu-img'],
  ['galactic_spc', 'samples/galactic_spc/nir.spc', 'galactic-spc'],
  ['foss_winisi', 'samples/foss_winisi/synthetic.cal', 'foss-winisi-cal'],
  ['viavi_micronir', 'samples/viavi_micronir/synthetic_micronir.sam', 'viavi-micronir-sam'],
  ['scio_csv', 'samples/scio/scio_app_scan.csv', 'scio-csv'],
  ['avantes_ascii', 'samples/avantes/avantes_export.ttt', 'avantes-ascii'],
  ['avantes_binary', 'samples/avantes/avantes2.TRM', 'avantes-legacy-binary'],
  ['ocean_optics', 'samples/ocean_optics/OOusb4000.txt', 'ocean-optics-text'],
  ['numpy_npy', 'samples/numpy/synthetic_nirs_X.npy', 'numpy-npy'],
  ['numpy_npz', 'samples/numpy/synthetic_nirs.npz', 'numpy-npz'],
  ['parquet_zstd', 'samples/parquet/synthetic_nirs.parquet', 'parquet-nirs-table'],
  ['parquet_uncompressed', 'samples/parquet/synthetic_nirs_uncompressed.parquet', 'parquet-nirs-table'],
  ['sed', 'samples/spectral_evolution/serbinsh_cvars_grape_leaf.sed', 'spectral-evolution-sed'],
  ['svc_sig', 'samples/svc_ger/BNL13001_000_moc.sig', 'svc-ger-sig'],
  ['siware_api_json', 'samples/siware_api/synthetic_siware_api.json', 'siware-api-json'],
  ['siware_api_csv', 'samples/siware_api/synthetic_siware_api.csv', 'row-spectral-table'],
  ['netcdf', 'samples/netcdf/synthetic_nirs.nc', 'netcdf-nirs'],
  ['hdf5', 'samples/hdf5/synthetic_nirs.h5', 'hdf5-nirs'],
  ['matlab_v5', 'samples/matlab/synthetic_nirs_v5.mat', 'matlab-mat-v5'],
  ['matlab_v73', 'samples/matlab/synthetic_nirs_v73.mat', 'matlab-mat-v73'],
  ['rdata', 'samples/matlab/prospectr_NIRsoil.RData', 'rdata-prospectr-nirsoil'],
  ['excel', 'samples/excel/synthetic_nirs.xlsx', 'excel-xlsx'],
  ['pp_systems_spt', 'samples/pp_systems/synthetic_unispec.SPT', 'pp-systems-unispec'],
  ['pp_systems_spu', 'samples/pp_systems/synthetic_unispec_dc.SPU', 'pp-systems-unispec'],
  ['usgs_aref', 'samples/envi_sli/usgs_liba_AREF.txt', 'usgs-aref-single-column'],
  ['spectral_matrix', 'samples/metrohm/synthetic_visionair.csv', 'spectral-matrix'],
  ['sun_photometer', 'samples/mfr/synthetic_mfr.OUT', 'mfr-sun-photometer'],
  ['animl', 'samples/animl/synthetic_nirs.animl', 'animl'],
  ['allotrope_asm', 'samples/allotrope_asm/MD_SMP_absorbance_example.json', 'allotrope-asm-json'],
  ['msa', 'samples/msa_iso22029/example1.msa', 'emsa-mas-msa'],
  ['witec_wip', 'samples/raman_witec/Sa4.wip', 'witec-wip'],
];

const sidecarCases = [
  [
    'envi_sli',
    'samples/envi_sli/synthetic_lib.sli',
    'envi-sli',
    [['synthetic_lib.hdr', 'samples/envi_sli/synthetic_lib.hdr']],
  ],
  [
    'envi_standard_cube',
    'samples/envi_sli/cubescope-mini-cube.img',
    'envi-standard-cube',
    [['cubescope-mini-cube.hdr', 'samples/envi_sli/cubescope-mini-cube.hdr']],
  ],
  [
    'erdas_lan',
    'samples/hyperspectral_cubes/92AV3C.lan',
    'erdas-lan-aviris',
    [['92AV3C.spc', 'samples/hyperspectral_cubes/92AV3C.spc']],
  ],
  [
    'fgi_xml',
    'samples/fgi/synthetic_fgi.xml',
    'fgi-hdf5-xml',
    [['synthetic_fgi.h5', 'samples/fgi/synthetic_fgi.h5']],
  ],
];

const optionalLocalOpenCases = [
  [
    'allotrope_adf_local',
    'samples_local/allotrope_adf/adfsee_example.adf',
    'allotrope-adf',
  ],
  [
    'perkin_elmer_csv_local',
    'samples_local/perkin_elmer/426475_1.csv',
    'perkin-elmer-csv',
  ],
].filter(([, relative]) => fs.existsSync(path.join(repoRoot(), relative)));

const refusalCases = [
  {
    label: 'mzml_non_nirs',
    filename: 'example.mzML',
    bytes: () => readBytes('samples/mzml/example.mzML'),
    expectedReader: 'nirs4all_formats::readers::mzml',
    expectedMessage: 'mass-spectrometry data, not NIRS spectroscopy',
  },
  {
    label: 'pp_systems_derived_indices',
    filename: 'arc_lter_unispec_dc_2007_2019_indices.csv',
    bytes: () => Buffer.from('SCAN ID,YEAR,DATE,DOY,DegDay,TIME,SITE,EXPERIMENT,BLOCK,TREATMENT,REP,NDVI (MODIS),EVI (MODIS),EVI2 (MODIS),PRI (550 Reference),PRI (570 Ref),WBI,Chl Index,LAI\n1,2007,2007-06-01,152,0,12:00,A,E,B,T,1,0.1,0.2,0.3,0.4,0.5,0.6,0.7,0.8\n'),
    expectedReader: 'nirs4all_formats::readers::pp_systems',
    expectedMessage: 'not wavelength-indexed spectra',
  },
];

let opened = 0;
const exercisedReaders = new Set();
for (const [label, relative, expectedFormat] of openCases) {
  const records = wasm.openBytes(basename(relative), readBytes(relative));
  assert.ok(Array.isArray(records), label);
  assert.ok(records.length > 0, `${label}: no records`);
  assert.equal(records[0].provenance.format, expectedFormat, label);
  assert.ok(records[0].provenance.reader, `${label}: missing provenance.reader`);
  exercisedReaders.add(records[0].provenance.reader);
  opened += 1;
}

for (const [label, relative, expectedFormat, sidecarEntries] of sidecarCases) {
  const records = wasm.openWithSidecars(
    basename(relative),
    readBytes(relative),
    sidecars(sidecarEntries)
  );
  assert.ok(Array.isArray(records), label);
  assert.ok(records.length > 0, `${label}: no records`);
  assert.equal(records[0].provenance.format, expectedFormat, label);
  assert.ok(records[0].provenance.reader, `${label}: missing provenance.reader`);
  exercisedReaders.add(records[0].provenance.reader);
  opened += 1;
}

for (const [label, relative, expectedFormat] of optionalLocalOpenCases) {
  const records = wasm.openBytes(basename(relative), readBytes(relative));
  assert.ok(Array.isArray(records), label);
  assert.ok(records.length > 0, `${label}: no records`);
  assert.equal(records[0].provenance.format, expectedFormat, label);
  assert.ok(records[0].provenance.reader, `${label}: missing provenance.reader`);
  exercisedReaders.add(records[0].provenance.reader);
  opened += 1;
}

for (const { label, filename, bytes, expectedReader, expectedMessage } of refusalCases) {
  const payload = bytes();
  const probes = wasm.probeBytes(filename, payload);
  assert.equal(probes[0]?.reader, expectedReader, `${label}: probe reader`);
  exercisedReaders.add(expectedReader);
  assert.throws(
    () => wasm.openBytes(filename, payload),
    (err) => String(err).includes(expectedMessage),
    label
  );
}

const allowedLocalOnlyReaders = new Set([
  // ADF public samples are not redistributable; see samples/allotrope_adf/README.md.
  'nirs4all_formats::readers::allotrope_adf',
]);
const missingReaders = wasm.readerCatalog()
  .map((entry) => entry.reader)
  .filter((reader) => !exercisedReaders.has(reader) && !allowedLocalOnlyReaders.has(reader));
assert.deepEqual(missingReaders, [], 'every committed WASM reader needs fixture or refusal coverage');

console.log(
  `OK: WASM fixture matrix opened ${opened} format fixtures, checked ${refusalCases.length} expected refusals, and covered ${exercisedReaders.size} reader(s)`
);
