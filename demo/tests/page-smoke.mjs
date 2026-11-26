// Page-level check of the Formats site: library code tabs, the sortable
// catalogue, no horizontal overflow at phone width, and (in the single-file
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
  await page.waitForFunction(() => document.querySelectorAll("#soft-list li").length > 3);
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

  // A decoded file always shows its conversion panel.
  await page.setInputFiles("#file", path.join(demo, "..", "samples", "ocean_optics", "whiteref.ProcSpec"));
  await page.waitForSelector("#convert-panel #conv-plan .conv-status", { timeout: 20000 });
  if (offline) {
    await page.selectOption("#conv-target", "winisi");
    await page.click("#conv-run");
    await page.waitForSelector("#conv-result .legacy-note[role=alert]");
    const text = await page.textContent("#conv-result");
    assert(/single-file build does not include nirs4all-methods/.test(text), text);
  }
  await page.close();
}
await browser.close();
assert(errors.length === 0, `console errors:\n${errors.join("\n")}`);
console.log(`OK: Formats page smoke passed (${pageUrl})`);
