// SPDX-License-Identifier: CECILL-2.1 OR AGPL-3.0-or-later
//
// Generic preprocessing surface over libn4m. One opaque handle wraps any
// supported preprocessing operator; the numerics live entirely in libn4m
// (n4m_pp_*). This TS layer is marshalling only — it never computes a spectrum.
//
// Operators are addressed by their catalog `type` token + a numeric params
// vector (in the documented order per operator). All supported operators are
// shape-preserving (out is n×p). Stateful operators (MSC) expose their fitted
// state as plain doubles via getState/setState so it round-trips into a saved
// model bundle for predict-later, without re-fitting on the training data.
//
// Param vectors (order matters):
//   StandardNormalVariate : []                       (SNV; with_mean/std on)
//   MSC                   : []                        (stateful — call fit first)
//   SavitzkyGolay         : [window, polyorder, deriv, mode?, cval?]
//                           mode follows n4m_pp_savgol_mode_t:
//                           0 mirror, 1 constant, 2 nearest, 3 wrap, 4 interp
//   Derivative            : [order]                   (1 → 1st, ≥2 → 2nd derivative)
//   Detrend               : [polyorder]
// Row L2 uses the generic RowL2Normalize transformer; ppCreate has no Normalize token.
//   GaussianFilter        : [sigma]
import { getModule, checkStatus } from "./ffi.js";
function pushF64(m, data) {
    const ptr = m._malloc(Math.max(1, data.length) * 8);
    if (data.length > 0)
        m.HEAPF64.set(data, ptr / 8);
    return ptr;
}
/** Validate matrix dims + that the buffer length matches n*p before crossing into C. */
function checkDims(X, n, p) {
    if (!Number.isSafeInteger(n) || !Number.isSafeInteger(p) || n < 0 || p < 0) {
        throw new Error(`@nirs4all/methods: invalid matrix dims ${n}×${p}`);
    }
    if (X.length !== n * p) {
        throw new Error(`@nirs4all/methods: data length ${X.length} != n*p (${n * p})`);
    }
}
/** Create a preprocessing operator by catalog type token + numeric params. */
export function ppCreate(op, params = []) {
    if (op === "StandardNormalVariate" && !(params === undefined ||
        (Array.isArray(params) && params.length === 0))) {
        throw new TypeError("SNV accepts only an empty parameter Array; use SNV/RolePipeline for options");
    }
    const m = getModule();
    let pPtr = 0;
    let ptr = 0;
    try {
        if (params.length > 0) {
            pPtr = m._malloc(params.length * 8);
            m.HEAPF64.set(Float64Array.from(params), pPtr / 8);
        }
        ptr = m.ccall("n4m_wasm_pp_create", "number", ["string", "number", "number"], [op, pPtr, params.length]);
    }
    finally {
        if (pPtr !== 0)
            m._free(pPtr);
    }
    if (ptr === 0) {
        throw new Error(`@nirs4all/methods: unknown/unconstructable preprocessing operator "${op}"`);
    }
    return { _ptr: ptr };
}
/** Fit a stateful operator on training data (no-op for stateless operators). */
export function ppFit(op, X, n, p) {
    checkDims(X, n, p);
    const m = getModule();
    const xp = pushF64(m, X);
    try {
        checkStatus(m.ccall("n4m_wasm_pp_fit", "number", ["number", "number", "number", "number"], [op._ptr, xp, n, p]));
    }
    finally {
        m._free(xp);
    }
}
/** Transform X (n×p, row-major) → a fresh Float64Array (n×p). */
export function ppTransform(op, X, n, p) {
    checkDims(X, n, p);
    const m = getModule();
    const xp = pushF64(m, X);
    const out = m._malloc(Math.max(1, n * p) * 8);
    try {
        checkStatus(m.ccall("n4m_wasm_pp_transform", "number", ["number", "number", "number", "number", "number"], [op._ptr, xp, n, p, out]));
        return Float64Array.from(m.HEAPF64.subarray(out / 8, out / 8 + n * p));
    }
    finally {
        m._free(xp);
        m._free(out);
    }
}
/** Serialize the fitted state (empty for stateless operators). */
export function ppGetState(op) {
    const m = getModule();
    const len = m.ccall("n4m_wasm_pp_state_len", "number", ["number"], [op._ptr]);
    if (len <= 0)
        return new Float64Array(0);
    const out = m._malloc(len * 8);
    try {
        checkStatus(m.ccall("n4m_wasm_pp_get_state", "number", ["number", "number"], [op._ptr, out]));
        return Float64Array.from(m.HEAPF64.subarray(out / 8, out / 8 + len));
    }
    finally {
        m._free(out);
    }
}
/** Restore a fitted state from getState() output (no-op for stateless ops). */
export function ppSetState(op, state) {
    if (state.length === 0)
        return;
    const m = getModule();
    const sp = pushF64(m, state);
    try {
        checkStatus(m.ccall("n4m_wasm_pp_set_state", "number", ["number", "number", "number"], [op._ptr, sp, state.length]));
    }
    finally {
        m._free(sp);
    }
}
/** Free the operator handle. */
export function ppDestroy(op) {
    if (op._ptr === 0)
        return;
    getModule().ccall("n4m_wasm_pp_destroy", null, ["number"], [op._ptr]);
    op._ptr = 0;
}
