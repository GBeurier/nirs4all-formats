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

for (const viewport of [{ width: 1280, height: 900 }, { width: 390, height: 844 }]) {
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
  // The library frame holds only the tabs and the code; each tab names its install channel.
  assert(await page.$$eval("#library .fmt-head, #library h2", (ns) => ns.length) === 0, "library heading prose is still there");
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

  // A decoded file shows its properties, then the chart with the Convert panel beside it.
  await page.setInputFiles("#file", path.join(demo, "..", "samples", "ocean_optics", "whiteref.ProcSpec"));
  await page.waitForSelector("#convert-panel #conv-plan .conv-status", { timeout: 20000 });
  const layout = await page.evaluate(() => {
    window.scrollTo(0, 0); // the chart is sticky on desktop: measure before it sticks
    const top = (sel) => document.querySelector(sel).getBoundingClientRect();
    return { props: top("#signals-panel").top, plot: top(".plot-panel").top, plotLeft: top(".plot-panel").left, side: top("#convert-panel").top, sideLeft: top("#convert-panel").left };
  });
  assert(layout.props < layout.plot, "properties are not above the chart");
  if (viewport.width >= 1100) assert(layout.sideLeft > layout.plotLeft && Math.abs(layout.side - layout.plot) < 2, `side panel is not beside the chart: ${JSON.stringify(layout)}`);
  else assert(layout.side > layout.plot, "on a phone the side panel stacks under the chart");
  // Every control has a visible label and help text; selects are readable.
  const controls = await page.$$eval("#convert-panel select, #convert-panel input", (ns) => ns.map((n) => ({
    id: n.id, label: document.querySelector(`label[for="${n.id}"]`)?.textContent.trim() || "", font: parseFloat(getComputedStyle(n).fontSize),
  })));
  assert(controls.length >= 8 && controls.every((c) => c.label && c.font >= 13), `controls: ${JSON.stringify(controls)}`);
  assert(await page.textContent("#conv-run") === "Convert", "the button is not labelled Convert");
  const visible = await page.evaluate(() => document.body.innerText);
  assert(!/with nirs4all-methods|plan: nirs4all-formats/.test(visible), "implementation wording is visible");
  // Top Export menu: three maturity groups, keyboard reachable, Esc closes.
  await page.focus("#export-btn");
  await page.keyboard.press("Enter");
  const heads = await page.$$eval("#export-menu .menu-head", (ns) => ns.map((n) => n.firstChild.textContent.trim()));
  assert(heads.join("|") === "Standard formats|Experimental|Other formats", heads.join("|"));
  assert(await page.evaluate(() => document.activeElement?.getAttribute("role")) === "menuitem", "focus did not enter the menu");
  await page.keyboard.press("Escape");
  assert(await page.$eval("#export-menu", (n) => n.hidden) && await page.evaluate(() => document.activeElement?.id) === "export-btn", "Esc did not close the menu");
  // Side gutter: no card touches the viewport edges (body clips overflow-x, so measure boxes).
  const edge = await page.$$eval("#results .panel, #results .detect, #results .kpi, #results .results-head", (ns) => ns
    .filter((n) => { const r = n.getBoundingClientRect(); return r.left < 15 || r.right > window.innerWidth - 15; }).length);
  assert(edge === 0, `${viewport.width}px: ${edge} workspace boxes reach the viewport edge`);
  const overflowFile = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
  assert(overflowFile <= 1, `${viewport.width}px with a file: horizontal overflow of ${overflowFile}px`);
  if (offline) {
    await page.selectOption("#conv-target", "winisi");
    await page.click("#conv-run");
    await page.waitForSelector("#conv-result .conv-note[role=alert]");
    const text = await page.textContent("#conv-result");
    assert(/single-file build does not include nirs4all-methods/.test(text), text);
  }
  await page.close();
}
await browser.close();
assert(errors.length === 0, `console errors:\n${errors.join("\n")}`);
console.log(`OK: Formats page smoke passed (${pageUrl})`);
