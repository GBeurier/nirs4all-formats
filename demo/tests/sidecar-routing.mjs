// Run with Node 20+: node demo/tests/sidecar-routing.mjs
// N4F_WASM_MODULE optionally selects a real nodejs WASM build. Otherwise the
// standard bindings/wasm/pkg-node build is used when present, or explicitly skipped.
// N4F_PAGE_URL additionally exercises the real file input with Playwright;
// PLAYWRIGHT_MODULE and CHROME_BIN override the browser dependencies.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import vm from "node:vm";
import test from "node:test";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const app = readFileSync(path.join(root, "demo/app.js"), "utf8");
// Exercise the actual routing functions without booting the unrelated UI.
const routing = app.slice(app.indexOf("const fileBaseName ="), app.indexOf("function hintFor("));
assert.ok(routing.includes("function decodeGroup("));
function pipeline(wasm) {
  const context = vm.createContext({ WASM: wasm, confRank: { definite: 3, likely: 2, possible: 1, no: 0 } });
  vm.runInContext(`${routing}\nglobalThis.api = { groupFiles, decodeGroup, sidecarKeyAliases };`, context);
  return context.api;
}
const plain = (value) => JSON.parse(JSON.stringify(value));
const buffer = (name) => ({ name, bytes: new Uint8Array([1, 2]), size: 2 });
const names = (groups) => plain(groups.map((group) => group.map((file) => file.name)));
function fakeWasm(requirements = () => []) {
  return {
    sidecarRequirements: requirements,
    probeBytes: () => [{ confidence: "likely" }],
    openBytes: () => [{ targets: {} }],
    openWithSidecars: () => [{ targets: { reference: 1 } }],
  };
}
const cnRequirements = (name) => name.toLowerCase().endsWith(".da")
  ? [{ path: "sample.CN", alternatives: ["sample.cn"], required: false }] : [];

test("optional companions reach the decoder before a successful standalone read", () => {
  const wasm = fakeWasm(cnRequirements);
  const calls = [];
  wasm.openBytes = () => { calls.push("bare"); return [{ targets: {} }]; };
  wasm.openWithSidecars = (name, bytes, sidecars) => {
    calls.push("paired");
    assert.equal(name, "batch/sample.da");
    assert.deepEqual(Object.keys(sidecars).sort(), ["batch/SAMPLE.cn", "SAMPLE.cn"].sort());
    assert.equal(sidecars["SAMPLE.cn"][0], 1);
    return [{ targets: { reference: 1 } }];
  };
  const api = pipeline(wasm);
  const groups = api.groupFiles([buffer("batch/SAMPLE.cn"), buffer("batch/sample.da")]);
  assert.deepEqual(names(groups), [["batch/sample.da", "batch/SAMPLE.cn"]]);
  assert.equal(api.decodeGroup(groups[0]).records[0].targets.reference, 1);
  assert.deepEqual(calls, ["paired"]);
});

test("one file still uses openBytes and passes errors through", () => {
  const wasm = fakeWasm(cnRequirements);
  wasm.openWithSidecars = () => { throw new Error("unexpected sidecar route"); };
  const api = pipeline(wasm);
  assert.deepEqual(plain(api.decodeGroup([buffer("sample.da")]).records), [{ targets: {} }]);
  wasm.openBytes = () => { throw new Error("bad primary"); };
  assert.throws(() => api.decodeGroup([buffer("sample.da")]), /bad primary/);
});

test("declared companions stay in the same directory and include alternatives", () => {
  const api = pipeline(fakeWasm((name) => name.endsWith(".main")
    ? [{ path: "missing.ref", alternatives: ["actual.REF"], required: false }] : []));
  assert.deepEqual(names(api.groupFiles([
    buffer("first/item.main"), buffer("second/actual.ref"), buffer("first/ACTUAL.ref"),
  ])), [["first/item.main", "first/ACTUAL.ref"], ["second/actual.ref"]]);
  const nested = pipeline(fakeWasm(() => [{ path: "elsewhere/actual.ref", alternatives: [] }]));
  assert.deepEqual(names(nested.groupFiles([buffer("item.main"), buffer("actual.ref")])), [["item.main"], ["actual.ref"]]);
});

test("all real case-colliding companions reach Rust and its error is not hidden", () => {
  const wasm = fakeWasm(cnRequirements);
  wasm.openBytes = () => { throw new Error("standalone must not run"); };
  wasm.openWithSidecars = (_name, _bytes, sidecars) => {
    assert.deepEqual(Object.keys(sidecars).sort(), ["sample.CN", "sample.cn"].sort());
    throw new Error("ambiguous companions");
  };
  const api = pipeline(wasm);
  const groups = api.groupFiles([buffer("sample.cn"), buffer("sample.da"), buffer("sample.CN")]);
  assert.equal(groups.length, 1);
  assert.equal(groups[0].length, 3);
  assert.throws(() => api.decodeGroup(groups[0]), /ambiguous companions/);
});

test("aliases preserve spelling instead of fabricating competing extensions", () => {
  const api = pipeline(fakeWasm());
  assert.deepEqual(plain(api.sidecarKeyAliases("batch/item.Cn", "batch/item.da")), ["batch/item.Cn", "item.Cn"]);
  assert.deepEqual(plain(api.sidecarKeyAliases("item.hdr", "item.img")), ["item.hdr"]);
});

test("a declared primary wins over a standalone-readable payload", () => {
  const wasm = fakeWasm((name) => name.endsWith(".xml") ? [{ path: "data.h5" }] : []);
  wasm.probeBytes = (name) => [{ confidence: name.endsWith(".h5") ? "definite" : "likely" }];
  wasm.openWithSidecars = (name) => {
    assert.equal(name, "metadata.xml");
    return [{ targets: {} }];
  };
  const api = pipeline(wasm);
  const groups = api.groupFiles([buffer("data.h5"), buffer("metadata.xml")]);
  assert.equal(groups.length, 1);
  assert.equal(api.decodeGroup(groups[0]).primaryName, "metadata.xml");
});

test("fallback ENVI and AVIRIS grouping does not merge directories", () => {
  const wasm = fakeWasm();
  delete wasm.sidecarRequirements;
  const api = pipeline(wasm);
  assert.deepEqual(names(api.groupFiles([
    buffer("one/a.img"), buffer("two/a.hdr"), buffer("one/a.hdr"),
  ])), [["one/a.img", "one/a.hdr"], ["two/a.hdr"]]);
  assert.deepEqual(names(api.groupFiles([
    buffer("one/92AV3C.lan"), buffer("two/92AV3GT.GIS"),
    buffer("one/92AV3C.spc"), buffer("one/92AV3GT.GIS"),
  ])), [["one/92AV3C.lan", "one/92AV3C.spc", "one/92AV3GT.GIS"], ["two/92AV3GT.GIS"]]);
});

const wasmModule = process.env.N4F_WASM_MODULE
  || path.join(root, "bindings/wasm/pkg-node/nirs4all_formats_wasm.js");
if (process.env.N4F_WASM_MODULE) assert.ok(existsSync(wasmModule), "requested WASM module is missing");
const wasmAvailable = existsSync(wasmModule);
const realWasm = wasmAvailable ? createRequire(import.meta.url)(path.resolve(wasmModule)) : null;
function sample(relative, name = path.basename(relative)) {
  const bytes = new Uint8Array(readFileSync(path.join(root, relative)));
  return { name, bytes, size: bytes.length };
}

test("real WASM recovers optional CN targets and preserves ambiguity errors", { skip: !wasmAvailable && "nodejs WASM build unavailable" }, () => {
  const api = pipeline(realWasm);
  const da = sample("samples/foss_nsas_da/references700.da", "batch/references700.da");
  const cn = sample("samples/foss_nsas_da/references700.cn", "batch/references700.cn");
  const groups = api.groupFiles([cn, da]);
  assert.equal(groups.length, 1);
  const result = api.decodeGroup(groups[0]);
  assert.equal(result.records.length, 3);
  assert.deepEqual(result.records.map((r) => [r.targets.water, r.targets.oil]), [[1.25, 0], [0, 3.25], [2.5, 1.5]]);
  assert.ok(result.records[0].provenance.sources.some((s) => s.role === "constituent_reference"));
  assert.deepEqual(api.decodeGroup([da]).records[0].targets, {});
  const ambiguous = api.groupFiles([da, cn, { ...cn, name: "batch/references700.CN" }]);
  assert.equal(ambiguous[0].length, 3);
  assert.throws(() => api.decodeGroup(ambiguous[0]), /ambiguous/i);
  const malformed = { ...cn, bytes: cn.bytes.slice(0, 384) };
  assert.throws(() => api.decodeGroup(api.groupFiles([da, malformed])[0]), /CN size/);
});

test("real WASM ENVI and FGI sidecar decoding handles real extension casing", { skip: !wasmAvailable && "nodejs WASM build unavailable" }, () => {
  const api = pipeline(realWasm);
  for (const [primary, companion, companionName] of [
    ["samples/envi_sli/cubescope-mini-cube.img", "samples/envi_sli/cubescope-mini-cube.hdr", "cubescope-mini-cube.HDR"],
    ["samples/fgi/synthetic_fgi.xml", "samples/fgi/synthetic_fgi.h5", "synthetic_fgi.H5"],
  ]) {
    const groups = api.groupFiles([sample(companion, companionName), sample(primary)]);
    assert.equal(groups.length, 1, primary);
    assert.ok(api.decodeGroup(groups[0]).records.length > 0, primary);
  }
});

test("real WASM AVIRIS retains lowercase GIS references without fabricated aliases", { skip: !wasmAvailable && "nodejs WASM build unavailable" }, () => {
  const api = pipeline(realWasm);
  const groups = api.groupFiles([
    sample("samples/hyperspectral_cubes/92AV3GT.GIS", "92av3gt.gis"),
    sample("samples/hyperspectral_cubes/92AV3C.spc", "92AV3C.SPC"),
    sample("samples/hyperspectral_cubes/92AV3C.lan"),
  ]);
  assert.equal(groups.length, 1);
  const records = api.decodeGroup(groups[0]).records;
  assert.equal(records.length, 145 * 145);
  assert.ok(records.some((record) => Object.hasOwn(record.targets, "land_cover_class")));
  assert.ok(records[0].provenance.sources.some((source) => source.role === "ground_truth_sidecar"));
});

test("browser file input retains CN references in displayed records and JSON export", { skip: !process.env.N4F_PAGE_URL && "N4F_PAGE_URL not supplied" }, async () => {
  const moduleName = process.env.PLAYWRIGHT_MODULE || "playwright";
  const { chromium } = await import(path.isAbsolute(moduleName) ? pathToFileURL(moduleName).href : moduleName);
  const browser = await chromium.launch({ executablePath: process.env.CHROME_BIN || "/usr/bin/google-chrome", headless: true });
  try {
    const page = await browser.newPage({ acceptDownloads: true });
    await page.goto(process.env.N4F_PAGE_URL);
    await page.waitForFunction(() => typeof WASM !== "undefined" && WASM !== null);
    await page.setInputFiles("#file", [
      path.join(root, "samples/foss_nsas_da/references700.cn"),
      path.join(root, "samples/foss_nsas_da/references700.da"),
    ]);
    await page.waitForFunction(() => state.records?.length === 3 && state.records[0].targets?.water === 1.25);
    const targets = await page.evaluate(() => state.records.map((r) => [r.targets.water, r.targets.oil]));
    assert.deepEqual(targets, [[1.25, 0], [0, 3.25], [2.5, 1.5]]);
    const [download] = await Promise.all([page.waitForEvent("download"), page.click("#exp-json")]);
    const records = JSON.parse(readFileSync(await download.path(), "utf8"));
    assert.deepEqual(records.map((r) => [r.targets.water, r.targets.oil]), targets);
  } finally {
    await browser.close();
  }
});
