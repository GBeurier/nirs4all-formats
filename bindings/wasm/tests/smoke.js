// Node.js smoke test for the WASM binding.
//
// Run with: node bindings/wasm/tests/smoke.js
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

function readSample(relative) {
  return fs.readFileSync(path.join(repoRoot(), relative));
}

console.log('version:', wasm.version());
console.log('features:', wasm.features());

// Compiled reader catalog - emitted by the Rust registry, not duplicated in JS.
{
  const catalog = wasm.readerCatalog();
  const readers = catalog.map((entry) => entry.reader);
  console.log('reader catalog:', readers.length, 'readers');
  assert.ok(readers.length >= 40, 'expected broad reader coverage in the default bundle');
  assert.ok(readers.includes('nirs4all_formats::readers::jcamp'));
  assert.ok(readers.includes('nirs4all_formats::readers::hdf5'));
  assert.ok(readers.includes('nirs4all_formats::readers::matlab'));
  assert.ok(readers.includes('nirs4all_formats::readers::parquet'));
}

// Delimited CSV - extension-likely sniff
{
  const filename = 'synthetic_nirs.csv';
  const bytes = readSample('samples/csv_tsv/synthetic_nirs.csv');
  const probes = wasm.probeBytes(filename, bytes);
  assert.ok(Array.isArray(probes));
  assert.ok(probes.length > 0, 'expected at least one candidate');
  const top = probes[0];
  console.log('csv probe top:', top);
  assert.equal(top.format, 'delimited-text');
}

// JCAMP-DX - definite signature
{
  const bytes = readSample('samples/jcamp_dx/TESTSPEC.DX');
  const probes = wasm.probeBytes('TESTSPEC.DX', bytes);
  console.log('jcamp probe top:', probes[0]);
  assert.equal(probes[0].format, 'jcamp-dx');
  assert.equal(probes[0].confidence, 'definite');
}

// ASD binary - definite signature
{
  const bytes = readSample('samples/asd/soil.asd');
  const probes = wasm.probeBytes('soil.asd', bytes);
  console.log('asd probe top:', probes[0]);
  assert.equal(probes[0].format, 'asd-fieldspec');
}

// Unknown PDF - no candidates
{
  const bytes = readSample('samples/galactic_spc/spc_format_spec.pdf');
  const probes = wasm.probeBytes('spec.pdf', bytes);
  console.log('pdf probes:', probes.length);
  assert.equal(probes.length, 0);
}

// openBytes: full decode in WASM for a CSV fixture
{
  const bytes = readSample('samples/csv_tsv/synthetic_nirs.csv');
  const records = wasm.openBytes('synthetic_nirs.csv', bytes);
  console.log('csv records:', records.length, 'first signal keys:', Object.keys(records[0].signals));
  assert.equal(records.length, 50);
  const signalKey = Object.keys(records[0].signals)[0];
  assert.ok(Array.isArray(records[0].signals[signalKey].values));
  assert.equal(records[0].signals[signalKey].values.length, 200);
}

// openBytes: JCAMP-DX decode
{
  const bytes = readSample('samples/jcamp_dx/TESTSPEC.DX');
  const records = wasm.openBytes('TESTSPEC.DX', bytes);
  console.log('jcamp records:', records.length);
  assert.ok(records.length >= 1);
  assert.equal(records[0].provenance.format, 'jcamp-dx');
}

// openBytes: ASD binary decode
{
  const bytes = readSample('samples/asd/soil.asd');
  const records = wasm.openBytes('soil.asd', bytes);
  console.log('asd records:', records.length);
  assert.equal(records.length, 1);
  assert.equal(records[0].provenance.format, 'asd-fieldspec');
}

// openBytes refuses ERDAS LAN (sidecar needed)
{
  const bytes = new Uint8Array(128);
  Buffer.from('HEAD74').copy(bytes, 0);
  const sidecars = wasm.sidecarRequirements('synthetic.lan', bytes);
  assert.ok(sidecars.some((s) => s.role === 'wavelength_sidecar' && s.required));
  try {
    wasm.openBytes('synthetic.lan', bytes);
    assert.fail('ERDAS LAN should error without sidecar');
  } catch (err) {
    console.log('ERDAS LAN refusal:', String(err).slice(0, 80));
  }
}

// openBytes refuses PP Systems derived vegetation-index products with the
// domain message, not the generic in-memory/filesystem fallback.
{
  const text = 'SCAN ID,YEAR,DATE,DOY,DegDay,TIME,SITE,EXPERIMENT,BLOCK,TREATMENT,REP,NDVI (MODIS),EVI (MODIS),EVI2 (MODIS),PRI (550 Reference),PRI (570 Ref),WBI,Chl Index,LAI\\n1,2007,2007-06-01,152,0,12:00,A,E,B,T,1,0.1,0.2,0.3,0.4,0.5,0.6,0.7,0.8\\n';
  const bytes = Buffer.from(text);
  const probes = wasm.probeBytes('arc_lter_unispec_dc_2007_2019_indices.csv', bytes);
  assert.equal(probes[0].format, 'pp-systems-unispec-derived-indices');
  try {
    wasm.openBytes('arc_lter_unispec_dc_2007_2019_indices.csv', bytes);
    assert.fail('PP Systems derived indices should be refused as non-spectral');
  } catch (err) {
    const message = String(err);
    console.log('PP Systems refusal:', message.slice(0, 100));
    assert.ok(message.includes('not wavelength-indexed spectra'));
    assert.ok(!message.includes('does not support in-memory reads'));
  }
}

console.log('OK: WASM smoke tests passed (probe + open_bytes)');
