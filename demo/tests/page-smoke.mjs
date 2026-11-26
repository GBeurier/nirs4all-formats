// Page-level check of the Formats site: page order (no standalone Convert
// section), library code tabs with their install lines, the sortable catalogue,
// the decoded-file workspace (Export menu, side Convert panel with labelled
// controls), no horizontal overflow at phone width, and (in the single-file
// build) a clear message instead of a conversion.
//
//   N4F_PAGE_URL       served page (http://…/) or the standalone file URL;
//                      default: demo/nirs4all-formats-demo.html.
//   CHROME_BIN         Chrome/Chromium executable (default /usr/bin/google-chrome).
//   PLAYWRIGHT_MODULE  optional absolute path / specifier of the Playwright module.
import { pathToFileURL } from "node:url";
import path from "node:path";

const here = path.dirname(new URL(import.meta.url).pathname);
const demo = path.resolve(here, "..");
const pageUrl = process.env.N4F_PAGE_URL || pathToFileURL(path.join(demo, "nirs4all-formats-demo.html")).href;
const offline = pageUrl.startsWith("file:");
const moduleName = process.env.PLAYWRIGHT_MODULE || "playwright";
const { chromium } = await import(moduleName.startsWith("/") ? pathToFileURL(moduleName).href : moduleName);
const browser = await chromium.launch({
  executablePath: process.env.CHROME_BIN || "/usr/bin/google-chrome",
  headless: true,
  args: ["--allow-file-access-from-files"],
});
const EXTERNAL = /googleapis|gstatic|zgo\.at|goatcounter/;
const errors = [];
function assert(condition, message) {
  if (!condition) throw new Error(message);
}

for (const viewport of [{ width: 1280, height: 900 }, { width: 320, height: 844 }]) {
  const page = await browser.newPage({ viewport });
  page.on("console", (msg) => {
    if (msg.type() === "error" && !EXTERNAL.test(msg.location().url || "")) errors.push(msg.text());
  });
  page.on("pageerror", (err) => errors.push(err.message));
  await page.goto(pageUrl, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => document.querySelector("#stat-version")?.textContent.trim() !== "·", null, { timeout: 20000 });
  const order = await page.evaluate(() => [...document.querySelectorAll("main > section")].map((n) => n.id));
  assert(order.join() === "results,library,formats", `page order: ${order}`);
  assert(await page.$("#convert") === null && await page.$(".channels") === null, "the Convert section or the channel grid is still there");
  const codeTitle = page.getByRole("heading", { name: "Code examples" });
  assert(await codeTitle.isVisible(), "code examples heading missing");
  assert(await codeTitle.evaluate((h) => getComputedStyle(h).borderTopWidth !== "0px"), "code examples divider missing");
  await page.getByRole("link", { name: "Catalog", exact: true }).click();
  assert(new URL(page.url()).hash === "#formats", "Catalog link does not reach the format list");
  const installs = await page.$$eval(".code-panel .code-install", (ns) => ns.length);
  assert(installs === 10, `install lines: ${installs}`);
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
  assert(overflow <= 1, `${viewport.width}px: horizontal overflow of ${overflow}px`);

  // Library tabs: one selected tab, arrow keys and Home/End move the selection.
  const tabs = await page.$$eval(".code-tab", (ts) => ts.map((t) => t.textContent));
  assert(tabs.join("|") === "Python|R|Rust|JavaScript / WASM|CLI|C|Java|C#|Julia|Docker", tabs.join("|"));
  await page.focus("#tab-python");
  await page.keyboard.press("ArrowRight");
  assert(await page.$eval("#tab-r", (t) => t.getAttribute("aria-selected")) === "true", "ArrowRight");
  assert(await page.$eval("#panel-python", (p) => p.hidden), "Python panel still shown");
  await page.keyboard.press("End");
  assert(await page.$eval("#panel-docker", (p) => !p.hidden), "End");
  assert(await page.$eval("#panel-docker", (p) => /docker run --rm/.test(p.textContent)), "Docker snippet");
  // Every language tab shows a highlighted snippet with a copy button.
  const blocks = await page.$$eval(".code-panel", (ps) => ps.map((p) => p.querySelector(".code-block pre code .c") !== null));
  assert(blocks.length === 10 && blocks.every(Boolean), `code blocks: ${blocks}`);
  await page.keyboard.press("Home");
  assert(await page.$eval("#tab-python", (t) => t.getAttribute("aria-selected")) === "true", "Home");

  // Catalogue: header buttons sort by name, coverage and status, with aria-sort.
  const names = () => page.$$eval("#fmt-body .f-name", (ns) => ns.map((n) => n.textContent));
  const original = await names();
  await page.click('.fmt-sort[data-sort="name"]');
  const sorted = [...original].sort((a, b) => a.localeCompare(b, "en", { sensitivity: "base" }));
  assert(JSON.stringify(await names()) === JSON.stringify(sorted), "name ascending");
  await page.focus('.fmt-sort[data-sort="name"]');
  await page.keyboard.press("Enter");
  assert(JSON.stringify(await names()) === JSON.stringify([...sorted].reverse()), "name descending");
  const sortState = () => page.$$eval(".fmt-table th[aria-sort]", (ths) => ths.map((th) => th.getAttribute("aria-sort")));
  assert((await sortState()).join() === "descending,none,none", (await sortState()).join());
  await page.click('.fmt-sort[data-sort="coverage"]');
  assert((await sortState()).join() === "none,ascending,none", (await sortState()).join());
  await page.click('.fmt-sort[data-sort="status"]');
  const statuses = await page.$$eval("#fmt-body .st", (ns) => ns.map((n) => n.textContent.trim()));
  assert(statuses[0] === "Supported" && statuses.at(-1) !== "Supported", statuses.join());
  assert((await names()).length === original.length, "rows lost by sorting");

  await page.setInputFiles("#file", path.join(demo, "..", "samples", "ocean_optics", "whiteref.ProcSpec"));
  await page.waitForSelector("#convert-panel");
  const layout = await page.evaluate(() => {
    const rect = (sel) => document.querySelector(sel).getBoundingClientRect();
    return {
      header: rect(".results-head").bottom, props: rect(".file-properties").top,
      propsEnd: rect(".file-properties").bottom, plot: rect(".plot-panel").top,
      plotEnd: rect(".plot-panel").bottom, metadata: rect(".metadata-panel").top,
      metadataEnd: rect(".metadata-panel").bottom, library: rect("#library-title").top,
      controls: [...document.querySelectorAll(".convert-row select")].map((s) => {
        const { left, right, top, width } = s.getBoundingClientRect();
        const label = s.closest("label");
        return { top, width, fits: left >= label.getBoundingClientRect().left - 1
          && right <= label.getBoundingClientRect().right + 1 && label.scrollWidth <= label.clientWidth + 1 };
      }),
    };
  });
  assert(layout.header <= layout.props && layout.propsEnd <= layout.plot, "file, properties and spectra order");
  assert(layout.plotEnd <= layout.metadata && layout.metadataEnd <= layout.library, "spectra, metadata and code examples order");
  assert(layout.controls.every((control) => control.fits && control.width >= 85), "conversion controls overflow or are too narrow");
  const controlRows = new Set(layout.controls.map((control) => Math.round(control.top)));
  assert(controlRows.size === (viewport.width >= 1000 ? 1 : 2), "conversion controls use the wrong number of rows");
  const vendor = page.locator(".metadata-nested").filter({ has: page.getByRole("heading", { name: "vendor", exact: true }) });
  assert(await vendor.locator(".metadata-field code").first().isVisible(), "decoded vendor metadata has no visible preview");
  assert(await page.getByRole("button", { name: "Clear", exact: true }).count() === 1, "Clear action missing");
  assert(await page.locator("#conv-run, #conv-target, #sel-clear").count() === 0, "obsolete actions");
  assert(await page.locator(".ugroup-chart summary:visible").count() === 0, "band table shown");
  await page.getByRole("button", { name: "Conversion settings and limits" }).click();
  assert(await page.getByRole("dialog").getByText("Limits", { exact: true }).isVisible(), "limits missing");
  await page.keyboard.press("Escape");
  // Top Export menu: three maturity groups, keyboard reachable, Esc closes.
  await page.focus("#export-btn");
  await page.keyboard.press("Enter");
  const heads = await page.$$eval("#export-menu .menu-head", (ns) => ns.map((n) => n.firstChild.textContent.trim()));
  assert(heads.join("|") === "File formats|Experimental|Other formats", heads.join("|"));
  assert(await page.evaluate(() => document.activeElement?.getAttribute("role")) === "menuitem", "focus did not enter the menu");
  await page.keyboard.press("Escape");
  assert(await page.$eval("#export-menu", (n) => n.hidden) && await page.evaluate(() => document.activeElement?.id) === "export-btn", "Esc did not close the menu");
  // Side gutter: no card touches the viewport edges (body clips overflow-x, so measure boxes).
  const edge = await page.$$eval("#results .panel, #results .detect, #results .results-head", (ns) => ns
    .filter((n) => n.checkVisibility()).filter((n) => { const r = n.getBoundingClientRect(); return r.left < 15 || r.right > window.innerWidth - 15; }).length);
  assert(edge === 0, `${viewport.width}px: ${edge} workspace boxes reach the viewport edge`);
  const overflowFile = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
  assert(overflowFile <= 1, `${viewport.width}px with a file: horizontal overflow of ${overflowFile}px`);
  if (offline) {
    await page.selectOption("#conv-type", "absorbance");
    await page.waitForSelector("#conv-plan .conv-note[role=alert]");
    const text = await page.textContent("#conv-plan");
    assert(/single-file build does not include nirs4all-methods/.test(text), text);
  }
  if (viewport.width >= 1000) {
    // Exercise per-record metadata, nested fields, long text and entries beyond the old display limit.
    await page.evaluate(() => {
      const provenance = { format: "fixture" };
      const records = [
        { signals: {}, provenance, metadata: { sample_id: "S1", instrument_model: "Spectrometer", scan_count: 3,
          vendor: { log: "a".repeat(240) + "<end>", channels: Array.from({ length: 103 }, (_, i) => i),
            ...Object.fromEntries(Array.from({ length: 8 }, (_, i) => [`setting_${i}`, `value_${i}`])) } }, targets: { protein: 12.1, moisture: 3.4 } },
        { signals: {}, provenance, metadata: { sample_id: "S2" }, targets: { protein: 16.2 } },
      ];
      renderDatasets([{ ok: true, name: "metadata-fixture", records, probes: [], size: 0, fileCount: 1 }]);
    });
    const metadata = page.getByRole("region", { name: "Metadata" });
    const headings = metadata.locator(".metadata-group > h4");
    assert((await headings.allTextContents()).join("|") === "Reference targets|Sample|Instrument|Acquisition|File and format", "metadata categories");
    assert(await metadata.getByText("S1", { exact: true }).isVisible() && await metadata.getByText("value_0", { exact: true }).isVisible(),
      "sample and nested vendor fields must be visible on load");
    const targetCells = await metadata.locator(".metadata-field").filter({ hasText: /protein|moisture/ })
      .evaluateAll((cells) => cells.map((cell) => ({ x: cell.offsetLeft, y: cell.offsetTop })));
    assert(targetCells.length === 2 && targetCells[0].x !== targetCells[1].x && targetCells[0].y === targetCells[1].y,
      "two reference targets should fit on the same row");
    await metadata.locator("summary").filter({ hasText: /^log/ }).click();
    await metadata.getByText("a".repeat(240) + "<end>", { exact: true }).waitFor();
    await metadata.locator("summary").filter({ hasText: "More fields" }).click();
    await metadata.getByText("value_7", { exact: true }).waitFor();
    await metadata.locator("summary").filter({ hasText: "channels" }).click();
    await metadata.locator("summary").filter({ hasText: "Entries 101–103" }).click();
    await metadata.locator(".metadata-field code").filter({ hasText: /^102$/ }).waitFor();
    await metadata.getByRole("button", { name: "Next metadata record" }).click();
    assert(await metadata.getByText("S2", { exact: true }).isVisible() && await metadata.getByText("16.2", { exact: true }).isVisible(), "second record metadata or targets missing");
  }
  await page.close();
}
await browser.close();
assert(errors.length === 0, `console errors:\n${errors.join("\n")}`);
console.log(`OK: Formats page smoke passed (${pageUrl})`);
