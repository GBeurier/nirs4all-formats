// SPDX-License-Identifier: CECILL-2.1 OR AGPL-3.0-or-later
// Thin, caller-buffer wrappers for the public Methods Preflight diagnostics.
import { checkStatus, getModule, makeMatrixView } from "./ffi.js";
import { Context } from "./context.js";
import { N4mError, Status } from "./types.js";
function validatePermutation(nPermutations, seed) {
    if (!Number.isInteger(nPermutations) || nPermutations < 0 || nPermutations > 1_000_000) {
        throw new RangeError("permutation count must be an integer in [0, 1000000]");
    }
    if (typeof seed === "number" && !Number.isSafeInteger(seed)) {
        throw new RangeError("seeds supplied as numbers must be safe integers");
    }
    const seed64 = BigInt(seed);
    if (seed64 < 0n || seed64 >= (1n << 64n))
        throw new RangeError("seed is outside uint64 range");
    return seed64;
}
function validateMatrix(X) {
    if (!Number.isSafeInteger(X.rows) || !Number.isSafeInteger(X.cols) ||
        X.rows < 1 || X.cols < 1 || X.data.length !== X.rows * X.cols) {
        throw new RangeError("invalid diagnostic matrix dimensions");
    }
}
/** Integer in [lo, hi] and representable as a C int32_t (Emscripten passes
 * "number" arguments as 32-bit integers and would wrap larger values). */
function isInt32In(value, lo, hi) {
    return Number.isInteger(value) && value >= -0x80000000 && value <= 0x7fffffff &&
        value >= lo && value <= hi;
}
/** Validated Savitzky-Golay domain of the native noise proxy. */
const NOISE_PROXY_MAX_WINDOW = 1001;
const NOISE_PROXY_MAX_POLYORDER = 10;
function validateGroupIds(values) {
    for (const value of values) {
        if (typeof value === "number" && !Number.isSafeInteger(value)) {
            throw new RangeError("group IDs supplied as numbers must be safe integers");
        }
        const id = BigInt(value);
        if (id < -(1n << 63n) || id >= (1n << 63n)) {
            throw new RangeError("group ID is outside signed int64 range");
        }
    }
}
class NativeScope {
    ctx = Context.create();
    releases = [];
    alloc(bytes) {
        if (!Number.isSafeInteger(bytes) || bytes < 0 || bytes > 0x7fffffff) {
            throw new RangeError("diagnostic buffer exceeds WASM allocation limit");
        }
        const m = getModule();
        const ptr = m._malloc(Math.max(1, bytes));
        if (ptr === 0)
            throw new N4mError(Status.ERR_OUT_OF_MEMORY, "WASM allocation failed");
        this.releases.push(() => m._free(ptr));
        return ptr;
    }
    matrix(data, rows, cols) {
        const v = makeMatrixView(data, rows, cols);
        this.releases.push(v.free);
        return v;
    }
    i64(values) {
        const ptr = this.alloc(values.length * 8);
        const dv = new DataView(getModule().HEAPU8.buffer);
        for (let i = 0; i < values.length; i++) {
            dv.setBigInt64(ptr + i * 8, BigInt(values[i]), true);
        }
        return ptr;
    }
    close() {
        for (let i = this.releases.length - 1; i >= 0; --i)
            this.releases[i]();
        this.ctx.destroy();
    }
}
function copyF64(ptr, n) {
    return new Float64Array(getModule().HEAPF64.slice(ptr / 8, ptr / 8 + n));
}
/** Centered PCA (exact Gram eigendecomposition in Methods). Output matrices are row-major. */
export function preflightPca(X, k) {
    validateMatrix(X);
    if (!isInt32In(k, 1, Math.min(X.rows, X.cols))) {
        throw new RangeError("invalid PCA component count");
    }
    const m = getModule();
    const scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const x = scope.matrix(X.data, X.rows, X.cols);
        const scores = scope.matrix(new Float64Array(X.rows * k), X.rows, k);
        const loadings = scope.matrix(new Float64Array(k * X.cols), k, X.cols);
        const eigen = scope.alloc(k * 8), moments = scope.alloc(16);
        const resolution = scope.alloc(8), unresolved = scope.alloc(k);
        const st = m.ccall("n4m_preflight_pca_compute_resolved", "number", ["number", "number", "number", "number", "number", "number", "number", "number",
            "number"], [ctx.handle, x.viewPtr, k, scores.viewPtr, loadings.viewPtr, eigen, moments,
            resolution, unresolved]);
        checkStatus(st, ctx.handle);
        return {
            scores: { data: copyF64(scores.dataPtr, X.rows * k), rows: X.rows, cols: k },
            loadings: { data: copyF64(loadings.dataPtr, k * X.cols), rows: k, cols: X.cols },
            eigenvalues: copyF64(eigen, k),
            fullMoments: [m.HEAPF64[moments / 8], m.HEAPF64[moments / 8 + 1]],
            resolution: m.HEAPF64[resolution / 8],
            unresolved: m.HEAPU8.slice(unresolved, unresolved + k),
        };
    }
    finally {
        scope.close();
    }
}
/** Per-feature pooled repeatability and balanced ICC(1,1). */
export function preflightRepeatability(X, groupIds, validMask) {
    validateMatrix(X);
    if (groupIds.length !== X.rows || (validMask && validMask.length !== X.data.length)) {
        throw new RangeError("repeatability group or mask length mismatch");
    }
    validateGroupIds(groupIds);
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const x = scope.matrix(X.data, X.rows, X.cols);
        const groups = scope.i64(groupIds);
        const mask = validMask ? scope.alloc(validMask.length) : 0;
        const sd = scope.alloc(X.cols * 8), icc = scope.alloc(X.cols * 8);
        const counts = scope.alloc(X.cols * 8), status = scope.alloc(X.cols * 4);
        if (validMask)
            m.HEAPU8.set(validMask, mask);
        const st = m.ccall("n4m_preflight_repeatability_compute", "number", Array(8).fill("number"), [ctx.handle, x.viewPtr, groups, mask, sd, icc, counts, status]);
        checkStatus(st, ctx.handle);
        const dv = new DataView(m.HEAPU8.buffer);
        return {
            pooledSd: copyF64(sd, X.cols), icc: copyF64(icc, X.cols),
            nCompleteGroups: Array.from({ length: X.cols }, (_, j) => Number(dv.getBigInt64(counts + j * 8, true))),
            bandStatus: new Int32Array(m.HEAP32.slice(status / 4, status / 4 + X.cols)),
        };
    }
    finally {
        scope.close();
    }
}
/** Finite-value summary: count, min, max, mean, sample SD, Q1, median, Q3. */
export function preflightSummary(values, validMask) {
    if (validMask && validMask.length !== values.length)
        throw new RangeError("summary mask length mismatch");
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const input = scope.alloc(values.length * 8);
        const mask = validMask ? scope.alloc(validMask.length) : 0;
        const out = scope.alloc(64);
        m.HEAPF64.set(values, input / 8);
        if (validMask)
            m.HEAPU8.set(validMask, mask);
        const st = m.ccall("n4m_preflight_summary_compute", "number", ["number", "number", "number", "i64", "number"], [ctx.handle, input, mask, BigInt(values.length), out]);
        checkStatus(st, ctx.handle);
        return copyF64(out, 8);
    }
    finally {
        scope.close();
    }
}
/** Euclidean reference neighbors, optionally excluding equal group IDs. */
export function preflightNearestNeighbors(query, reference, k, queryGroups, referenceGroups) {
    validateMatrix(query);
    validateMatrix(reference);
    if (query.cols !== reference.cols || !isInt32In(k, 1, 0x7fffffff) ||
        (queryGroups === undefined) !== (referenceGroups === undefined) ||
        (queryGroups && queryGroups.length !== query.rows) ||
        (referenceGroups && referenceGroups.length !== reference.rows)) {
        throw new RangeError("invalid neighbor shapes, groups or k");
    }
    if (queryGroups)
        validateGroupIds(queryGroups);
    if (referenceGroups)
        validateGroupIds(referenceGroups);
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const q = scope.matrix(query.data, query.rows, query.cols);
        const r = scope.matrix(reference.data, reference.rows, reference.cols);
        const qg = queryGroups ? scope.i64(queryGroups) : 0;
        const rg = referenceGroups ? scope.i64(referenceGroups) : 0;
        const len = query.rows * k;
        const idx = scope.alloc(len * 8), dist = scope.alloc(len * 8);
        const st = m.ccall("n4m_preflight_nearest_neighbors", "number", Array(8).fill("number"), [ctx.handle, q.viewPtr, r.viewPtr, qg, rg, k, idx, dist]);
        checkStatus(st, ctx.handle);
        const dv = new DataView(m.HEAPU8.buffer);
        return {
            indices: Array.from({ length: len }, (_, j) => Number(dv.getBigInt64(idx + j * 8, true))),
            distances: copyF64(dist, len), rows: query.rows, k,
        };
    }
    finally {
        scope.close();
    }
}
/** Local Y coherence using an existing row-major n x k neighbor-index table. */
export function preflightLocalContrast(targets, neighborIndices, k, categorical, groupIds) {
    if (!isInt32In(k, 1, 0x7fffffff) || targets.length < 1 ||
        neighborIndices.length !== targets.length * k ||
        (groupIds && groupIds.length !== targets.length)) {
        throw new RangeError("invalid local contrast dimensions or groups");
    }
    validateGroupIds(neighborIndices);
    if (groupIds)
        validateGroupIds(groupIds);
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const y = scope.alloc(targets.length * 8);
        const neighbors = scope.i64(neighborIndices);
        const groups = groupIds ? scope.i64(groupIds) : 0;
        const out = scope.alloc(32);
        m.HEAPF64.set(targets, y / 8);
        const st = m.ccall("n4m_preflight_local_contrast", "number", ["number", "number", "number", "number", "i64", "number", "number", "number"], [ctx.handle, y, neighbors, groups, BigInt(targets.length), k,
            categorical ? 1 : 0, out]);
        checkStatus(st, ctx.handle);
        const result = copyF64(out, 4);
        return {
            contrast: result[0], fractionRowsWithNeighbors: result[1],
            observedSameClassFraction: result[2], expectedSameClassFraction: result[3],
        };
    }
    finally {
        scope.close();
    }
}
/** Euclidean HDBSCAN* (scikit-learn 1.9.1 semantics, stable MST tie order). */
export function clusterHdbscan(X, options) {
    validateMatrix(X);
    const { minClusterSize, minSamples } = options;
    if (X.rows < 2 || !Number.isSafeInteger(minClusterSize) || minClusterSize < 2 ||
        !Number.isSafeInteger(minSamples) || minSamples < 1 || minSamples > X.rows) {
        throw new RangeError("invalid HDBSCAN dimensions or parameters");
    }
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const x = scope.matrix(X.data, X.rows, X.cols);
        const perRow = () => scope.alloc(X.rows * 8);
        const labels = perRow(), membership = perRow(), outlier = perRow();
        const persistence = perRow(), parent = perRow(), size = perRow(), label = perRow();
        const birth = perRow(), death = perRow(), stability = perRow();
        const nClusters = scope.alloc(8), nNodes = scope.alloc(8);
        const st = m.ccall("n4m_cluster_hdbscan", "number", ["number", "number", "i64", "i64", "number", "number", ...Array(12).fill("number")], [ctx.handle, x.viewPtr, BigInt(minClusterSize), BigInt(minSamples),
            options.selection === "leaf" ? 1 : 0, options.allowSingleCluster ? 1 : 0,
            labels, membership, outlier, nClusters, persistence, nNodes, parent, size, label,
            birth, death, stability]);
        checkStatus(st, ctx.handle);
        const dv = new DataView(m.HEAPU8.buffer);
        const i64 = (ptr, n) => Array.from({ length: n }, (_, j) => Number(dv.getBigInt64(ptr + j * 8, true)));
        const k = Number(dv.getBigInt64(nNodes, true));
        const clusters = Number(dv.getBigInt64(nClusters, true));
        const parents = i64(parent, k), sizes = i64(size, k), treeLabels = i64(label, k);
        const births = copyF64(birth, k), deaths = copyF64(death, k);
        const stabilities = copyF64(stability, k);
        return {
            labels: i64(labels, X.rows),
            membership: copyF64(membership, X.rows),
            outlierScores: copyF64(outlier, X.rows),
            nClusters: clusters,
            persistence: copyF64(persistence, clusters),
            tree: Array.from({ length: k }, (_, j) => ({
                parent: parents[j], size: sizes[j], label: treeLabels[j],
                lambdaBirth: births[j], lambdaDeath: deaths[j], stability: stabilities[j],
            })),
        };
    }
    finally {
        scope.close();
    }
}
/** Adjusted Rand index; NaN for n<2 or identical trivial partitions. Noise is an ordinary label. */
export function preflightAdjustedRandIndex(a, b) {
    if (a.length !== b.length)
        throw new RangeError("ARI label length mismatch");
    validateGroupIds(a);
    validateGroupIds(b);
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const pa = scope.i64(a), pb = scope.i64(b), out = scope.alloc(8);
        const st = m.ccall("n4m_preflight_adjusted_rand_index", "number", ["number", "number", "number", "i64", "number"], [ctx.handle, pa, pb, BigInt(a.length), out]);
        checkStatus(st, ctx.handle);
        return m.HEAPF64[out / 8];
    }
    finally {
        scope.close();
    }
}
/**
 * SNV with rounding bounds (ABI 2.20 n4m_preflight_snv_compute). With
 * `groupIds` (dense codes 0..nGroups-1, every group non-empty), each output
 * row is the equal-weight mean of the SNV rows of one group.
 */
export function preflightSnv(X, groupIds, nGroups) {
    validateMatrix(X);
    const groups = groupIds ? nGroups : X.rows;
    if (X.cols < 2 || groups === undefined || !isInt32In(groups, 1, X.rows) ||
        (groupIds && groupIds.length !== X.rows) || (!groupIds && nGroups !== undefined &&
        nGroups !== X.rows)) {
        throw new RangeError("invalid SNV shape or groups");
    }
    if (groupIds)
        validateGroupIds(groupIds);
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const x = scope.matrix(X.data, X.rows, X.cols);
        const out = scope.matrix(new Float64Array(groups * X.cols), groups, X.cols);
        const gid = groupIds ? scope.i64(groupIds) : 0;
        const bounds = scope.alloc(groups * 8);
        const st = m.ccall("n4m_preflight_snv_compute", "number", ["number", "number", "number", "i64", "number", "number"], [ctx.handle, x.viewPtr, gid, BigInt(groups), out.viewPtr, bounds]);
        checkStatus(st, ctx.handle);
        return {
            matrix: { data: copyF64(out.dataPtr, groups * X.cols), rows: groups, cols: X.cols },
            errorBounds: copyF64(bounds, groups),
        };
    }
    finally {
        scope.close();
    }
}
/** Savitzky-Golay smoothing-residual noise proxy; interior features only. */
export function preflightNoiseProxy(X, windowLength, polyorder, validMask) {
    validateMatrix(X);
    if (!isInt32In(windowLength, 3, Math.min(NOISE_PROXY_MAX_WINDOW, X.cols)) ||
        windowLength % 2 === 0 ||
        !isInt32In(polyorder, 0, Math.min(NOISE_PROXY_MAX_POLYORDER, windowLength - 2)) ||
        (validMask && validMask.length !== X.data.length)) {
        throw new RangeError("noise proxy needs an odd window in [3, min(1001, cols)], " +
            "polyorder in [0, min(10, window - 2)] and a matching mask");
    }
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const x = scope.matrix(X.data, X.rows, X.cols);
        const mask = validMask ? scope.alloc(validMask.length) : 0;
        const band = scope.alloc(X.cols * 8), counts = scope.alloc(X.cols * 8);
        const out = scope.alloc(16);
        if (validMask)
            m.HEAPU8.set(validMask, mask);
        const st = m.ccall("n4m_preflight_noise_proxy_compute", "number", Array(8).fill("number"), [ctx.handle, x.viewPtr, mask, windowLength, polyorder, band, counts, out]);
        checkStatus(st, ctx.handle);
        const dv = new DataView(m.HEAPU8.buffer);
        return {
            bandNoise: copyF64(band, X.cols),
            bandCounts: Array.from({ length: X.cols }, (_, j) => Number(dv.getBigInt64(counts + j * 8, true))),
            pooled: m.HEAPF64[out / 8], gain: m.HEAPF64[out / 8 + 1],
        };
    }
    finally {
        scope.close();
    }
}
/**
 * Row pairs with root-mean-square feature difference <= tolerance. Without a
 * reference, query rows are compared with each other (i < h).
 */
export function preflightPairsWithinTolerance(query, tolerance, maxPairs, reference, queryGroups, referenceGroups) {
    validateMatrix(query);
    if (reference)
        validateMatrix(reference);
    if (!Number.isSafeInteger(maxPairs) || maxPairs < 0 ||
        (reference && reference.cols !== query.cols) ||
        (queryGroups && queryGroups.length !== query.rows) ||
        (reference && (queryGroups === undefined) !== (referenceGroups === undefined)) ||
        (referenceGroups && (!reference || referenceGroups.length !== reference.rows))) {
        throw new RangeError("invalid pair shapes, groups or listing budget");
    }
    if (queryGroups)
        validateGroupIds(queryGroups);
    if (referenceGroups)
        validateGroupIds(referenceGroups);
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const q = scope.matrix(query.data, query.rows, query.cols);
        const r = reference ? scope.matrix(reference.data, reference.rows, reference.cols) : undefined;
        const qg = queryGroups ? scope.i64(queryGroups) : 0;
        const rg = referenceGroups ? scope.i64(referenceGroups) : 0;
        const idx = maxPairs ? scope.alloc(maxPairs * 16) : 0;
        const dist = maxPairs ? scope.alloc(maxPairs * 8) : 0;
        const qf = scope.alloc(query.rows);
        const rf = reference ? scope.alloc(reference.rows) : 0;
        const counts = scope.alloc(16);
        const st = m.ccall("n4m_preflight_pairs_within_tolerance", "number", ["number", "number", "number", "number", "number", "number", "i64",
            "number", "number", "number", "number", "number"], [ctx.handle, q.viewPtr, r ? r.viewPtr : 0, qg, rg, tolerance, BigInt(maxPairs),
            idx, dist, qf, rf, counts]);
        checkStatus(st, ctx.handle);
        const dv = new DataView(m.HEAPU8.buffer);
        const total = Number(dv.getBigInt64(counts, true));
        const listed = Math.min(total, maxPairs);
        return {
            pairs: Array.from({ length: listed * 2 }, (_, j) => Number(dv.getBigInt64(idx + j * 8, true))),
            distances: copyF64(dist, listed),
            queryFlags: new Uint8Array(m.HEAPU8.slice(qf, qf + query.rows)),
            referenceFlags: reference ? new Uint8Array(m.HEAPU8.slice(rf, rf + reference.rows))
                : new Uint8Array(0),
            totalPairs: total,
            candidatePairs: Number(dv.getBigInt64(counts + 8, true)),
        };
    }
    finally {
        scope.close();
    }
}
/** Factor-target association over exchangeable units with seeded permutations.
 * The seed is an unsigned 64-bit integer; pass a bigint above 2^53. */
export function preflightFactorAssociation(factorCodes, targets, categorical, nPermutations, seed) {
    if (factorCodes.length !== targets.length || targets.length < 2 ||
        !isInt32In(nPermutations, 0, 1_000_000)) {
        throw new RangeError("invalid association dimensions or permutation count");
    }
    validateGroupIds(factorCodes);
    if (typeof seed === "number" && !Number.isSafeInteger(seed)) {
        throw new RangeError("seeds supplied as numbers must be safe integers");
    }
    const seed64 = BigInt(seed);
    if (seed64 < 0n || seed64 >= (1n << 64n))
        throw new RangeError("seed is outside uint64 range");
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const codes = scope.i64(factorCodes);
        const y = scope.alloc(targets.length * 8);
        const out = scope.alloc(64), counts = scope.alloc(16);
        m.HEAPF64.set(targets, y / 8);
        const st = m.ccall("n4m_preflight_factor_association", "number", ["number", "number", "number", "i64", "number", "number", "i64", "number", "number"], [ctx.handle, codes, y, BigInt(targets.length), categorical ? 1 : 0,
            nPermutations, seed64, out, counts]);
        checkStatus(st, ctx.handle);
        const r = copyF64(out, 8);
        const dv = new DataView(m.HEAPU8.buffer);
        return {
            effectSize: r[0], statistic: r[1], df1: r[2], df2: r[3],
            nLevels: r[4], nTargetValues: r[5], log10DistinctPermutations: r[6],
            pValue: r[7],
            nAtLeastObserved: Number(dv.getBigInt64(counts, true)),
            nPermutationsExecuted: Number(dv.getBigInt64(counts + 8, true)),
        };
    }
    finally {
        scope.close();
    }
}
/** Holm step-down adjusted p-values for one hypothesis family. */
export function preflightHolmAdjust(pValues) {
    if (pValues.length < 1)
        throw new RangeError("Holm family must not be empty");
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const input = scope.alloc(pValues.length * 8), out = scope.alloc(pValues.length * 8);
        m.HEAPF64.set(pValues, input / 8);
        const st = m.ccall("n4m_preflight_holm_adjust", "number", ["number", "number", "i64", "number"], [ctx.handle, input, BigInt(pValues.length), out]);
        checkStatus(st, ctx.handle);
        return copyF64(out, pValues.length);
    }
    finally {
        scope.close();
    }
}
/** Pearson or Spearman correlation over exchangeable units with a seeded
 * two-sided permutation test of y. The seed is an unsigned 64-bit integer;
 * pass a bigint above 2^53. */
export function statsCorrelationTest(x, y, method, nPermutations, seed) {
    if (x.length !== y.length || x.length < 2 || (method !== "pearson" && method !== "spearman") ||
        !Number.isInteger(nPermutations) || nPermutations < 0 || nPermutations > 1_000_000) {
        throw new RangeError("invalid correlation dimensions, method or permutation count");
    }
    if (typeof seed === "number" && !Number.isSafeInteger(seed)) {
        throw new RangeError("seeds supplied as numbers must be safe integers");
    }
    const seed64 = BigInt(seed);
    if (seed64 < 0n || seed64 >= (1n << 64n))
        throw new RangeError("seed is outside uint64 range");
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const xs = scope.alloc(x.length * 8), ys = scope.alloc(y.length * 8);
        const out = scope.alloc(40), counts = scope.alloc(16);
        m.HEAPF64.set(x, xs / 8);
        m.HEAPF64.set(y, ys / 8);
        const st = m.ccall("n4m_stats_correlation_test", "number", ["number", "number", "number", "i64", "number", "number", "i64", "number", "number"], [ctx.handle, xs, ys, BigInt(x.length), method === "spearman" ? 1 : 0,
            nPermutations, seed64, out, counts]);
        checkStatus(st, ctx.handle);
        const r = copyF64(out, 5);
        const dv = new DataView(m.HEAPU8.buffer);
        return {
            coefficient: r[0], nDistinctX: r[1], nDistinctY: r[2],
            log10DistinctPermutations: r[3], pValue: r[4],
            nAtLeastObserved: Number(dv.getBigInt64(counts, true)),
            nPermutationsExecuted: Number(dv.getBigInt64(counts + 8, true)),
        };
    }
    finally {
        scope.close();
    }
}
/** Benjamini-Hochberg step-up adjusted p-values for one hypothesis family. */
export function statsBhAdjust(pValues) {
    if (pValues.length < 1)
        throw new RangeError("Benjamini-Hochberg family must not be empty");
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const input = scope.alloc(pValues.length * 8), out = scope.alloc(pValues.length * 8);
        m.HEAPF64.set(pValues, input / 8);
        const st = m.ccall("n4m_stats_bh_adjust", "number", ["number", "number", "i64", "number"], [ctx.handle, input, BigInt(pValues.length), out]);
        checkStatus(st, ctx.handle);
        return copyF64(out, pValues.length);
    }
    finally {
        scope.close();
    }
}
function exactResult(out) {
    const r = copyF64(out, 4);
    return { nArrangements: r[0], pValue: r[1], minimumPValue: r[2], maximumStatistic: r[3] };
}
/** Exact permutation distribution of the factor-target association statistic
 * (eta^2 or Cramer's V^2) by enumerating distinct arrangements. */
export function statsFactorAssociationExact(factorCodes, targets, categorical, maxArrangements) {
    if (factorCodes.length !== targets.length || targets.length < 2 ||
        !Number.isSafeInteger(maxArrangements) || maxArrangements < 1 || maxArrangements > 10_000_000) {
        throw new RangeError("invalid exact association dimensions or arrangement budget");
    }
    validateGroupIds(factorCodes);
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const codes = scope.i64(factorCodes);
        const y = scope.alloc(targets.length * 8), out = scope.alloc(32);
        m.HEAPF64.set(targets, y / 8);
        const st = m.ccall("n4m_stats_factor_association_exact", "number", ["number", "number", "number", "i64", "number", "i64", "number"], [ctx.handle, codes, y, BigInt(targets.length), categorical ? 1 : 0,
            BigInt(maxArrangements), out]);
        checkStatus(st, ctx.handle);
        return exactResult(out);
    }
    finally {
        scope.close();
    }
}
/** Exact permutation distribution of |r| (Pearson or Spearman). */
export function statsCorrelationExact(x, y, method, maxArrangements) {
    if (x.length !== y.length || x.length < 2 || (method !== "pearson" && method !== "spearman") ||
        !Number.isSafeInteger(maxArrangements) || maxArrangements < 1 || maxArrangements > 10_000_000) {
        throw new RangeError("invalid exact correlation dimensions, method or arrangement budget");
    }
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const xs = scope.alloc(x.length * 8), ys = scope.alloc(y.length * 8), out = scope.alloc(32);
        m.HEAPF64.set(x, xs / 8);
        m.HEAPF64.set(y, ys / 8);
        const st = m.ccall("n4m_stats_correlation_exact", "number", ["number", "number", "number", "i64", "number", "i64", "number"], [ctx.handle, xs, ys, BigInt(x.length), method === "spearman" ? 1 : 0,
            BigInt(maxArrangements), out]);
        checkStatus(st, ctx.handle);
        return exactResult(out);
    }
    finally {
        scope.close();
    }
}
/** PERMANOVA of one categorical term on Euclidean distances between rows,
 * optionally given strata (permutations stay inside each stratum). */
export function statsPermanova(X, termCodes, strataCodes, nPermutations, seed) {
    validateMatrix(X);
    if (X.rows < 2 || termCodes.length !== X.rows ||
        (strataCodes !== undefined && strataCodes.length !== X.rows)) {
        throw new RangeError("invalid PERMANOVA dimensions");
    }
    validateGroupIds(termCodes);
    if (strataCodes)
        validateGroupIds(strataCodes);
    const seed64 = validatePermutation(nPermutations, seed);
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const x = scope.matrix(X.data, X.rows, X.cols);
        const term = scope.i64(termCodes);
        const strata = strataCodes ? scope.i64(strataCodes) : 0;
        const out = scope.alloc(15 * 8), counts = scope.alloc(16), exponent = scope.alloc(4);
        const st = m.ccall("n4m_stats_permanova", "number", ["number", "number", "number", "number", "number", "i64", "number", "number", "number"], [ctx.handle, x.viewPtr, term, strata, nPermutations, seed64, out, counts,
            exponent]);
        checkStatus(st, ctx.handle);
        const r = copyF64(out, 15);
        const dv = new DataView(m.HEAPU8.buffer);
        return {
            ssExponent: dv.getInt32(exponent, true),
            ssTotal: r[0], ssStrata: r[1], ssTerm: r[2], ssResidual: r[3],
            r2: r[4], partialR2: r[5], pseudoF: r[6], dfTerm: r[7], dfResidual: r[8],
            nLevels: r[9], nStrata: r[10], log10DistinctPermutations: r[11], pValue: r[12],
            residualResolution: r[13], fNumericallyUnresolved: r[14] === 1,
            nAtLeastObserved: Number(dv.getBigInt64(counts, true)),
            nPermutationsExecuted: Number(dv.getBigInt64(counts + 8, true)),
        };
    }
    finally {
        scope.close();
    }
}
/** PERMDISP (betadisper type "centroid"): distances to group centroids, their
 * one-way ANOVA F and a residual-permutation test. Not an exchangeability check. */
export function statsPermdisp(X, groupCodes, nPermutations, seed) {
    validateMatrix(X);
    if (X.rows < 2 || groupCodes.length !== X.rows) {
        throw new RangeError("invalid PERMDISP dimensions");
    }
    validateGroupIds(groupCodes);
    const seed64 = validatePermutation(nPermutations, seed);
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const x = scope.matrix(X.data, X.rows, X.cols);
        const groups = scope.i64(groupCodes);
        const distances = scope.alloc(X.rows * 8), means = scope.alloc(X.rows * 8);
        const out = scope.alloc(8 * 8), counts = scope.alloc(16), exponent = scope.alloc(4);
        const st = m.ccall("n4m_stats_permdisp", "number", ["number", "number", "number", "number", "i64", "number", "number", "number", "number",
            "number"], [ctx.handle, x.viewPtr, groups, nPermutations, seed64, distances, means, out,
            counts, exponent]);
        checkStatus(st, ctx.handle);
        const r = copyF64(out, 8);
        const dv = new DataView(m.HEAPU8.buffer);
        return {
            distanceExponent: dv.getInt32(exponent, true),
            distances: copyF64(distances, X.rows),
            groupMeanDistances: copyF64(means, r[3]),
            f: r[0], df1: r[1], df2: r[2], nGroups: r[3], log10DistinctPermutations: r[4],
            pValue: r[5], residualResolution: r[6], fNumericallyUnresolved: r[7] === 1,
            nAtLeastObserved: Number(dv.getBigInt64(counts, true)),
            nPermutationsExecuted: Number(dv.getBigInt64(counts + 8, true)),
        };
    }
    finally {
        scope.close();
    }
}
/** Partial PERMANOVA of a term given the additive main effects of condition
 * factors (one code array per factor), tested by Freedman-Lane permutation of
 * the reduced-model residuals. */
export function statsPermanovaFreedmanLane(X, termCodes, conditionCodes, nPermutations, seed) {
    validateMatrix(X);
    if (X.rows < 2 || termCodes.length !== X.rows || conditionCodes.length > 64 ||
        conditionCodes.some((c) => c.length !== X.rows)) {
        throw new RangeError("invalid Freedman-Lane PERMANOVA dimensions");
    }
    validateGroupIds(termCodes);
    conditionCodes.forEach(validateGroupIds);
    const seed64 = validatePermutation(nPermutations, seed);
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const x = scope.matrix(X.data, X.rows, X.cols);
        const term = scope.i64(termCodes);
        const rowMajor = [];
        for (let i = 0; i < X.rows; i++)
            for (const c of conditionCodes)
                rowMajor.push(c[i]);
        const conditions = conditionCodes.length ? scope.i64(rowMajor) : 0;
        const out = scope.alloc(14 * 8), counts = scope.alloc(16), exponent = scope.alloc(4);
        const st = m.ccall("n4m_stats_permanova_freedman_lane", "number", ["number", "number", "number", "number", "number", "number", "i64", "number", "number",
            "number"], [ctx.handle, x.viewPtr, term, conditions, conditionCodes.length, nPermutations, seed64,
            out, counts, exponent]);
        checkStatus(st, ctx.handle);
        const r = copyF64(out, 14);
        const dv = new DataView(m.HEAPU8.buffer);
        return {
            ssExponent: dv.getInt32(exponent, true),
            ssTotal: r[0], ssReduced: r[1], ssTerm: r[2], ssResidual: r[3], r2: r[4],
            partialR2: r[5], pseudoF: r[6], dfTerm: r[7], dfResidual: r[8], rankReduced: r[9],
            log10DistinctPermutations: r[10], pValue: r[11],
            residualResolution: r[12], fNumericallyUnresolved: r[13] === 1,
            nAtLeastObserved: Number(dv.getBigInt64(counts, true)),
            nPermutationsExecuted: Number(dv.getBigInt64(counts + 8, true)),
        };
    }
    finally {
        scope.close();
    }
}
/** Energy distance between groups of rows with a K-sample permutation test.
 * groupCodes are integers in [0, nGroups); the seed is an unsigned 64-bit
 * integer (pass a bigint above 2^53). The native size limits are checked
 * before any WASM allocation and raise N4mError(ERR_UNSUPPORTED); inputs
 * outside the supported numerical domain raise the same status. */
export function statsEnergyDistance(X, groupCodes, nGroups, nPermutations, seed) {
    validateMatrix(X);
    if (groupCodes.length !== X.rows || !Number.isSafeInteger(nGroups) || nGroups < 2 ||
        nGroups > X.rows || groupCodes.some((c) => !Number.isInteger(c) || c < 0 || c >= nGroups)) {
        throw new RangeError("invalid energy distance groups");
    }
    const seed64 = validatePermutation(nPermutations, seed);
    const pairs = X.rows * (X.rows - 1) / 2;
    if (pairs > 20_000_000 || X.rows * X.cols > 50_000_000 || nGroups * nGroups > 10_000_000 ||
        pairs * (X.cols + nPermutations + 1) > 2.5e9) {
        throw new N4mError(Status.ERR_UNSUPPORTED, "energy distance exceeds its pair, cell, group or operation budget");
    }
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const x = scope.matrix(X.data, X.rows, X.cols);
        const codes = scope.i64(groupCodes);
        const g2 = nGroups * nGroups;
        const energy = scope.alloc(g2 * 8), bounds = scope.alloc(g2 * 8), mean = scope.alloc(g2 * 8);
        const out = scope.alloc(40), counts = scope.alloc(24);
        const st = m.ccall("n4m_stats_energy_distance", "number", ["number", "number", "number", "number", "number", "i64", "number", "number", "number",
            "number", "number"], [ctx.handle, x.viewPtr, codes, nGroups, nPermutations, seed64, energy, bounds, mean, out,
            counts]);
        checkStatus(st, ctx.handle);
        const r = copyF64(out, 5);
        const dv = new DataView(m.HEAPU8.buffer);
        return {
            energy: copyF64(energy, g2), energyBounds: copyF64(bounds, g2),
            meanDistances: copyF64(mean, g2),
            statistic: r[0], statisticBound: r[1], log10DistinctPermutations: r[2], pValue: r[3],
            pValueConservative: r[4],
            nAtLeastObserved: Number(dv.getBigInt64(counts, true)),
            nPermutationsExecuted: Number(dv.getBigInt64(counts + 8, true)),
            nUndecidable: Number(dv.getBigInt64(counts + 16, true)),
        };
    }
    finally {
        scope.close();
    }
}
/** PCA monitoring of rows against a reference PCA with out-of-fold limits.
 * The native size limits are checked before any WASM allocation. */
export function statsPcaMonitor(reference, monitored, maxComponents, nFolds, alpha) {
    validateMatrix(reference);
    validateMatrix(monitored);
    if (monitored.cols !== reference.cols || !Number.isSafeInteger(maxComponents) ||
        maxComponents < 1 || !Number.isSafeInteger(nFolds) || nFolds < 2 ||
        nFolds > reference.rows || !(alpha > 0 && alpha < 1)) {
        throw new RangeError("invalid PCA monitor dimensions");
    }
    const nr = reference.rows, nm = monitored.rows, p = reference.cols;
    const small = Math.min(nr, p);
    if ((nr + nm) * p > 50_000_000 ||
        (nFolds + 1) * small * small * Math.max(nr, p) + 4 * (nr + nm) * maxComponents * p > 1e10) {
        throw new N4mError(Status.ERR_UNSUPPORTED, "PCA monitor exceeds its cell or operation budget");
    }
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const r = scope.matrix(reference.data, nr, p);
        const mo = scope.matrix(monitored.data, nm, p);
        const eigen = scope.alloc(maxComponents * 8);
        const rt = scope.alloc(nr * 8), rq = scope.alloc(nr * 8);
        const mt = scope.alloc(nm * 8), mq = scope.alloc(nm * 8);
        const k = scope.alloc(4), counts = scope.alloc(48), out = scope.alloc(13 * 8);
        const st = m.ccall("n4m_stats_pca_monitor", "number", ["number", "number", "number", "number", "number", "number", "number", "number",
            "number", "number", "number", "number", "number", "number"], [ctx.handle, r.viewPtr, mo.viewPtr, maxComponents, nFolds, alpha, eigen, rt, rq, mt,
            mq, k, counts, out]);
        checkStatus(st, ctx.handle);
        const o = copyF64(out, 13);
        const dv = new DataView(m.HEAPU8.buffer);
        const count = (i) => Number(dv.getBigInt64(counts + 8 * i, true));
        return {
            eigenvalues: copyF64(eigen, maxComponents),
            effectiveComponents: dv.getInt32(k, true),
            referenceT2: copyF64(rt, nr), referenceQ: copyF64(rq, nr),
            monitoredT2: copyF64(mt, nm), monitoredQ: copyF64(mq, nm),
            nBeyondT2: count(0), nBeyondQ: count(1), nBeyondEither: count(2),
            nNearT2: count(3), nNearQ: count(4), nNearEither: count(5),
            t2Limit: o[0], qLimit: o[1], tolerance: o[9], condition: o[10],
            qToleranceScale: o[11], eigenvalueResolution: o[12], referenceTotalVariance: o[2],
            monitoredTotalVariance: o[3], centroidDistance: o[4],
            centroidSubspaceDistance: o[5], centroidResidualDistance: o[6], centroidT2: o[7],
            retainedFraction: o[8],
        };
    }
    finally {
        scope.close();
    }
}
function pairs(r, cols, offset) {
    const out = new Float64Array(2 * cols);
    for (let j = 0; j < cols; ++j) {
        out[2 * j] = r[8 * j + offset];
        out[2 * j + 1] = r[8 * j + offset + 1];
    }
    return out;
}
/** One-way random-effects variance components by REML, per feature. */
export function statsVarianceComponentsReml(X, groupIds, validMask) {
    validateMatrix(X);
    if (groupIds.length !== X.rows || (validMask && validMask.length !== X.data.length)) {
        throw new RangeError("variance-components group or mask length mismatch");
    }
    validateGroupIds(groupIds);
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const x = scope.matrix(X.data, X.rows, X.cols);
        const groups = scope.i64(groupIds);
        const mask = validMask ? scope.alloc(validMask.length) : 0;
        const out = scope.alloc(X.cols * 64), status = scope.alloc(X.cols * 4);
        if (validMask)
            m.HEAPU8.set(validMask, mask);
        const st = m.ccall("n4m_stats_variance_components_reml", "number", Array(6).fill("number"), [ctx.handle, x.viewPtr, groups, mask, out, status]);
        checkStatus(st, ctx.handle);
        const r = copyF64(out, X.cols * 8);
        const field = (k) => Float64Array.from({ length: X.cols }, (_, j) => r[8 * j + k]);
        const count = (k) => Array.from({ length: X.cols }, (_, j) => r[8 * j + k]);
        return {
            sigma2Between: field(0), sigma2Within: field(1), icc: field(2), logLikelihood: field(3),
            nGroups: count(4), nObservations: count(5), remlCandidates: count(6),
            bisectionIterations: count(7),
            status: new Int32Array(m.HEAP32.slice(status / 4, status / 4 + X.cols)),
        };
    }
    finally {
        scope.close();
    }
}
/** Exact and cluster-bootstrap confidence intervals of the same model.
 * clusterIds (optional) names the resampling unit; the seed is a uint64. */
export function statsVarianceComponentsIntervals(X, groupIds, options) {
    validateMatrix(X);
    const { confidence, nBootstrap, seed, clusterIds, validMask } = options;
    if (groupIds.length !== X.rows || (clusterIds && clusterIds.length !== X.rows) ||
        (validMask && validMask.length !== X.data.length)) {
        throw new RangeError("variance-components group, cluster or mask length mismatch");
    }
    if (!(confidence > 0 && confidence < 1) || !Number.isInteger(nBootstrap) ||
        nBootstrap < 0 || nBootstrap > 100_000) {
        throw new RangeError("invalid confidence or bootstrap count");
    }
    validateGroupIds(groupIds);
    if (clusterIds)
        validateGroupIds(clusterIds);
    if (typeof seed === "number" && !Number.isSafeInteger(seed)) {
        throw new RangeError("seeds supplied as numbers must be safe integers");
    }
    const seed64 = BigInt(seed);
    if (seed64 < 0n || seed64 >= (1n << 64n))
        throw new RangeError("seed is outside uint64 range");
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const x = scope.matrix(X.data, X.rows, X.cols);
        const groups = scope.i64(groupIds);
        const clusters = clusterIds ? scope.i64(clusterIds) : 0;
        const mask = validMask ? scope.alloc(validMask.length) : 0;
        const out = scope.alloc(X.cols * 64), valid = scope.alloc(X.cols * 8);
        if (validMask)
            m.HEAPU8.set(validMask, mask);
        const st = m.ccall("n4m_stats_variance_components_intervals", "number", ["number", "number", "number", "number", "number", "number", "number", "i64",
            "number", "number"], [ctx.handle, x.viewPtr, groups, clusters, mask, confidence, nBootstrap, seed64,
            out, valid]);
        checkStatus(st, ctx.handle);
        const r = copyF64(out, X.cols * 8);
        const dv = new DataView(m.HEAPU8.buffer);
        return {
            sdChiSquare: pairs(r, X.cols, 0), iccExactF: pairs(r, X.cols, 2),
            sdBootstrap: pairs(r, X.cols, 4), iccBootstrap: pairs(r, X.cols, 6),
            nValidReplicates: Array.from({ length: X.cols }, (_, j) => Number(dv.getBigInt64(valid + j * 8, true))),
        };
    }
    finally {
        scope.close();
    }
}
/** Equal-weight per-group column means (double-double sums): codes in
 * [0, nGroups), every group nonempty, values in the declared numerical domain
 * (nonzero |x| in [2^-256, 2^256]; a summary, so no spread limit), else N4mError. */
export function statsGroupMeans(X, groupCodes, nGroups) {
    validateMatrix(X);
    if (groupCodes.length !== X.rows || !Number.isSafeInteger(nGroups) || nGroups < 1 ||
        nGroups > X.rows || groupCodes.some((c) => !Number.isSafeInteger(c))) {
        throw new RangeError("invalid group-means dimensions");
    }
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const x = scope.matrix(X.data, X.rows, X.cols);
        const codes = scope.i64(groupCodes);
        const means = scope.alloc(nGroups * X.cols * 8), counts = scope.alloc(nGroups * 8);
        const st = m.ccall("n4m_stats_group_means", "number", ["number", "number", "number", "i64", "number", "number"], [ctx.handle, x.viewPtr, codes, BigInt(nGroups), means, counts]);
        checkStatus(st, ctx.handle);
        const dv = new DataView(m.HEAPU8.buffer);
        return {
            means: { data: copyF64(means, nGroups * X.cols), rows: nGroups, cols: X.cols },
            counts: Array.from({ length: nGroups }, (_, i) => Number(dv.getBigInt64(counts + i * 8, true))),
        };
    }
    finally {
        scope.close();
    }
}
/** RMSE, MAE and R² through n4m_stats_regression_scores (explicit residuals,
 * double-double sums). Inputs outside the declared numerical domain
 * (nonzero |x| in [2^-256, 2^256], spread <= 2^32) throw N4mError
 * ERR_UNSUPPORTED. */
export function statsRegressionScores(yTrue, yPred) {
    if (yTrue.length < 1 || yTrue.length !== yPred.length) {
        throw new RangeError("regression scores need paired nonempty vectors");
    }
    const m = getModule(), scope = new NativeScope();
    try {
        const t = scope.alloc(yTrue.length * 8), p = scope.alloc(yPred.length * 8);
        m.HEAPF64.set(yTrue, t / 8);
        m.HEAPF64.set(yPred, p / 8);
        const out = scope.alloc(4 * 8);
        const st = m.ccall("n4m_stats_regression_scores", "number", ["number", "number", "number", "i64", "number"], [scope.ctx.handle, t, p, BigInt(yTrue.length), out]);
        checkStatus(st, scope.ctx.handle);
        const r = copyF64(out, 4);
        return { rmse: r[0], mae: r[1], r2: r[2], r2Exponent: r[3] };
    }
    finally {
        scope.close();
    }
}
/** Cluster overlap between two partitions; negative labels are unassigned. */
export function clusterJaccardMatrix(labelsA, labelsB, nClustersA, nClustersB) {
    if (labelsA.length !== labelsB.length || !Number.isSafeInteger(nClustersA) || nClustersA < 0 ||
        !Number.isSafeInteger(nClustersB) || nClustersB < 0) {
        throw new RangeError("invalid Jaccard labels or cluster counts");
    }
    validateGroupIds(labelsA);
    validateGroupIds(labelsB);
    const cells = nClustersA * nClustersB;
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const pa = scope.i64(labelsA), pb = scope.i64(labelsB);
        const jac = scope.alloc(cells * 8), inter = scope.alloc(cells * 8);
        const sa = scope.alloc(nClustersA * 8), sb = scope.alloc(nClustersB * 8);
        const st = m.ccall("n4m_cluster_jaccard_matrix", "number", ["number", "number", "number", "i64", "i64", "i64", "number", "number", "number", "number"], [ctx.handle, pa, pb, BigInt(labelsA.length), BigInt(nClustersA), BigInt(nClustersB),
            jac, inter, sa, sb]);
        checkStatus(st, ctx.handle);
        const dv = new DataView(m.HEAPU8.buffer);
        const i64 = (ptr, n) => Array.from({ length: n }, (_, j) => Number(dv.getBigInt64(ptr + j * 8, true)));
        return {
            jaccard: { data: copyF64(jac, cells), rows: nClustersA, cols: nClustersB },
            intersections: i64(inter, cells), sizesA: i64(sa, nClustersA), sizesB: i64(sb, nClustersB),
        };
    }
    finally {
        scope.close();
    }
}
/** scipy.optimize.linear_sum_assignment (SciPy 1.18.1 tie order); +Infinity forbids a pair. */
export function linearSumAssignment(cost, maximize = false) {
    validateMatrix(cost);
    const k = Math.min(cost.rows, cost.cols);
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const c = scope.matrix(cost.data, cost.rows, cost.cols);
        const rows = scope.alloc(k * 8), cols = scope.alloc(k * 8), count = scope.alloc(8);
        const st = m.ccall("n4m_linear_sum_assignment", "number", ["number", "number", "number", "number", "number", "number"], [ctx.handle, c.viewPtr, maximize ? 1 : 0, rows, cols, count]);
        checkStatus(st, ctx.handle);
        const dv = new DataView(m.HEAPU8.buffer);
        const i64 = (ptr) => Array.from({ length: k }, (_, j) => Number(dv.getBigInt64(ptr + j * 8, true)));
        return { rowInd: i64(rows), colInd: i64(cols) };
    }
    finally {
        scope.close();
    }
}
/** numpy.random.default_rng(seed).multivariate_normal(mean, cov, size=n, method="cholesky").
 * The seed is an unsigned 64-bit integer; pass a bigint above 2^53. Returns a row-major n x d matrix. */
export function randomMultivariateNormal(mean, cov, n, seed) {
    validateMatrix(cov);
    if (cov.rows !== cov.cols || mean.length !== cov.rows || !Number.isSafeInteger(n) || n < 0) {
        throw new RangeError("invalid multivariate normal dimensions");
    }
    if (typeof seed === "number" && !Number.isSafeInteger(seed)) {
        throw new RangeError("seeds supplied as numbers must be safe integers");
    }
    const seed64 = BigInt(seed);
    if (seed64 < 0n || seed64 >= (1n << 64n))
        throw new RangeError("seed is outside uint64 range");
    const d = cov.rows;
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const c = scope.matrix(cov.data, d, d);
        const mu = scope.alloc(d * 8), out = scope.alloc(n * d * 8);
        m.HEAPF64.set(mean, mu / 8);
        const st = m.ccall("n4m_random_multivariate_normal", "number", ["number", "i64", "number", "number", "i64", "number"], [ctx.handle, seed64, mu, c.viewPtr, BigInt(n), out]);
        checkStatus(st, ctx.handle);
        return { data: copyF64(out, n * d), rows: n, cols: d };
    }
    finally {
        scope.close();
    }
}
/** Successive numpy.random.default_rng(seed).permutation(n) draws from one generator.
 * The seed is an unsigned 64-bit integer; pass a bigint above 2^53. */
export function randomPermutations(seed, n, nPermutations) {
    if (!Number.isSafeInteger(n) || n < 0 || !Number.isSafeInteger(nPermutations) || nPermutations < 0) {
        throw new RangeError("invalid permutation dimensions");
    }
    if (typeof seed === "number" && !Number.isSafeInteger(seed)) {
        throw new RangeError("seeds supplied as numbers must be safe integers");
    }
    const seed64 = BigInt(seed);
    if (seed64 < 0n || seed64 >= (1n << 64n))
        throw new RangeError("seed is outside uint64 range");
    const m = getModule(), scope = new NativeScope();
    try {
        const ctx = scope.ctx;
        const out = scope.alloc(n * nPermutations * 8);
        const st = m.ccall("n4m_random_permutations", "number", ["number", "i64", "i64", "i64", "number"], [ctx.handle, seed64, BigInt(n), BigInt(nPermutations), out]);
        checkStatus(st, ctx.handle);
        const dv = new DataView(m.HEAPU8.buffer);
        return Array.from({ length: nPermutations }, (_, b) => Array.from({ length: n }, (_, j) => Number(dv.getBigInt64(out + (b * n + j) * 8, true))));
    }
    finally {
        scope.close();
    }
}
