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
   openWithSidecars} either way. */
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
  loadSamples();
  renderFormats();

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
  supported: { label: "Supported", cls: "st-supported", dot: "var(--green)" },
  targeted:  { label: "Scoped",    cls: "st-targeted",  dot: "var(--cyan)" },
  partial:   { label: "Partial",   cls: "st-partial",   dot: "var(--amber)" },
  adjacent:  { label: "Adjacent",  cls: "st-adjacent",  dot: "var(--indigo)" },
  sourcing:  { label: "Sourcing",  cls: "st-sourcing",  dot: "var(--text-3)" },
  outscope:  { label: "Out of scope", cls: "st-outscope", dot: "#cbd5e1" },
};
const FMT_ORDER = ["supported", "targeted", "partial", "adjacent", "sourcing", "outscope"];
let FORMATS = null;
const fmtFilter = { q: "", status: new Set() };

async function renderFormats() {
  const section = $("#formats");
  const data = await getFormats();
  if (!data || !Array.isArray(data.formats) || !data.formats.length) { if (section) section.style.display = "none"; return; }
  FORMATS = data.formats;
  const s = data.summary || {};

  const stats = [
    { n: s.families ?? FORMATS.length, l: "format families" },
    { n: s.validated_variants ?? "—", l: "validated variants", c: "c-ok" },
    { n: s.supported ?? "—", l: "fully supported", c: "c-ok" },
    { n: s.partial_variants ?? "—", l: "partial variants", c: "c-part" },
    { n: s.sourcing ?? 0, l: "need samples", c: "c-src" },
  ];
  $("#fmt-stats").innerHTML = stats.map((x) => `<div class="fmt-stat"><div class="n ${x.c || ""}">${esc(x.n)}</div><div class="l">${esc(x.l)}</div></div>`).join("");

  const counts = {};
  for (const f of FORMATS) counts[f.status] = (counts[f.status] || 0) + 1;
  const legend = $("#fmt-legend");
  legend.innerHTML = FMT_ORDER.filter((k) => counts[k]).map((k) =>
    `<button class="fmt-fl" type="button" data-st="${k}"><span class="d" style="background:${FMT_META[k].dot}"></span>${esc(FMT_META[k].label)} <span style="color:var(--text-3)">${counts[k]}</span></button>`).join("");
  legend.querySelectorAll(".fmt-fl").forEach((b) => b.addEventListener("click", () => {
    const st = b.dataset.st;
    if (fmtFilter.status.has(st)) fmtFilter.status.delete(st); else fmtFilter.status.add(st);
    drawFormatRows();
  }));
  $("#fmt-search").addEventListener("input", (e) => { fmtFilter.q = e.target.value.toLowerCase().trim(); drawFormatRows(); });
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
  body.innerHTML = rows.length
    ? rows.map(formatRow).join("")
    : `<tr><td colspan="4" class="fmt-empty">No format matches “${esc(q)}”.</td></tr>`;
  $("#fmt-foot").textContent = `Showing ${rows.length} of ${FORMATS.length} families · generated from the project's FORMAT_MATRIX.md`;
}

function formatRow(f) {
  const m = FMT_META[f.status] || FMT_META.sourcing;
  const total = f.variants || (f.ok + f.partial + f.planned + f.blocked) || 1;
  const seg = (n, col) => (n > 0 ? `<span style="flex:${n};background:${col}"></span>` : "");
  const parts = f.ext.split(",").map((e) => e.trim()).filter(Boolean);
  const exts = parts.slice(0, 5).map((e) => `<code>${esc(e)}</code>`).join("") + (parts.length > 5 ? `<code>+${parts.length - 5}</code>` : "");
  return `<tr style="--rc:${m.dot}">
    <td><div class="f-name">${esc(f.name)}</div><div class="f-vendor">${esc(f.vendor)}</div></td>
    <td><div class="f-ext">${exts}</div></td>
    <td><div class="cov" title="${f.ok} validated · ${f.partial} partial · ${f.planned} planned · ${f.blocked} blocked">
      <div class="cov-bar">${seg(f.ok, "var(--teal)")}${seg(f.partial, "var(--amber)")}${seg(f.planned, "var(--cyan-d)")}${seg(f.blocked, "#cbd5e1")}</div>
      <span class="cov-num"><b>${f.ok}</b>/${total}</span></div></td>
    <td><span class="st ${m.cls}"><span class="d"></span>${esc(m.label)}</span></td>
  </tr>`;
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
const withExtCase = (n, fn) => {
  const s = String(n || "");
  return s.replace(/\.([^.\\/]+)$/, (_, ext) => "." + fn(ext));
};
function sidecarKeyAliases(name, primaryName) {
  const raw = String(name || "");
  const primaryDir = dirOf(primaryName);
  const rel = primaryDir && raw.replace(/\\/g, "/").startsWith(primaryDir)
    ? raw.replace(/\\/g, "/").slice(primaryDir.length)
    : raw;
  const base = fileBaseName(raw);
  const out = new Set([raw, rel, base]);
  for (const key of [raw, rel, base]) {
    out.add(withExtCase(key, (ext) => ext.toLowerCase()));
    out.add(withExtCase(key, (ext) => ext.toUpperCase()));
  }
  return [...out].filter(Boolean);
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

  // ERDAS LAN / AVIRIS uses a same-stem wavelength sidecar
  // (`92AV3C.spc`) plus, for the classic fixture, a different-stem
  // ground-truth sidecar (`92AV3GT.GIS`).
  for (const f of files) {
    if (used.has(f) || extOf(f.name) !== "lan") continue;
    const stem = stemOf(f.name);
    const g = [f];
    used.add(f);
    for (const other of files) {
      if (used.has(other)) continue;
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
    const k = stemOf(f.name);
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
      const score = p.length ? (confRank[p[0].confidence] ?? 0) : -1;
      if (score > best) { best = score; primary = b; probes = p; }
    }
  } else {
    try { probes = WASM.probeBytes(primary.name, primary.bytes); } catch (_) {}
  }
  let records;
  try {
    records = WASM.openBytes(primary.name, primary.bytes);
  } catch (e) {
    const needsSidecar = bufs.length > 1 || /sidecar/i.test(String(e?.message || e));
    if (!needsSidecar) throw e;
    if (bufs.length <= 1) throw e;
    const side = {};
    for (const b of bufs) {
      if (b === primary) continue;
      for (const key of sidecarKeyAliases(b.name, primary.name)) side[key] = b.bytes;
    }
    records = WASM.openWithSidecars(primary.name, primary.bytes, side);
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
const state = { records: null, fileName: "", signalNames: [], active: null, plot: null };

let DATASETS = [], dsActive = 0;

function renderDatasets(datasets) {
  DATASETS = datasets;
  dsActive = Math.max(0, datasets.findIndex((d) => d.ok));
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
    ${withReset ? `<button class="btn-reset" id="reset"><svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/></svg> New file</button>` : ""}`;
  wrap.appendChild(head);
  wrap.appendChild(renderDetect(probes, prov));
  const kpiRow = el("div", "kpis"); kpiRow.id = "kpis"; wrap.appendChild(kpiRow);
  wrap.appendChild(renderPlotPanel());

  const grid = el("div", "grid-2");
  grid.appendChild(renderSignalsPanel());
  const right = el("div"); right.style.display = "grid"; right.style.gap = "20px";
  right.appendChild(renderProvenancePanel(prov, records[0].quality_flags || []));
  right.appendChild(renderMetaPanel(records[0].metadata, records[0].targets));
  grid.appendChild(right);
  wrap.appendChild(grid);
  return wrap;
}

function activateResult() {
  initPlot();
  selectSignal(state.active);
}

function resetView() {
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

/* KPIs (records/signals constant; channels/range/type depend on active signal) */
function renderKpis(active) {
  const recs = state.records;
  const sigCount = Math.max(...recs.map((r) => Object.keys(r.signals || {}).length));
  const s0 = firstSignal(active);
  const ax = s0 ? s0.axis : null;
  const xs = ax ? ax.values.filter(Number.isFinite) : [];
  const lo = xs.length ? Math.min(...xs) : NaN, hi = xs.length ? Math.max(...xs) : NaN;
  const stype = s0 ? (s0.signal_type || recs[0].signal_type) : recs[0].signal_type;
  const kpis = [
    { label: "Records", val: fmtInt(recs.length), unit: recs.length === 1 ? "spectrum" : "spectra", acc: "var(--teal)" },
    { label: "Signals / record", val: sigCount, unit: state.signalNames.join(" · ").slice(0, 28), acc: "var(--cyan)" },
    { label: "Channels", val: s0 ? fmtInt(s0.values.length) : "—", unit: ax ? KIND[ax.kind] || ax.kind : "", acc: "var(--indigo)" },
    { label: "Axis range", val: xs.length ? `${fmtNum(lo, 5)}–${fmtNum(hi, 5)}` : "—", unit: ax ? prettyUnit(ax.unit) + (ax.order ? " · " + ax.order : "") : "", acc: "var(--amber)" },
    { label: "Signal type", val: sigLabel(stype), unit: ax ? KIND[ax.kind] || "" : "", acc: "var(--green)" },
  ];
  const host = $("#kpis"); host.innerHTML = "";
  for (const k of kpis) {
    const n = el("div", "kpi"); n.style.setProperty("--accent", k.acc);
    n.innerHTML = `<div class="k-label">${esc(k.label)}</div><div class="k-val">${esc(k.val)}</div><div class="k-unit">${esc(k.unit || "")}</div>`;
    host.appendChild(n);
  }
}

/* plot panel */
function renderPlotPanel() {
  const p = el("div", "panel plot-panel");
  p.innerHTML = `
    <div class="panel-head">
      <span class="ph-ico"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 3v18h18"/><path d="M4 15c3-1 4-9 7-9s3 7 5 7 2-4 4-5"/></svg></span>
      <h3>Spectra</h3>
      <div class="plot-tools"><div class="sig-tabs" id="sig-tabs"></div></div>
    </div>
    <div class="plot-wrap">
      <canvas class="spectra" id="spectra"></canvas>
      <div class="plot-readout" id="readout"></div>
    </div>
    <div class="plot-legend" id="legend"></div>`;
  return p;
}

/* signals table */
function renderSignalsPanel() {
  const p = el("div", "panel");
  p.innerHTML = `<div class="panel-head"><span class="ph-ico"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M4 4v16M4 20h16"/><path d="M7 14l3-4 3 3 4-6"/></svg></span><h3>Signal channels</h3><span class="ph-meta">${state.signalNames.length} channel${state.signalNames.length > 1 ? "s" : ""}</span></div>`;
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
    for (const [k, v] of Object.entries(targets)) grid.appendChild(el("span", "target", `${esc(k)} <b>${esc(typeof v === "object" ? JSON.stringify(v) : v)}</b>`));
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
  state.active = name;
  [...document.querySelectorAll(".sig-tab")].forEach((t) => t.classList.toggle("active", t.textContent.trim() === name));
  renderKpis(name);
  drawSignal(name);
}

/* collect overlay series for a signal across records (1-D only) */
function collectSeries(name) {
  const out = []; let skipped = 0; let axis = null;
  const recs = state.records;
  for (let i = 0; i < recs.length; i++) {
    const s = recs[i].signals && recs[i].signals[name];
    if (!s) continue;
    if (!s.dims || s.dims.length !== 1 || s.dims[0] !== "x") { skipped++; continue; }
    if (!axis) axis = s.axis;
    const label = recs.length > 1 ? `#${i + 1}` : sigLabel(s.signal_type || name);
    out.push({ x: s.axis.values, y: s.values, label, _i: i });
  }
  // colours
  const single = out.length === 1;
  const baseCol = sigColor(firstSignal(name)?.signal_type, state.signalNames.indexOf(name));
  if (out.length <= 8) {
    out.forEach((sr, i) => (sr.color = single ? baseCol : PALETTE[i % PALETTE.length]));
  } else {
    out.forEach((sr, i) => (sr.color = ramp(i / (out.length - 1)))); // teal→cyan→indigo stack
  }
  return { series: out, skipped, axis };
}

// teal → cyan → indigo ramp
function ramp(t) {
  const stops = [[13, 148, 136], [6, 182, 212], [79, 70, 229]];
  const seg = t < 0.5 ? 0 : 1, lt = t < 0.5 ? t / 0.5 : (t - 0.5) / 0.5;
  const a = stops[seg], b = stops[seg + 1];
  const c = a.map((v, k) => Math.round(v + (b[k] - v) * lt));
  return `rgb(${c[0]},${c[1]},${c[2]})`;
}

function drawSignal(name) {
  const { series, skipped, axis } = collectSeries(name);
  state.plot.setData(series, axis);
  renderLegend(series, skipped, axis);
}

function renderLegend(series, skipped, axis) {
  const host = $("#legend"); if (!host) return;
  host.innerHTML = "";
  if (!series.length) { host.appendChild(el("span", "leg-note", "No 1-D spectra to plot for this signal.")); return; }
  if (series.length <= 8) {
    for (const sr of series) host.appendChild(el("span", "leg-item", `<span class="sw" style="background:${sr.color}"></span>${esc(sr.label)}`));
  } else {
    const grad = el("span", "leg-item");
    grad.innerHTML = `<span class="leg-grad" style="background:linear-gradient(90deg,#0d9488,#06b6d4,#4f46e5)"></span><b>${series.length} spectra</b> overlaid`;
    host.appendChild(grad);
  }
  const unit = axis ? `${KIND[axis.kind] || axis.kind} / ${prettyUnit(axis.unit)}` : "";
  if (unit) host.appendChild(el("span", "leg-note", `· x: ${unit}`));
  if (skipped) host.appendChild(el("span", "leg-note", `· ${skipped} non-1-D record${skipped > 1 ? "s" : ""} skipped`));
}

/* ── canvas spectral plot ────────────────────────────────────────────── */
function initPlot() {
  state.plot = new SpectralPlot($("#spectra"), $("#readout"));
  buildTabs();
}

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
