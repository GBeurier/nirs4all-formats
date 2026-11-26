// Browser check of the "Convert…" flow of the served Formats demo.
//
//   N4F_PAGE_URL       served page, for example http://localhost:8000/ (required:
//                      nirs4all-methods is an ES module and needs http).
//   CHROME_BIN         Chrome/Chromium executable (default /usr/bin/google-chrome).
//   PLAYWRIGHT_MODULE  optional absolute path / specifier of the Playwright module.
//   N4F_SHOTS          optional directory for screenshots.
//
// Loads public samples, plans a conversion for WinISI and JCAMP-DX, runs it with
// the vendored @nirs4all/methods, checks the derived signal is plotted and
// exports it without console errors.
import { pathToFileURL } from "node:url";
import path from "node:path";

const here = path.dirname(new URL(import.meta.url).pathname);
const repo = path.resolve(here, "..", "..");
const pageUrl = process.env.N4F_PAGE_URL;
if (!pageUrl || !pageUrl.startsWith("http")) throw new Error("N4F_PAGE_URL must be the served page (http://…/)");
const shots = process.env.N4F_SHOTS || null;
const moduleName = process.env.PLAYWRIGHT_MODULE || "playwright";
const { chromium } = await import(moduleName.startsWith("/") ? pathToFileURL(moduleName).href : moduleName);
const browser = await chromium.launch({ executablePath: process.env.CHROME_BIN || "/usr/bin/google-chrome", headless: true });
const EXTERNAL = /googleapis|gstatic|zgo\.at|goatcounter/;
const errors = [];

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

async function open(sample) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 1100 }, acceptDownloads: true });
  page.on("console", (msg) => {
    if (msg.type() === "error" && !EXTERNAL.test(msg.location().url || "")) errors.push(`${sample}: ${msg.text()}`);
  });
  page.on("pageerror", (err) => errors.push(`${sample}: ${err.message}`));
  await page.goto(pageUrl, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => document.querySelector("#stat-version")?.textContent.trim() !== "·", null, { timeout: 20000 });
  await page.setInputFiles("#file", path.join(repo, "samples", sample));
  await page.waitForSelector("#conv-open", { timeout: 20000 });
  await page.click("#conv-open");
  await page.waitForSelector("#conv-plan .conv-status");
  return page;
}

async function status(page) {
  return page.$eval("#conv-plan .conv-status", (n) => n.className.replace("conv-status s-", "").trim());
}

async function convert(page, target, answers = []) {
  await page.selectOption("#conv-target", target);
  for (const label of answers) {
    await page.click(`#conv-plan .conv-q button:has-text("${label}")`);
  }
  assert((await status(page)) === "ready", `${target}: plan is ${await status(page)}: ${await page.textContent("#conv-plan")}`);
  const output = await page.evaluate(() => conv.plan.output.name);
  await page.click("#conv-run");
  await page.waitForSelector("#conv-result .conv-summary, #conv-result .legacy-note", { timeout: 60000 });
  const failed = await page.$("#conv-result .legacy-note[role=alert]");
  assert(!failed, `${target}: ${failed && (await failed.textContent())}`);
  // The derived signal is a tab, listed in the signal table and drawn.
  const active = await page.$eval(".sig-tab.active", (n) => n.textContent.trim());
  assert(active === output, `${target}: active tab ${active}, expected ${output}`);
  const listed = await page.$$eval("#signals-panel .s-name", (ns) => ns.map((n) => n.textContent.trim()));
  assert(listed.includes(output), `${target}: ${output} not in the signal table`);
  const drawn = await page.$$eval("#plot-groups .ugroup", (ns) => ns.length);
  assert(drawn > 0, `${target}: converted signal not plotted`);
  await page.click('#conv-compare button:has-text("Both")');
  const heads = await page.$$eval("#plot-groups .conv-compare-head", (ns) => ns.map((n) => n.textContent));
  assert(heads.length === 2, `${target}: before/after view missing`);
  const download = page.waitForEvent("download", { timeout: 20000 });
  await page.click("#conv-export");
  const file = await download;
  const note = await page.textContent("#conv-export-note");
  assert(note.startsWith("Exported"), `${target}: ${note}`);
  assert(/Converted:/.test(note), `${target}: conversions not reported`);
  const summary = await page.textContent("#conv-result .conv-summary");
  return { output, file: file.suggestedFilename(), summary, note };
}

// [sample, target, output signal type or "", grid or "", answers]
const cases = [
  ["ocean_optics/whiteref.ProcSpec", "winisi", "", "", []],
  ["ocean_optics/whiteref.ProcSpec", "jcamp-dx", "absorbance", "", []],
  ["svc_ger/ACPL_D2_P1_B_1_001.sig", "winisi", "", "", []],
  ["svc_ger/ACPL_D2_P1_B_1_001.sig", "jcamp-dx", "absorbance", "", []],
  ["foss_winisi/foss_xds_barleyground_sensAIfood.csv", "winisi", "", "winisi-700", []],
  ["foss_winisi/foss_xds_barleyground_sensAIfood.csv", "jcamp-dx", "reflectance", "", []],
  ["bruker_opus/test_spectra.0", "jcamp-dx", "absorbance", "", ["Fraction (0-1)"]],
];
const results = [];
for (const [sample, target, type, grid, answers] of cases) {
  const page = await open(sample);
  await page.selectOption("#conv-target", target);
  if (type) await page.selectOption("#conv-type", type);
  if (grid) await page.selectOption("#conv-grid", grid);
  const result = await convert(page, target, answers);
  results.push({ sample, target, output: result.output, file: result.file });
  if (shots) await page.screenshot({ path: path.join(shots, `${path.basename(sample)}-${target}.png`), fullPage: true });
  await page.close();
}

// Steps computed by the methods added in Methods 1.5.2: cm-1 axes, references and
// detector splices. With an older Methods the page says so and adds nothing.
// [sample, source signal or "", answers, step that needs the new method]
const newer = [
  ["bruker_opus/brukeropus_file.0", "", [], "axis_unit_conversion"],
  ["bruker_opus/test_spectra.0", "", ["Fraction (0-1)"], "axis_unit_conversion"],
  // white - dark is 0 below ~200 nm: those points are missing, so the grid starts at 400 nm.
  ["ocean_optics/whiteref.ProcSpec", "sample", [], "reflectance_from_references", 400],
  ["svc_ger/ACPL_D2_P1_B_1_001.sig", "target", [], "splice_correction"],
];
for (const [sample, signal, answers, needed, gridFrom] of newer) {
  const page = await open(sample);
  await page.selectOption("#conv-target", "winisi");
  if (signal) await page.selectOption("#conv-signal", signal);
  if (gridFrom) {
    await page.selectOption("#conv-grid", "even");
    await page.fill("#conv-min", String(gridFrom));
    await page.dispatchEvent("#conv-min", "change");
  }
  for (const label of answers) await page.click(`#conv-plan .conv-q button:has-text("${label}")`);
  assert((await status(page)) === "ready", `${sample}: plan for WinISI is not ready`);
  const steps = await page.evaluate(() => conv.plan.steps);
  const step = steps.find((s) => s.step === needed);
  assert(step, `${sample}: no ${needed} step in ${steps.map((s) => s.step)}`);
  await page.click("#conv-run");
  await page.waitForSelector("#conv-result .conv-summary, #conv-result .legacy-note", { timeout: 60000 });
  const text = await page.textContent("#conv-result");
  const available = await page.evaluate(async (id) => {
    const M = await import("./vendor/methods/index.js");
    try { M.methodClass(id); return true; } catch (_) { return false; }
  }, step.method_id);
  let exported = null;
  if (available) {
    assert(/converted to/.test(text), `${sample}: ${text}`);
    const download = page.waitForEvent("download", { timeout: 20000 }).catch(() => null);
    await page.click("#conv-export");
    await page.waitForSelector("#conv-export-note");
    const note = await page.textContent("#conv-export-note");
    assert(note.startsWith("Exported"), `${sample}: ${note}`);
    exported = (await download)?.suggestedFilename();
  } else {
    assert(/does not provide yet/.test(text), `${sample}: ${text}`);
  }
  results.push({ sample, target: "winisi", needed: step.method_id, available, exported, summary: text.slice(0, 400) });
  if (shots) await page.screenshot({ path: path.join(shots, `${path.basename(sample)}-${signal || "auto"}-winisi.png`), fullPage: true });
  await page.close();
}

await browser.close();
for (const r of results) console.log(JSON.stringify(r));
assert(errors.length === 0, `console errors:\n${errors.join("\n")}`);
console.log(`OK: convert smoke passed (${pageUrl})`);
