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
  try {
    wasm.openBytes('synthetic.lan', bytes);
    assert.fail('ERDAS LAN should error without sidecar');
  } catch (err) {
    console.log('ERDAS LAN refusal:', String(err).slice(0, 80));
  }
}

console.log('OK: WASM smoke tests passed (probe + open_bytes)');
