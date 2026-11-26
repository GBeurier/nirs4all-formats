// SPDX-License-Identifier: CECILL-2.1 OR AGPL-3.0-or-later
// One typed marshalling path to the nine native sample splitters.
import { checkStatus, getModule } from "./ffi.js";
const KINDS = {
    KennardStone: 0, SPXY: 1, SPXYFold: 2, SPXYGroupFold: 3,
    KMeans: 4, KBinsStratified: 5, BinnedStratGroupFold: 6,
    SystematicCircular: 7, DataTwinning: 8,
};
function validMatrix(value, name) {
    if (!Number.isSafeInteger(value.rows) || !Number.isSafeInteger(value.cols) ||
        value.rows < 1 || value.cols < 1 ||
        value.data.length !== value.rows * value.cols ||
        !value.data.every(Number.isFinite)) {
        throw new Error(`${name} must be a nonempty finite row-major matrix`);
    }
}
/** Return the native ordered row indices without sorting or host-side splitting. */
export function splitNative(kind, X, Y = null, options = {}) {
    if (!Object.prototype.hasOwnProperty.call(KINDS, kind)) {
        throw new Error(`unknown native splitter ${kind}`);
    }
    const code = KINDS[kind];
    const needsX = [0, 1, 2, 3, 4, 8].includes(code);
    const needsY = [1, 2, 3, 5, 6, 7].includes(code);
    const grouped = code === 3 || code === 6;
    const folded = code === 2 || code === 3 || code === 6;
    if ((X !== null) !== needsX || (Y !== null) !== needsY ||
        (options.groups !== undefined) !== grouped) {
        throw new Error("X/Y/groups must match the native splitter kind");
    }
    if (X !== null)
        validMatrix(X, "X");
    if (Y !== null)
        validMatrix(Y, "Y");
    const n = X?.rows ?? Y.rows;
    if (!Number.isSafeInteger(n) || n > 2147483647 ||
        (X !== null && Y !== null && X.rows !== Y.rows)) {
        throw new Error("native splitter requires aligned, int32-sized rows");
    }
    const ns = options.nSplits ?? 3;
    const fold = options.foldIndex ?? 0;
    if (!Number.isInteger(fold) || (folded ? fold < 0 || fold >= ns : fold !== 0) ||
        !Number.isInteger(ns) || ns < 2) {
        throw new Error("invalid native splitter fold configuration");
    }
    const rawGroups = options.groups ?? [];
    if (grouped && rawGroups.length !== n)
        throw new Error("groups length must match rows");
    const groupIds = rawGroups.map(value => {
        if (typeof value === "number" && (!Number.isSafeInteger(value))) {
            throw new Error("group IDs must be exact signed int64 integers");
        }
        const id = BigInt(value);
        if (id < -(1n << 63n) || id > (1n << 63n) - 1n) {
            throw new Error("group IDs must fit signed int64");
        }
        return id;
    });
    const seedRaw = options.seed ?? 0;
    if (typeof seedRaw === "number" && !Number.isSafeInteger(seedRaw)) {
        throw new Error("seed must be an exact unsigned integer");
    }
    const seed = BigInt(seedRaw);
    if (seed < 0n || seed > (1n << 64n) - 1n) {
        throw new Error("seed must fit unsigned int64");
    }
    const integers = [options.yMetric ?? 1, options.aggregation ?? 0,
        options.nBins ?? 5, options.strategy ?? 0,
        options.maxIter ?? 100];
    if (integers.some(value => !Number.isInteger(value) ||
        value < 0 || value > 2147483647)) {
        throw new Error("native splitter parameters must be nonnegative int32 values");
    }
    const testSize = options.testSize ?? 0.25;
    if (!Number.isFinite(testSize) || testSize <= 0 || testSize >= 1) {
        throw new Error("testSize must be between zero and one");
    }
    const m = getModule();
    const spec = m._malloc(48); // C n4m_splitter_spec_t, WASM32 8-byte alignment
    const xp = X === null ? 0 : m._malloc(X.data.byteLength);
    const yp = Y === null ? 0 : m._malloc(Y.data.byteLength);
    const gp = grouped ? m._malloc(n * 8) : 0;
    const train = m._malloc(n * 4);
    const test = m._malloc(n * 4);
    const nTrain = m._malloc(4);
    const nTest = m._malloc(4);
    try {
        if (X !== null)
            m.HEAPF64.set(X.data, xp / 8);
        if (Y !== null)
            m.HEAPF64.set(Y.data, yp / 8);
        const data = new DataView(m.HEAPU8.buffer);
        groupIds.forEach((id, i) => data.setBigInt64(gp + i * 8, id, true));
        [code, ns, integers[0], integers[1], integers[2], integers[3],
            options.shuffle === false ? 0 : 1, integers[4]].forEach((value, i) => data.setInt32(spec + i * 4, value, true));
        data.setFloat64(spec + 32, testSize, true);
        data.setBigUint64(spec + 40, seed, true);
        checkStatus(m.ccall("n4m_wasm_splitter_indices", "number", Array(12).fill("number"), [spec, xp, yp, n, X?.cols ?? 0, Y?.cols ?? 0, gp, fold,
            train, nTrain, test, nTest]));
        const trainCount = m.getValue(nTrain, "i32");
        const testCount = m.getValue(nTest, "i32");
        if (trainCount < 0 || testCount < 0 || trainCount + testCount !== n) {
            throw new Error("invalid native splitter result sizes");
        }
        return {
            trainIndices: Int32Array.from(m.HEAP32.subarray(train / 4, train / 4 + trainCount)),
            testIndices: Int32Array.from(m.HEAP32.subarray(test / 4, test / 4 + testCount)),
        };
    }
    finally {
        for (const ptr of [spec, xp, yp, gp, train, test, nTrain, nTest]) {
            if (ptr !== 0)
                m._free(ptr);
        }
    }
}
