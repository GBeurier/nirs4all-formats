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
  wireMenusGlobally();

  wasmReady = initWasm();
  wasmReady.then(() => {
    try { $("#stat-version").textContent = WASM.version(); } catch (_) {}
    try { const f = WASM.features(); $("#stat-hdf5").style.display = f && f.hdf5 ? "" : "none"; } catch (_) {}
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
  document.querySelectorAll(".code-install .ci-copy").forEach((button) => {
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
const state = { records: [], fileName: "", signalNames: [], active: null, view: "signal", mounts: [], legacy: [] };

function mountChart(method, host, props) {
  // Match the SVG coordinate system to its container so axis labels stay legible on phones.
  const size = (width) => ({ ...props, width, height: width < 500 ? 260 : 340 });
  let width = host.clientWidth;
  const chart = window.Nirs4allUIBrowser[method](host, size(width));
  const observer = new ResizeObserver(([entry]) => {
    const next = Math.round(entry.contentRect.width);
    if (next > 0 && next !== width) {
      width = next;
      chart.update(size(width));
    }
  });
  observer.observe(host);
  state.mounts.push({ unmount() { observer.disconnect(); chart.unmount(); } });
}
function unmountAll() {
  for (const h of state.mounts.splice(0)) {
    try { h.unmount(); } catch (e) { console.error(e); }
  }
  for (const p of state.legacy.splice(0)) p.destroy();
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
    <button class="btn-reset" id="reset"><svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/></svg> Clear</button>`;
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

// Only the active file has a mounted workspace.
const ICON_DOWNLOAD = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><path d="M7 10l5 5 5-5"/><path d="M12 15V3"/></svg>`;
const ICON_CARET = `<svg class="caret" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m6 9 6 6 6-6"/></svg>`;

function buildResult(ds, withReset) {
  const { records, name: fileName, probes, size, fileCount } = ds;
  const names = [];
  for (const r of records) for (const k of Object.keys(r.signals || {})) if (!names.includes(k)) names.push(k);
  state.records = records; state.fileName = fileName; state.signalNames = names; state.active = names[0] || null; state.view = "signal";
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
      <div class="menu-wrap">
        <button class="btn-primary" id="export-btn" type="button" aria-haspopup="menu" aria-expanded="false" aria-controls="export-menu">${ICON_DOWNLOAD} Export ${ICON_CARET}</button>
        <div class="menu-pop" id="export-menu" role="menu" aria-labelledby="export-btn" hidden></div>
      </div>
      ${withReset ? `<button class="btn-reset" id="reset"><svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/></svg> Clear</button>` : ""}
    </div>`;
  wrap.appendChild(head);
  conv = newConversion(records, names);
  wireExportMenu(head);

  const grid = el("section", "file-properties");
  grid.setAttribute("aria-label", "File properties");
  const provenance = renderProvenancePanel(prov, records[0].quality_flags || []);
  if (probes.length) {
    const details = el("details", "file-details");
    details.append(el("summary", null, "Format detection"), renderDetect(probes, prov));
    $(".panel-body", provenance).appendChild(details);
  }
  grid.append(renderSignalsPanel(), provenance);
  wrap.append(grid, renderPlotPanel(), renderMetaPanel(records));
  return wrap;
}

/* ── menus: one open at a time, keyboard and outside-click aware ───────── */
function openMenu(button, pop) {
  closeMenus();
  pop.hidden = false;
  button.setAttribute("aria-expanded", "true");
  pop.querySelector("[role=menuitem]")?.focus();
}
function closeMenus(returnFocus) {
  document.querySelectorAll(".menu-pop:not([hidden])").forEach((pop) => {
    pop.hidden = true;
    const button = document.querySelector(`[aria-controls="${pop.id}"]`);
    button?.setAttribute("aria-expanded", "false");
    if (returnFocus) button?.focus();
  });
}
function wireMenusGlobally() {
  document.addEventListener("click", (e) => {
    if (!e.target.closest(".menu-wrap")) closeMenus(false);
  });
}
// Disclosure button + role=menu list: Enter/Space/↓ open, ↑/↓/Home/End move, Esc/Tab close.
function wireMenu(button, pop, onPick) {
  const items = () => [...pop.querySelectorAll("[role=menuitem]")];
  button.addEventListener("click", (e) => {
    e.stopPropagation();
    if (pop.hidden) openMenu(button, pop); else closeMenus(true);
  });
  button.addEventListener("keydown", (e) => {
    if (e.key === "ArrowDown" && pop.hidden) { e.preventDefault(); openMenu(button, pop); }
  });
  pop.addEventListener("keydown", (e) => {
    const list = items(), at = list.indexOf(document.activeElement);
    const move = { ArrowDown: at + 1, ArrowUp: at - 1, Home: 0, End: list.length - 1 }[e.key];
    if (move !== undefined) { e.preventDefault(); list[(move + list.length) % list.length].focus(); }
    else if (e.key === "Escape") { e.preventDefault(); closeMenus(true); }
    else if (e.key === "Tab") closeMenus(false);
  });
  pop.addEventListener("click", (e) => {
    const item = e.target.closest("[role=menuitem]");
    if (!item) return;
    closeMenus(true);
    onPick(item.dataset.target);
  });
}

/* ── top Export: targets grouped by the writers' maturity (Formats owns it) ── */
const ZIP_TARGET = "__zip";
// Display order of the standard formats; anything else keeps the catalogue order.
const FORMAT_ORDER = ["csv", "xlsx", "jcamp-dx", "galactic-spc", "foss-winisi-nir", "records-json"];
const MATURITY_GROUPS = [
  { id: "standard", title: "File formats", note: "" },
  { id: "experimental", title: "Experimental", note: "reverse-engineered layouts, no published specification" },
  { id: "other", title: "Other formats", note: "" },
];

function exportCatalog() {
  const targets = WASM.exportTargets();
  // The round-tripped WinISI NIR layout is also offered in the main menu group.
  const menuGroup = new Map(targets.formats.map((f) => [f.format, f.format === "foss-winisi-nir" ? "standard" : f.maturity || "other"]));
  const rank = (f) => { const i = FORMAT_ORDER.indexOf(f.format); return i < 0 ? FORMAT_ORDER.length : i; };
  const groups = Object.fromEntries(MATURITY_GROUPS.map((g) => [g.id, { formats: [], software: [] }]));
  for (const f of [...targets.formats].sort((a, b) => rank(a) - rank(b))) (groups[menuGroup.get(f.format)] || groups.other).formats.push(f);
  for (const p of targets.software) (groups[menuGroup.get(p.format)] || groups.other).software.push(p);
  return { targets, groups };
}

function targetName(id) {
  if (id === ZIP_TARGET) return "ZIP archive";
  const { software, formats } = WASM.exportTargets();
  const profile = software.find((p) => p.id === id);
  if (profile) return profile.label;
  return formats.find((f) => f.format === id)?.label || id;
}

function exportMenuHtml() {
  const { targets, groups } = exportCatalog();
  const formatLabel = new Map(targets.formats.map((f) => [f.format, f.label]));
  const item = (target, label, sub) =>
    `<button class="menu-item" type="button" role="menuitem" data-target="${esc(target)}"><span class="mi-label">${esc(label)}</span><span class="mi-sub">${esc(sub)}</span></button>`;
  return MATURITY_GROUPS.map((g) => {
    const { formats, software } = groups[g.id];
    if (!formats.length && !software.length) return "";
    const headId = `menu-h-${g.id}`;
    let html = `<div class="menu-group" role="group" aria-labelledby="${headId}"><div class="menu-head${g.id === "experimental" ? " exp" : ""}" id="${headId}">${esc(g.title)}${g.note ? ` <small>${esc(g.note)}</small>` : ""}</div><div class="menu-items">`;
    html += formats.map((f) => item(f.format, f.label, `.${f.extension}`)).join("");
    if (g.id === "standard") html += item(ZIP_TARGET, "ZIP archive", "every signal as CSV · .zip");
    html += "</div>";
    if (software.length) {
      html += `<div class="menu-sub">For software</div><div class="menu-items">`
        + software.map((p) => item(p.id, `For ${p.label}`, `.${p.extension} · ${formatLabel.get(p.format) || p.format}`)).join("")
        + "</div>";
    }
    return html + "</div>";
  }).join("");
}

function wireExportMenu(head) {
  const button = $("#export-btn", head), pop = $("#export-menu", head);
  if (!state.signalNames.length) { button.disabled = true; button.title = "No spectral signal to export"; return; }
  pop.innerHTML = exportMenuHtml();
  wireMenu(button, pop, (target) => exportTarget(target));
}

// strip directory + extension from the source name for a sensible download name
const exportBaseName = () => (state.fileName || "spectra").replace(/^.*[\\/]/, "").replace(/\.[^.]+$/, "") || "spectra";

function axisTolerance() {
  const value = Number($("#conv-tolerance")?.value);
  return value > 0 ? value : 0;
}

const lossLine = (loss) =>
  `${loss.message} (${loss.count}×${loss.max_error === undefined ? "" : `, largest error ${loss.max_error.toPrecision(3)}`}).`;

function untestedLine(target) {
  const profile = WASM.exportTargets().software.find((p) => p.id === target);
  return profile && !profile.tested ? `Not tested in ${profile.label} yet. Basis: ${profile.evidence}. ${profile.notes}`.trim() : null;
}

// One report under the header for every export: what was converted, what the target lost.
function showReport(title, lines, alert = false) {
  $("#export-report")?.remove();
  const box = el("div", `report${alert ? " alert" : ""}`); box.id = "export-report";
  box.setAttribute("role", alert ? "alert" : "status");
  const body = el("div", "r-body");
  const strong = el("strong"); strong.textContent = title; body.appendChild(strong);
  if (lines.length) {
    const list = el("ul");
    for (const line of lines) { const li = el("li"); li.textContent = line; list.appendChild(li); }
    body.appendChild(list);
  }
  const close = el("button", "r-close", "×"); close.type = "button"; close.setAttribute("aria-label", "Dismiss");
  close.addEventListener("click", () => box.remove());
  box.append(body, close);
  $(".results-head")?.after(box);
  setStatus([title, ...lines].join(" "));
}

function download(filename, bytes, mime) {
  const url = URL.createObjectURL(new Blob([bytes], { type: mime }));
  const a = el("a"); a.href = url; a.download = filename;
  document.body.appendChild(a); a.click();
  setTimeout(() => { URL.revokeObjectURL(url); a.remove(); }, 0);
}

// Writing happens in Rust; several files are always delivered as one ZIP.
function writeAndDownload(records, target, options) {
  let output = WASM.writeRecords(records, target, options);
  if (output.files.length > 1) output = WASM.writeRecords(records, target, { ...options, zip: true });
  const file = output.files[0];
  download(file.name, file.bytes, file.media_type);
  return { output, file };
}

// Plan for an export target: the converted signal when there is one, else the
// signal on screen, else the planner's own choice.
function planExport(records, target, answers, preferred, allowFallback) {
  const base = { ...answers, axis_tolerance: axisTolerance() };
  if (!allowFallback) return WASM.planConversion(records, target, { ...base, signal: preferred });
  let plan = null;
  if (preferred) {
    try { plan = WASM.planConversion(records, target, { ...base, signal: preferred }); } catch (_) { plan = null; }
  }
  if (!plan || plan.status === "refused") {
    const auto = WASM.planConversion(records, target, base);
    if (!plan || auto.status !== "refused") plan = auto;
  }
  return plan;
}

// Export to a format or a software preset. The planner says what the target needs;
// questions are asked in a dialog, any export-specific conversion runs on a copy,
// then the Rust writer produces the file and lists what it could not carry.
async function exportTarget(id, { records = state.records, signal, baseName = exportBaseName() } = {}) {
  const source = signal || conv.result?.name || state.active;
  const allowFallback = !signal && !conv.result;
  const header = $(".results-head");
  const name = targetName(id);
  const options = { base_name: baseName, axis_tolerance: axisTolerance() };
  try {
    if (id === ZIP_TARGET) {
      const { file, output } = writeAndDownload(records, "csv", { ...options, zip: true });
      showReport(`Exported ${file.name}: every signal as CSV.`, output.losses.map(lossLine));
      return;
    }
    const lines = [];
    if (id !== "records-json") {
      let answers = {};
      let plan = planExport(records, id, answers, source, allowFallback);
      while (plan.status === "needs_answers") {
        answers = await askQuestions(name, plan, answers);
        if (!answers) { setStatus(`Export for ${name} cancelled.`); return; }
        plan = planExport(records, id, answers, source, allowFallback);
      }
      if (plan.status === "refused") { showRefusal(name, plan); return; }
      if (plan.status === "ready") {
        setStatus(`Converting for ${name}…`);
        const methods = await loadMethods();
        if (!header.isConnected) return;
        const converted = computeConversion(plan, records, methods);
        records = converted.records;
        lines.push(`Converted first: ${plan.steps.map((step) => step.description).join(" ")}`);
      }
      options.signal = plan.status === "ready" ? plan.output.name : plan.source_signal;
    }
    const { output, file } = writeAndDownload(records, id, options);
    if (output.conversions.length && !lines.length) {
      lines.push(`Conversions of '${options.signal}', recorded with the export: ${[...new Set(output.conversions.map((c) => STEP_LABEL[c.step] || c.step))].join(", ").toLowerCase()}.`);
    }
    lines.push(...output.losses.map(lossLine));
    const untested = untestedLine(id);
    if (untested) lines.push(untested);
    showReport(`Exported ${file.name} for ${name}${options.signal ? ` from '${options.signal}'` : ""}.`, lines);
  } catch (e) {
    showReport(String(e?.message || e), ["Nothing was downloaded."], true);
  }
}

function dialogShell(title, lead) {
  const dlg = $("#export-dialog");
  dlg.innerHTML = `<div class="dlg-head"><h3 id="dlg-title">${esc(title)}</h3>${lead ? `<p>${esc(lead)}</p>` : ""}</div><div class="dlg-body"></div><div class="dlg-actions"></div>`;
  return { dlg, body: $(".dlg-body", dlg), actions: $(".dlg-actions", dlg) };
}

// The planner's questions, answered in a small dialog; resolves to the answers or null.
function askQuestions(name, plan, answers) {
  return new Promise((resolve) => {
    const { dlg, body, actions } = dialogShell(`Export for ${name}`, "The file does not say this. Answer, and the conversion runs before the download.");
    const listener = new AbortController();
    let settled = false;
    const finish = (value) => {
      if (settled) return;
      settled = true;
      listener.abort();
      if (dlg.open) dlg.close();
      resolve(value);
    };
    for (const question of plan.questions) {
      body.appendChild(questionNode(question, plan, answers[question.id], (value) => finish({ ...answers, [question.id]: value })));
    }
    const cancel = el("button", "btn-export", "Cancel"); cancel.type = "button";
    cancel.addEventListener("click", () => finish(null));
    actions.appendChild(cancel);
    dlg.addEventListener("cancel", () => finish(null), { once: true, signal: listener.signal });
    dlg.showModal();
    body.querySelector(".choice")?.focus();
  });
}

function showRefusal(name, plan) {
  const { dlg, body, actions } = dialogShell(`${name} cannot be written from this file`, "Why, and how to fix it:");
  for (const refusal of plan.refusals) body.appendChild(refusalNode(refusal));
  const close = el("button", "btn-primary", "Close"); close.type = "button";
  close.addEventListener("click", () => dlg.close());
  actions.appendChild(close);
  dlg.showModal();
  close.focus();
  showReport(`Not exported for ${name}.`, plan.refusals.map((r) => `${r.reason}. Fix: ${r.fix}`), true);
}

/* ── chart export: exactly the spectra on screen ──────────────────────── */
const unitSlug = (u) => String(u || "").replace(/[^\w]+/g, "") || "axis";

// Records restricted to the visible signals. In the overlay, an original shown on the
// converted axis is written on that axis (computed by Methods), as its own signal.
function overlayRecords() {
  const r = conv.result;
  const names = [r.source, r.name];
  let derived = null;
  if (r.beforeView) {
    derived = `${r.source}_${unitSlug(r.outUnit)}`;
    while (state.signalNames.includes(derived)) derived += "_view";
  }
  const records = [];
  state.records.forEach((rec, i) => {
    const signals = {};
    for (const n of names) if (rec.signals && rec.signals[n]) signals[n] = rec.signals[n];
    if (!Object.keys(signals).length) return;
    let metadata = rec.metadata;
    const view = derived && signals[r.source] ? r.beforeView.get(i) : null;
    if (view) {
      const src = signals[r.source];
      delete signals[r.source];
      signals[derived] = { ...src, axis: { values: view.axis, unit: r.outUnit, kind: view.kind, order: "ascending" }, values: view.values, shape: [view.values.length] };
      const prior = Array.isArray(rec.metadata?.conversions) ? rec.metadata.conversions.filter((c) => c.signal !== derived) : [];
      metadata = { ...(rec.metadata || {}), conversions: [...prior, conversionEntry(r.axisStep, derived, r.version)] };
    }
    records.push({ ...rec, signals, metadata });
  });
  return records;
}

function spectraDialog() {
  const { dlg, body, actions } = dialogShell("Export spectra");
  const both = state.view === "both";
  const names = state.signalNames;
  const source = el("select"); source.id = "spectra-source";
  names.forEach((name, i) => source.add(new Option(name === conv.result?.name ? `Converted · ${name}` : name, String(i))));
  if (both) source.add(new Option("Original and converted", "both"));
  source.value = both ? "both" : String(names.indexOf(state.active));
  const format = el("select"); format.id = "spectra-format";
  const addField = (label, control) => {
    const field = el("div", "field"), title = el("label", null, label);
    title.htmlFor = control.id;
    field.append(title, control); body.appendChild(field);
  };
  if (names.length > 1) addField("Spectra", source);
  addField("File format", format);
  const setFormats = () => {
    const previous = format.value;
    format.replaceChildren();
    for (const f of WASM.exportTargets().formats) {
      if (source.value !== "both" || ["csv", "records-json"].includes(f.format))
        format.add(new Option(`${f.label} (.${f.extension})`, f.format));
    }
    format.value = [...format.options].some((o) => o.value === previous) ? previous : "csv";
  };
  setFormats();
  source.addEventListener("change", setFormats);
  const cancel = el("button", "btn-export", "Cancel"), submit = el("button", "btn-primary", "Export");
  actions.append(cancel, submit);
  cancel.onclick = () => dlg.close();
  submit.onclick = () => {
    dlg.close();
    const signal = names[Number(source.value)];
    const inputs = signal === conv.result?.name ? state.records : conv.records;
    const baseName = `${exportBaseName()}__spectra`;
    if (source.value === "both" || ["csv", "records-json"].includes(format.value)) {
      try {
        const records = source.value === "both" ? overlayRecords() : inputs
          .filter((r) => r.signals?.[signal])
          .map((r) => ({ ...r, signals: { [signal]: r.signals[signal] } }));
        const { file, output } = writeAndDownload(records, format.value, { base_name: baseName });
        showReport(`Exported spectra as ${file.name}.`, output.losses.map(lossLine));
      } catch (e) { showReport(String(e?.message || e), [], true); }
    } else {
      exportTarget(format.value, { records: inputs.filter((r) => r.signals?.[signal]), signal, baseName });
    }
  };
  dlg.showModal();
}

function activateResult() {
  mountConvert();
  if (!state.signalNames.length) selectSignal(null);
}

function resetView() {
  conv = newConversion();
  DATASETS = [];
  state.records = [];
  state.signalNames = [];
  state.active = null;
  setStatus("File cleared.");
  unmountAll();
  results.classList.remove("show");
  results.innerHTML = "";
  fileInput.value = "";
  $("#pick").focus({ preventScroll: true });
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

function renderPlotPanel() {
  const p = el("div", "panel plot-panel");
  p.innerHTML = `
    <div class="panel-head">
      <span class="ph-ico"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 3v18h18"/><path d="M4 15c3-1 4-9 7-9s3 7 5 7 2-4 4-5"/></svg></span>
      <h3>Spectra</h3>
      <div class="view-tools">
        <div class="seg hidden" id="view-switch" role="group" aria-label="Show the original spectra, the converted spectra or both">
          <button type="button" data-view="before" aria-pressed="false">Original</button><button type="button" data-view="after" aria-pressed="false">Converted</button><button type="button" data-view="both" aria-pressed="false">Both</button>
        </div>
        <button class="btn-export" id="spectra-export" type="button" aria-haspopup="dialog">${ICON_DOWNLOAD} Export spectra</button>
      </div>
    </div>
    <div id="convert-host"></div><div id="plot-groups"></div>`;
  p.querySelectorAll("#view-switch button").forEach((b) => b.addEventListener("click", () => setView(b.dataset.view)));
  $("#spectra-export", p).disabled = !state.signalNames.length;
  $("#spectra-export", p).addEventListener("click", spectraDialog);
  return p;
}

function setView(mode) {
  const r = conv.result;
  if (!r) return;
  if (mode === "before") selectSignal(r.source);
  else if (mode === "after") selectSignal(r.name);
  else drawCompare();
}

function syncViewSwitch() {
  const r = conv.result, host = $("#view-switch");
  if (!host) return;
  host.classList.toggle("hidden", !r);
  if (!r) return;
  const mode = state.view === "both" ? "both" : state.active === r.source ? "before" : state.active === r.name ? "after" : null;
  host.querySelectorAll("button").forEach((b) => b.setAttribute("aria-pressed", String(b.dataset.view === mode)));
}

/* signals table */
function renderSignalsPanel() {
  const p = el("div", "panel"); p.id = "signals-panel";
  p.innerHTML = `<div class="panel-head"><span class="ph-ico"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M4 4v16M4 20h16"/><path d="M7 14l3-4 3 3 4-6"/></svg></span><h3>Signal channels</h3><span class="ph-meta">${fmtInt(state.records.length)} record${state.records.length === 1 ? "" : "s"}</span></div>`;
  const body = el("div", "panel-body");
  const r0 = state.records[0];
  const channels = el("ul", "channel-list");
  const details = el("details", "file-details");
  details.appendChild(el("summary", null, "Channel details"));
  const tbl = el("table", "sig-table");
  tbl.innerHTML = `<thead><tr><th>Name</th><th>Role</th><th>Type</th><th>Dims · shape</th><th>Axis</th></tr></thead>`;
  const tb = el("tbody");
  state.signalNames.forEach((name, i) => {
    const s = r0.signals[name] || firstSignal(name);
    const ax = s ? s.axis : null;
    const col = sigColor(s && s.signal_type, i);
    const item = el("li");
    item.innerHTML = `<span class="sw" style="background:${col}"></span>${esc(name)}`;
    channels.appendChild(item);
    let lo = Infinity, hi = -Infinity;
    for (const x of ax?.values || []) if (Number.isFinite(x)) { lo = Math.min(lo, x); hi = Math.max(hi, x); }
    const range = lo <= hi ? `${fmtNum(lo, 5)}–${fmtNum(hi, 5)} ${prettyUnit(ax.unit)}` : "—";
    const tr = el("tr");
    tr.innerHTML = `
      <td><span class="s-name"><span class="sw" style="background:${col}"></span>${esc(name)}</span>${s && s.unit ? `<div class="mono-sub">${esc(prettyUnit(s.unit))}</div>` : ""}</td>
      <td><span class="pill-s">${esc(s ? s.role || "—" : "—")}</span><div class="mono-sub">${esc(s ? s.source || "" : "")}</div></td>
      <td>${esc(sigLabel(s && s.signal_type))}</td>
      <td><span class="pill-s">${esc(s ? (s.dims || []).join("·") : "")}</span><div class="mono-sub">[${s ? (s.shape || []).join(", ") : ""}]</div></td>
      <td>${ax ? `${esc(KIND[ax.kind] || ax.kind)}<div class="mono-sub">${esc(range)} · ${esc(ax.order || "")}</div>` : "—"}</td>`;
    tb.appendChild(tr);
  });
  tbl.appendChild(tb); details.appendChild(tbl); body.append(channels, details); p.appendChild(body);
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
  const reader = (prov.reader || "—").replace(/^nirs4all_formats::readers::/, "");
  add("Reader", esc(`${reader}${prov.reader_version ? " v" + prov.reader_version : ""}`));
  body.appendChild(dl);

  const details = el("details", "file-details");
  details.appendChild(el("summary", null, `Sources and diagnostics${prov.warnings?.length ? ` · ${prov.warnings.length} warnings` : ""}`));
  if (prov.record_schema_version) details.appendChild(el("p", null, `Record schema ${esc(prov.record_schema_version)}`));
  const sources = prov.sources || [];
  for (const src of sources) {
    if (!src.sha256) continue;
    const short = src.sha256.slice(0, 10) + "…" + src.sha256.slice(-6);
    const row = el("div"); row.style.marginTop = "12px";
    row.innerHTML = `<div class="k-label" style="font-size:.66rem;text-transform:uppercase;letter-spacing:.1em;color:var(--text-3);font-weight:600;margin-bottom:5px">SHA-256${src.role ? " · " + esc(src.role) : ""}</div>
      <span class="hash"><code title="${esc(src.sha256)}">${esc(short)}</code><button class="copy-btn" title="Copy full hash"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="11" height="11" rx="2"/><path d="M5 15V5a2 2 0 0 1 2-2h10"/></svg></button></span>`;
    row.querySelector(".copy-btn").addEventListener("click", (e) => copy(src.sha256, e.currentTarget));
    details.appendChild(row);
  }

  const warnings = prov.warnings || [];
  if (warnings.length) {
    const wh = el("div"); wh.style.marginTop = "14px";
    wh.innerHTML = `<div class="k-label" style="font-size:.66rem;text-transform:uppercase;letter-spacing:.1em;color:var(--amber);font-weight:600;margin-bottom:6px">${warnings.length} reader warning${warnings.length > 1 ? "s" : ""}</div>`;
    for (const w of warnings) {
      const [code, ...rest] = String(w).split(":");
      wh.appendChild(el("div", "warn", `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 9v4m0 4h.01M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h16.9a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z"/></svg><span><code>${esc(code)}</code>${rest.length ? " " + esc(rest.join(":").trim()) : ""}</span>`));
    }
    details.appendChild(wh);
  }

  if (qflags && qflags.length) {
    const q = el("div"); q.style.marginTop = "12px";
    q.innerHTML = `<div class="k-label" style="font-size:.66rem;text-transform:uppercase;letter-spacing:.1em;color:var(--rose);font-weight:600;margin-bottom:6px">Quality flags</div>`;
    for (const f of qflags) q.appendChild(el("span", "pill-s", esc(typeof f === "string" ? f : JSON.stringify(f))));
    details.appendChild(q);
  }

  if (details.childElementCount > 1) body.appendChild(details);
  p.appendChild(body);
  return p;
}

/* Metadata and targets belong to individual decoded records. */
const METADATA_CATEGORIES = [
  ["Sample", /^(sample(?:_|$)|replicate(?:s|_|$)|record_index$|row_index$)/],
  ["Instrument", /^(instrument(?:_|$)|spectrometer(?:_|$)|device(?:_|$)|serial_number$)/],
  ["Acquisition", /^(acquisition(?:_|$)|acquired(?:_|$)|integration_time(?:_|$)|scan(?:s|_|$)|timestamp(?:_|$)|measurement(?:_|$))/],
  ["File and format", /^(file(?:_|$)|source(?:_|$)|vendor(?:_|$)|archive(?:_|$))/],
];

function renderMetaPanel(records) {
  const panel = el("section", "panel metadata-panel");
  panel.setAttribute("aria-labelledby", "metadata-title");
  const head = el("div", "panel-head", '<h3 id="metadata-title">Metadata</h3>');
  const body = el("div", "metadata-groups");
  panel.append(head, body);

  let index = 0;
  const renderRecord = () => {
    const { metadata, targets } = records[index];
    const groups = new Map([...METADATA_CATEGORIES.map(([label]) => [label, []]), ["Other metadata", []]]);
    for (const field of Object.entries(metadata || {})) {
      const [key] = field;
      const category = METADATA_CATEGORIES.find(([, pattern]) => pattern.test(key.toLowerCase()));
      groups.get(category?.[0] || "Other metadata").push(field);
    }
    body.replaceChildren();
    for (const [label, entries] of [["Reference targets", Object.entries(targets || {})], ...groups]) {
      if (!entries.length) continue;
      const group = el("section", "metadata-group");
      if (entries.length > 4 || entries.some(([, value]) => value && typeof value === "object" && !Array.isArray(value))) {
        group.classList.add("wide");
      }
      const heading = el("h4");
      heading.textContent = label;
      group.append(heading, metadataPreview(entries, true));
      body.appendChild(group);
    }
    if (!body.childElementCount) body.appendChild(el("p", "empty-note", "No metadata or reference targets for this record."));
  };

  if (records.length > 1) {
    const picker = el("div", "metadata-record");
    const label = el("label", null, "Record");
    const input = el("input");
    input.id = "metadata-record";
    input.type = "number"; input.min = 1; input.max = records.length; input.step = 1;
    label.htmlFor = input.id;
    const count = el("span", null, `of ${fmtInt(records.length)}`);
    input.setAttribute("aria-label", `Metadata record (1–${records.length})`);
    const previous = el("button", null, "‹"), next = el("button", null, "›");
    previous.type = next.type = "button";
    previous.setAttribute("aria-label", "Previous metadata record");
    next.setAttribute("aria-label", "Next metadata record");
    const showRecord = (selected) => {
      index = selected;
      input.value = index + 1;
      previous.disabled = index === 0;
      next.disabled = index === records.length - 1;
      renderRecord();
    };
    input.addEventListener("change", () => {
      if (input.validity.valid && Number.isInteger(input.valueAsNumber)) showRecord(input.valueAsNumber - 1);
      else input.value = index + 1;
    });
    previous.addEventListener("click", () => showRecord(index - 1));
    next.addEventListener("click", () => showRecord(index + 1));
    picker.append(label, previous, input, next, count);
    head.appendChild(picker);
    showRecord(0);
  } else {
    renderRecord();
  }
  return panel;
}

function metadataDetails(label, count, render, preview) {
  const details = el("details");
  const summary = el("summary");
  const name = el("span");
  name.textContent = label;
  summary.appendChild(name);
  if (count != null) summary.appendChild(el("span", "metadata-count", fmtInt(count)));
  if (preview !== undefined) {
    const excerpt = el("code", "metadata-excerpt");
    excerpt.textContent = preview + "…";
    summary.appendChild(excerpt);
  }
  details.appendChild(summary);
  const populate = () => {
    if (!details.open) return;
    details.appendChild(render());
    details.removeEventListener("toggle", populate);
  };
  details.addEventListener("toggle", populate);
  return details;
}

function metadataPreview(entries, showObjects = false) {
  const visible = 8;
  const fields = renderMetadataFields(entries.slice(0, visible), showObjects);
  if (entries.length > visible) {
    const more = metadataDetails("More fields", entries.length - visible,
      () => renderMetadataFields(entries.slice(visible)));
    more.classList.add("metadata-more");
    fields.appendChild(more);
  }
  return fields;
}

function renderMetadataFields(entries, showObjects = false) {
  const fields = el("div", "metadata-fields");
  if (entries.length > 100) {
    const size = Math.max(100, Math.ceil(entries.length / 20));
    for (let start = 0; start < entries.length; start += size) {
      const end = Math.min(start + size, entries.length);
      fields.appendChild(metadataDetails(`Entries ${fmtInt(start + 1)}–${fmtInt(end)}`, end - start,
        () => renderMetadataFields(entries.slice(start, end))));
    }
    return fields;
  }

  for (const [key, value] of entries) {
    if (value !== null && typeof value === "object") {
      const children = Object.entries(value);
      if (children.length) {
        if (showObjects && !Array.isArray(value)) {
          const group = el("div", "metadata-nested");
          const heading = el("h5");
          heading.textContent = key;
          group.append(heading, metadataPreview(children));
          fields.appendChild(group);
        } else {
          fields.appendChild(metadataDetails(key, children.length, () => renderMetadataFields(children)));
        }
        continue;
      }
    }
    if (typeof value === "string" && (value.length > 64 || value.includes("\n"))) {
      const preview = value.split("\n", 1)[0].slice(0, 36);
      fields.appendChild(metadataDetails(key, null, () => {
        const full = el("code", "metadata-full");
        full.textContent = value;
        return full;
      }, preview));
      continue;
    }
    const cell = el("div", "metadata-field");
    const name = el("span"), display = el("code");
    name.textContent = key;
    display.textContent = value == null ? "null" : Array.isArray(value) ? "[]" : typeof value === "object" ? "{}" : String(value);
    cell.append(name, display);
    fields.appendChild(cell);
  }
  return fields;
}

/* ── signal selection ────────────────────────────────────────────────── */
function firstSignal(name) {
  for (const r of state.records) if (r.signals && r.signals[name]) return r.signals[name];
  return null;
}

function selectSignal(name) {
  // New signal → new axis geometry: release every island before touching the DOM,
  // then remount (the explorer keeps hover/band state that a shorter axis would strand).
  unmountAll();
  state.active = name;
  state.view = "signal";
  const plan = planSignal(name);
  drawSignal(name, plan);
  syncViewSwitch();
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
  const total = plan.groups.length + plan.legacy.length;
  plan.groups.forEach((g, k) => appendSharedGroup(host, g, k, total, name));
  plan.legacy.forEach((g, k) => appendLegacyGroup(host, g, plan.groups.length + k, total, name));
  if (plan.multi.length) host.appendChild(multiNote(plan.multi));
  if (!total) host.appendChild(el("div", "plot-legend", `<span class="leg-note">No 1-D spectra to plot for this signal.</span>`));
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
  return el("div", "ugroup-head", `${total > 1 ? `<h4>Axis group ${k + 1} of ${total}</h4>` : ""}<span class="ugroup-meta">${esc(declared)} · ${fmtInt(count)} record${count === 1 ? "" : "s"}</span>`);
}

function appendSharedGroup(host, g, k, total, name) {
  const { axis, members } = g;
  const declared = axisLabel(axis);
  const node = el("section", "ugroup"); node.dataset.route = "shared";
  node.setAttribute("aria-label", `Axis group ${k + 1} of ${total}: ${declared}`);
  node.appendChild(groupHead(k, total, declared, members.length));
  const chartHost = el("div", "ugroup-chart"); node.appendChild(chartHost);
  const legend = el("div", "plot-legend"); node.appendChild(legend);
  host.appendChild(node);

  const colorAt = seriesColors(members.length, sigColor(members[0].sig.signal_type, state.signalNames.indexOf(name)));
  const series = members.map(({ i, sig }, c) => {
    const id = displayId(i, name);
    const s = { id, values: Array.from(sig.values, (v) => (v === undefined ? null : v)), color: colorAt(c) };
    const label = sampleLabel(state.records[i]);
    if (label !== undefined) s.label = label;
    return s;
  });
  legendInto(legend, members.map(({ i, sig }, c) => ({ color: colorAt(c), label: state.records.length > 1 ? `#${i + 1}` : sigLabel(sig.signal_type || name) })));

  const axisArr = Array.from(axis.values);
  const yType = commonSignalType(members);
  const title = `${fileBaseName(state.fileName)} · ${name} · ${declared}${total > 1 ? ` · group ${k + 1} of ${total}` : ""}`;
  mountChart("mountSpectraExplorer", chartHost, { axis: axisArr, series, axisUnit: prettyUnit(axis.unit) || kindLabel(axis), yLabel: yType ? sigLabel(yType) : name, title, locale: "en" });
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

/* ── Conversion toolbar ───────────────────────────────────────────────
   nirs4all-formats (this page's WASM) decides WHAT a target needs and plans
   the steps (planConversion). Every value is computed by nirs4all-methods
   (its own WASM, vendored in ./vendor/methods/ and loaded only when a
   conversion runs). This file only moves arrays between the two: it selects
   points, builds the grid axis the planner names and counts what the
   validation summary reports. It never computes a spectrum. */
const METHODS_URL = "./vendor/methods/index.js";
let methodsLoading;
async function loadMethods() {
  if (EMBED) {
    throw new Error("Conversions need the hosted page (https://formats.nirs4all.org/) or a local server: the single-file build does not include nirs4all-methods.");
  }
  if (!methodsLoading) {
    methodsLoading = import(METHODS_URL).then(async (module) => {
      await module.loadModule();
      return module;
    }).catch((error) => {
      methodsLoading = null;
      throw error;
    });
  }
  return methodsLoading;
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
function newConversion(records = [], names = []) {
  return { records, names, answers: {}, result: null, revision: 0 };
}

function mountConvert() {
  const host = $("#convert-host");
  if (!host || !conv.names.length) return;
  host.appendChild(convertPanel());
  replan();
}

function convertPanel() {
  const panel = el("section", "convert-panel");
  panel.id = "convert-panel";
  panel.setAttribute("aria-labelledby", "conv-title");
  const select = (id, label, options) => `<label>${label}<select id="${id}">${options}</select></label>`;
  const types = ["absorbance", "reflectance", "transmittance", "kubelka_munk"]
    .map((t) => `<option value="${t}">${esc(sigLabel(t))}</option>`).join("");
  const signals = conv.names.map((name) => `<option value="${esc(name)}">${esc(name)}</option>`).join("");
  const units = '<option value="nm">nm</option><option value="um">µm</option><option value="cm-1">cm⁻¹</option>';
  const number = (id, label, placeholder) => `<div class="field"><label for="${id}">${label}</label><input id="${id}" type="number" step="any" placeholder="${placeholder}"></div>`;
  panel.innerHTML = `<div class="convert-row">
    <h4 id="conv-title">Convert</h4>
    <button type="button" class="conv-help" aria-label="Conversion settings and limits" aria-haspopup="dialog">?</button>
    <div class="convert-controls">
      ${conv.names.length > 1 ? select("conv-signal", "From", signals) : ""}
      ${select("conv-type", "Signal type", '<option value="">Original</option>' + types)}
      ${select("conv-unit", "Axis unit", '<option value="">Original</option>' + units)}
      ${select("conv-grid", "Grid", '<option value="">Measured</option><option value="even">Even</option>')}
    </div>
    </div><div id="conv-plan" aria-live="polite"></div>
    <dialog class="exp-dialog" id="conv-dialog" aria-labelledby="conv-settings-title">
      <div class="dlg-head"><h3 id="conv-settings-title">Conversion settings and limits</h3></div>
      <div class="dlg-body">
        <div class="conv-even hidden">
          <h4>Even grid</h4><div class="field-row">
          ${number("conv-step", "Step", "Default")}${number("conv-min", "From", "Start")}${number("conv-max", "To", "End")}</div>
          <div class="field">${select("conv-interp", "Interpolation", '<option value="linear">Linear</option><option value="nearest">Nearest</option><option value="cubic">Cubic</option>')}</div>
        </div>
        <div class="field conv-splices hidden"><label for="conv-splices">Detector splice wavelengths (nm)</label><input id="conv-splices" placeholder="1001, 1801"></div>
        ${number("conv-tolerance", "Axis tolerance for export", "0")}
        <h4>Limits</h4><ul class="limits-list">${CONVERT_LIMITS.map((l) => `<li>${esc(l)}</li>`).join("")}</ul>
        <div id="conv-result"></div>
      </div><form method="dialog" class="dlg-actions"><button class="btn-export">Close</button></form>
    </dialog>`;
  const signal = $("#conv-signal", panel);
  if (signal) {
    try {
      const preferred = WASM.planConversion(conv.records, null, {}).source_signal;
      if (conv.names.includes(preferred)) signal.value = preferred;
    } catch (_) { /* Every decoded channel remains available to view. */ }
  }
  $(".conv-help", panel).onclick = () => $("#conv-dialog", panel).showModal();
  panel.addEventListener("change", (event) => {
    if (event.target === signal) {
      conv.answers = {};
      $("#conv-splices", panel).value = "";
    }
    replan();
  });
  return panel;
}

function convOptions() {
  const value = (id) => ($(id)?.value || "").trim();
  const options = { ...conv.answers };
  options.signal = value("#conv-signal") || conv.names[0];
  if (value("#conv-type")) options.signal_type = value("#conv-type");
  if (value("#conv-unit")) options.axis_unit = value("#conv-unit");
  if (value("#conv-grid")) options.grid = value("#conv-grid");
  if (options.grid === "even") {
    for (const [id, key] of [["#conv-step", "grid_step"], ["#conv-min", "grid_min"], ["#conv-max", "grid_max"]]) {
      if (value(id)) options[key] = Number(value(id));
    }
  }
  if (options.grid === "even" && value("#conv-interp") !== "linear") options.interpolation = value("#conv-interp");
  if (value("#conv-splices")) {
    const splices = value("#conv-splices").split(/[\s,;]+/).filter(Boolean).map(Number);
    if (splices.some((x) => !Number.isFinite(x))) throw new Error("Detector splices must be numbers separated by commas, for example 1001, 1801.");
    options.splices = splices;
  }
  const tolerance = axisTolerance();
  if (tolerance > 0) options.axis_tolerance = tolerance;
  return options;
}

async function replan() {
  const workspace = conv;
  const revision = ++workspace.revision;
  const host = $("#conv-plan");
  const current = () => host.isConnected && workspace.revision === revision;
  const source = $("#conv-signal")?.value || workspace.names[0];
  const display = (records, result) => {
    state.records = records;
    state.signalNames = result ? [...workspace.names, result.name] : workspace.names;
    workspace.result = result;
    selectSignal(result?.name || source);
  };
  host.replaceChildren();
  $("#conv-result").replaceChildren();
  $(".conv-even").classList.toggle("hidden", $("#conv-grid").value !== "even");
  try {
    const options = convOptions();
    if (!options.signal_type && !options.axis_unit && !options.grid) {
      $(".conv-splices").classList.add("hidden");
      display(workspace.records, null);
      return;
    }
    const plan = WASM.planConversion(workspace.records, null, options);
    $(".conv-splices").classList.toggle("hidden", !plan.steps.some((s) => s.step === "reflectance_from_references") && !options.splices);
    if (plan.status !== "ready") {
      display(workspace.records, null);
      for (const question of plan.questions) {
        host.appendChild(questionNode(question, plan, workspace.answers[question.id], (answer) => {
          workspace.answers[question.id] = answer;
          replan();
        }));
      }
      for (const refusal of plan.refusals) host.appendChild(refusalNode(refusal));
      return;
    }
    host.textContent = "Converting…";
    $("#export-btn").disabled = $("#spectra-export").disabled = true;
    const methods = await loadMethods();
    if (!current()) return;
    const result = computeConversion(plan, workspace.records, methods);
    display(result.records, result);
    renderResult();
    host.replaceChildren();
    setStatus(`Showing ${result.name}.`);
  } catch (e) {
    if (!current()) return;
    display(workspace.records, null);
    host.replaceChildren(convNote(String(e?.message || e), `Showing original signal '${source}'.`, "alert"));
  } finally {
    if (current()) $("#export-btn").disabled = $("#spectra-export").disabled = false;
  }
}

function convNote(title, detail, role) {
  const note = el("div", "conv-note"); note.setAttribute("role", role);
  const head = el("strong"); head.textContent = title; note.appendChild(head);
  if (detail) { const p = el("div"); p.textContent = detail; note.appendChild(p); }
  return note;
}

function refusalNode(refusal) {
  const node = el("div", "conv-refusal");
  const why = el("strong"); why.textContent = `${refusal.reason}.`;
  const fix = el("div"); fix.textContent = `How to fix it: ${refusal.fix}`;
  node.append(why, fix);
  return node;
}

const plural = (n, one, many) => `${n} ${n === 1 ? one : many}`;
// Method ids and parameters stay available, collapsed.
function techDetails(steps, version) {
  const box = el("details", "tech");
  box.innerHTML = `<summary>Technical details</summary><ul>${steps.map((step) =>
    `<li><code>${esc(step.method_id || "selection, no computation")}</code>${Object.keys(step.params || {}).length ? ` <code>${esc(JSON.stringify(step.params))}</code>` : ""}</li>`).join("")}</ul>`
    + (version ? `<p>Computed in the browser by nirs4all-methods ${esc(version)}; planned by nirs4all-formats.</p>` : `<p>Planned by nirs4all-formats; the values are computed in the browser by nirs4all-methods.</p>`);
  return box;
}

// A planner question with its choices as buttons; `onChoose(value)` records the answer.
function questionNode(question, plan, answered, onChoose) {
  const node = el("fieldset", "conv-q");
  const legend = el("legend"); legend.textContent = question.prompt; node.appendChild(legend);
  const choices = el("div", "choices");
  for (const choice of question.choices) {
    const b = el("button", "choice"); b.type = "button"; b.textContent = choice.label;
    b.setAttribute("aria-pressed", String(answered === choice.value));
    b.addEventListener("click", () => onChoose(choice.value));
    choices.appendChild(b);
  }
  const out = el("div", "suggest"); out.setAttribute("role", "status");
  if (question.id !== "absorbance_origin") {
    const suggest = el("button", "choice ghost"); suggest.type = "button"; suggest.textContent = "Suggest from the values";
    suggest.addEventListener("click", () => suggestAnswer(question, plan, out));
    choices.appendChild(suggest);
  }
  node.appendChild(choices);
  const why = el("div", "why"); why.textContent = question.why; node.appendChild(why);
  node.appendChild(out);
  return node;
}

// utilities.signal_type_detector only suggests; the answer stays the user's.
async function suggestAnswer(question, plan, out) {
  out.textContent = "Looking at the values…";
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
    if (!type) out.textContent = `The values do not tell (confidence ${confidence}). Answer from what you know about the file.`;
    else out.textContent = `The values look like ${sigLabel(type)}${scale ? ` in ${scale}` : ""} (confidence ${confidence}). This is only a suggestion: answer from what you know about the file.`;
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

const newSummary = () => ({ records: 0, points: 0, clamped: 0, nan: 0, dropped: 0, axis: null, resampled: null });

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

// Run a ready plan on every record and add the result as a derived signal. Used by
// the Convert panel and by Export when a target needs conversions. When the axis unit
// changes, the original values are also put on the converted axis (by the same Methods
// AxisUnitConversion step) so the overlay can draw both on one x-axis.
function computeConversion(plan, records, M) {
  const version = M.version().split("+")[0];
  const classes = {};
  for (const step of plan.steps) {
    if (!step.method_id) continue;
    try { classes[step.method_id] = M.methodClass(step.method_id); }
    catch (_) {
      throw new Error(`${STEP_LABEL[step.step] || step.step} needs ${step.method_id}, which the nirs4all-methods ${version} loaded here does not provide yet. It comes with a later Methods release; until then choose another source signal or target.`);
    }
  }
  const out = plan.output, source = plan.source_signal;
  const summary = newSummary();
  const axisChanged = plan.steps.some((s) => s.step === "axis_unit_conversion" || s.step === "resample");
  const axisStep = plan.steps.find((s) => s.step === "axis_unit_conversion") || null;
  const outKind = (sig) => (out.axis_unit === sig.axis.unit ? sig.axis.kind : out.axis_unit === "cm-1" ? "wavenumber" : "wavelength");
  // Compute every record first: the page state changes only when all succeed.
  const computed = [];
  records.forEach((record, i) => {
    const sig = record.signals && record.signals[source];
    if (!sig) return;
    let axis = Array.from(sig.axis.values), values = Array.from(sig.values);
    for (const step of plan.steps) ({ axis, values } = runStep(step, plan.grid, record, axis, values, classes, summary));
    let view = null;
    if (axisStep) {
      try { view = runStep(axisStep, null, record, Array.from(sig.axis.values), Array.from(sig.values), classes, newSummary()); }
      catch (_) { view = null; }
    }
    computed.push({ i, sig, axis, values, view });
  });
  const converted = [...records];
  for (const { i, sig, axis, values } of computed) {
    const record = records[i];
    const ordered = axisChanged || plan.steps.some((s) => s.step === "select_points");
    const derived = {
      axis: { values: axis, unit: out.axis_unit, kind: outKind(sig), order: ordered ? "ascending" : sig.axis.order },
      values, shape: [values.length], dims: ["x"], signal_type: out.signal_type, unit: out.unit ?? null,
      role: "converted", source: `from ${source}`,
    };
    const prior = record.metadata?.conversions || [];
    converted[i] = {
      ...record, signals: { ...record.signals, [out.name]: derived },
      metadata: { ...record.metadata, conversions: [...prior, ...plan.steps.map((step) => conversionEntry(step, out.name, version))] },
    };
    summary.records++; summary.points = values.length;
  }
  const beforeView = axisStep && computed.length && computed.every((c) => c.view)
    ? new Map(computed.map((c) => [c.i, { axis: c.view.axis, values: c.view.values, kind: outKind(c.sig) }]))
    : null;
  return { records: converted, source, name: out.name, version, summary, steps: plan.steps, outUnit: out.axis_unit, axisStep, beforeView };
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
  const host = $("#conv-result");
  if (!host) return;
  const box = el("div", "conv-summary"); box.setAttribute("role", "status");
  const lines = [
    `${plural(s.records, "spectrum", "spectra")} of ${s.points} points added as '${r.name}', next to '${r.source}'.`,
    `Points clamped before a logarithm or Kubelka-Munk: ${s.clamped}.`,
    `Missing values produced: ${s.nan}${s.nan ? " (for example white − dark ≤ 0); formats without missing values refuse them, so narrow the grid range" : ""}.`,
  ];
  if (s.dropped) lines.push(`Overlapping detector points dropped: ${s.dropped}.`);
  lines.push(s.axis ? `Axis converted: ${s.axis}.` : "Axis unit unchanged.");
  lines.push(s.resampled ? `Interpolated onto ${s.resampled}; nothing extrapolated.` : "Not resampled.");
  box.innerHTML = `<strong>Current conversion</strong><ul>${lines.map((l) => `<li>${esc(l)}</li>`).join("")}</ul>`;
  box.appendChild(techDetails(r.steps, r.version));
  host.replaceChildren(box);
  syncViewSwitch();
}

// Original and converted spectra overlaid on one chart: shared x-axis, a right
// y-axis when the units differ. The original is drawn on the converted axis
// computed by Methods when the unit changed; otherwise on its own axis values.
function compareProps(r) {
  const before = [], after = [];
  state.records.forEach((rec, i) => {
    const src = rec.signals && rec.signals[r.source], out = rec.signals && rec.signals[r.name];
    if (!src || !out) return;
    const view = r.beforeView ? r.beforeView.get(i) : null;
    before.push({ i, sig: src, axis: view ? view.axis : Array.from(src.axis.values), values: view ? view.values : Array.from(src.values) });
    after.push({ i, sig: out, axis: Array.from(out.axis.values), values: Array.from(out.values) });
  });
  if (!before.length) return null;
  const shared = (list) => list.every((m) => m.values.length === m.axis.length
    && m.axis.length === list[0].axis.length && m.axis.every((v, k) => v === list[0].axis[k]));
  if (!shared(before) || !shared(after)) return null;
  const outAxis = after[0].sig.axis;
  if ((r.beforeView ? r.outUnit : before[0].sig.axis.unit) !== outAxis.unit) return null;
  const yLabel = (sig) => `${sigLabel(sig.signal_type || "unknown")}${sig.unit ? ` (${sig.unit})` : ""}`;
  const colorAt = seriesColors(after.length, sigColor(after[0].sig.signal_type, 0));
  const series = (list, name, color) => list.map((m, k) => {
    const s = { id: displayId(m.i, name), values: m.values.map((v) => (v === undefined ? null : v)), color: color(k) };
    const label = sampleLabel(state.records[m.i]);
    if (label !== undefined) s.label = label;
    return s;
  });
  return {
    title: `${fileBaseName(state.fileName)} · '${r.source}' and '${r.name}' overlaid`,
    axisUnit: `${kindLabel(outAxis)} (${prettyUnit(outAxis.unit)})`,
    xDescending: REVERSED_KIND.has(outAxis.kind),
    before: { id: "before", label: `Original: ${r.source}`, yLabel: yLabel(before[0].sig), axis: before[0].axis, series: series(before, r.source, () => (before.length === 1 ? "#64748b" : "#94a3b8")), dashed: true },
    after: { id: "after", label: `Converted: ${r.name}`, yLabel: yLabel(after[0].sig), axis: after[0].axis, series: series(after, r.name, colorAt) },
    locale: "en",
  };
}

function drawCompare() {
  const r = conv.result;
  if (!r) return;
  unmountAll();
  state.active = r.name;
  state.view = "both";
  const host = $("#plot-groups"); if (!host) return;
  host.replaceChildren();
  const props = compareProps(r);
  const node = el("section", "ugroup"); node.dataset.route = "compare";
  node.appendChild(el("div", "ugroup-head", `<h4>Original and converted, overlaid</h4><span class="ugroup-meta">${esc(r.source)} → ${esc(r.name)}</span>`));
  host.appendChild(node);
  if (props) {
    const chartHost = el("div", "compare-host"); node.appendChild(chartHost);
    mountChart("mountSpectraCompare", chartHost, props);
  } else {
    // Records on several axes cannot share one overlay: show them one above the other.
    const note = el("p", "legacy-note"); note.setAttribute("role", "note");
    note.textContent = "The records do not share one axis, so the original and converted spectra are shown one above the other.";
    node.appendChild(note);
    for (const [label, name] of [["Original", r.source], ["Converted", r.name]]) {
      const plan = planSignal(name);
      const head = el("h4", "conv-compare-head"); head.textContent = `${label}: ${name}`; host.appendChild(head);
      const total = plan.groups.length + plan.legacy.length;
      plan.groups.forEach((g, k) => appendSharedGroup(host, g, k, total, name));
      plan.legacy.forEach((g, k) => appendLegacyGroup(host, g, plan.groups.length + k, total, name));
    }
  }
  syncViewSwitch();
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
