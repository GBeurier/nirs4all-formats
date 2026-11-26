// Browser check of the top Export menu and the chart "Download view" of the served
// Formats demo.
//
//   N4F_PAGE_URL       served page, for example http://localhost:8000/ (required:
//                      nirs4all-methods is an ES module and needs http).
//   CHROME_BIN         Chrome/Chromium executable (default /usr/bin/google-chrome).
//   PLAYWRIGHT_MODULE  optional absolute path / specifier of the Playwright module.
//
// A standard format writes as is; an experimental target (FOSS WinISI) forces the
// conversions it needs, asking the planner's question in a dialog; ZIP bundles every
// signal; the chart view downloads exactly what is drawn, including the original put
// on the converted axis in the before/after overlay.
import { pathToFileURL } from "node:url";
import path from "node:path";
import { readFile } from "node:fs/promises";

const here = path.dirname(new URL(import.meta.url).pathname);
const repo = path.resolve(here, "..", "..");
const pageUrl = process.env.N4F_PAGE_URL;
if (!pageUrl || !pageUrl.startsWith("http")) throw new Error("N4F_PAGE_URL must be the served page (http://…/)");
const moduleName = process.env.PLAYWRIGHT_MODULE || "playwright";
const { chromium } = await import(moduleName.startsWith("/") ? pathToFileURL(moduleName).href : moduleName);
const browser = await chromium.launch({ executablePath: process.env.CHROME_BIN || "/usr/bin/google-chrome", headless: true });
const EXTERNAL = /googleapis|gstatic|zgo\.at|goatcounter/;
const errors = [];

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

async function open(sample) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 1000 }, acceptDownloads: true });
  page.on("console", (msg) => {
    if (msg.type() === "error" && !EXTERNAL.test(msg.location().url || "")) errors.push(`${sample}: ${msg.text()}`);
  });
  page.on("pageerror", (err) => errors.push(`${sample}: ${err.message}`));
  await page.goto(pageUrl, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => document.querySelector("#stat-version")?.textContent.trim() !== "·", null, { timeout: 20000 });
  await page.setInputFiles("#file", path.join(repo, "samples", sample));
  await page.waitForSelector("#convert-panel #conv-plan .conv-status", { timeout: 20000 });
  return page;
}

// Pick a menu entry and wait for the download it starts.
async function pick(page, button, menu, target) {
  const download = page.waitForEvent("download", { timeout: 60000 });
  await page.click(button);
  await page.click(`${menu} [data-target="${target}"]`);
  return download;
}

const results = [];

// 1. Standard format, nothing to convert: written as it is.
{
  const page = await open("foss_winisi/foss_xds_barleyground_sensAIfood.csv");
  const file = await pick(page, "#export-btn", "#export-menu", "csv");
  assert(file.suggestedFilename().endsWith(".csv"), file.suggestedFilename());
  const report = await page.textContent("#export-report");
  assert(report.startsWith("Exported") && !/Converted first/.test(report), report);
  assert(await page.$eval("#view-switch", (n) => n.classList.contains("hidden")), "a standard export converted something");
  results.push({ target: "csv", file: file.suggestedFilename() });

  // 2. ZIP: every signal as CSV in one archive.
  const zip = await pick(page, "#export-btn", "#export-menu", "__zip");
  assert(zip.suggestedFilename().endsWith(".zip"), zip.suggestedFilename());
  const bytes = await readFile(await zip.path());
  assert(bytes[0] === 0x50 && bytes[1] === 0x4b, "the ZIP export is not a ZIP archive");
  assert((await page.textContent("#export-report")).includes("every signal as CSV"), "ZIP report missing");
  results.push({ target: "zip", file: zip.suggestedFilename() });
  await page.close();
}

// 3. Experimental target that forces conversions, with the planner's question.
{
  const page = await open("bruker_opus/test_spectra.0");
  const experimental = await page.$$eval("#export-menu .menu-group", (gs) => gs
    .find((g) => g.querySelector(".menu-head").textContent.startsWith("Experimental"))
    .querySelectorAll("[data-target]").length);
  assert(experimental >= 4, `experimental group too small: ${experimental}`);
  const download = page.waitForEvent("download", { timeout: 60000 });
  await page.click("#export-btn");
  await page.click('#export-menu [data-target="winisi"]');
  await page.waitForSelector("#export-dialog[open] .conv-q");
  const prompt = await page.textContent("#export-dialog .conv-q legend");
  assert(/percent|fraction/i.test(prompt + (await page.textContent("#export-dialog"))), `unexpected question: ${prompt}`);
  await page.click('#export-dialog .choice:has-text("Fraction (0-1)")');
  const file = await download;
  assert(file.suggestedFilename().endsWith(".nir"), file.suggestedFilename());
  const report = await page.textContent("#export-report");
  assert(report.startsWith("Exported") && /Converted first/.test(report) && /Not tested in/.test(report), report);
  assert(!(await page.$eval("#export-dialog", (d) => d.open)), "the dialog stayed open");
  const output = await page.evaluate(() => conv.result.name);
  const tabs = await page.$$eval(".sig-tab", (ns) => ns.map((n) => n.textContent.trim()));
  assert(tabs.includes(output), `converted signal not on the page: ${tabs}`);
  results.push({ target: "winisi", file: file.suggestedFilename(), output });

  // 4. Both: one overlay; the original (cm-1) is drawn on the converted nm axis.
  await page.click('#view-switch button[data-view="both"]');
  await page.waitForSelector("#plot-groups figure.n4q-compare");
  assert(await page.$$eval("#plot-groups figure.n4q-compare svg", (ns) => ns.length) === 1, "the overlay is not one chart");
  const axisTitle = await page.$eval("#plot-groups figure.n4q-compare .n4q-axis-title", (n) => n.textContent);
  assert(/nm/.test(axisTitle), `shared x-axis is not in nm: ${axisTitle}`);
  const view = await page.evaluate(() => {
    const r = conv.result;
    const v = r.beforeView && r.beforeView.get(0);
    const src = state.records[0].signals[r.source];
    return { has: !!v, len: v ? v.axis.length : 0, srcLen: src.values.length, ascending: v ? v.axis.every((x, i) => i === 0 || x > v.axis[i - 1]) : false };
  });
  assert(view.has && view.len === view.srcLen && view.ascending, `original not placed on the converted axis: ${JSON.stringify(view)}`);

  // 5. Download view: exactly the two drawn signals.
  const json = await pick(page, "#view-dl-btn", "#view-dl-menu", "records-json");
  const records = JSON.parse(await readFile(await json.path(), "utf8"));
  const names = Object.keys(records[0].signals).sort();
  assert(names.length === 2 && names.includes(output), `view JSON signals: ${names}`);
  const derived = names.find((n) => n !== output);
  assert(records[0].signals[derived].axis.unit === "nm", `the original is not exported on the nm axis: ${records[0].signals[derived].axis.unit}`);
  const recorded = (records[0].metadata.conversions || []).filter((c) => c.signal === derived).map((c) => c.step);
  assert(recorded.includes("axis_unit_conversion"), `axis conversion of the original not recorded: ${recorded}`);
  const csv = await pick(page, "#view-dl-btn", "#view-dl-menu", "csv");
  assert(/\.(csv|zip)$/.test(csv.suggestedFilename()), csv.suggestedFilename());
  // Single view: one signal, one CSV.
  await page.click('#view-switch button[data-view="after"]');
  const single = await pick(page, "#view-dl-btn", "#view-dl-menu", "csv");
  assert(single.suggestedFilename().endsWith(".csv"), single.suggestedFilename());
  results.push({ view: "both", json: json.suggestedFilename(), signals: names, csv: csv.suggestedFilename(), single: single.suggestedFilename() });
  await page.close();
}

await browser.close();
for (const r of results) console.log(JSON.stringify(r));
assert(errors.length === 0, `console errors:\n${errors.join("\n")}`);
console.log(`OK: export smoke passed (${pageUrl})`);
