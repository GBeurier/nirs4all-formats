// SPDX-License-Identifier: CECILL-2.1 OR AGPL-3.0-or-later
// Train-only X->X augmentation facade. All numerical work remains in C++.
import { checkStatus, getModule } from "./ffi.js";
const KINDS = {
    GaussianNoise: [0, 1], MultiplicativeNoise: [1, 1],
    SpikeNoise: [2, 4], HeteroNoise: [3, 2], LinearDrift: [4, 4],
    PathLength: [5, 2], BandPerturb: [6, 7], BandMask: [7, 5],
    ChannelDropout: [8, 2], GaussJitter: [9, 3], UnsharpMask: [10, 4],
    LocalClip: [11, 3], RotateTranslate: [12, 2], RandomXOp: [13, 3],
    ScatterSimMSC: [14, 4], DeadBand: [15, 6], BatchEffect: [16, 4],
    SplineSmoothing: [17, 0], SplineXPerturb: [18, 4],
    SplineYPerturb: [19, 2], SplineXSimplify: [20, 2],
    SplineCurveSimplify: [21, 2],
};
/** Apply one seeded native augmenter to training X; no Y or fitted state. */
export function augmentNative(kind, X, params, seed = 0) {
    if (!Object.prototype.hasOwnProperty.call(KINDS, kind))
        throw new Error(`unknown native augmentation ${kind}`);
    const [code, count] = KINDS[kind];
    if (!Number.isSafeInteger(X.rows) || !Number.isSafeInteger(X.cols) ||
        X.rows < 1 || X.cols < 1 || X.rows * X.cols > 2147483647 ||
        X.data.length !== X.rows * X.cols || !X.data.every(Number.isFinite))
        throw new Error("X must be a nonempty finite row-major matrix");
    if (params.length !== count || !params.every(Number.isFinite))
        throw new Error("invalid native augmentation parameter vector");
    if (typeof seed === "number" && !Number.isSafeInteger(seed))
        throw new Error("seed must be an exact unsigned integer");
    const seed64 = BigInt(seed);
    if (seed64 < 0n || seed64 > (1n << 64n) - 1n)
        throw new Error("seed must fit unsigned int64");
    const m = getModule();
    const xp = m._malloc(X.data.byteLength);
    const pp = count === 0 ? 0 : m._malloc(count * 8);
    const op = m._malloc(X.data.byteLength);
    try {
        m.HEAPF64.set(X.data, xp / 8);
        if (count > 0)
            m.HEAPF64.set(params, pp / 8);
        checkStatus(m.ccall("n4m_wasm_augmentation_apply", "number", Array(9).fill("number"), [code, pp, count, Number(seed64 & 0xffffffffn),
            Number(seed64 >> 32n), xp, X.rows, X.cols, op]));
        return { rows: X.rows, cols: X.cols,
            data: Float64Array.from(m.HEAPF64.subarray(op / 8, op / 8 + X.data.length)) };
    }
    finally {
        for (const ptr of [xp, pp, op])
            if (ptr !== 0)
                m._free(ptr);
    }
}
