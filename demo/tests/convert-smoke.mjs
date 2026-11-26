// Browser check of automatic conversions in the served Formats demo.
//
//   N4F_PAGE_URL       served page, for example http://localhost:8000/ (required:
//                      nirs4all-methods is an ES module and needs http).
//   CHROME_BIN         Chrome/Chromium executable (default /usr/bin/google-chrome).
//   PLAYWRIGHT_MODULE  optional absolute path / specifier of the Playwright module.
//   N4F_SHOTS          optional directory for screenshots.
//
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
  await page.waitForSelector("#convert-panel", { timeout: 20000 });
  return page;
}

const cases = [
  ["ocean_optics/whiteref.ProcSpec", "reflectance", "absorbance"],
  ["svc_ger/ACPL_D2_P1_B_1_001.sig", "reflectance", "absorbance"],
  ["foss_winisi/foss_xds_barleyground_sensAIfood.csv", "signal", "reflectance"],
  ["bruker_opus/test_spectra.0", "reflectance", "absorbance", "Fraction (0-1)"],
];

try {
  for (const [sample, source, type, answer] of cases) {
    const page = await open(sample);
    const single = await page.evaluate(() => conv.names.length === 1);
    assert(await page.locator("#conv-signal").count() === Number(!single), "unexpected source selector");
    if (!single) await page.selectOption("#conv-signal", source);
    await page.selectOption("#conv-type", type);
    if (answer) await page.getByRole("button", { name: answer, exact: true }).click();
    await page.waitForFunction(() => conv.result && state.active === conv.result.name);
    const verify = (unit) => page.evaluate(({ source, type, unit }) => {
      const result = conv.result;
      const original = conv.records[0].signals[source];
      const signal = state.records[0].signals[result.name];
      return signal.signal_type === type && signal.axis.unit === (unit || original.axis.unit)
        && result.source === source && state.signalNames.length === conv.names.length + 1
        && !Object.hasOwn(conv.records[0].signals, result.name);
    }, { source, type, unit });
    assert(await verify(null), `${sample}: initial conversion`);
    // Rapid changes replace the conversion. The last unit wins and originals stay intact.
    await page.evaluate(() => {
      for (const unit of ["um", "nm", "um"]) {
        const select = document.querySelector("#conv-unit");
        select.value = unit;
        select.dispatchEvent(new Event("change", { bubbles: true }));
      }
    });
    await page.waitForFunction(() => conv.result?.outUnit === "um" && !document.querySelector("#export-btn").disabled);
    assert(await verify("um"), `${sample}: unit changes accumulated signals`);
    await page.click('#view-switch button[data-view="both"]');
    await page.waitForSelector('[data-route="compare"]');
    await page.selectOption("#conv-type", "");
    await page.selectOption("#conv-unit", "");
    assert(await page.evaluate(() => !conv.result && state.records === conv.records && state.active === (document.querySelector("#conv-signal")?.value || conv.names[0])), "original not restored");
    if (!single) {
      const alternate = await page.locator("#conv-signal option").first().getAttribute("value");
      await page.selectOption("#conv-signal", alternate);
      assert(await page.evaluate((name) => state.active === name, alternate), "source change did not update chart");
    }
    if (shots) await page.screenshot({ path: path.join(shots, `${path.basename(sample)}.png`), fullPage: true });
    await page.close();
  }
  // Reference and splice steps still come from Methods when selected in the toolbar.
  for (const [sample, source, step] of [
    ["ocean_optics/whiteref.ProcSpec", "sample", "reflectance_from_references"],
    ["svc_ger/ACPL_D2_P1_B_1_001.sig", "target", "reflectance_from_references"],
  ]) {
    const page = await open(sample);
    await page.selectOption("#conv-signal", source);
    await page.selectOption("#conv-type", "reflectance");
    await page.waitForFunction(() => conv.result || document.querySelector("#conv-plan [role=alert]"));
    const result = await page.evaluate(() => ({ steps: conv.result?.steps.map((s) => s.step), message: document.querySelector("#conv-plan").innerText }));
    assert(result.steps?.includes(step) || result.message.includes("does not provide yet"), JSON.stringify(result));
    await page.close();
  }
  assert(errors.length === 0, errors.join("\n"));
  console.log("OK: automatic conversion smoke passed");
} finally {
  await browser.close();
}
