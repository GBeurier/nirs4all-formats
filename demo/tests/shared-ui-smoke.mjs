// DOM check for the shared nirs4all-ui islands in the Formats demo.
//
//   N4F_PAGE_URL   served page (http://…/index.html) or the standalone file URL;
//                  default: demo/nirs4all-formats-demo.html (offline build).
//   CHROME_BIN     Chrome/Chromium executable (default /usr/bin/google-chrome).
//   PLAYWRIGHT_MODULE  optional absolute path / specifier of the Playwright module.
//
// The decoder selftest (_selftest.html) is unchanged and still owns the per-format
// decode assertions. This script covers what the shared UI adds: mount / update /
// unmount / remount lifecycle, exact-axis grouping, gap markers, legacy routing.
// Presentation fixtures below are hand-written display records for DOM behaviour;
// they make no claim about any decoder or scientific result.
import { pathToFileURL } from "node:url";
import path from "node:path";
import { readFile } from "node:fs/promises";

const here = path.dirname(new URL(import.meta.url).pathname);
const demo = path.resolve(here, "..");
const pageUrl = process.env.N4F_PAGE_URL || pathToFileURL(path.join(demo, "nirs4all-formats-demo.html")).href;
const offline = pageUrl.startsWith("file:");
const chromeExecutable = process.env.CHROME_BIN || "/usr/bin/google-chrome";

async function loadPlaywright() {
  const moduleName = process.env.PLAYWRIGHT_MODULE || "playwright";
  const specifier = moduleName.startsWith("/") ? pathToFileURL(moduleName).href : moduleName;
  return import(specifier);
}

const { chromium } = await loadPlaywright();
const browser = await chromium.launch({
  executablePath: chromeExecutable,
  headless: true,
  args: ["--allow-file-access-from-files"],
});

const errors = [];
const uiRequests = [];
// Fonts and analytics are external to the demo and unreachable offline: not a UI failure.
const EXTERNAL = /googleapis|gstatic|zgo\.at|goatcounter/;

async function newPage() {
  const page = await browser.newPage({ viewport: { width: 1280, height: 1000 }, acceptDownloads: true });
  page.on("console", (msg) => {
    if (msg.type() === "error" && !EXTERNAL.test(msg.location().url || "")) errors.push(msg.text());
  });
  page.on("pageerror", (err) => errors.push(err.message));
  page.on("request", (req) => { if (req.url().includes("vendor/ui")) uiRequests.push(req.url()); });
  await page.goto(pageUrl, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(
    () => window.Nirs4allUIBrowser && document.querySelector("#stat-version")?.textContent.trim() !== "·",
    null,
    { timeout: 20000 },
  );
  return page;
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

// Display records: tiny, explicit, and only for DOM behaviour.
function installFixture(page, { datasets }) {
  return page.evaluate((count) => {
    const sig = (axisValues, values, o = {}) => ({
      values,
      axis: { kind: o.kind ?? "wavelength", unit: o.unit ?? "nm", order: o.order ?? "ascending", values: axisValues },
      dims: o.dims ?? ["x"], shape: o.shape ?? [values.length],
      signal_type: "absorbance", role: "signal", source: "file",
    });
    const A = [1000, 1010, 1020, 1030];
    const records = [
      { signals: { absorbance: sig(A, [0.1, 0.2, NaN, 0.4]), second: sig([1000, 1010, 1020], [1, 2, 3]) }, metadata: { sample_id: "s1" } },
      { signals: { absorbance: sig(A, [0.2, 0.3, 0.4, 0.5]), second: sig([1000, 1010, 1020], [3, 2, 1]) } },
      { signals: { absorbance: sig([1000, 1010, 1020, 1035], [0.3, 0.4, 0.5, 0.6]) } },            // differs by one axis value
      { signals: { absorbance: sig([4000, 4010, 4020, 4030], [1, 2, 3, 4], { kind: "wavenumber", unit: "cm-1" }) } },
      { signals: { absorbance: sig([1000, NaN, 1020, 1030], [1, 2, 3, 4]) } },                      // non-finite axis
      { signals: { absorbance: sig([1000, 1010, 1020, 1030], [1, 2, 3]) } },                        // length mismatch
      { signals: { absorbance: sig([1000, 1010], [1, 2, 3, 4], { dims: ["x", "y"], shape: [2, 2] }) } }, // multidimensional
    ];
    const ds = () => ({ ok: true, name: "fixture.csv", size: 1, probes: [], records, fileCount: 1 });
    renderDatasets(Array.from({ length: count }, ds));
    return records.length;
  }, datasets);
}

const counts = (page) => page.evaluate(() => ({
  shared: document.querySelectorAll("#plot-groups section[data-route=shared]").length,
  legacy: document.querySelectorAll("#plot-groups section[data-route=legacy]").length,
  cards: document.querySelectorAll("#plot-groups article[data-dataset-id]").length,
  charts: document.querySelectorAll("#plot-groups figure.n4q-explorer").length,
  handles: state.mounts.length,
}));

async function waitMounted(page, shared) {
  await page.waitForFunction(
    (n) => document.querySelectorAll("#plot-groups figure.n4q-explorer").length === n
      && document.querySelectorAll("#plot-groups article[data-dataset-id]").length === n,
    shared,
    { timeout: 10000 },
  );
}

// Remember the dedicated host divs so a later teardown can prove nothing is left in them.
const rememberHosts = (page) => page.evaluate(() => {
  window.__hosts = [...document.querySelectorAll("#plot-groups .ugroup-card, #plot-groups .ugroup-chart")];
  window.__svg0 = document.querySelector("#plot-groups figure.n4q-explorer svg");
  return window.__hosts.length;
});
const hostsReleased = (page) => page.evaluate(() => window.__hosts.every((h) => !h.isConnected && h.childElementCount === 0));

try {
  await fixtureFlow();
  await dedicatedTeardownFlow();
  await csvExportFlow();
  await spectralTableContractFlow();
  await realDecodeFlow();
  await bundleFlow();
  if (errors.length) throw new Error(`browser errors:\n${errors.join("\n")}`);
  console.log(`OK: Formats shared-UI smoke passed (${pageUrl})`);
} finally {
  await browser.close();
}

async function fixtureFlow() {
  const page = await newPage();
  await installFixture(page, { datasets: 1 });
  await waitMounted(page, 2);

  // Exact-axis grouping: records 1-2 share an axis, record 3 differs by one value.
  // The two other 1-D routes stay on the original canvas; the 2-D record is listed.
  const c = await counts(page);
  assert(c.shared === 2 && c.legacy === 2, `expected 2 shared + 2 legacy groups, got ${JSON.stringify(c)}`);
  assert(c.cards === 2 && c.charts === 2 && c.handles === 4, `card/chart mounts do not match groups: ${JSON.stringify(c)}`);
  const kpis = await page.textContent("#kpis");
  assert(kpis.includes("Chart groups") && kpis.includes("2 shared · 2 legacy"), "page KPIs do not report the groups");
  const ranges = await page.locator("#plot-groups section[data-route=shared] .kpi", { hasText: "Axis range" }).allTextContents();
  assert(ranges.length === 2 && ranges[0].includes("1000–1030") && ranges[1].includes("1000–1035"), `per-group axis ranges wrong: ${ranges}`);
  const descs = await page.locator("#plot-groups figure.n4q-explorer svg desc").allTextContents();
  assert(descs.length === 2 && descs[0] !== descs[1], "the two axis groups render identical chart summaries");

  // Card: view only, record/channel wording, no sample or split claims. Read the DOM
  // text (textContent), not innerText: the stat labels are shown uppercase by CSS
  // (text-transform), so innerText would read RECORDS / CHANNELS.
  const cards = page.locator("#plot-groups article[data-dataset-id]");
  const statsOf = (card) => card.locator(".n4ui-b-stat").evaluateAll((items) => items.map((item) => ({
    label: item.querySelector(".n4ui-b-stat-label")?.textContent,
    value: item.querySelector(".n4ui-b-stat-value")?.textContent,
  })));
  // Group 1: records 1-2 on a 4-point axis; group 2: record 3 alone on its own 4-point axis.
  assert(JSON.stringify(await statsOf(cards.nth(0))) === JSON.stringify([{ label: "Records", value: "2" }, { label: "Channels", value: "4" }]),
    `group 1 card stats are wrong: ${JSON.stringify(await statsOf(cards.nth(0)))}`);
  assert(JSON.stringify(await statsOf(cards.nth(1))) === JSON.stringify([{ label: "Records", value: "1" }, { label: "Channels", value: "4" }]),
    `group 2 card stats are wrong: ${JSON.stringify(await statsOf(cards.nth(1)))}`);
  const cardText = await cards.first().textContent();
  assert(!/samples/i.test(cardText) && !/split/i.test(cardText), "card makes a sample or split claim");

  // Gap marker: the single NaN cell is flagged in group 1 and nowhere in group 2.
  const gaps = await page.locator("#plot-groups section[data-route=shared]").evaluateAll(
    (sections) => sections.map((s) => s.querySelectorAll("path.n4q-explorer-missing").length));
  assert(gaps[0] === 1 && gaps[1] === 0, `gap marker counts wrong: ${gaps}`);

  // Exactly one data table per shared chart (text alternative, charts-graphs skill).
  const tables = await page.locator("#plot-groups section[data-route=shared]").evaluateAll(
    (sections) => sections.map((s) => s.querySelectorAll("table").length));
  assert(tables.every((n) => n === 1), `each shared chart needs exactly one data table: ${tables}`);

  // Axes and values reach the chart exactly as decoded: same length, ===, NaN kept in place.
  const exact = await page.evaluate(() => {
    const plan = planSignal(state.active);
    return plan.groups.every((g, k) => {
      const p = state.explorers[k].props([]);
      return p.axis.length === g.axis.values.length && p.axis.every((v, i) => v === g.axis.values[i])
        && g.members.every(({ sig }, m) => {
          const vals = p.series[m].values;
          return vals.length === sig.values.length && vals.every((v, i) => Object.is(v, sig.values[i]));
        });
    });
  });
  assert(exact, "axis or series values were altered on the way to the shared chart");

  // Legacy routing is explicit: nothing silently dropped or reversed.
  const legacy = await page.locator("#plot-groups section[data-route=legacy]").evaluateAll((sections) => sections.map((s) => ({
    note: s.querySelector(".legacy-note")?.textContent || "", canvas: !!s.querySelector("canvas.spectra"),
  })));
  assert(legacy.every((l) => l.canvas && l.note.includes("Original canvas renderer")), "legacy group lacks canvas or note");
  assert(legacy.some((l) => l.note.includes("high-to-low")), "wavenumber orientation is not disclosed");
  assert(legacy.some((l) => l.note.includes("non-finite") && l.note.includes("prefix") && l.note.includes("2 of 2")), "invalid axes are not disclosed or were dropped");
  const multi = await page.textContent("#plot-groups > p.legacy-note");
  assert(multi.includes("multidimensional") && multi.includes("Records: 7"), "multidimensional record is not listed");

  // Selection: new host state, update only (same chart node, no remount).
  await page.evaluate(() => { window.__svg0 = document.querySelector("#plot-groups figure.n4q-explorer svg"); });
  await page.locator("#plot-groups figure.n4q-explorer svg").first().focus();
  await page.keyboard.press("ArrowDown");
  await page.waitForFunction(() => document.querySelector("#sel-text")?.textContent.startsWith("Selected: record 1 · signal absorbance · axis group 1"));
  assert((await page.textContent("#sel-text")).includes("s1"), "selection line lost the sample label");
  assert(await page.evaluate(() => window.__svg0 === document.querySelector("#plot-groups figure.n4q-explorer svg") && window.__svg0.isConnected),
    "selection remounted the chart instead of updating it");
  const focusPaths = await page.locator("#plot-groups section[data-route=shared]").evaluateAll(
    (sections) => sections.map((s) => s.querySelectorAll("path.n4q-explorer-focus").length));
  assert(focusPaths[0] === 1 && focusPaths[1] === 0, `selection emphasis is wrong: ${focusPaths}`);
  await page.keyboard.press("Escape");
  await page.waitForFunction(() => document.querySelector("#sel-text")?.textContent.startsWith("No record selected"));
  await page.keyboard.press("ArrowDown");
  await page.waitForFunction(() => document.querySelector("#sel-text")?.textContent.startsWith("Selected:"));
  await page.click("#sel-clear");
  assert((await page.textContent("#sel-text")).startsWith("No record selected"), "clear selection did not clear");

  // Signal change: every island of the old signal is unmounted and its host emptied; new ones mount.
  await page.locator("#plot-groups figure.n4q-explorer svg").first().focus();
  await page.keyboard.press("ArrowDown");
  await page.waitForFunction(() => document.querySelector("#sel-text")?.textContent.startsWith("Selected:"));
  await rememberHosts(page);
  await page.locator(".sig-tab", { hasText: "second" }).click();
  await waitMounted(page, 1);
  assert(await hostsReleased(page), "signal change left React roots or nodes in the old hosts");
  assert(await page.evaluate(() => !window.__svg0.isConnected), "signal change reused the old chart instead of remounting");
  const after = await counts(page);
  assert(after.shared === 1 && after.legacy === 0 && after.cards === 1 && after.charts === 1 && after.handles === 2,
    `signal change left orphan islands: ${JSON.stringify(after)}`);
  assert((await page.textContent("#sel-text")).startsWith("No record selected"), "selection survived a signal change");

  // Reset unmounts everything before clearing the container.
  await page.locator(".sig-tab", { hasText: "absorbance" }).click();
  await waitMounted(page, 2);
  await rememberHosts(page);
  await page.click("#reset");
  assert(await hostsReleased(page), "reset left React roots or nodes in the old hosts");
  const reset = await page.evaluate(() => ({
    figures: document.querySelectorAll("figure.n4q-explorer, article[data-dataset-id]").length,
    handles: state.mounts.length, legacy: state.legacy.length, results: document.querySelector("#results").innerHTML,
  }));
  assert(reset.figures === 0 && reset.handles === 0 && reset.legacy === 0 && reset.results === "", `reset leaked state: ${JSON.stringify(reset)}`);
  await page.close();
}

async function dedicatedTeardownFlow() {
  // Several datasets → tabs; switching tabs must release the previous dataset's islands.
  const page = await newPage();
  await installFixture(page, { datasets: 2 });
  await waitMounted(page, 2);
  await rememberHosts(page);
  await page.locator(".ds-tab").nth(1).click();
  await waitMounted(page, 2);
  assert(await hostsReleased(page), "dataset tab switch left React roots or nodes in the old hosts");
  const c = await counts(page);
  assert(c.handles === 4 && c.charts === 2 && c.cards === 2, `dataset tab switch leaked islands: ${JSON.stringify(c)}`);
  // A new decode over an existing result also tears down first.
  await rememberHosts(page);
  await installFixture(page, { datasets: 1 });
  await waitMounted(page, 2);
  assert(await hostsReleased(page), "re-render over a result left React roots or nodes in the old hosts");
  await page.close();
}

// CSV needs one header for every row: records on different axes are refused (no file),
// records on one exact axis export unchanged, whatever the chart can orient.
async function csvExportFlow() {
  const page = await newPage();
  // Heterogeneous axes (the shared fixture): refusal, no download, JSON still lossless.
  await installFixture(page, { datasets: 1 });
  await waitMounted(page, 2);
  const downloaded = page.waitForEvent("download", { timeout: 1500 }).then(() => true, () => false);
  await page.click("#exp-csv");
  assert(!(await downloaded), "CSV was downloaded for records on different axes");
  const refusal = await page.textContent("#csv-refusal");
  assert(refusal.includes("CSV export refused") && refusal.includes("record 3") && refusal.includes("JSON export"), `CSV refusal is missing or unclear: ${refusal}`);
  assert((await page.textContent("#status")).includes("CSV export refused"), "CSV refusal was not announced in the status line");
  const [json] = await Promise.all([page.waitForEvent("download"), page.click("#exp-json")]);
  const records = JSON.parse(await readFile(await json.path(), "utf8"));
  assert(Array.isArray(records) && records.length === 7, "JSON export no longer holds every record");
  await page.locator(".sig-tab", { hasText: "second" }).click();
  assert(await page.locator("#csv-refusal").count() === 0, "a stale CSV refusal survived a signal change");

  // One exact axis, even a wavenumber one the shared chart does not orient: CSV unchanged.
  await page.evaluate(() => {
    const axis = { kind: "wavenumber", unit: "cm-1", order: "ascending", values: [4000, 4010, 4020] };
    const sig = (values) => ({ values, axis, dims: ["x"], shape: [3], signal_type: "absorbance", role: "signal", source: "file" });
    renderDatasets([{ ok: true, name: "same_axis.csv", size: 1, probes: [], fileCount: 1,
      records: [{ signals: { absorbance: sig([1, 2, 3]) } }, { signals: { absorbance: sig([4, NaN, 6]) } }] }]);
  });
  await page.waitForSelector("#plot-groups section[data-route=legacy]");
  const [csv] = await Promise.all([page.waitForEvent("download"), page.click("#exp-csv")]);
  const text = await readFile(await csv.path(), "utf8");
  assert(text === "sample,4000,4010,4020\nrecord_0,1,2,3\nrecord_1,4,NaN,6", `homogeneous CSV changed: ${JSON.stringify(text)}`);
  assert(await page.locator("#csv-refusal").count() === 0, "a refusal is shown for a valid homogeneous export");
  await page.close();
}

// Pure widget contract of the shared spectra explorer's band table, driven through the real
// browser IIFE (mount / update / unmount) with literal presentation props. No decoder output,
// owner controller or result is involved: the numbers below are display fixtures only.
async function spectralTableContractFlow() {
  const page = await newPage();
  await page.evaluate(() => {
    const host = document.createElement("div");
    host.id = "contract-host";
    document.body.appendChild(host);
    window.__calls = { first: [], second: [] };
    window.__props = (calls, extra = {}) => ({
      axis: [1000, 1010, 1020, 1030],
      series: [
        { id: "a", values: [1, 2, NaN, 4], color: "#1f77b4", label: "sample A" },
        { id: "b", values: [10, 20, 30, 40], color: "#d62728", label: "sample B" },
      ],
      axisUnit: "nm", yLabel: "Absorbance", title: "Contract spectra",
      selected: [], onSelect: (id) => calls.push(id), ...extra,
    });
    window.__handle = Nirs4allUIBrowser.mountSpectraExplorer(host, window.__props(window.__calls.first));
  });
  const host = page.locator("#contract-host");
  const cells = () => host.locator("tbody tr").evaluateAll((rows) => rows.map((r) => [...r.children].map((c) => ({ text: c.textContent, value: c.getAttribute("data-value") }))));
  const caption = () => host.locator("table caption").textContent();
  const bandInput = host.locator('input[type="number"]');

  // Exactly one spectral table (text alternative), none duplicated by the host wrapper.
  assert(await host.locator("table").count() === 1, "the explorer must render exactly one data table");
  assert(await host.locator("figure.n4q-explorer").count() === 1, "expected exactly one explorer figure");

  // Opening the details and typing a band index changes the caption and the cells.
  await host.locator("details.n4q-datatable").first().locator("summary").first().click();
  assert(await caption() === "Contract spectra · 1000 nm", `initial caption is wrong: ${await caption()}`);
  assert((await cells())[1][2].text === "10", "band 1 cell for series b is wrong");
  await bandInput.fill("3");
  assert(await caption() === "Contract spectra · 1020 nm", `typing band 3 did not change the caption: ${await caption()}`);
  let rows = await cells();
  assert(rows[1][2].text === "30" && rows[1][2].value === "30", `band 3 value of series b is wrong: ${JSON.stringify(rows[1])}`);
  // The missing (NaN) cell reads "not available" and carries no numeric data-value.
  assert(rows[0][2].text === "not available" && rows[0][2].value === "", `NaN cell must read not available: ${JSON.stringify(rows[0])}`);
  assert(!(await host.locator("table").textContent()).includes("NaN"), "a NaN leaked into the table text");

  // Controlled selection through the host callback; id passed unchanged.
  await host.locator("figure.n4q-explorer svg").first().focus();
  await page.keyboard.press("ArrowDown");
  assert(JSON.stringify(await page.evaluate(() => window.__calls.first)) === JSON.stringify(["a"]), "onSelect did not receive the series id");

  // Update (full replacement props, new callback): band and selection survive; only the new callback fires.
  await page.evaluate(() => window.__handle.update(window.__props(window.__calls.second, { selected: ["b"] })));
  assert(await bandInput.inputValue() === "3" && await caption() === "Contract spectra · 1020 nm", "band was lost by update");
  assert(await host.locator("path.n4q-explorer-focus").count() === 1, "controlled selection did not emphasise one series");
  assert(await host.locator("table").count() === 1, "update duplicated the table");
  await host.locator("figure.n4q-explorer svg").first().focus();
  await page.keyboard.press("Escape");
  const calls = await page.evaluate(() => window.__calls);
  assert(JSON.stringify(calls.first) === JSON.stringify(["a"]) && JSON.stringify(calls.second) === JSON.stringify([null]),
    `callback identity not preserved across update: ${JSON.stringify(calls)}`);

  // Update with a shorter axis AND correspondingly shorter value vectors: the band clamps.
  await page.evaluate(() => window.__handle.update(window.__props(window.__calls.second, {
    axis: [1000, 1010],
    series: [{ id: "a", values: [1, 2], color: "#1f77b4" }, { id: "b", values: [10, 20], color: "#d62728" }],
  })));
  assert(await bandInput.inputValue() === "2" && await bandInput.getAttribute("max") === "2", "band index was not clamped to the shorter axis");
  assert(await caption() === "Contract spectra · 1010 nm", `clamped caption is wrong: ${await caption()}`);
  rows = await cells();
  assert(rows.length === 2 && rows[1][2].text === "20" && rows[0][2].text === "2", `clamped cells are wrong: ${JSON.stringify(rows)}`);
  assert(!(await host.locator("table").textContent()).includes("undefined"), "undefined leaked after clamping");

  // Unmount releases the root; a second unmount is harmless.
  await page.evaluate(() => { window.__handle.unmount(); window.__handle.unmount(); });
  assert(await host.evaluate((h) => h.childElementCount) === 0, "unmount left nodes in the host");
  await page.close();
}

async function realDecodeFlow() {
  // Real decoder output for two bundled samples through the real file input.
  for (const sample of ["synthetic_nirs.csv", "opus_ftir.0"]) {
    const page = await newPage();
    await page.setInputFiles("#file", path.join(demo, "samples", sample));
    await page.waitForFunction((name) => document.querySelector("#result-heading")?.textContent === name, sample, { timeout: 30000 });
    const settled = await page.evaluate(() => {
      const plan = planSignal(state.active);
      const members = plan.groups.reduce((n, g) => n + g.members.length, 0)
        + plan.legacy.reduce((n, g) => n + g.members.length, 0) + plan.multi.length;
      const carrying = state.records.filter((r) => r.signals && r.signals[state.active]).length;
      return { shared: plan.groups.length, legacy: plan.legacy.length, members, carrying };
    });
    assert(settled.members === settled.carrying, `${sample}: records were dropped by grouping`);
    assert(settled.shared + settled.legacy >= 1, `${sample}: nothing to plot`);
    if (settled.shared) await waitMounted(page, settled.shared);
    const c = await counts(page);
    assert(c.shared === settled.shared && c.legacy === settled.legacy, `${sample}: DOM groups differ from the plan: ${JSON.stringify(c)}`);
    // Existing exports are untouched: the JSON export still holds every decoded record.
    const [download] = await Promise.all([page.waitForEvent("download"), page.click("#exp-json")]);
    const exported = JSON.parse(await readFile(await download.path(), "utf8"));
    assert(Array.isArray(exported) && exported.length === await page.evaluate(() => state.records.length), `${sample}: JSON export changed`);
    await page.close();
  }
}

async function bundleFlow() {
  const page = await newPage();
  const tags = await page.evaluate(() => ({
    scripts: document.querySelectorAll('script[src*="vendor/ui"]').length,
    links: document.querySelectorAll('link[href*="vendor/ui"]').length,
    notices: !!document.getElementById("nirs4all-ui-notices"),
  }));
  if (offline) {
    assert(tags.scripts === 0 && tags.links === 0, "standalone page still references the UI files");
    assert(tags.notices, "standalone page lacks the embedded UI notices");
    assert(uiRequests.length === 0, `standalone page fetched UI assets: ${uiRequests}`);
    // The complete staged MIT texts (not just their headers) are embedded, each once.
    const embedded = await page.evaluate(() => document.getElementById("nirs4all-ui-notices").textContent);
    for (const name of ["react", "react-dom", "scheduler"]) {
      const rel = `LICENSES/THIRD-PARTY/${name}.LICENSE.txt`;
      const staged = (await readFile(path.join(demo, "vendor", "ui", rel), "utf8")).replace(/\r\n?/g, "\n");
      assert(embedded.split(`=== ${rel} ===`).length - 1 === 1, `${rel}: path header is not embedded exactly once`);
      assert(embedded.split(`=== ${rel} ===\n${staged}`).length - 1 === 1, `${rel}: embedded text differs from the staged file or is repeated`);
    }
  } else {
    assert(tags.scripts === 1 && tags.links === 1, "served page does not load the staged UI files");
    assert(uiRequests.length >= 2, "served page did not request the UI bundle and stylesheet");
  }
  await page.close();
}
