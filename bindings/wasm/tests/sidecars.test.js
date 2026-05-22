// Node.js test for the WASM `openWithSidecars` entry point.
//
// Run with: node bindings/wasm/tests/sidecars.test.js
//
// Requires the `pkg-node` directory built via
// `wasm-pack build --target nodejs --release --out-dir pkg-node`.

const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');

const wasm = require('../pkg-node/nirs4all_io_wasm.js');

function repoRoot() {
  return path.resolve(__dirname, '..', '..', '..');
}

function readSample(relative) {
  return fs.readFileSync(path.join(repoRoot(), relative));
}

function readBytes(absolute) {
  return new Uint8Array(fs.readFileSync(absolute));
}

// ENVI Standard cube — header + binary primary
{
  const primary = path.join(
    repoRoot(),
    'samples/envi_sli/cubescope-mini-cube.img'
  );
  if (!fs.existsSync(primary)) {
    console.warn('skip: ENVI Standard fixture missing');
  } else {
    const sidecars = {
      'cubescope-mini-cube.hdr': readBytes(
        path.join(path.dirname(primary), 'cubescope-mini-cube.hdr')
      ),
    };
    const records = wasm.openWithSidecars(
      'cubescope-mini-cube.img',
      readBytes(primary),
      sidecars
    );
    assert.ok(Array.isArray(records));
    assert.ok(
      records.length > 0,
      'ENVI Standard openWithSidecars produced no records'
    );
    console.log(
      'ENVI Standard records:',
      records.length,
      'first signal keys:',
      Object.keys(records[0].signals)
    );
  }
}

// ERDAS LAN — axis + ground-truth sidecars
{
  const primary = path.join(
    repoRoot(),
    'samples/hyperspectral_cubes/92AV3C.lan'
  );
  if (!fs.existsSync(primary)) {
    console.warn('skip: ERDAS LAN fixture missing');
  } else {
    const dir = path.dirname(primary);
    const sidecars = {
      '92AV3C.spc': readBytes(path.join(dir, '92AV3C.spc')),
      '92AV3GT.GIS': readBytes(path.join(dir, '92AV3GT.GIS')),
    };
    const records = wasm.openWithSidecars(
      '92AV3C.lan',
      readBytes(primary),
      sidecars
    );
    assert.ok(Array.isArray(records));
    assert.equal(records.length, 145 * 145, 'ERDAS LAN pixel count');
    const first = records[0];
    const signalKey = Object.keys(first.signals)[0];
    assert.equal(
      first.signals[signalKey].values.length,
      220,
      'ERDAS LAN bands per pixel'
    );
  }
}

// FGI HDF5+XML is excluded from the default WASM build because `fmt-hdf5`
// is off; assert openWithSidecars surfaces a useful error rather than
// silently producing nothing.
{
  const primary = path.join(repoRoot(), 'samples/fgi/synthetic_fgi.xml');
  if (!fs.existsSync(primary)) {
    console.warn('skip: FGI fixture missing');
  } else {
    const sidecars = {
      'synthetic_fgi.h5': readBytes(
        path.join(path.dirname(primary), 'synthetic_fgi.h5')
      ),
    };
    let raised = null;
    try {
      wasm.openWithSidecars(
        'synthetic_fgi.xml',
        readBytes(primary),
        sidecars
      );
    } catch (err) {
      raised = String(err);
    }
    assert.ok(
      raised !== null,
      'FGI HDF5+XML should error in the WASM build until fmt-hdf5 ships'
    );
    console.log('FGI WASM refusal:', raised.slice(0, 120));
  }
}

console.log('OK: WASM openWithSidecars tests passed');
