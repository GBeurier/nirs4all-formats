// nirs4all-formats — in-browser demo.
//
// The Rust reader registry is compiled to WebAssembly (`./pkg/`). This module
// only drives the UI: it reads dropped files into byte buffers, calls the WASM
// `probeBytes` / `openBytes` / `openWithSidecars` entry points, and renders the
// returned `SpectralRecord[]`. No format parsing happens here — that is the
// whole point of the library (parsers live only in Rust).

/* ── WASM access (dual-mode) ───────────────────────────────────────────
   This page runs two ways:
   • Served over http(s): the wasm-pack `--target web` ES module in ./pkg/ is
     imported lazily, and samples are fetched from ./samples/.
   • Opened directly from a file:// path (the standalone single-file build):
     the wasm, the no-modules glue and the samples are all embedded in
     `window.__N4F_EMBED__`, so there is NO module import and NO fetch — both
     of which browsers block on file://.
   `WASM` ends up holding {version, features, probeBytes, openBytes,
   openWithSidecars, sidecarRequirements} either way. */
let WASM = null;
const EMBED = (typeof window !== "undefined") ? window.__N4F_EMBED__ : null;

function b64ToBytes(b64) {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

async function initWasm() {
  if (EMBED) {
    // no-modules build: global `wasm_bindgen` init fn; instantiate from the
    // embedded bytes directly (no fetch). Exports hang off the function.
    await wasm_bindgen(b64ToBytes(EMBED.wasm)); // eslint-disable-line no-undef
    WASM = wasm_bindgen; // eslint-disable-line no-undef
  } else {
    const mod = await import("./pkg/nirs4all_formats_wasm.js");
    await mod.default();
    WASM = mod;
  }
}

async function getManifest() {
  if (EMBED) return EMBED.manifest;
  return (await fetch("./samples/manifest.json")).json();
}

async function sampleFile(name) {
  const logicalName = String(name);
  const fileName = fileBaseName(logicalName);
  if (EMBED) {
    const b64 = EMBED.samples[logicalName];
    if (!b64) throw new Error(`unknown sample: ${logicalName}`);
    const file = new File([b64ToBytes(b64)], fileName);
    file.n4fName = logicalName;
    return file;
  }
  const buf = await (await fetch(`./samples/${logicalName}`)).arrayBuffer();
  const file = new File([buf], fileName);
  file.n4fName = logicalName;
  return file;
}

async function sampleFiles(sample) {
  const entry = typeof sample === "string" ? { file: sample } : sample;
  const names = [entry.file, ...(entry.sidecars || [])];
  return Promise.all(names.map(sampleFile));
}

async function getFormats() {
  if (EMBED) return EMBED.formats || null;
  try { return await (await fetch("./formats.json")).json(); }
  catch (_) { return null; }
}

/* ── palette ─────────────────────────────────────────────────────────── */
const PALETTE = ["#0d9488", "#06b6d4", "#4f46e5", "#d97706", "#10b981", "#0891b2", "#7c3aed", "#e11d48"];
const SIG_COLORS = {
  absorbance: "#0d9488", reflectance: "#06b6d4", transmittance: "#0891b2",
  raw_counts: "#d97706", raw: "#d97706", single_beam: "#4f46e5",
  kubelka_munk: "#7c3aed", radiance: "#10b981", interferogram: "#e11d48",
  derivative: "#4338ca", preprocessed: "#0f766e", unknown: "#64748b",
};
const sigColor = (t, i = 0) => SIG_COLORS[t] || PALETTE[i % PALETTE.length];

/* ── tiny helpers ────────────────────────────────────────────────────── */
const $ = (sel, root = document) => root.querySelector(sel);
const el = (tag, cls, html) => { const n = document.createElement(tag); if (cls) n.className = cls; if (html != null) n.innerHTML = html; return n; };
const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
const setStatus = (m) => { const s = document.getElementById("status"); if (s) s.textContent = m; };
const confRank = { definite: 3, likely: 2, possible: 1, no: 0 };

const UNIT = { "cm-1": "cm⁻¹", um: "µm", nm: "nm", thz: "THz", index: "index", hz: "Hz", ev: "eV", s: "s" };
const prettyUnit = (u) => (u ? (UNIT[String(u).toLowerCase()] || u) : "");
const KIND = { wavelength: "Wavelength", wavenumber: "Wavenumber", frequency: "Frequency", energy: "Energy", time: "Time", index: "Index" };
const titleCase = (s) => String(s || "").replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
const SIG_LABEL = { kubelka_munk: "Kubelka–Munk", raw_counts: "Raw counts", single_beam: "Single-beam", raw_dn: "Raw DN" };
const sigLabel = (t) => SIG_LABEL[t] || titleCase(t);
const REVERSED_KIND = new Set(["wavenumber", "frequency", "energy"]);

function fmtNum(v, sig = 4) {
  if (!Number.isFinite(v)) return "—";
  const a = Math.abs(v);
  if (a !== 0 && (a < 1e-3 || a >= 1e6)) return v.toExponential(2);
  let s = Number(v.toPrecision(sig)).toString();
  return s;
}
const fmtInt = (n) => n.toLocaleString("en-US");

async function copy(text, btn) {
  try {
    await navigator.clipboard.writeText(text);
    if (btn) { const o = btn.innerHTML; btn.innerHTML = "✓"; setTimeout(() => (btn.innerHTML = o), 1100); }
  } catch (_) { /* clipboard may be blocked on file:// */ }
}

/* ── boot ────────────────────────────────────────────────────────────── */
const drop = $("#drop"), fileInput = $("#file"), results = $("#results");

// `run()` awaits this so a file dropped before the (multi-MB) wasm finishes
// loading is still decoded once it's ready — instead of being dropped silently.
let wasmReady = null;

(function boot() {
  // Arm the dropzone and the page-wide drag guards FIRST — synchronously, before
  // the wasm load — so a drop is never missed and (critically on file://) never
  // navigates the browser away to the dropped file.
  wireDropzone();
  initDropWave();
  loadSamples();
  renderFormats();
  initCodeTabs();

  wasmReady = initWasm();
  wasmReady.then(() => {
    try { $("#stat-version").textContent = WASM.version(); } catch (_) {}
    try { const f = WASM.features(); $("#stat-hdf5").style.display = f && f.hdf5 ? "" : "none"; } catch (_) {}
    renderConvertCatalog();
    maybeAutoLoad();
  }).catch((e) => {
    results.classList.add("show");
    results.innerHTML = statusError("WebAssembly failed to load", String(e),
      EMBED ? "" : "Open <code>demo/nirs4all-formats-demo.html</code> instead: it embeds the WASM and samples, so no server is needed.");
  });
})();

// Deep link: `…/#sample=PE1800.DX` auto-loads a bundled sample on open,
// so a particular decode can be linked or embedded directly.
async function maybeAutoLoad() {
  const m = /(?:^|[#&])sample=([^&]+)/.exec(location.hash);
  if (!m) return;
  const file = decodeURIComponent(m[1]);
  try {
    const man = await getManifest();
    const sample = man.samples.find((s) => s.file === file) || { file };
    run(await sampleFiles(sample));
  } catch (_) {
    try { run([await sampleFile(file)]); } catch (_) { /* unknown sample name — ignore */ }
  }
}

/* ── dropzone wiring ─────────────────────────────────────────────────── */
function wireDropzone() {
  $("#pick").addEventListener("click", (e) => { e.stopPropagation(); fileInput.click(); });
  drop.addEventListener("click", () => fileInput.click());
  drop.addEventListener("keydown", (e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); fileInput.click(); } });
  fileInput.addEventListener("change", () => { if (fileInput.files.length) run([...fileInput.files]); });

  const hasFiles = (e) => {
    const t = e.dataTransfer && e.dataTransfer.types;
    return !!t && Array.prototype.indexOf.call(t, "Files") !== -1;
  };

  // Drop ANYWHERE on the page. The whole window is the drop target (a veil
  // shows while dragging), so the user never has to aim at the small zone — and
  // because these guards are armed at boot (before the wasm finishes loading),
  // a dropped file is never missed nor allowed to navigate the page away.
  let depth = 0;
  window.addEventListener("dragenter", (e) => { if (!hasFiles(e)) return; e.preventDefault(); if (depth++ === 0) document.body.classList.add("dragging"); });
  window.addEventListener("dragover", (e) => { if (hasFiles(e)) e.preventDefault(); });
  window.addEventListener("dragleave", (e) => { if (!hasFiles(e)) return; if (--depth <= 0) { depth = 0; document.body.classList.remove("dragging"); } });
  window.addEventListener("drop", (e) => {
    if (!hasFiles(e)) return;
    e.preventDefault();
    depth = 0; document.body.classList.remove("dragging");
    const fs = [...(e.dataTransfer.files || [])];
    if (fs.length) run(fs);
  });

  // extra highlight on the zone itself when the cursor is over it
  drop.addEventListener("dragenter", () => drop.classList.add("drag"));
  drop.addEventListener("dragover", () => drop.classList.add("drag"));
  drop.addEventListener("dragleave", (e) => { if (!drop.contains(e.relatedTarget)) drop.classList.remove("drag"); });
  drop.addEventListener("drop", () => drop.classList.remove("drag"));
}

/* ── drop-zone spectral wave ──────────────────────────────────────────
   Ported from the nirs4all.org hero animation (overlapping Math.sin spectral
   curves with short-lived dots that ride them and dotted cross-wave
   connectors), scaled down to the drop-zone band and the demo's teal/cyan
   palette. Pure DOM/SVG — runs in both the served and standalone builds. */
function initDropWave() {
  const svg = document.querySelector(".drop-wave");
  const dotsGroup = document.getElementById("dw-dots");
  const lines = [0, 1, 2].map((i) => document.getElementById("dw-line" + i));
  const areas = [0, 1, 2].map((i) => document.getElementById("dw-area" + i));
  if (!svg || !dotsGroup || !lines[0]) return;

  const W = 600, H = 160, STEP = 4;
  const waves = [
    { baseY: 100, amp: 34, freq: 0.0125, phase: 0,   speed: 0.00034, color: "#0f766e", dotColor: "rgba(15,118,110,0.95)" },
    { baseY: 116, amp: 26, freq: 0.0108, phase: 2.1, speed: 0.00027, color: "#0891b2", dotColor: "rgba(8,145,178,0.85)" },
    { baseY: 86,  amp: 20, freq: 0.0150, phase: 4.2, speed: 0.00021, color: "#059669", dotColor: "rgba(5,150,105,0.75)" },
  ];

  const getY = (wave, x, now) =>
    wave.baseY
    + Math.sin(x * wave.freq + wave.phase + now * wave.speed) * wave.amp
    + Math.sin(x * wave.freq * 1.7 + wave.phase * 0.6 + now * wave.speed * 0.7) * wave.amp * 0.2;

  const drawCurves = (now) => {
    waves.forEach((wave, i) => {
      let d = "";
      for (let x = 0; x <= W; x += STEP) d += (x === 0 ? "M" : "L") + x.toFixed(1) + "," + getY(wave, x, now).toFixed(1);
      lines[i].setAttribute("d", d);
      areas[i].setAttribute("d", d + " L" + W + "," + H + " L0," + H + " Z");
    });
  };

  // Respect reduced-motion: render one static frame, no loop, no dots.
  if (window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
    drawCurves(0);
    return;
  }

  // Dots ride the curves; ellipses counter preserveAspectRatio="none" scaling.
  const MAX_DOTS = 9, DOT_SPAWN_INTERVAL = 360, MAX_CONN_DX = 95;
  const dots = [];
  let lastSpawn = 0, invSx = 1, invSy = 1;
  const updateScale = () => {
    const r = svg.getBoundingClientRect();
    if (r.width > 0 && r.height > 0) { invSx = W / r.width; invSy = H / r.height; }
  };
  updateScale();
  window.addEventListener("resize", updateScale);

  const spawnDot = (now) => {
    if (dots.length >= MAX_DOTS) return;
    const waveIdx = Math.floor(Math.random() * waves.length);
    const wave = waves[waveIdx];
    const x = 30 + Math.random() * (W - 60);
    const el = document.createElementNS("http://www.w3.org/2000/svg", "ellipse");
    el.setAttribute("cx", x.toFixed(1));
    el.setAttribute("cy", getY(wave, x, now).toFixed(1));
    el.setAttribute("rx", (2.6 * invSx).toFixed(2));
    el.setAttribute("ry", (2.6 * invSy).toFixed(2));
    el.setAttribute("fill", wave.dotColor);
    el.setAttribute("opacity", "0");
    el.classList.add("wave-dot");
    el.style.color = wave.color;
    dotsGroup.appendChild(el);
    dots.push({ el, waveIdx, x, born: now, lifetime: 2200 + Math.random() * 800, connectors: [] });
  };

  // Connectors link dots living on DIFFERENT waves within a proximity window,
  // so the dotted traces read as cross-spectral correlations.
  const updateConnectors = (now) => {
    dots.forEach((d) => { d.connectors.forEach((c) => c.remove()); d.connectors = []; });
    const pos = dots.map((d) => ({ d, op: parseFloat(d.el.getAttribute("opacity") || "0"), y: getY(waves[d.waveIdx], d.x, now) }));
    const seen = new Set();
    for (let i = 0; i < pos.length; i++) {
      const a = pos[i];
      if (a.op < 0.18) continue;
      let best = null;
      for (let j = 0; j < pos.length; j++) {
        if (i === j) continue;
        const b = pos[j];
        if (b.d.waveIdx === a.d.waveIdx || b.op < 0.18) continue;
        const dx = Math.abs(a.d.x - b.d.x);
        if (dx > MAX_CONN_DX) continue;
        if (!best || dx < best.dx) best = { b, j, dx };
      }
      if (!best) continue;
      const key = i < best.j ? i + ":" + best.j : best.j + ":" + i;
      if (seen.has(key)) continue;
      seen.add(key);
      const b = best.b;
      const op = Math.min(a.op, b.op) * 0.55 * (0.35 + 0.65 * (1 - best.dx / MAX_CONN_DX));
      if (op < 0.03) continue;
      const line = document.createElementNS("http://www.w3.org/2000/svg", "path");
      line.setAttribute("d", "M" + a.d.x.toFixed(1) + "," + a.y.toFixed(1) + " L" + b.d.x.toFixed(1) + "," + b.y.toFixed(1));
      line.setAttribute("stroke", waves[a.d.waveIdx].color);
      line.setAttribute("stroke-width", "0.75");
      line.setAttribute("opacity", op.toFixed(3));
      line.classList.add("wave-connector");
      dotsGroup.insertBefore(line, dotsGroup.firstChild);
      a.d.connectors.push(line);
    }
  };

  const updateDots = (now) => {
    for (let i = dots.length - 1; i >= 0; i--) {
      const dot = dots[i];
      const progress = (now - dot.born) / dot.lifetime;
      if (progress >= 1) { dot.el.remove(); dot.connectors.forEach((c) => c.remove()); dots.splice(i, 1); continue; }
      dot.el.setAttribute("cy", getY(waves[dot.waveIdx], dot.x, now).toFixed(1));
      let op;
      if (progress < 0.15) op = progress / 0.15;
      else if (progress < 0.7) op = 1;
      else op = 1 - (progress - 0.7) / 0.3;
      dot.el.setAttribute("opacity", (op * 0.9).toFixed(3));
      const r = 2.1 + op * 1.3;
      dot.el.setAttribute("rx", (r * invSx).toFixed(2));
      dot.el.setAttribute("ry", (r * invSy).toFixed(2));
    }
  };

  let frameId = 0;
  const animate = (now) => {
    drawCurves(now);
    if (now - lastSpawn > DOT_SPAWN_INTERVAL) { spawnDot(now); lastSpawn = now; }
    updateDots(now);
    updateConnectors(now);
    frameId = requestAnimationFrame(animate);
  };
  frameId = requestAnimationFrame(animate);
  window.addEventListener("pagehide", () => cancelAnimationFrame(frameId), { once: true });
}

async function loadSamples() {
  const host = $("#samples");
  try {
    const man = await getManifest();
    for (const s of man.samples) {
      const chip = el("button", `s-chip acc-${s.accent || "teal"}`);
      chip.type = "button";
      chip.innerHTML = `<span class="dot"></span>${esc(s.label)}`;
      chip.title = `${s.format} — ${s.blurb}`;
      chip.addEventListener("click", async (e) => {
        e.stopPropagation();
        try {
          run(await sampleFiles(s));
        } catch (err) {
          showResult(statusError("Could not load sample", String(err)));
        }
      });
      host.appendChild(chip);
    }
  } catch (_) {
    host.innerHTML = `<span class="leg-note">Bundled samples unavailable here. Open the standalone HTML to use them without a server.</span>`;
  }
}

/* ── formats coverage table ──────────────────────────────────────────── */
const FMT_META = {
  supported: { label: "Supported", cls: "st-supported", dot: "var(--green)",  desc: "Fully supported: these files decode reliably, checked against trusted reference readers." },
  targeted:  { label: "Scoped",    cls: "st-targeted",  dot: "var(--cyan)",   desc: "Supported for this format's common variants; rarer or exotic files of it may not decode." },
  partial:   { label: "Partial",   cls: "st-partial",   dot: "var(--amber)",  desc: "Partly supported: the file opens, but some variants or columns aren't decoded yet." },
  adjacent:  { label: "Adjacent",  cls: "st-adjacent",  dot: "var(--indigo)", desc: "A neighbouring, non-NIRS format (Raman, mass-spec, AFM): recognised and partly read, but not the focus here." },
  sourcing:  { label: "Sourcing",  cls: "st-sourcing",  dot: "var(--text-3)", desc: "Not supported yet: building the reader needs real sample files or the format spec." },
  outscope:  { label: "Out of scope", cls: "st-outscope", dot: "#cbd5e1",     desc: "Intentionally not supported: outside what this library aims to read." },
};
const FMT_ORDER = ["supported", "targeted", "partial", "adjacent", "sourcing", "outscope"];
let FORMATS = null;
const fmtFilter = { q: "", status: new Set() };
// Column sort: null keeps the matrix order; dir 1 ascending, -1 descending.
const fmtSort = { key: null, dir: 1 };
const fmtCoverage = (f) => {
  const total = f.variants || (f.ok + f.partial + f.planned + f.blocked) || 1;
  return f.ok / total;
};
const FMT_SORTS = {
  name: (a, b) => a.name.localeCompare(b.name, "en", { sensitivity: "base" }),
  coverage: (a, b) => fmtCoverage(a) - fmtCoverage(b) || a.ok - b.ok,
  status: (a, b) => FMT_ORDER.indexOf(a.status) - FMT_ORDER.indexOf(b.status),
};
const SORT_LABEL = { name: "format name", coverage: "coverage", status: "status" };

function wireFormatSort() {
  document.querySelectorAll(".fmt-sort").forEach((button) => button.addEventListener("click", () => {
    const key = button.dataset.sort;
    fmtSort.dir = fmtSort.key === key ? -fmtSort.dir : 1;
    fmtSort.key = key;
    document.querySelectorAll(".fmt-table th[aria-sort]").forEach((th) => {
      const own = th.querySelector(".fmt-sort").dataset.sort === key;
      th.setAttribute("aria-sort", own ? (fmtSort.dir === 1 ? "ascending" : "descending") : "none");
      th.querySelector(".arr").textContent = own ? (fmtSort.dir === 1 ? "↑" : "↓") : "↕";
    });
    drawFormatRows();
    setStatus(`Formats sorted by ${SORT_LABEL[key]}, ${fmtSort.dir === 1 ? "ascending" : "descending"}.`);
  }));
}

async function renderFormats() {
  const section = $("#formats");
  const data = await getFormats();
  if (!data || !Array.isArray(data.formats) || !data.formats.length) { if (section) section.style.display = "none"; return; }
  FORMATS = data.formats;
  const s = data.summary || {};

  const stats = [
    { n: s.families ?? FORMATS.length, l: "format families", a: "var(--teal)" },
    { n: s.validated_variants ?? "—", l: "validated variants", c: "c-ok", a: "var(--teal-d)" },
    { n: s.supported ?? "—", l: "fully supported", c: "c-ok", a: "var(--teal-d)" },
    { n: s.partial_variants ?? "—", l: "partial variants", c: "c-part", a: "var(--amber)" },
    { n: s.sourcing ?? 0, l: "need samples", c: "c-src", a: "var(--text-3)" },
  ];
  $("#fmt-stats").innerHTML = stats.map((x) => `<div class="fmt-stat" style="--accent:${x.a}"><div class="n ${x.c || ""}">${esc(x.n)}</div><div class="l">${esc(x.l)}</div></div>`).join("");

  const counts = {};
  for (const f of FORMATS) counts[f.status] = (counts[f.status] || 0) + 1;
  const legend = $("#fmt-legend");
  legend.innerHTML = FMT_ORDER.filter((k) => counts[k]).map((k) =>
    `<button class="fmt-fl" type="button" data-st="${k}" title="${esc(FMT_META[k].desc + " · click to filter")}"><span class="d" style="background:${FMT_META[k].dot}"></span>${esc(FMT_META[k].label)} <span style="color:var(--text-3)">${counts[k]}</span></button>`).join("");
  legend.querySelectorAll(".fmt-fl").forEach((b) => b.addEventListener("click", () => {
    const st = b.dataset.st;
    if (fmtFilter.status.has(st)) fmtFilter.status.delete(st); else fmtFilter.status.add(st);
    drawFormatRows();
  }));
  $("#fmt-search").addEventListener("input", (e) => { fmtFilter.q = e.target.value.toLowerCase().trim(); drawFormatRows(); });
  wireFormatSort();
  drawFormatRows();
}

function drawFormatRows() {
  const body = $("#fmt-body"); if (!body) return;
  const { q, status } = fmtFilter;
  $("#fmt-legend").querySelectorAll(".fmt-fl").forEach((b) => b.classList.toggle("off", status.size > 0 && !status.has(b.dataset.st)));
  const rows = FORMATS.filter((f) => {
    if (status.size && !status.has(f.status)) return false;
    if (!q) return true;
    return `${f.name} ${f.vendor} ${f.ext} ${FMT_META[f.status].label}`.toLowerCase().includes(q);
  });
  if (fmtSort.key) {
    const compare = FMT_SORTS[fmtSort.key];
    // Stable: equal rows keep the matrix order.
    rows.sort((a, b) => fmtSort.dir * compare(a, b) || FORMATS.indexOf(a) - FORMATS.indexOf(b));
  }
  body.innerHTML = rows.length
    ? rows.map(formatRow).join("")
    : `<tr><td colspan="4" class="fmt-empty">No format matches “${esc(q)}”.</td></tr>`;
  $("#fmt-foot").textContent = `Showing ${rows.length} of ${FORMATS.length} families · generated from the project's FORMAT_MATRIX.md`;
}

function formatRow(f) {
  const m = FMT_META[f.status] || FMT_META.sourcing;
  const total = f.variants || (f.ok + f.partial + f.planned + f.blocked) || 1;
  // Bar shows what works: validated (teal) + partial (amber); everything else
  // (planned/blocked) is left as the empty track so a 0-validated row reads as
  // "nothing yet" instead of an arbitrary mix of colours.
  const rest = Math.max(0, total - f.ok - f.partial);
  const seg = (n, col) => (n > 0 ? `<span style="flex:${n};background:${col}"></span>` : "");
  const covTip = `${f.ok} of ${total} variants decode reliably`
    + (f.partial ? ` · ${f.partial} partial` : "")
    + (rest ? ` · ${rest} not yet` : "");
  const parts = f.ext.split(",").map((e) => e.trim()).filter(Boolean);
  const exts = parts.slice(0, 5).map((e) => `<code>${esc(e)}</code>`).join("") + (parts.length > 5 ? `<code>+${parts.length - 5}</code>` : "");
  return `<tr style="--rc:${m.dot}">
    <td><div class="f-name">${esc(f.name)}</div><div class="f-vendor">${esc(f.vendor)}</div></td>
    <td><div class="f-ext">${exts}</div></td>
    <td><div class="cov" title="${esc(covTip)}">
      <div class="cov-bar">${seg(f.ok, "var(--teal)")}${seg(f.partial, "var(--amber)")}${seg(rest, "transparent")}</div>
      <span class="cov-num"><b>${f.ok}</b>/${total}</span></div></td>
    <td><span class="st ${m.cls}" title="${esc(m.desc)}"><span class="d"></span>${esc(m.label)}</span></td>
  </tr>`;
}

/* ── library code tabs ───────────────────────────────────────────────── */
function initCodeTabs() {
  const tabs = [...document.querySelectorAll(".code-tab")];
  const select = (tab, focus) => {
    for (const other of tabs) {
      const on = other === tab;
      other.setAttribute("aria-selected", String(on));
      other.tabIndex = on ? 0 : -1;
      document.getElementById(other.getAttribute("aria-controls")).hidden = !on;
    }
    if (focus) tab.focus();
  };
  tabs.forEach((tab, index) => {
    tab.addEventListener("click", () => select(tab, false));
    tab.addEventListener("keydown", (event) => {
      const next = { ArrowRight: index + 1, ArrowLeft: index - 1, Home: 0, End: tabs.length - 1 }[event.key];
      if (next === undefined) return;
      event.preventDefault();
      select(tabs[(next + tabs.length) % tabs.length], true);
    });
  });
  document.querySelectorAll(".code-block pre[data-lang]").forEach(highlightCode);
  document.querySelectorAll(".code-block .copy-btn").forEach((button) => {
    button.addEventListener("click", () => copy(button.parentElement.querySelector("code").textContent, button));
  });
}

function highlightCode(pre) {
  const marker = ["rust", "js", "c", "java", "csharp"].includes(pre.dataset.lang) ? "//" : "#";
  const code = pre.querySelector("code");
  code.innerHTML = code.textContent.split("\n").map((line) => {
    let out = "", quoted = false;
    for (let i = 0; i < line.length; i++) {
      if (!quoted && line.startsWith(marker, i)) return `${out}<span class="c">${esc(line.slice(i))}</span>`;
      if (line[i] === '"') { out += quoted ? '"</span>' : '<span class="s">"'; quoted = !quoted; continue; }
      out += esc(line[i]);
    }
    return quoted ? `${out}</span>` : out;
  }).join("\n");
}

// The conversion section lists the software presets of the loaded writers.
function renderConvertCatalog() {
  const limits = $("#limit-list");
  if (limits) limits.innerHTML = CONVERT_LIMITS.map((l) => `<li>${esc(l)}</li>`).join("");
  const list = $("#soft-list");
  if (!list) return;
  try {
    const targets = WASM.exportTargets();
    list.innerHTML = targets.software.map((p) => `<li>${esc(p.label)} <code>${esc(p.format)}</code></li>`).join("");
    $("#soft-count").textContent = `${targets.software.length} software · ${targets.formats.length} formats`;
  } catch (e) {
    list.innerHTML = `<li class="mono-sub">${esc(String(e?.message || e))}</li>`;
  }
}

/* ── decode pipeline ─────────────────────────────────────────────────── */
async function run(files) {
  drop.classList.add("busy");
  const names = files.map((f) => f.name).join(", ");
  setStatus(`Decoding ${names}…`);
  showResult(statusLoading(names));
  // let the spinner paint before the (synchronous) WASM decode (with a timeout
  // fallback so it never hangs if rAF doesn't fire, e.g. a backgrounded tab)
  await new Promise((r) => { requestAnimationFrame(() => requestAnimationFrame(r)); setTimeout(r, 80); });

  try {
    // a file may be dropped before the wasm module has finished loading
    await wasmReady;
    const bufs = await Promise.all(files.map(async (f) => ({ name: fileLogicalName(f), bytes: new Uint8Array(await f.arrayBuffer()), size: f.size })));

    // Partition the dropped files into datasets: sidecar sets (ENVI .img+.hdr,
    // FGI .xml+.h5, …) are grouped; everything else is an independent dataset.
    const groups = groupFiles(bufs);
    const datasets = groups.map((g) => {
      const size = g.reduce((a, b) => a + (b.size || b.bytes.length), 0);
      try {
        const { probes, records, primaryName } = decodeGroup(g);
        return { ok: true, name: primaryName, size, probes, records, fileCount: g.length };
      } catch (e) {
        return { ok: false, name: g[0].name, size, error: String(e?.message || e), fileCount: g.length };
      }
    });
    renderDatasets(datasets);
  } catch (e) {
    const msg = String(e?.message || e);
    showResult(statusError("Could not decode", msg.length ? msg : "No registered reader recognised this file.", hintFor(msg)));
  } finally {
    drop.classList.remove("busy");
  }
}

/* split a dropped file list into datasets, grouping sidecar sets by stem */
const fileBaseName = (n) => String(n || "").replace(/^.*[\\/]/, "");
const fileLogicalName = (f) => f.n4fName || f.webkitRelativePath || f.name;
const stemOf = (n) => { const b = fileBaseName(n); const d = b.lastIndexOf("."); return (d > 0 ? b.slice(0, d) : b).toLowerCase(); };
const extOf = (n) => { const m = /\.([^.\\/]+)$/.exec(n); return m ? m[1].toLowerCase() : ""; };
const dirOf = (n) => {
  const s = String(n || "").replace(/\\/g, "/");
  const i = s.lastIndexOf("/");
  return i >= 0 ? s.slice(0, i + 1) : "";
};
function sidecarKeyAliases(name, primaryName) {
  const raw = String(name || "");
  const primaryDir = dirOf(primaryName);
  const rel = primaryDir && raw.replace(/\\/g, "/").startsWith(primaryDir)
    ? raw.replace(/\\/g, "/").slice(primaryDir.length)
    : raw;
  // Preserve real spelling. Fabricated extension-case aliases can look like
  // conflicting physical companions to readers that reject ambiguity.
  return [...new Set([raw, rel, fileBaseName(raw)])].filter(Boolean);
}
function declaredCompanions(primary, files) {
  let requirements = [];
  try { requirements = WASM.sidecarRequirements(primary.name, primary.bytes); } catch (_) {}
  const names = new Set();
  for (const requirement of requirements || []) {
    for (const name of [requirement.path, ...(requirement.alternatives || [])]) {
      // Declarations are relative to the primary directory. Do not replace a
      // reference to another directory with a same-basename file beside it.
      const relative = String(name || "").replace(/\\/g, "/").replace(/^\.\//, "");
      if (relative && !dirOf(relative)) names.add(relative.toLowerCase());
    }
  }
  return files.filter((other) => other !== primary
    && dirOf(other.name) === dirOf(primary.name)
    && names.has(fileBaseName(other.name).toLowerCase()));
}
// Sidecar lookup is exact. When no spelling a reader declares was supplied and
// exactly one companion differs only by case, expose it under the declared name.
function addDeclaredSpellings(side, primary) {
  let requirements = [];
  try { requirements = WASM.sidecarRequirements(primary.name, primary.bytes); } catch (_) {}
  const supplied = Object.keys(side);
  for (const requirement of requirements || []) {
    const declared = [requirement.path, ...(requirement.alternatives || [])].map(String);
    if (declared.some((name) => Object.hasOwn(side, name))) continue;
    for (const name of declared) {
      const matches = supplied.filter((key) => key.toLowerCase() === name.toLowerCase());
      if (matches.length === 1) { side[name] = side[matches[0]]; break; }
    }
  }
}
function looksLikeSidecarSet(exts) {
  const s = new Set(exts);
  if (s.has("hdr")) return true;                                       // ENVI standard / ENVI SLI
  if (s.has("xml") && (s.has("h5") || s.has("hdf5"))) return true;      // FGI XML+HDF5
  if ((s.has("yaml") || s.has("yml")) && (s.has("nc") || s.has("cdf"))) return true; // NetCDF MFRSR + QC
  if (s.has("lan") || s.has("gis")) return true;                       // AVIRIS / ERDAS LAN
  return false;
}
function groupFiles(files) {
  const used = new Set();
  const groups = [];

  // Rust owns companion declarations, including optional files. Keep every
  // matching real spelling so ambiguous companions reach the native reader.
  for (const primary of files) {
    if (used.has(primary)) continue;
    const companions = declaredCompanions(primary, files).filter((file) => !used.has(file));
    if (!companions.length) continue;
    const group = [primary, ...companions];
    group.forEach((file) => used.add(file));
    groups.push(group);
  }

  // ERDAS LAN / AVIRIS uses a same-stem wavelength sidecar
  // (`92AV3C.spc`) plus, for the classic fixture, a different-stem
  // ground-truth sidecar (`92AV3GT.GIS`).
  for (const f of files) {
    if (used.has(f) || extOf(f.name) !== "lan") continue;
    const stem = stemOf(f.name);
    const g = [f];
    used.add(f);
    for (const other of files) {
      if (used.has(other) || dirOf(other.name) !== dirOf(f.name)) continue;
      const ext = extOf(other.name);
      if ((ext === "spc" && stemOf(other.name) === stem) || ext === "gis") {
        g.push(other);
        used.add(other);
      }
    }
    groups.push(g);
  }

  const byStem = new Map();
  for (const f of files) {
    if (used.has(f)) continue;
    const k = JSON.stringify([dirOf(f.name), stemOf(f.name)]);
    if (!byStem.has(k)) byStem.set(k, []);
    byStem.get(k).push(f);
  }
  for (const g of byStem.values()) {
    if (g.length > 1 && looksLikeSidecarSet(g.map((f) => extOf(f.name)))) groups.push(g);
    else for (const f of g) groups.push([f]); // independent files → separate datasets
  }
  return groups;
}

/* decode one group (single file, or a primary + sidecars) */
function decodeGroup(bufs) {
  let primary = bufs[0], probes = [];
  if (bufs.length > 1) {
    let best = -2;
    for (const b of bufs) {
      let p = [];
      try { p = WASM.probeBytes(b.name, b.bytes); } catch (_) {}
      // A declared primary outranks a companion that happens to probe as a
      // readable standalone container (for example an HDF5 payload).
      const score = (declaredCompanions(b, bufs).length ? 10 : 0)
        + (p.length ? (confRank[p[0].confidence] ?? 0) : -1);
      if (score > best) { best = score; primary = b; probes = p; }
    }
  } else {
    try { probes = WASM.probeBytes(primary.name, primary.bytes); } catch (_) {}
  }
  let records;
  if (bufs.length > 1) {
    const side = {};
    for (const b of bufs) {
      if (b === primary) continue;
      for (const key of sidecarKeyAliases(b.name, primary.name)) side[key] = b.bytes;
    }
    addDeclaredSpellings(side, primary);
    // Optional companions can supply targets even when the primary alone is
    // readable. Pass them on the first attempt; never discard them on success.
    records = WASM.openWithSidecars(primary.name, primary.bytes, side);
  } else {
    records = WASM.openBytes(primary.name, primary.bytes);
  }
  if (!Array.isArray(records) || records.length === 0) throw new Error("The reader returned an empty result.");
  return { probes, records, primaryName: primary.name };
}

function hintFor(msg) {
  if (/92AV3C\.spc|ERDAS LAN|AVIRIS/i.test(msg))
    return "This AVIRIS/ERDAS LAN cube needs its companions. Drop <code>92AV3C.lan</code> together with <code>92AV3C.spc</code>; add <code>92AV3GT.GIS</code> if you want ground-truth labels.";
  if (/sidecar|missing ENVI binary|companion|next to/i.test(msg))
    return "This is a <b>multi-file format</b>. Drop the data file together with its sidecar(s) — e.g. ENVI <code>.img</code> + <code>.hdr</code>, or FGI <code>.xml</code> + <code>.h5</code>.";
  if (/parquet.*error/i.test(msg))
    return "This Parquet file could not be decoded. Uncompressed, snappy and <b>zstd</b> Parquet all decode in the browser; the rarer gzip/brotli/lz4 codecs don't yet — re-save with snappy or zstd, or use the Python/CLI build.";
  if (/not NIRS|not spectroscopy|no supported axis|no spectra|no numeric spectral|contains no/i.test(msg))
    return "No spectral data was found — this file looks like metadata, a non-NIRS format, or a report rather than spectra.";
  if (/unsupported|no reader|not recognised|unrecognized/i.test(msg))
    return "The file did not match any registered reader. See the <a href='https://github.com/GBeurier/nirs4all-formats/blob/main/docs/FORMAT_MATRIX.md'>format matrix</a>.";
  return "";
}

/* ── status cards ────────────────────────────────────────────────────── */
function showResult(node) {
  unmountAll();
  results.innerHTML = "";
  results.appendChild(node);
  results.classList.add("show");
  results.scrollIntoView({ behavior: "smooth", block: "start" });
}
function statusLoading(name) {
  const c = el("div", "status-card");
  c.innerHTML = `<div class="spinner"></div><h3>Decoding…</h3><p>${esc(name)}</p>`;
  return c;
}
function statusError(title, detail, hint) {
  setStatus(title);
  const c = el("div", "status-card");
  c.innerHTML = `<div class="err-ico"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z"/><path d="M12 9v4m0 4h.01"/></svg></div>
    <h3>${esc(title)}</h3><p>${hint || ""}</p>${detail ? `<div class="detail">${esc(detail)}</div>` : ""}
    <div style="margin-top:16px"><button class="btn-reset" id="err-reset"><svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/></svg> Try another file</button></div>`;
  setTimeout(() => $("#err-reset", c)?.addEventListener("click", () => fileInput.click()), 0);
  return c;
}

/* ── main render ─────────────────────────────────────────────────────── */
// `mounts` holds every live shared-UI handle (card + explorer roots); `explorers`
// the subset that takes selection updates; `legacy` the canvas renderers; `ids`
// maps each unique DISPLAY id (record + signal) back to its record. The ids are
// labels only: they are never parsed and are not scientific observation ids.
const state = { records: null, fileName: "", signalNames: [], active: null, mounts: [], explorers: [], legacy: [], ids: new Map(), selected: null };

/* ── shared nirs4all-ui browser entry ────────────────────────────────────
   Per-group record summary (DatasetPreviewCard, view only) and the valid 1-D
   spectra chart (SpectraExplorer) are React islands from the local
   `Nirs4allUIBrowser` IIFE. Each island lives in a dedicated, empty host div
   that this file never writes into with innerHTML; every handle is retained and
   unmounted BEFORE any container is cleared, so no React root is orphaned. */
function sharedUI() {
  const ui = window.Nirs4allUIBrowser;
  if (!ui || typeof ui.mountDatasetPreview !== "function" || typeof ui.mountSpectraExplorer !== "function")
    throw new Error("The shared nirs4all-ui browser bundle is not loaded.");
  return ui;
}
function mountShared(fn, host, props) {
  const handle = sharedUI()[fn](host, props);
  state.mounts.push(handle);
  return handle;
}
function unmountAll() {
  for (const h of state.mounts.splice(0)) {
    try { h.unmount(); } catch (e) { console.error(e); }
  }
  state.explorers = [];
  for (const p of state.legacy.splice(0)) p.destroy();
  state.selected = null;
}

let DATASETS = [], dsActive = 0;

function renderDatasets(datasets) {
  DATASETS = datasets;
  dsActive = Math.max(0, datasets.findIndex((d) => d.ok));
  unmountAll();
  results.innerHTML = "";
  results.classList.add("show");

  if (datasets.length === 1) {
    const ds = datasets[0];
    if (ds.ok) {
      results.appendChild(buildResult(ds, true));
      $("#reset").addEventListener("click", resetView);
      activateResult();
      setStatus(datasetSummary(ds));
      $("#result-heading")?.focus({ preventScroll: true });
    } else {
      results.appendChild(statusError("Could not decode this file", ds.error, hintFor(ds.error)));
    }
    results.scrollIntoView({ behavior: "smooth", block: "start" });
    return;
  }

  // multiple datasets → one tab each
  const okCount = datasets.filter((d) => d.ok).length;
  const bar = el("div", "ds-tabbar");
  bar.innerHTML = `<div class="ds-tabs" id="ds-tabs" role="tablist"></div>
    <button class="btn-reset" id="reset"><svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/></svg> New files</button>`;
  results.appendChild(bar);
  const content = el("div"); content.id = "ds-content"; results.appendChild(content);

  const tabsEl = $("#ds-tabs");
  datasets.forEach((ds, i) => {
    const fmt = ds.ok ? (ds.records[0].provenance?.format || "") : "error";
    const t = el("button", "ds-tab" + (i === dsActive ? " active" : ""));
    t.dataset.i = i; t.setAttribute("role", "tab");
    t.innerHTML = `<span class="ds-dot ${ds.ok ? "ok" : "err"}"></span><span class="ds-name">${esc(ds.name)}</span><span class="ds-fmt">${esc(fmt)}</span>`;
    t.addEventListener("click", () => showDataset(i));
    tabsEl.appendChild(t);
  });
  $("#reset").addEventListener("click", resetView);
  showDataset(dsActive);
  setStatus(`Decoded ${datasets.length} datasets (${okCount} ok) from ${datasets.length} file group${datasets.length > 1 ? "s" : ""}.`);
  results.scrollIntoView({ behavior: "smooth", block: "start" });
}

function showDataset(i) {
  dsActive = i;
  document.querySelectorAll(".ds-tab").forEach((t) => t.classList.toggle("active", +t.dataset.i === i));
  const content = $("#ds-content"); if (!content) return;
  unmountAll();
  content.innerHTML = "";
  const ds = DATASETS[i];
  if (ds.ok) { content.appendChild(buildResult(ds, false)); activateResult(); }
  else content.appendChild(statusError(`Could not decode ${ds.name}`, ds.error, hintFor(ds.error)));
}

const datasetSummary = (ds) => {
  const fmt = ds.records[0].provenance?.format || "unknown";
  const nSig = state.signalNames.length;
  return `Decoded ${ds.name}: ${fmt}, ${ds.records.length} record${ds.records.length > 1 ? "s" : ""}, ${nSig} channel${nSig > 1 ? "s" : ""}.`;
};

// build a full result panel for one dataset; sets `state` to it (only one is
// ever mounted at a time, so the plot/kpi singleton ids stay unique).
function buildResult(ds, withReset) {
  const { records, name: fileName, probes, size, fileCount } = ds;
  const names = [];
  for (const r of records) for (const k of Object.keys(r.signals || {})) if (!names.includes(k)) names.push(k);
  state.records = records; state.fileName = fileName; state.signalNames = names; state.active = names[0] || null;
  const prov = records[0].provenance || {};

  const wrap = el("div");
  const head = el("div", "results-head");
  head.innerHTML = `
    <div class="file-id">
      <div class="file-ico"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/><path d="M8 13h8M8 17h5"/></svg></div>
      <div><div class="file-name" id="result-heading" tabindex="-1">${esc(fileName)}</div>
      <div class="file-sub">${fmtBytes(size)}${fileCount > 1 ? ` · ${fileCount} files (with sidecars)` : ""} · decoded locally</div></div>
    </div>
    <div class="results-actions">
      <button class="btn-export" id="exp-csv" title="Download the active signal as a CSV matrix (one row per spectrum)"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><path d="M7 10l5 5 5-5"/><path d="M12 15V3"/></svg> CSV</button>
      <button class="btn-export" id="exp-json" title="Download the full decoded records as JSON"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><path d="M7 10l5 5 5-5"/><path d="M12 15V3"/></svg> JSON</button>
      <span class="export-to">
        <label class="sr-only" for="exp-target">Export for</label>
        <select id="exp-target" title="Export the active signal for another software or format">${exportTargetOptions()}</select>
        <label class="sr-only" for="exp-tolerance">Axis tolerance</label>
        <input id="exp-tolerance" type="number" min="0" step="any" placeholder="axis tol." title="Largest axis deviation, in axis units, accepted when the target needs an evenly spaced axis (default 0); the deviation is reported">
        <button class="btn-export" id="exp-go" title="Export the active signal to the selected target">Export</button>
      </span>
      <button class="btn-export" id="conv-open" title="Go to the conversion panel: convert the spectra for a software or format and check them on the plot first">Convert</button>
      ${withReset ? `<button class="btn-reset" id="reset"><svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/></svg> New file</button>` : ""}
    </div>`;
  wrap.appendChild(head);
  wireExport(head);
  conv = newConversion();
  $("#conv-open", head).addEventListener("click", openConvert);
  wrap.appendChild(renderDetect(probes, prov));
  const kpiRow = el("div", "kpis"); kpiRow.id = "kpis"; wrap.appendChild(kpiRow);
  wrap.appendChild(renderPlotPanel());
  const convertHost = el("div"); convertHost.id = "convert-host"; wrap.appendChild(convertHost);

  const grid = el("div", "grid-2");
  grid.appendChild(renderSignalsPanel());
  const right = el("div"); right.style.display = "grid"; right.style.gap = "20px";
  right.appendChild(renderProvenancePanel(prov, records[0].quality_flags || []));
  right.appendChild(renderMetaPanel(records[0].metadata, records[0].targets));
  grid.appendChild(right);
  wrap.appendChild(grid);
  return wrap;
}

/* ── export (CSV / JSON) ─────────────────────────────────────────────────
   Reads the singleton `state` (only one dataset panel is mounted at a time),
   so the buttons always export the dataset currently on screen. */
function wireExport(head) {
  $("#exp-json", head)?.addEventListener("click", exportJSON);
  const go = $("#exp-go", head);
  if (go) {
    if (state.signalNames.length) go.addEventListener("click", exportTo);
    else go.disabled = true;
  }
  const csv = $("#exp-csv", head);
  if (!csv) return;
  if (state.signalNames.length) csv.addEventListener("click", exportCSV);
  else { csv.disabled = true; csv.title = "No spectral channels to export — try the JSON export"; }
}

// strip directory + extension from the source name for a sensible download name
const exportBaseName = () => (state.fileName || "spectra").replace(/^.*[\\/]/, "").replace(/\.[^.]+$/, "") || "spectra";

// Targets come from the Rust registry; the page holds no format list.
function exportTargetOptions() {
  const targets = WASM.exportTargets();
  const options = (items, id) => items
    .map((item) => `<option value="${esc(item[id])}">${esc(item.label)}</option>`).join("");
  return `<optgroup label="Software">${options(targets.software, "id")}</optgroup>`
    + `<optgroup label="Format">${options(targets.formats, "format")}</optgroup>`;
}

function exportNote(title, lines, role) {
  $("#export-note")?.remove();
  const note = el("div", "legacy-note"); note.id = "export-note"; note.setAttribute("role", role);
  const head = el("strong"); head.textContent = title; note.appendChild(head);
  if (lines.length) {
    const list = el("ul");
    for (const line of lines) { const li = el("li"); li.textContent = line; list.appendChild(li); }
    note.appendChild(list);
  }
  $(".results-head")?.after(note);
  setStatus([title, ...lines].join(" "));
}

const lossLine = (loss) =>
  `${loss.message} (${loss.count}×${loss.max_error === undefined ? "" : `, largest error ${loss.max_error.toPrecision(3)}`}).`;

// Writing happens in Rust; every loss it declares is shown next to the download.
function exportTo() {
  const target = $("#exp-target").value;
  try {
    const options = { signal: state.active, base_name: exportBaseName(), axis_tolerance: Number($("#exp-tolerance").value) || 0 };
    let output = WASM.writeRecords(state.records, target, options);
    if (output.files.length > 1) output = WASM.writeRecords(state.records, target, { ...options, zip: true });
    const file = output.files[0];
    download(file.name, file.bytes, file.media_type);
    const targets = WASM.exportTargets();
    const profile = targets.software.find((p) => p.id === target);
    const lines = output.losses.map(lossLine);
    if (profile && !profile.tested) lines.push(`Not tested in ${profile.label} yet. Basis: ${profile.evidence}. ${profile.notes}`.trim());
    exportNote(`Exported ${file.name}.`, lines, "note");
  } catch (e) {
    exportNote(String(e?.message || e), [], "alert");
  }
}

function download(filename, text, mime) {
  const url = URL.createObjectURL(new Blob([text], { type: mime }));
  const a = el("a"); a.href = url; a.download = filename;
  document.body.appendChild(a); a.click();
  setTimeout(() => { URL.revokeObjectURL(url); a.remove(); }, 0);
}

// JSON.stringify rejects bigints (Renishaw/TriVista 64-bit metadata) → stringify them.
const exportJSON = () =>
  download(`${exportBaseName()}.json`, JSON.stringify(state.records, (_, v) => (typeof v === "bigint" ? v.toString() : v), 2), "application/json");

const csvCell = (v) => {
  if (v === null || v === undefined) return "";
  const s = String(v);
  return /[",\n\r]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
};

// Records can share one CSV header only when their axes match exactly (same
// comparator as the shared chart groups: kind, unit, order, length, every value ===)
// and each record's values line up with that axis. Chart orientation is irrelevant here.
function csvAxisProblem(recs, sig) {
  const first = recs[0].signals[sig];
  for (let i = 0; i < recs.length; i++) {
    const s = recs[i].signals[sig];
    if (!first.axis || !first.axis.values) {
      if (s.axis && s.axis.values) return `record ${i + 1} has an axis but record 1 has none.`;
      continue;
    }
    if (!s.axis || !s.axis.values) return `record ${i + 1} of signal "${sig}" has no axis.`;
    if (!sameAxis(first.axis, s.axis)) return `record ${i + 1} of signal "${sig}" has a different axis than record 1.`;
    if (!s.values || s.values.length !== first.axis.values.length) return `record ${i + 1} of signal "${sig}" has ${s.values ? s.values.length : 0} values for ${first.axis.values.length} axis points.`;
  }
  return null;
}

// the active signal as a wide matrix: one row per record, axis values as the
// header columns (the canonical NIRS spectra.csv layout). Other signals/metadata
// stay in the lossless JSON export.
function exportCSV() {
  const sig = state.active;
  const recs = state.records.filter((r) => r.signals && r.signals[sig]);
  $("#csv-refusal")?.remove();
  if (!recs.length) return;
  const problem = csvAxisProblem(recs, sig);
  if (problem) {
    // One header row cannot describe rows on different axes: refuse, never write a
    // header that is wrong for some rows. The JSON export keeps every record's own axis.
    const msg = `CSV export refused: ${problem} A single CSV header cannot describe records on different axes. Use the JSON export, which keeps each record's own axis.`;
    const note = el("p", "legacy-note"); note.id = "csv-refusal"; note.setAttribute("role", "alert"); note.textContent = msg;
    $(".results-head")?.after(note);
    setStatus(msg);
    return;
  }
  const axis = recs[0].signals[sig].axis;
  const header = ["sample", ...(axis ? axis.values : []).map(csvCell)];
  const lines = [header.join(",")];
  recs.forEach((r, i) => {
    lines.push([`record_${i}`, ...(r.signals[sig].values || []).map(csvCell)].join(","));
  });
  download(`${exportBaseName()}__${String(sig).replace(/[^\w.-]+/g, "_")}.csv`, lines.join("\n"), "text/csv");
}

function activateResult() {
  buildTabs();
  selectSignal(state.active);
  mountConvert();
}

function resetView() {
  unmountAll();
  results.classList.remove("show");
  results.innerHTML = "";
  fileInput.value = "";
  window.scrollTo({ top: 0, behavior: "smooth" });
}

/* detection bar */
function renderDetect(probes, prov) {
  const top = probes[0];
  const fmt = (top && top.format) || prov.format || "unknown";
  const conf = (top && top.confidence) || "likely";
  const reader = (prov.reader || (top && top.reader) || "").replace(/^nirs4all_formats::readers::/, "");
  const node = el("div", "detect");
  node.innerHTML = `
    <div class="lead-fmt">
      <span class="conf conf-${conf}"><span class="d"></span>${esc(conf)}</span>
      <div>
        <div class="fmt-name">${esc(fmt)}</div>
        <div class="fmt-reader">reader · ${esc(reader || "—")}${prov.reader_version ? " v" + esc(prov.reader_version) : ""}</div>
      </div>
    </div>`;
  if (probes.length > 1) {
    const btn = el("button", "alt-toggle", `${probes.length - 1} other candidate${probes.length > 2 ? "s" : ""} <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-6"/></svg>`);
    const list = el("div", "alt-list");
    probes.forEach((p, i) => {
      const row = el("div", "alt-row");
      row.innerHTML = `<span class="conf conf-${p.confidence}" style="min-width:84px"><span class="d"></span>${esc(p.confidence)}</span><span class="a-fmt">${esc(p.format)}</span><span class="a-reason">${esc(p.reason || "")}</span>`;
      list.appendChild(row);
    });
    btn.addEventListener("click", () => { btn.classList.toggle("open"); list.classList.toggle("open"); });
    node.appendChild(btn);
    node.appendChild(list);
  }
  return node;
}

/* KPI cards. The page-level row only holds quantities that do not depend on one
   axis (records, signals, group count); channels / axis range / signal type are
   reported per axis group, never from the first record's axis. */
function kpiNode(k) {
  const n = el("div", "kpi"); n.style.setProperty("--accent", k.acc);
  n.innerHTML = `<div class="k-label">${esc(k.label)}</div><div class="k-val">${esc(k.val)}</div><div class="k-unit">${esc(k.unit || "")}</div>`;
  return n;
}

function renderKpis(plan) {
  const recs = state.records;
  const sigCount = Math.max(...recs.map((r) => Object.keys(r.signals || {}).length));
  const kpis = [
    { label: "Records", val: fmtInt(recs.length), unit: recs.length === 1 ? "spectrum" : "spectra", acc: "var(--teal)" },
    { label: "Signals / record", val: sigCount, unit: state.signalNames.join(" · ").slice(0, 28), acc: "var(--cyan)" },
    { label: "Chart groups", val: plan.groups.length + plan.legacy.length, unit: `${plan.groups.length} shared · ${plan.legacy.length} legacy`, acc: "var(--indigo)" },
  ];
  const host = $("#kpis"); host.innerHTML = "";
  for (const k of kpis) host.appendChild(kpiNode(k));
}

/* plot panel: a dedicated container; its children are built and torn down by drawSignal */
function renderPlotPanel() {
  const p = el("div", "panel plot-panel");
  p.innerHTML = `
    <div class="panel-head">
      <span class="ph-ico"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 3v18h18"/><path d="M4 15c3-1 4-9 7-9s3 7 5 7 2-4 4-5"/></svg></span>
      <h3>Spectra</h3>
      <div class="plot-tools"><div class="sig-tabs" id="sig-tabs"></div></div>
    </div>
    <div id="plot-groups"></div>`;
  return p;
}

/* signals table */
function renderSignalsPanel() {
  const p = el("div", "panel"); p.id = "signals-panel";
  p.innerHTML = `<div class="panel-head"><span class="ph-ico"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M4 4v16M4 20h16"/><path d="M7 14l3-4 3 3 4-6"/></svg></span><h3>Signal channels</h3><span class="ph-meta">${state.signalNames.length} channel${state.signalNames.length > 1 ? "s" : ""}${state.records.length > 1 ? " · described from record 1" : ""}</span></div>`;
  const body = el("div", "panel-body");
  const r0 = state.records[0];
  const tbl = el("table", "sig-table");
  tbl.innerHTML = `<thead><tr><th>Name</th><th>Role</th><th>Type</th><th>Dims · shape</th><th>Axis</th></tr></thead>`;
  const tb = el("tbody");
  state.signalNames.forEach((name, i) => {
    const s = r0.signals[name] || firstSignal(name);
    const ax = s ? s.axis : null;
    const col = sigColor(s && s.signal_type, i);
    const xs = ax ? ax.values.filter(Number.isFinite) : [];
    const range = xs.length ? `${fmtNum(Math.min(...xs), 5)}–${fmtNum(Math.max(...xs), 5)} ${prettyUnit(ax.unit)}` : "—";
    const tr = el("tr");
    tr.innerHTML = `
      <td><span class="s-name"><span class="sw" style="background:${col}"></span>${esc(name)}</span>${s && s.unit ? `<div class="mono-sub">${esc(prettyUnit(s.unit))}</div>` : ""}</td>
      <td><span class="pill-s">${esc(s ? s.role || "—" : "—")}</span><div class="mono-sub">${esc(s ? s.source || "" : "")}</div></td>
      <td>${esc(sigLabel(s && s.signal_type))}</td>
      <td><span class="pill-s">${esc(s ? (s.dims || []).join("·") : "")}</span><div class="mono-sub">[${s ? (s.shape || []).join(", ") : ""}]</div></td>
      <td>${ax ? `${esc(KIND[ax.kind] || ax.kind)}<div class="mono-sub">${esc(range)} · ${esc(ax.order || "")}</div>` : "—"}</td>`;
    tb.appendChild(tr);
  });
  tbl.appendChild(tb); body.appendChild(tbl); p.appendChild(body);
  return p;
}

/* provenance */
function renderProvenancePanel(prov, qflags) {
  const p = el("div", "panel");
  p.innerHTML = `<div class="panel-head"><span class="ph-ico"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2 4 6v6c0 5 8 8 8 8s8-3 8-8V6z"/></svg></span><h3>Provenance</h3></div>`;
  const body = el("div", "panel-body");
  const dl = el("dl", "kv");
  const add = (k, v) => { dl.appendChild(el("dt", null, esc(k))); const dd = el("dd"); dd.innerHTML = v; dl.appendChild(dd); };
  add("Format", esc(prov.format || "—"));
  add("Reader", esc((prov.reader || "—").replace(/^nirs4all_formats::readers::/, "")));
  add("Reader version", esc(prov.reader_version || "—"));
  if (prov.record_schema_version) add("Schema", esc(prov.record_schema_version));
  body.appendChild(dl);

  const sources = prov.sources || [];
  for (const src of sources) {
    if (!src.sha256) continue;
    const short = src.sha256.slice(0, 10) + "…" + src.sha256.slice(-6);
    const row = el("div"); row.style.marginTop = "12px";
    row.innerHTML = `<div class="k-label" style="font-size:.66rem;text-transform:uppercase;letter-spacing:.1em;color:var(--text-3);font-weight:600;margin-bottom:5px">SHA-256${src.role ? " · " + esc(src.role) : ""}</div>
      <span class="hash"><code title="${esc(src.sha256)}">${esc(short)}</code><button class="copy-btn" title="Copy full hash"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="11" height="11" rx="2"/><path d="M5 15V5a2 2 0 0 1 2-2h10"/></svg></button></span>`;
    row.querySelector(".copy-btn").addEventListener("click", (e) => copy(src.sha256, e.currentTarget));
    body.appendChild(row);
  }

  const warnings = prov.warnings || [];
  if (warnings.length) {
    const wh = el("div"); wh.style.marginTop = "14px";
    wh.innerHTML = `<div class="k-label" style="font-size:.66rem;text-transform:uppercase;letter-spacing:.1em;color:var(--amber);font-weight:600;margin-bottom:6px">${warnings.length} reader warning${warnings.length > 1 ? "s" : ""}</div>`;
    for (const w of warnings) {
      const [code, ...rest] = String(w).split(":");
      wh.appendChild(el("div", "warn", `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 9v4m0 4h.01M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h16.9a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z"/></svg><span><code>${esc(code)}</code>${rest.length ? " " + esc(rest.join(":").trim()) : ""}</span>`));
    }
    body.appendChild(wh);
  } else {
    const ok = el("div", "ok-line"); ok.style.marginTop = "14px";
    ok.innerHTML = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg> No reader warnings`;
    body.appendChild(ok);
  }

  if (qflags && qflags.length) {
    const q = el("div"); q.style.marginTop = "12px";
    q.innerHTML = `<div class="k-label" style="font-size:.66rem;text-transform:uppercase;letter-spacing:.1em;color:var(--rose);font-weight:600;margin-bottom:6px">Quality flags</div>`;
    for (const f of qflags) q.appendChild(el("span", "pill-s", esc(typeof f === "string" ? f : JSON.stringify(f))));
    body.appendChild(q);
  }

  p.appendChild(body);
  return p;
}

/* metadata + targets */
function renderMetaPanel(metadata, targets) {
  const p = el("div", "panel");
  const hasTargets = targets && Object.keys(targets).length;
  p.innerHTML = `<div class="panel-head"><span class="ph-ico"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M4 7h16M4 12h16M4 17h10"/></svg></span><h3>Metadata${hasTargets ? " &amp; targets" : ""}</h3></div>`;
  const body = el("div", "panel-body");

  if (hasTargets) {
    const tg = el("div"); tg.style.marginBottom = "14px";
    tg.innerHTML = `<div class="k-label" style="font-size:.66rem;text-transform:uppercase;letter-spacing:.1em;color:var(--text-3);font-weight:600;margin-bottom:7px">Reference targets</div>`;
    const grid = el("div", "targets-grid");
    for (const [k, v] of Object.entries(targets)) grid.appendChild(el("span", "target", `${esc(k)} <b>${esc(v == null ? "missing" : typeof v === "object" ? JSON.stringify(v) : v)}</b>`));
    tg.appendChild(grid); body.appendChild(tg);
  }

  if (metadata && Object.keys(metadata).length) {
    const tree = el("div", "json-tree");
    tree.appendChild(jsonNode(metadata, true));
    body.appendChild(tree);
  } else if (!hasTargets) {
    body.appendChild(el("p", "empty-note", "No instrument metadata reported for this file."));
  }
  p.appendChild(body);
  return p;
}

/* collapsible JSON tree */
function jsonNode(value, openTop = false) {
  // serde-wasm-bindgen serializes Rust `None` as `undefined`; treat it like null.
  if (value === null || value === undefined) return el("span", "jnull", "null");
  const t = typeof value;
  if (t === "bigint") return el("span", "jn", esc(String(value)));
  if (t === "number") return el("span", "jn", esc(Number.isFinite(value) ? value : String(value)));
  if (t === "boolean") return el("span", "jb", String(value));
  if (t === "string") return el("span", "js", `"${esc(value.length > 200 ? value.slice(0, 200) + "…" : value)}"`);
  const isArr = Array.isArray(value);
  const entries = isArr ? value.map((v, i) => [i, v]) : Object.entries(value);
  if (entries.length === 0) return el("span", "muted", isArr ? "[ ]" : "{ }");

  const det = el("details"); if (openTop) det.open = true;
  const sum = el("summary");
  sum.innerHTML = `<span class="muted">${isArr ? `[${entries.length}]` : `{${entries.length}}`}</span>`;
  det.appendChild(sum);

  const shown = entries.slice(0, 80);
  for (const [k, v] of shown) {
    const isLeaf = v === null || typeof v !== "object" || (Array.isArray(v) ? v.length === 0 : Object.keys(v).length === 0);
    const line = el("div", isLeaf ? "leaf" : null);
    const key = el("span", "jk", esc(k) + ": ");
    line.appendChild(key);
    line.appendChild(jsonNode(v));
    det.appendChild(line);
  }
  if (entries.length > shown.length) det.appendChild(el("div", "leaf muted", `… ${entries.length - shown.length} more`));
  return det;
}

/* ── signal selection ────────────────────────────────────────────────── */
function firstSignal(name) {
  for (const r of state.records) if (r.signals && r.signals[name]) return r.signals[name];
  return null;
}

function buildTabs() {
  const host = $("#sig-tabs"); if (!host) return;
  host.innerHTML = "";
  if (state.signalNames.length <= 1) { host.style.display = "none"; return; }
  state.signalNames.forEach((name, i) => {
    const s = firstSignal(name);
    const col = sigColor(s && s.signal_type, i);
    const tab = el("button", "sig-tab" + (name === state.active ? " active" : ""), `<span class="sw" style="background:${col}"></span>${esc(name)}`);
    tab.addEventListener("click", () => selectSignal(name));
    host.appendChild(tab);
  });
}

function selectSignal(name) {
  // New signal → new axis geometry: release every island before touching the DOM,
  // then remount (the explorer keeps hover/band state that a shorter axis would strand).
  unmountAll();
  $("#csv-refusal")?.remove();
  $("#export-note")?.remove();
  state.active = name;
  [...document.querySelectorAll(".sig-tab")].forEach((t) => t.classList.toggle("active", t.textContent.trim() === name));
  const plan = planSignal(name);
  renderKpis(plan);
  drawSignal(name, plan);
}

/* ── axis grouping ────────────────────────────────────────────────────────
   Records carrying the active signal are split into
   • shared groups: valid 1-D spectra whose axes match EXACTLY (kind, unit, native
     order, length and every value ===; no tolerance, resampling, reversal or
     flattening) and whose kind the shared chart orients correctly;
   • legacy groups: everything the shared chart must not draw. They stay on the
     original canvas renderer (one canvas per kind/unit) with an explicit note;
   • multidimensional signals, which neither renderer draws (as before), listed
     in a note. */
const SHARED_KINDS = new Set(["wavelength", "index", "time"]);
const LEGACY_WHY = {
  "reversed-orientation": "wavenumber, frequency and energy axes are drawn high-to-low by the original canvas; the shared chart draws ascending only, and arrays are never reversed",
  "unsupported-orientation": "the axis kind has no declared orientation in the shared chart",
  "nonfinite-axis": "the axis holds non-finite values",
  "length-mismatch": "the axis length differs from the number of values (the canvas plots the shared prefix)",
  "empty-axis": "the axis is empty",
  "missing-axis": "the signal has no axis",
  "missing-values": "the signal has no values",
};

function legacyReason(sig) {
  const ax = sig.axis;
  if (!ax || !ax.values) return "missing-axis";
  if (!sig.values) return "missing-values";
  if (ax.values.length !== sig.values.length) return "length-mismatch";
  if (ax.values.length === 0) return "empty-axis";
  for (let i = 0; i < ax.values.length; i++) if (typeof ax.values[i] !== "number" || !Number.isFinite(ax.values[i])) return "nonfinite-axis";
  if (!SHARED_KINDS.has(ax.kind)) return REVERSED_KIND.has(ax.kind) ? "reversed-orientation" : "unsupported-orientation";
  return null;
}

function sameAxis(a, b) {
  if (a.kind !== b.kind || a.unit !== b.unit || a.order !== b.order || a.values.length !== b.values.length) return false;
  for (let i = 0; i < a.values.length; i++) if (a.values[i] !== b.values[i]) return false;
  return true;
}

function planSignal(name) {
  const groups = [], legacy = new Map(), multi = [];
  state.records.forEach((rec, i) => {
    const sig = rec.signals && rec.signals[name];
    if (!sig) return;
    if (!sig.dims || sig.dims.length !== 1 || sig.dims[0] !== "x") { multi.push({ i, sig }); return; }
    const reason = legacyReason(sig);
    if (reason) {
      const ax = sig.axis || null;
      const key = ax ? `${ax.kind}|${ax.unit}` : "none";
      if (!legacy.has(key)) legacy.set(key, { axis: ax, members: [] });
      legacy.get(key).members.push({ i, sig, reason });
      return;
    }
    let g = groups.find((c) => sameAxis(c.axis, sig.axis));
    if (!g) { g = { axis: sig.axis, members: [] }; groups.push(g); }
    g.members.push({ i, sig });
  });
  return { groups, legacy: [...legacy.values()], multi };
}

const displayId = (i, name) => `record ${i + 1} · ${name}`;
const kindLabel = (ax) => (ax ? KIND[ax.kind] || ax.kind || "unknown kind" : "no axis");
const axisLabel = (ax) => (ax ? `${kindLabel(ax)}${ax.unit ? " / " + prettyUnit(ax.unit) : ""}` : "no axis");
function sampleLabel(rec) {
  const m = rec && rec.metadata;
  if (!m) return undefined;
  for (const k of ["sample_id", "sample_name", "name"]) {
    if (typeof m[k] === "string" && m[k]) return m[k];
    if (typeof m[k] === "number") return String(m[k]);
  }
  return undefined;
}
function seriesColors(count, base) {
  if (count <= 8) return (k) => (count === 1 ? base : PALETTE[k % PALETTE.length]);
  return (k) => ramp(k / (count - 1)); // teal→cyan→indigo stack
}
function commonSignalType(members) {
  const types = new Set(members.map((m) => m.sig.signal_type || state.records[m.i].signal_type));
  return types.size === 1 ? [...types][0] : null;
}

// teal → cyan → indigo ramp
function ramp(t) {
  const stops = [[13, 148, 136], [6, 182, 212], [79, 70, 229]];
  const seg = t < 0.5 ? 0 : 1, lt = t < 0.5 ? t / 0.5 : (t - 0.5) / 0.5;
  const a = stops[seg], b = stops[seg + 1];
  const c = a.map((v, k) => Math.round(v + (b[k] - v) * lt));
  return `rgb(${c[0]},${c[1]},${c[2]})`;
}

function drawSignal(name, plan) {
  const host = $("#plot-groups"); if (!host) return;
  host.replaceChildren(); // islands were already unmounted by selectSignal
  state.ids = new Map();
  const total = plan.groups.length + plan.legacy.length;
  plan.groups.forEach((g, k) => appendSharedGroup(host, g, k, total, name));
  plan.legacy.forEach((g, k) => appendLegacyGroup(host, g, plan.groups.length + k, total, name));
  if (plan.multi.length) host.appendChild(multiNote(plan.multi));
  if (!total) host.appendChild(el("div", "plot-legend", `<span class="leg-note">No 1-D spectra to plot for this signal.</span>`));
  if (plan.groups.length) host.appendChild(selectionLine());
}

function legendInto(host, items) {
  if (items.length <= 8) {
    for (const it of items) host.appendChild(el("span", "leg-item", `<span class="sw" style="background:${it.color}"></span>${esc(it.label)}`));
  } else if (items.length) {
    const grad = el("span", "leg-item");
    grad.innerHTML = `<span class="leg-grad" style="background:linear-gradient(90deg,#0d9488,#06b6d4,#4f46e5)"></span><b>${items.length} spectra</b> overlaid`;
    host.appendChild(grad);
  }
}

function groupHead(k, total, declared, count) {
  return el("div", "ugroup-head", `<h4>Axis group ${k + 1} of ${total}</h4><span class="ugroup-meta">${esc(declared)} · ${fmtInt(count)} record${count === 1 ? "" : "s"}</span>`);
}

// Card view for one exact-axis group. Built host-side and passed as `view` only:
// the card's `dataset` input would default the unit to nm and word counts as
// "samples". Records and channels are what the decoded group actually holds.
function groupView(k, name, axis, members) {
  const declared = axisLabel(axis);
  const n = axis.values.length;
  const recLabel = `${fmtInt(members.length)} record${members.length === 1 ? "" : "s"}`;
  const chLabel = `${fmtInt(n)} channel${n === 1 ? "" : "s"}`;
  return {
    id: `axis-group-${k + 1}`,
    title: `${name} · group ${k + 1}`,
    description: `${declared} axis, ${axis.order ? axis.order + " native order" : "native order"}`,
    taskKind: "unknown",
    taskLabel: "Spectral records",
    sampleCount: members.length,
    sampleCountLabel: recLabel,
    featureCount: n,
    featureCountLabel: chLabel,
    targetCount: null,
    targetCountLabel: null,
    classCount: null,
    classCountLabel: null,
    spectralRangeLabel: null,
    splitCounts: [],
    splitSummaryLabel: null,
    tags: [],
    badges: [{ id: "axis", label: declared, tone: "muted" }],
    stats: [
      { id: "samples", label: "Records", value: fmtInt(members.length), detail: null, tone: "default" },
      { id: "features", label: "Channels", value: fmtInt(n), detail: declared, tone: "default" },
    ],
  };
}

function appendSharedGroup(host, g, k, total, name) {
  const { axis, members } = g;
  const declared = axisLabel(axis);
  const node = el("section", "ugroup"); node.dataset.route = "shared";
  node.setAttribute("aria-label", `Axis group ${k + 1} of ${total}: ${declared}`);
  node.appendChild(groupHead(k, total, declared, members.length));
  const cardHost = el("div", "ugroup-card"); node.appendChild(cardHost);

  let lo = Infinity, hi = -Infinity;
  for (const v of axis.values) { if (v < lo) lo = v; if (v > hi) hi = v; }
  const stype = commonSignalType(members);
  const kp = el("div", "kpis");
  kp.appendChild(kpiNode({ label: "Axis range", val: `${fmtNum(lo, 5)}–${fmtNum(hi, 5)}`, unit: prettyUnit(axis.unit) + (axis.order ? " · " + axis.order : ""), acc: "var(--amber)" }));
  kp.appendChild(kpiNode({ label: "Signal type", val: stype ? sigLabel(stype) : "mixed", unit: kindLabel(axis), acc: "var(--green)" }));
  node.appendChild(kp);

  const chartHost = el("div", "ugroup-chart"); node.appendChild(chartHost);
  const legend = el("div", "plot-legend"); node.appendChild(legend);
  host.appendChild(node);

  const colorAt = seriesColors(members.length, sigColor(members[0].sig.signal_type, state.signalNames.indexOf(name)));
  const ids = new Set();
  const series = members.map(({ i, sig }, c) => {
    const id = displayId(i, name);
    ids.add(id);
    state.ids.set(id, { recordIndex: i, signal: name, group: k });
    const s = { id, values: Array.from(sig.values, (v) => (v === undefined ? null : v)), color: colorAt(c) };
    const label = sampleLabel(state.records[i]);
    if (label !== undefined) s.label = label;
    return s;
  });
  legendInto(legend, members.map(({ i, sig }, c) => ({ color: colorAt(c), label: state.records.length > 1 ? `#${i + 1}` : sigLabel(sig.signal_type || name) })));

  const axisArr = Array.from(axis.values);
  const yType = commonSignalType(members);
  const title = `${fileBaseName(state.fileName)} · ${name} · ${declared}${total > 1 ? ` · group ${k + 1} of ${total}` : ""}`;
  const props = (selected) => ({ axis: axisArr, series, axisUnit: prettyUnit(axis.unit) || kindLabel(axis), yLabel: yType ? sigLabel(yType) : name, title, selected, onSelect: selectRecord, locale: "en" });

  mountShared("mountDatasetPreview", cardHost, { view: groupView(k, name, axis, members) });
  const handle = mountShared("mountSpectraExplorer", chartHost, props([]));
  state.explorers.push({ handle, props, ids });
}

function appendLegacyGroup(host, g, k, total, name) {
  const { axis, members } = g;
  const declared = axisLabel(axis);
  const node = el("section", "ugroup"); node.dataset.route = "legacy";
  node.setAttribute("aria-label", `Axis group ${k + 1} of ${total}: ${declared} (original canvas)`);
  node.appendChild(groupHead(k, total, declared, members.length));
  const note = el("p", "legacy-note"); note.setAttribute("role", "note"); node.appendChild(note);
  const wrap = el("div", "plot-wrap");
  const canvas = el("canvas", "spectra");
  const readout = el("div", "plot-readout");
  wrap.append(canvas, readout); node.appendChild(wrap);
  const legend = el("div", "plot-legend"); node.appendChild(legend);
  host.appendChild(node);

  const colorAt = seriesColors(members.length, sigColor(members[0].sig.signal_type, state.signalNames.indexOf(name)));
  const series = members.map(({ i, sig }, c) => ({
    x: sig.axis ? sig.axis.values : undefined, y: sig.values, _i: i, color: colorAt(c),
    label: state.records.length > 1 ? `#${i + 1}` : sigLabel(sig.signal_type || name),
  }));
  const plot = new SpectralPlot(canvas, readout);
  plot.setData(series, axis);
  state.legacy.push(plot);

  const counts = {};
  for (const m of members) counts[m.reason] = (counts[m.reason] || 0) + 1;
  const why = Object.entries(counts).map(([r, c]) => `${c} record${c === 1 ? "" : "s"}: ${LEGACY_WHY[r]}`).join("; ");
  const drawn = plot.series.length;
  note.textContent = `Original canvas renderer, ${drawn} of ${members.length} record${members.length === 1 ? "" : "s"} drawn. ${why}.`;
  canvas.setAttribute("role", "img");
  canvas.setAttribute("aria-label", `${name}, ${declared}: ${drawn} spectra drawn on the original canvas`);
  legendInto(legend, series.filter((s) => s.x && s.y && s.x.length));
}

function multiNote(multi) {
  const shown = multi.slice(0, 12).map((m) => m.i + 1).join(", ") + (multi.length > 12 ? `, … +${multi.length - 12} more` : "");
  const dims = [...new Set(multi.map((m) => (m.sig.dims || []).join("·") || "no dims"))].join(", ");
  const p = el("p", "legacy-note");
  p.setAttribute("role", "note");
  p.textContent = `${multi.length} record${multi.length === 1 ? "" : "s"} carry a multidimensional signal (dims ${dims}); the 1-D charts do not draw them. Records: ${shown}.`;
  return p;
}

/* ── selection: new host display state, resolved through the host-held id map ── */
function selectionText() {
  const hit = state.selected !== null ? state.ids.get(state.selected) : null;
  if (!hit) return "No record selected. Hover a curve and click it, or focus a chart and press ↑ / ↓ (Esc clears).";
  const label = sampleLabel(state.records[hit.recordIndex]);
  return `Selected: record ${hit.recordIndex + 1} · signal ${hit.signal} · axis group ${hit.group + 1}${label ? " · " + label : ""}`;
}

function selectionLine() {
  const line = el("div", "sel-line");
  const text = el("span"); text.id = "sel-text"; text.setAttribute("role", "status"); text.textContent = selectionText();
  const clear = el("button", null, "Clear selection"); clear.type = "button"; clear.id = "sel-clear";
  clear.addEventListener("click", () => selectRecord(null));
  line.append(text, clear);
  return line;
}

function selectRecord(id) {
  if (id !== null && !state.ids.has(id)) return; // only ids this host issued
  state.selected = id;
  // same geometry: update selection only, never remount
  for (const ex of state.explorers) ex.handle.update(ex.props(id !== null && ex.ids.has(id) ? [id] : []));
  const out = $("#sel-text"); if (out) out.textContent = selectionText();
}

/* ── convert for market software ──────────────────────────────────────────
   nirs4all-formats (this page's WASM) decides WHAT a target needs and plans
   the steps (planConversion). Every value is computed by nirs4all-methods
   (its own WASM, vendored in ./vendor/methods/ and loaded only when a
   conversion is requested). This file only moves arrays between the two:
   it selects points, builds the grid axis the planner names and counts
   what the validation summary reports. It never computes a spectrum. */
const METHODS_URL = "./vendor/methods/index.js";
let METHODS = null;
async function loadMethods() {
  if (EMBED) {
    throw new Error("Conversions need the hosted page (https://formats.nirs4all.org/) or a local server: the single-file build does not include nirs4all-methods.");
  }
  if (!METHODS) {
    const mod = await import(METHODS_URL);
    await mod.loadModule();
    METHODS = mod;
  }
  return METHODS;
}

const CONVERT_LIMITS = [
  "Absorbance computed as log10(1/R) is apparent absorbance; it is not corrected for scattering.",
  "Whether reflectance is stored in percent or as a fraction must be known: a wrong answer shifts absorbance by 2.",
  "Values are never extrapolated: grids always lie inside the measured range.",
  "Interpolation smooths, and down-sampling high-resolution spectra (FTIR) can alias narrow bands.",
  "nm and cm-1 are converted as vacuum wavelengths (nm = 10^7 / cm-1), without air correction.",
  "Raw counts need white (and dark) references measured with the sample; vendor-computed reflectance is preferred when the file holds it.",
  "Files written for vendor software have not been tested in that software yet.",
];
const STEP_LABEL = {
  reflectance_from_references: "Ratio to references",
  declare_signal_type: "Declared signal type",
  signal_conversion: "Signal conversion",
  select_points: "Point selection",
  splice_correction: "Detector splice correction",
  axis_unit_conversion: "Axis conversion",
  resample: "Resampling",
};
// n4m_signal_type_t codes of utilities.signal_type_detector.
const DETECTED = {
  1: ["absorbance", null], 2: ["reflectance", "fraction"], 3: ["reflectance", "percent"],
  4: ["transmittance", "fraction"], 5: ["transmittance", "percent"], 6: ["kubelka_munk", null],
  7: ["absorbance", null], 8: ["absorbance", null],
};

let conv = newConversion();
function newConversion() {
  return { plan: null, answers: {}, result: null };
}

// Every decoded file gets the panel under its plot.
function mountConvert() {
  const host = $("#convert-host");
  if (!host || host.firstChild || !state.signalNames.length) return;
  host.appendChild(convertPanel());
  replan();
}

function openConvert() {
  mountConvert();
  const panel = $("#convert-panel");
  if (!panel) return;
  panel.scrollIntoView({ behavior: "smooth", block: "start" });
  $("#conv-target")?.focus({ preventScroll: true });
}

function convertPanel() {
  const panel = el("section", "panel convert-panel");
  panel.id = "convert-panel";
  panel.setAttribute("aria-label", "Convert for market software");
  const types = ["absorbance", "reflectance", "transmittance", "kubelka_munk"]
    .map((t) => `<option value="${t}">${esc(sigLabel(t))}</option>`).join("");
  panel.innerHTML = `
    <div class="panel-head"><span class="ph-ico"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M4 7h13l-3-3M20 17H7l3 3"/></svg></span>
      <h3>Convert for market software</h3><span class="ph-meta">plan: nirs4all-formats · values: nirs4all-methods</span></div>
    <div class="panel-body">
      <div class="conv-controls">
        <label>Target<select id="conv-target"><option value="">No export target (view only)</option>${exportTargetOptions()}</select></label>
        <label>Source signal<select id="conv-signal"><option value="">Automatic</option></select></label>
        <label>Output signal<select id="conv-type"><option value="">As the target needs</option>${types}</select></label>
        <label>Axis unit<select id="conv-unit"><option value="">As the target needs</option><option value="nm">nm</option><option value="um">µm</option><option value="cm-1">cm⁻¹</option></select></label>
        <label>Grid<select id="conv-grid"><option value="">Automatic</option></select></label>
        <label class="conv-even hidden">Grid step<input id="conv-step" type="number" min="0" step="any" placeholder="default"></label>
        <label class="conv-even hidden">Grid from<input id="conv-min" type="number" step="any" placeholder="measured start"></label>
        <label class="conv-even hidden">Grid to<input id="conv-max" type="number" step="any" placeholder="measured end"></label>
        <label>Interpolation<select id="conv-interp"><option value="linear">Linear</option><option value="nearest">Nearest</option><option value="cubic">Cubic</option></select></label>
        <label class="conv-splices hidden">Detector splices<input id="conv-splices" type="text" placeholder="first nm of each next detector, e.g. 1001, 1801"></label>
      </div>
      <div id="conv-plan" aria-live="polite"></div>
      <div class="conv-actions">
        <button class="btn-export" id="conv-run" disabled>Convert with nirs4all-methods</button>
        <button class="btn-export" id="conv-export" disabled title="Write the converted signal with the selected target">Export converted</button>
        <span class="conv-compare" id="conv-compare" role="group" aria-label="Compare before and after"></span>
      </div>
      <div id="conv-result"></div>
      <details class="conv-limits"><summary>Limits of these conversions</summary><ul>${CONVERT_LIMITS.map((l) => `<li>${esc(l)}</li>`).join("")}</ul></details>
    </div>`;
  for (const id of ["#conv-target", "#conv-signal", "#conv-type", "#conv-unit", "#conv-grid", "#conv-interp", "#conv-step", "#conv-min", "#conv-max", "#conv-splices"]) {
    $(id, panel).addEventListener("change", () => {
      if (id === "#conv-target" || id === "#conv-signal") conv.answers = {};
      replan();
    });
  }
  $("#conv-run", panel).addEventListener("click", runConversion);
  $("#conv-export", panel).addEventListener("click", exportConverted);
  return panel;
}

function convOptions() {
  const value = (id) => ($(id)?.value || "").trim();
  const options = { ...conv.answers };
  if (value("#conv-signal")) options.signal = value("#conv-signal");
  if (value("#conv-type")) options.signal_type = value("#conv-type");
  if (value("#conv-unit")) options.axis_unit = value("#conv-unit");
  if (value("#conv-grid")) options.grid = value("#conv-grid");
  if (options.grid === "even") {
    for (const [id, key] of [["#conv-step", "grid_step"], ["#conv-min", "grid_min"], ["#conv-max", "grid_max"]]) {
      if (value(id)) options[key] = Number(value(id));
    }
  }
  if (value("#conv-interp") !== "linear") options.interpolation = value("#conv-interp");
  if (value("#conv-splices")) {
    const splices = value("#conv-splices").split(/[\s,;]+/).filter(Boolean).map(Number);
    if (splices.some((x) => !Number.isFinite(x))) throw new Error("Detector splices must be numbers separated by commas, for example 1001, 1801.");
    options.splices = splices;
  }
  const tolerance = Number($("#exp-tolerance")?.value);
  if (tolerance > 0) options.axis_tolerance = tolerance;
  return options;
}

function convTarget() {
  return $("#conv-target")?.value || null;
}

function targetFormat(target) {
  if (!target) return null;
  const targets = WASM.exportTargets();
  const profile = targets.software.find((p) => p.id === target);
  return profile ? profile.format : target;
}

function replan() {
  const host = $("#conv-plan"); if (!host) return;
  $("#conv-run").disabled = true;
  try {
    conv.plan = WASM.planConversion(state.records, convTarget(), convOptions());
  } catch (e) {
    conv.plan = null;
    host.replaceChildren(convNote(String(e?.message || e), "Change the option and the plan is made again.", "alert"));
    return;
  }
  refreshPlanControls(conv.plan);
  renderPlan(conv.plan);
}

// Selects offered by the plan: usable source signals and the target's grids.
function refreshPlanControls(plan) {
  const signal = $("#conv-signal"), chosen = signal.value;
  signal.innerHTML = `<option value="">Automatic${plan.source_signal && !chosen ? ` (${esc(plan.source_signal)})` : ""}</option>`
    + plan.candidates.map((c) => `<option value="${esc(c.name)}"${c.usable ? "" : " disabled"}>${esc(c.name)} · ${esc(sigLabel(c.signal_type))}${c.usable ? "" : " (not convertible)"}</option>`).join("");
  signal.value = chosen;

  const grid = $("#conv-grid"), gridChosen = grid.value;
  const format = targetFormat(convTarget());
  const requirements = format ? WASM.exportTargets().requirements.find((r) => r.format === format) : null;
  const range = plan.measured_range;
  const unitOk = (g) => !range || (g.first >= range[0] && g.last <= range[1]);
  grid.innerHTML = `<option value="">Automatic</option><option value="measured">Keep the measured axis</option>`
    + (requirements ? requirements.grids : []).map((g) => `<option value="${esc(g.id)}"${unitOk(g) ? "" : " disabled"}>${esc(g.label)}${unitOk(g) ? "" : " (outside the measured range)"}</option>`).join("")
    + `<option value="even">Even grid inside the measured range</option>`;
  grid.value = [...grid.options].some((o) => o.value === gridChosen && !o.disabled) ? gridChosen : "";
  document.querySelectorAll(".conv-even").forEach((n) => n.classList.toggle("hidden", grid.value !== "even"));
  const references = plan.steps.some((s) => s.step === "reflectance_from_references") || $("#conv-splices").value;
  document.querySelectorAll(".conv-splices").forEach((n) => n.classList.toggle("hidden", !references));
}

function convNote(title, detail, role) {
  const note = el("div", "legacy-note"); note.setAttribute("role", role);
  const head = el("strong"); head.textContent = title; note.appendChild(head);
  if (detail) { const p = el("div"); p.textContent = detail; note.appendChild(p); }
  return note;
}

const STATUS_TEXT = {
  ready: (p) => `Ready: ${p.steps.length} step${p.steps.length > 1 ? "s" : ""} on '${p.source_signal}' of ${p.records} record${p.records > 1 ? "s" : ""}. Review them, then convert.`,
  unchanged: (p) => (p.target
    ? `No conversion needed: '${p.source_signal}' is written to ${p.target} as it is. Use Export above.`
    : `Nothing to convert: choose an output signal, an axis unit or a grid.`),
  needs_answers: () => "Information is missing: the file does not say what the answer below needs.",
  refused: () => "This cannot be converted for this target. Why, and how to fix it:",
};

function renderPlan(plan) {
  const host = $("#conv-plan");
  host.replaceChildren();
  const status = el("p", `conv-status s-${plan.status}`);
  status.textContent = STATUS_TEXT[plan.status](plan);
  host.appendChild(status);
  if (plan.steps.length) {
    const list = el("ol", "conv-steps");
    for (const step of plan.steps) {
      const li = el("li");
      li.innerHTML = `<b>${esc(STEP_LABEL[step.step] || step.step)}.</b> ${esc(step.description)}`
        + `<div class="mono-sub">${esc(step.method_id || "selection, no computation")}${Object.keys(step.params).length ? " · " + esc(JSON.stringify(step.params)) : ""}</div>`
        + (step.notes && step.notes.length ? `<ul>${step.notes.map((n) => `<li>${esc(n)}</li>`).join("")}</ul>` : "");
      list.appendChild(li);
    }
    host.appendChild(list);
  }
  for (const question of plan.questions) host.appendChild(questionNode(question, plan));
  for (const refusal of plan.refusals) host.appendChild(convNote(refusal.reason + ".", "Fix: " + refusal.fix, "alert"));
  if (plan.notes.length) {
    const notes = el("ul", "conv-notes");
    for (const note of plan.notes) { const li = el("li"); li.textContent = note; notes.appendChild(li); }
    host.appendChild(notes);
  }
  $("#conv-run").disabled = plan.status !== "ready";
}

function questionNode(question, plan) {
  const node = el("div", "conv-q"); node.setAttribute("role", "group");
  const prompt = el("div"); prompt.textContent = question.prompt; node.appendChild(prompt);
  const choices = el("div", "choices");
  for (const choice of question.choices) {
    const b = el("button", "btn-export"); b.type = "button"; b.textContent = choice.label;
    b.addEventListener("click", () => { conv.answers[question.id] = choice.value; replan(); });
    choices.appendChild(b);
  }
  const suggest = el("button", "btn-export"); suggest.type = "button"; suggest.textContent = "Suggest with nirs4all-methods";
  const out = el("div", "suggest"); out.setAttribute("role", "status");
  suggest.addEventListener("click", () => suggestAnswer(question, plan, out));
  if (question.id !== "absorbance_origin") choices.appendChild(suggest);
  node.appendChild(choices);
  const why = el("div", "why"); why.textContent = question.why; node.appendChild(why);
  node.appendChild(out);
  return node;
}

// utilities.signal_type_detector only suggests; the answer stays the user's.
async function suggestAnswer(question, plan, out) {
  out.textContent = "Loading nirs4all-methods…";
  try {
    const M = await loadMethods();
    const record = state.records.find((r) => r.signals && r.signals[plan.source_signal]);
    const sig = record.signals[plan.source_signal];
    const X = { data: Float64Array.from(sig.values), rows: 1, cols: sig.values.length };
    const axis = Array.from(sig.axis.values);
    const ascending = axis.every((x, i) => i === 0 || x > axis[i - 1]);
    const found = new M.SignalTypeDetector({}).run(X, undefined, ascending ? { axis } : {});
    const [type, scale] = DETECTED[found.signal_type] || [null, null];
    const confidence = Number(found.confidence).toFixed(2);
    if (!type) out.textContent = `nirs4all-methods cannot tell (confidence ${confidence}). Answer from what you know about the file.`;
    else out.textContent = `nirs4all-methods suggests ${sigLabel(type)}${scale ? ` in ${scale}` : ""} (confidence ${confidence}). It is a suggestion from the values only: answer from what you know about the file.`;
  } catch (e) {
    out.textContent = String(e?.message || e);
  }
}

function sliceKept(values, keep) {
  const out = [];
  for (const [start, end] of keep) for (let i = start; i < end; i++) out.push(values[i]);
  return out;
}

function countIf(values, test) {
  let n = 0;
  for (const v of values) if (test(v)) n++;
  return n;
}

// One plan step on one record. Methods computes; this moves arrays.
function runStep(step, grid, record, axis, values, classes, summary) {
  const matrix = (v) => ({ data: Float64Array.from(v), rows: 1, cols: v.length });
  const inputs = step.inputs || {};
  if (step.step === "declare_signal_type") return { axis, values };
  if (step.step === "select_points") {
    const kept = sliceKept(values, inputs.keep);
    summary.dropped += values.length - kept.length;
    return { axis: sliceKept(axis, inputs.keep), values: kept };
  }
  const params = { ...step.params };
  if (step.step === "reflectance_from_references") {
    params.white_reference = Array.from(record.signals[inputs.white_reference].values);
    params.dark_reference = inputs.dark_reference ? Array.from(record.signals[inputs.dark_reference].values) : [];
  }
  const estimator = new classes[step.method_id](params);
  try {
    estimator.fit(matrix(values), undefined, inputs.axis ? { axis } : {});
    const out = Array.from(estimator.transform(matrix(values)).data);
    // Validation counts: comparisons of the input with the domain Methods clamps.
    if (step.method_id.endsWith(".to_absorbance") || step.method_id.endsWith(".kubelka_munk")) {
      const scale = step.params.is_percent ? 100 : 1, eps = 1e-10 * scale;
      const high = step.method_id.endsWith(".kubelka_munk") ? scale - eps : Infinity;
      summary.clamped += countIf(values, (v) => Number.isFinite(v) && (v <= eps || v >= high));
    }
    let nextAxis = axis;
    if (step.step === "axis_unit_conversion") {
      nextAxis = Array.from(estimator.fitResult().axis.data);
      summary.axis = `${step.params.source_unit} → ${step.params.target_unit}, put in increasing order`;
    } else if (step.step === "resample") {
      nextAxis = Array.from({ length: grid.points }, (_, i) => grid.first + grid.step * i);
      summary.resampled = `${grid.label}, inside the measured ${fmtNum(Math.min(...axis), 6)}–${fmtNum(Math.max(...axis), 6)} ${prettyUnit(grid.unit)}`;
    }
    const before = countIf(values, (v) => !Number.isFinite(v));
    summary.nan += Math.max(0, countIf(out, (v) => !Number.isFinite(v)) - before);
    return { axis: nextAxis, values: out };
  } finally {
    estimator.dispose();
  }
}

async function runConversion() {
  const plan = conv.plan;
  if (!plan || plan.status !== "ready") return;
  const run = $("#conv-run"), result = $("#conv-result");
  run.disabled = true; run.textContent = "Converting…";
  result.replaceChildren();
  try {
    const M = await loadMethods();
    const version = M.version().split("+")[0];
    const classes = {};
    for (const step of plan.steps) {
      if (!step.method_id) continue;
      try { classes[step.method_id] = M.methodClass(step.method_id); }
      catch (_) {
        throw new Error(`${STEP_LABEL[step.step] || step.step} needs ${step.method_id}, which the @nirs4all/methods ${version} loaded here does not provide yet. It comes with a later Methods release; until then choose another source signal or target.`);
      }
    }
    const out = plan.output, source = plan.source_signal;
    const summary = { records: 0, points: 0, clamped: 0, nan: 0, dropped: 0, axis: null, resampled: null };
    const axisChanged = plan.steps.some((s) => s.step === "axis_unit_conversion" || s.step === "resample");
    // Compute every record first: the page state changes only when all succeed.
    const computed = [];
    state.records.forEach((record, i) => {
      const sig = record.signals && record.signals[source];
      if (!sig) return;
      let axis = Array.from(sig.axis.values), values = Array.from(sig.values);
      for (const step of plan.steps) ({ axis, values } = runStep(step, plan.grid, record, axis, values, classes, summary));
      computed.push({ i, sig, axis, values });
    });
    for (const { i, sig, axis, values } of computed) {
      const record = state.records[i];
      const kind = out.axis_unit === sig.axis.unit ? sig.axis.kind : out.axis_unit === "cm-1" ? "wavenumber" : "wavelength";
      const ordered = axisChanged || plan.steps.some((s) => s.step === "select_points");
      record.signals[out.name] = {
        axis: { values: axis, unit: out.axis_unit, kind, order: ordered ? "ascending" : sig.axis.order },
        values, shape: [values.length], dims: ["x"], signal_type: out.signal_type, unit: out.unit ?? null,
        role: "converted", source: `nirs4all-methods ${version}`,
      };
      record.metadata = record.metadata || {};
      const prior = Array.isArray(record.metadata.conversions) ? record.metadata.conversions.filter((c) => c.signal !== out.name) : [];
      record.metadata.conversions = [...prior, ...plan.steps.map((step) => conversionEntry(step, out.name, version))];
      summary.records++; summary.points = values.length;
    }
    if (!state.signalNames.includes(out.name)) state.signalNames.push(out.name);
    conv.result = { source, name: out.name, target: plan.target, version, summary, steps: plan.steps };
    $("#signals-panel")?.replaceWith(renderSignalsPanel());
    buildTabs();
    selectSignal(out.name);
    renderResult();
    $(".plot-panel")?.scrollIntoView({ behavior: "smooth", block: "start" });
  } catch (e) {
    result.replaceChildren(convNote(String(e?.message || e), "Nothing was added to the page. Change the plan and convert again.", "alert"));
  } finally {
    run.textContent = "Convert with nirs4all-methods";
    run.disabled = !conv.plan || conv.plan.status !== "ready";
  }
}

// Conversion history the Formats writers report and embed (record metadata "conversions").
function conversionEntry(step, signal, version) {
  const entry = { signal, step: step.step, params: { ...step.params, ...(step.inputs || {}) } };
  if (step.method_id) { entry.method_id = step.method_id; entry.methods_version = version; }
  if (step.notes && step.notes.length) entry.notes = step.notes;
  return entry;
}

function renderResult() {
  const r = conv.result, s = r.summary;
  const box = el("div", "conv-summary"); box.setAttribute("role", "status");
  const lines = [
    `${s.records} record${s.records > 1 ? "s" : ""} converted to '${r.name}' (${s.points} points), added next to '${r.source}' in the plot and the signal list.`,
    `Points clamped before a logarithm or Kubelka-Munk: ${s.clamped}.`,
    `Missing values produced: ${s.nan}${s.nan ? " (for example white − dark ≤ 0); formats without missing values refuse them, so narrow the grid range" : ""}.`,
  ];
  if (s.dropped) lines.push(`Overlapping detector points dropped: ${s.dropped}.`);
  lines.push(s.axis ? `Axis converted: ${s.axis}.` : "Axis unit unchanged.");
  lines.push(s.resampled ? `Interpolated onto ${s.resampled}; nothing extrapolated.` : "Not resampled.");
  lines.push(`Computed by nirs4all-methods ${r.version}: ${[...new Set(r.steps.map((st) => st.method_id).filter(Boolean))].join(", ") || "no computation"}.`);
  box.innerHTML = `<strong>Converted — check the spectra before exporting.</strong><ul>${lines.map((l) => `<li>${esc(l)}</li>`).join("")}</ul>`;
  $("#conv-result").replaceChildren(box);
  const exp = $("#conv-export");
  exp.disabled = !r.target;
  exp.title = r.target ? `Write '${r.name}' as ${r.target}` : "Choose a target and convert again to export";
  const compare = $("#conv-compare"); compare.replaceChildren();
  for (const [label, mode] of [["Before", "before"], ["After", "after"], ["Both", "both"]]) {
    const b = el("button", "btn-export"); b.type = "button"; b.textContent = label;
    b.setAttribute("aria-pressed", String(mode === "after"));
    b.addEventListener("click", () => {
      compare.querySelectorAll("button").forEach((x) => x.setAttribute("aria-pressed", String(x === b)));
      if (mode === "before") selectSignal(r.source);
      else if (mode === "after") selectSignal(r.name);
      else drawCompare(r.source, r.name);
    });
    compare.appendChild(b);
  }
}

// Original and converted spectra one above the other, each on its own axis.
function drawCompare(source, converted) {
  unmountAll();
  state.active = converted;
  [...document.querySelectorAll(".sig-tab")].forEach((t) => t.classList.toggle("active", t.textContent.trim() === converted));
  const host = $("#plot-groups"); if (!host) return;
  host.replaceChildren();
  state.ids = new Map();
  for (const [label, name] of [["Before", source], ["After", converted]]) {
    const plan = planSignal(name);
    const head = el("h4", "conv-compare-head"); head.textContent = `${label}: ${name}`; host.appendChild(head);
    const total = plan.groups.length + plan.legacy.length;
    plan.groups.forEach((g, k) => appendSharedGroup(host, g, k, total, name));
    plan.legacy.forEach((g, k) => appendLegacyGroup(host, g, plan.groups.length + k, total, name));
  }
  host.appendChild(selectionLine());
  renderKpis(planSignal(converted));
}

function exportConverted() {
  const r = conv.result;
  if (!r || !r.target) return;
  $("#conv-export-note")?.remove();
  let note;
  try {
    const options = { signal: r.name, base_name: exportBaseName(), axis_tolerance: Number($("#exp-tolerance")?.value) || 0 };
    let output = WASM.writeRecords(state.records, r.target, options);
    if (output.files.length > 1) output = WASM.writeRecords(state.records, r.target, { ...options, zip: true });
    const file = output.files[0];
    download(file.name, file.bytes, file.media_type);
    const lines = output.conversions.map((c) => `Converted: ${STEP_LABEL[c.step] || c.step}${c.method_id ? ` by ${c.method_id} (nirs4all-methods ${c.methods_version})` : " (selection, no computation)"}.`);
    lines.push(...output.losses.map(lossLine));
    const profile = WASM.exportTargets().software.find((p) => p.id === r.target);
    if (profile && !profile.tested) lines.push(`Not tested in ${profile.label} yet. Basis: ${profile.evidence}. ${profile.notes}`.trim());
    note = el("div", "legacy-note"); note.setAttribute("role", "note");
    note.innerHTML = `<strong>${esc(`Exported ${file.name} with '${r.name}'.`)}</strong><ul>${lines.map((l) => `<li>${esc(l)}</li>`).join("")}</ul>`;
  } catch (e) {
    note = convNote(String(e?.message || e), "The converted spectra stay on the page; change the plan (for example a narrower grid) and convert again.", "alert");
  }
  note.id = "conv-export-note";
  $("#conv-result").appendChild(note);
  setStatus(note.textContent);
}

/* ── canvas spectral plot (original renderer) ─────────────────────────── */

class SpectralPlot {
  constructor(canvas, readout) {
    this.cv = canvas; this.ctx = canvas.getContext("2d"); this.readout = readout;
    this.base = document.createElement("canvas");
    this.series = []; this.axis = null;
    this.pad = { l: 56, r: 16, t: 16, b: 34 };
    this.dpr = Math.max(1, window.devicePixelRatio || 1);
    this._ro = new ResizeObserver(() => this.resize());
    this._ro.observe(canvas);
    canvas.addEventListener("pointermove", (e) => this.onMove(e));
    canvas.addEventListener("pointerleave", () => this.clearCursor());
  }

  destroy() { this._ro.disconnect(); }

  setData(series, axis) {
    this.series = series.filter((s) => s.x && s.y && s.x.length);
    this.axis = axis;
    this.reversed = axis ? REVERSED_KIND.has(axis.kind) : false;
    this.computeDomain();
    this.resize();
  }

  computeDomain() {
    let xmin = Infinity, xmax = -Infinity, ymin = Infinity, ymax = -Infinity;
    for (const s of this.series) {
      const { x, y } = s; const n = Math.min(x.length, y.length);
      for (let i = 0; i < n; i++) {
        const xv = x[i], yv = y[i];
        if (Number.isFinite(xv)) { if (xv < xmin) xmin = xv; if (xv > xmax) xmax = xv; }
        if (Number.isFinite(yv)) { if (yv < ymin) ymin = yv; if (yv > ymax) ymax = yv; }
      }
    }
    if (!Number.isFinite(xmin)) { xmin = 0; xmax = 1; }
    if (!Number.isFinite(ymin)) { ymin = 0; ymax = 1; }
    if (xmin === xmax) { xmin -= 0.5; xmax += 0.5; }
    const yPad = (ymax - ymin) * 0.06 || 0.5;
    this.dom = { xmin, xmax, ymin: ymin - yPad, ymax: ymax + yPad };
  }

  resize() {
    const w = this.cv.clientWidth || 600, h = this.cv.clientHeight || 380;
    this.dpr = Math.max(1, window.devicePixelRatio || 1);
    for (const c of [this.cv, this.base]) { c.width = Math.round(w * this.dpr); c.height = Math.round(h * this.dpr); }
    this.w = w; this.h = h;
    this.plotW = w - this.pad.l - this.pad.r;
    this.plotH = h - this.pad.t - this.pad.b;
    this.renderBase();
    this.paint();
  }

  xToPx(v) { let t = (v - this.dom.xmin) / (this.dom.xmax - this.dom.xmin); if (this.reversed) t = 1 - t; return this.pad.l + t * this.plotW; }
  yToPx(v) { const t = (v - this.dom.ymin) / (this.dom.ymax - this.dom.ymin); return this.pad.t + (1 - t) * this.plotH; }
  pxToX(px) { let t = (px - this.pad.l) / this.plotW; if (this.reversed) t = 1 - t; return this.dom.xmin + t * (this.dom.xmax - this.dom.xmin); }

  // per-column representative points (decimation + hover cache)
  columns(s) {
    const cols = Math.max(2, Math.round(this.plotW));
    const rep = new Array(cols + 1).fill(null);
    const { x, y } = s; const n = Math.min(x.length, y.length);
    for (let i = 0; i < n; i++) {
      const xv = x[i], yv = y[i];
      if (!Number.isFinite(xv) || !Number.isFinite(yv)) continue;
      const px = this.xToPx(xv);
      let col = Math.round(px - this.pad.l);
      if (col < 0 || col > cols) continue;
      const center = this.pad.l + col;
      const d = Math.abs(px - center);
      const cur = rep[col];
      if (!cur || d < cur.d) rep[col] = { d, px, py: this.yToPx(yv), x: xv, y: yv };
    }
    const pts = [];
    for (let c = 0; c <= cols; c++) if (rep[c]) pts.push(rep[c]);
    return pts;
  }

  renderBase() {
    const ctx = this.base.getContext("2d");
    ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    ctx.clearRect(0, 0, this.w, this.h);
    const { l, t } = this.pad, pw = this.plotW, ph = this.plotH;

    // plot bg
    ctx.fillStyle = "#fcfdfe"; ctx.fillRect(l, t, pw, ph);

    if (!this.series.length || !this.axis) {
      ctx.fillStyle = "#94a3b8"; ctx.font = "13px ui-monospace, monospace"; ctx.textAlign = "center";
      ctx.fillText("no plottable spectrum", l + pw / 2, t + ph / 2);
      ctx.strokeStyle = "#e2e8f0"; ctx.strokeRect(l + .5, t + .5, pw, ph);
      this.drawn = []; return;
    }

    const xticks = niceTicks(this.dom.xmin, this.dom.xmax, 6);
    const yticks = niceTicks(this.dom.ymin, this.dom.ymax, 5);

    // grid
    ctx.lineWidth = 1; ctx.font = "11px 'JetBrains Mono', ui-monospace, monospace";
    ctx.strokeStyle = "#eef2f6"; ctx.fillStyle = "#94a3b8";
    ctx.textAlign = "center"; ctx.textBaseline = "top";
    for (const xt of xticks) {
      const px = this.xToPx(xt); if (px < l - 1 || px > l + pw + 1) continue;
      ctx.beginPath(); ctx.moveTo(px, t); ctx.lineTo(px, t + ph); ctx.stroke();
      ctx.fillText(fmtNum(xt, 5), px, t + ph + 7);
    }
    ctx.textAlign = "right"; ctx.textBaseline = "middle";
    for (const yt of yticks) {
      const py = this.yToPx(yt); if (py < t - 1 || py > t + ph + 1) continue;
      ctx.beginPath(); ctx.moveTo(l, py); ctx.lineTo(l + pw, py); ctx.stroke();
      ctx.fillStyle = "#94a3b8"; ctx.fillText(fmtNum(yt, 4), l - 9, py);
    }

    // axis frame
    ctx.strokeStyle = "#cbd5e1"; ctx.strokeRect(l + .5, t + .5, pw, ph);

    // axis title
    ctx.fillStyle = "#64748b"; ctx.textAlign = "right"; ctx.textBaseline = "bottom"; ctx.font = "11px 'JetBrains Mono', ui-monospace, monospace";
    ctx.fillText(`${KIND[this.axis.kind] || this.axis.kind} / ${prettyUnit(this.axis.unit)}${this.reversed ? "  ◂" : "  ▸"}`, l + pw, t + ph + 30);

    // lines
    this.drawn = [];
    const many = this.series.length > 8;
    for (const s of this.series) {
      const pts = this.columns(s);
      if (!pts.length) continue;
      // area fill for single spectrum
      if (this.series.length === 1) {
        const g = ctx.createLinearGradient(0, t, 0, t + ph);
        g.addColorStop(0, hexA(s.color, 0.16)); g.addColorStop(1, hexA(s.color, 0));
        ctx.beginPath(); ctx.moveTo(pts[0].px, t + ph);
        for (const p of pts) ctx.lineTo(p.px, p.py);
        ctx.lineTo(pts[pts.length - 1].px, t + ph); ctx.closePath();
        ctx.fillStyle = g; ctx.fill();
      }
      ctx.beginPath();
      pts.forEach((p, i) => (i ? ctx.lineTo(p.px, p.py) : ctx.moveTo(p.px, p.py)));
      ctx.lineWidth = this.series.length === 1 ? 2 : 1.3;
      ctx.strokeStyle = many ? hexA(s.color, 0.78) : s.color;
      ctx.lineJoin = "round"; ctx.lineCap = "round";
      ctx.stroke();
      this.drawn.push({ label: s.label, color: s.color, pts });
    }
  }

  paint() {
    const ctx = this.ctx;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, this.cv.width, this.cv.height);
    ctx.drawImage(this.base, 0, 0);
  }

  onMove(e) {
    if (!this.drawn || !this.drawn.length) return;
    const rect = this.cv.getBoundingClientRect();
    const mx = e.clientX - rect.left, my = e.clientY - rect.top;
    const { l, t } = this.pad;
    if (mx < l || mx > l + this.plotW || my < t || my > t + this.plotH) { this.clearCursor(); return; }

    // nearest drawn point across series
    let best = null;
    for (const d of this.drawn) {
      // binary-ish: pts are column-ordered by px; find closest by px then check py
      let lo = 0, hi = d.pts.length - 1, mid;
      while (lo < hi) { mid = (lo + hi) >> 1; if (d.pts[mid].px < mx) lo = mid + 1; else hi = mid; }
      for (const j of [lo - 1, lo, lo + 1]) {
        const p = d.pts[j]; if (!p) continue;
        const dist = Math.abs(p.px - mx) + Math.abs(p.py - my) * 0.25;
        if (!best || dist < best.dist) best = { dist, p, d };
      }
    }
    this.paint();
    const ctx = this.ctx; ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    // crosshair
    ctx.strokeStyle = "rgba(15,23,42,.22)"; ctx.lineWidth = 1; ctx.setLineDash([4, 4]);
    ctx.beginPath(); ctx.moveTo(mx, t); ctx.lineTo(mx, t + this.plotH); ctx.stroke();
    ctx.setLineDash([]);
    if (best) {
      ctx.beginPath(); ctx.arc(best.p.px, best.p.py, 4.5, 0, 7); ctx.fillStyle = best.d.color; ctx.fill();
      ctx.lineWidth = 2; ctx.strokeStyle = "#fff"; ctx.stroke();
      const ro = this.readout;
      ro.classList.add("on");
      ro.innerHTML = `<div class="ro-series"><span class="sw" style="background:${best.d.color}"></span>${esc(best.d.label)}</div>
        <div><span class="muted">x</span> <span class="ro-x">${fmtNum(best.p.x, 6)}</span> ${prettyUnit(this.axis.unit)}</div>
        <div><span class="muted">y</span> <span class="ro-y">${fmtNum(best.p.y, 6)}</span></div>`;
      // keep readout inside the plot. The readout is positioned relative to
      // .plot-wrap, whose padding insets the canvas by (8px left, 6px top),
      // so offset canvas-relative coords by that padding and clamp both edges.
      const padL = 8, padT = 6, roW = 170;
      let left = best.p.px > this.w * 0.6 ? mx - roW : mx + 18;
      left = Math.max(0, Math.min(this.w - roW, left)) + padL;
      ro.style.left = left + "px";
      ro.style.top = Math.max(0, my - 10) + padT + "px";
    }
    ctx.setTransform(1, 0, 0, 1, 0, 0);
  }

  clearCursor() { this.readout.classList.remove("on"); this.paint(); }
}

/* nice axis ticks */
function niceTicks(min, max, count) {
  if (!(max > min)) return [min];
  const span = max - min, step0 = span / count, mag = Math.pow(10, Math.floor(Math.log10(step0)));
  const norm = step0 / mag, step = (norm >= 5 ? 5 : norm >= 2 ? 2 : norm >= 1 ? 1 : 0.5) * mag;
  const start = Math.ceil(min / step) * step, out = [];
  for (let v = start; v <= max + step * 1e-6; v += step) out.push(Math.round(v / step) * step);
  return out;
}

/* colour helpers */
function hexA(c, a) {
  if (c.startsWith("rgb")) return c.replace("rgb(", "rgba(").replace(")", `,${a})`);
  const h = c.replace("#", "");
  const r = parseInt(h.slice(0, 2), 16), g = parseInt(h.slice(2, 4), 16), b = parseInt(h.slice(4, 6), 16);
  return `rgba(${r},${g},${b},${a})`;
}

function fmtBytes(n) {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(2)} MB`;
}
