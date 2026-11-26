// SPDX-License-Identifier: CECILL-2.1 OR AGPL-3.0-or-later
// Generic native estimator roles (ABI 2.13; model-intrinsic structure ABI 2.24).
//
// The helpers exported below (contexts, allocations, typed parameters, fit
// inputs) are shared with rolePipeline.ts; the package index does not
// re-export them.
//
// Every class in estimatorRolesGenerated.ts extends NativeEstimator and
// implements exactly the role interfaces its native method declares
// (Regressor, Transformer, ...). Parameters, defaults, required inputs,
// fitting and the portable N4ME state are native; this file marshals only.
import { checkStatus, getModule, makeMatrixView } from "./ffi.js";
// n4m_fit_inputs_v1_t on wasm32 (pointers 4 bytes, int64 8-aligned); checked
// against offsetof() of the C header when the layout was written.
const FIT_INPUTS_SIZE = 120;
const OFF = {
    X: 4, Y: 8, labels: 12, nLabels: 16, sampleWeight: 24, nSampleWeight: 32, groups: 40, nGroups: 48,
    featureGroups: 56, nFeatureGroups: 64, blocks: 72, nBlocks: 80, axis: 88,
    nAxis: 96, XTarget: 104, foldIds: 108, nFoldIds: 112,
};
/** Runs `fn` with a fresh native context, destroyed afterwards. */
export function withContext(fn) {
    const m = getModule();
    const out = m._malloc(4);
    try {
        m.setValue(out, 0, "i32");
        checkStatus(m.ccall("n4m_context_create", "number", ["number"], [out]));
        const ctx = m.getValue(out, "i32");
        try {
            return fn(ctx);
        }
        finally {
            m.ccall("n4m_context_destroy", null, ["number"], [ctx]);
        }
    }
    finally {
        m._free(out);
    }
}
export function readI64(ptr) {
    const m = getModule();
    return Number(m.getValue(ptr, "i64"));
}
function allocF64(values) {
    const m = getModule();
    const ptr = m._malloc(Math.max(1, values.length) * 8);
    m.HEAPF64.set(values instanceof Float64Array ? values : Float64Array.from(values), ptr / 8);
    return { ptr, free: () => m._free(ptr) };
}
function allocI64(values) {
    const m = getModule();
    const ptr = m._malloc(Math.max(1, values.length) * 8);
    values.forEach((v, i) => m.setValue(ptr + 8 * i, BigInt(v), "i64"));
    return { ptr, free: () => m._free(ptr) };
}
const registry = new Map();
export function cString(s) {
    const m = getModule();
    const n = m.lengthBytesUTF8(s) + 1;
    const ptr = m._malloc(n);
    m.stringToUTF8(s, ptr, n);
    return { ptr, free: () => m._free(ptr) };
}
/** SNV host types must survive marshalling without coercion or i32 overflow. */
function validateSnvParams(method) {
    if (method.methodId !== "preprocessing.scatter.snv")
        return;
    for (const name of ["with_mean", "with_std"]) {
        const value = method.params[name];
        if (value !== undefined && typeof value !== "boolean") {
            throw new TypeError(`SNV ${name} must be a boolean`);
        }
    }
    const ddof = method.params.ddof;
    if (ddof !== undefined && (typeof ddof !== "number" ||
        !Number.isSafeInteger(ddof) || ddof < 0 || ddof > 2147483647)) {
        throw new TypeError("SNV ddof must be an integer number in [0, INT32_MAX]");
    }
}
/** Validated native parameters of a method; the caller destroys them. */
export function nativeParams(ctx, method) {
    validateSnvParams(method);
    const m = getModule();
    const indexPtr = m._malloc(4);
    const out = m._malloc(4);
    const allocs = [];
    const id = cString(method.methodId);
    allocs.push(id);
    try {
        checkStatus(m.ccall("n4m_method_find", "number", ["number", "number"], [id.ptr, indexPtr]));
        m.setValue(out, 0, "i32");
        checkStatus(m.ccall("n4m_params_create", "number", ["number", "number", "number"], [ctx, m.getValue(indexPtr, "i32"), out]), ctx);
        const params = m.getValue(out, "i32");
        try {
            for (const [name, value] of Object.entries(method.params)) {
                if (value === undefined)
                    continue;
                const type = method.paramTypes[name];
                if (type === undefined)
                    throw new Error(`${method.methodId}: unknown parameter '${name}'`);
                const key = cString(name);
                allocs.push(key);
                let status;
                if (type === "int") {
                    status = m.ccall("n4m_params_set_int", "number", ["number", "number", "i64"], [params, key.ptr, BigInt(value)]);
                }
                else if (type === "double") {
                    status = m.ccall("n4m_params_set_double", "number", ["number", "number", "number"], [params, key.ptr, value]);
                }
                else if (type === "bool") {
                    status = m.ccall("n4m_params_set_bool", "number", ["number", "number", "number"], [params, key.ptr, value ? 1 : 0]);
                }
                else if (type === "enum") {
                    const choice = cString(String(value));
                    allocs.push(choice);
                    status = m.ccall("n4m_params_set_enum", "number", ["number", "number", "number"], [params, key.ptr, choice.ptr]);
                }
                else if (type === "int_array") {
                    const arr = allocI64(value);
                    allocs.push(arr);
                    status = m.ccall("n4m_params_set_int_array", "number", ["number", "number", "number", "i64"], [params, key.ptr, arr.ptr, BigInt(value.length)]);
                }
                else {
                    const arr = allocF64(value);
                    allocs.push(arr);
                    status = m.ccall("n4m_params_set_double_array", "number", ["number", "number", "number", "i64"], [params, key.ptr, arr.ptr, BigInt(value.length)]);
                }
                if (status !== 0)
                    throw new Error(`${method.methodId}: invalid value for parameter '${name}'`);
            }
            checkStatus(m.ccall("n4m_params_validate", "number", ["number", "number"], [ctx, params]), ctx);
            return params;
        }
        catch (error) {
            m.ccall("n4m_params_destroy", null, ["number"], [params]);
            throw error;
        }
    }
    finally {
        allocs.forEach((a) => a.free());
        m._free(indexPtr);
        m._free(out);
    }
}
function checkMatrix(name, M) {
    if (!Number.isInteger(M.rows) || !Number.isInteger(M.cols) || M.rows < 1 || M.cols < 1 ||
        M.data.length !== M.rows * M.cols) {
        throw new Error(`${name} must be a non-empty rows x cols matrix with rows*cols values; ` +
            `got ${M.rows} x ${M.cols} with ${M.data.length} values`);
    }
}
function checkLength(name, values, length, what) {
    if (values.length !== length) {
        throw new Error(`${name} must have length ${length} (${what}); got ${values.length}`);
    }
}
function checkIntegers(name, values) {
    for (let i = 0; i < values.length; ++i) {
        if (!Number.isSafeInteger(values[i]))
            throw new Error(`${name} must contain integers`);
    }
}
/** The target as a matrix with one row per row of X (a vector is one column). */
function targetMatrix(y, rows) {
    if ("data" in y && "rows" in y) {
        const ym = y;
        checkMatrix("y", ym);
        if (ym.rows !== rows) {
            throw new Error(`y must have ${rows} rows (one per row of X); got ${ym.rows} x ${ym.cols}`);
        }
        return ym;
    }
    const values = y;
    checkLength("y", values, rows, "one per row of X");
    return { data: Float64Array.from(values), rows, cols: 1 };
}
/**
 * Runs `fn` over an n4m_fit_inputs_v1_t built from the given data. Per-row
 * inputs (y, labels, sampleWeight, groups, foldIds) must have one entry per
 * row of X and per-column inputs (featureGroups, axis) one per column; the
 * lengths are checked here with the argument named, and again natively.
 */
export function withFitInputs(X, y, labels, inputs, fn) {
    const m = getModule();
    const allocs = [];
    const hold = (a) => (allocs.push(a), a.ptr);
    const struct = m._malloc(FIT_INPUTS_SIZE);
    try {
        m.HEAPU8.fill(0, struct, struct + FIT_INPUTS_SIZE);
        m.setValue(struct, FIT_INPUTS_SIZE, "i32");
        checkMatrix("X", X);
        const rows = "one per row of X";
        const cols = "one per column of X";
        if (inputs.sampleWeight)
            checkLength("sampleWeight", inputs.sampleWeight, X.rows, rows);
        for (const [name, values] of [["groups", inputs.groups], ["foldIds", inputs.foldIds]]) {
            if (values) {
                checkLength(name, values, X.rows, rows);
                checkIntegers(name, values);
            }
        }
        if (inputs.featureGroups) {
            checkLength("featureGroups", inputs.featureGroups, X.cols, cols);
            checkIntegers("featureGroups", inputs.featureGroups);
        }
        if (inputs.blocks)
            checkIntegers("blocks", inputs.blocks);
        if (inputs.axis)
            checkLength("axis", inputs.axis, X.cols, cols);
        if (inputs.XTarget)
            checkMatrix("XTarget", inputs.XTarget);
        const xv = makeMatrixView(X.data, X.rows, X.cols);
        allocs.push({ ptr: xv.viewPtr, free: xv.free });
        m.setValue(struct + OFF.X, xv.viewPtr, "i32");
        const setArray = (ptrOff, lenOff, a, n) => {
            m.setValue(struct + ptrOff, hold(a), "i32");
            m.setValue(struct + lenOff, BigInt(n), "i64");
        };
        if (y !== undefined && labels) {
            const ids = Array.from(y);
            checkLength("labels", ids, X.rows, rows);
            checkIntegers("labels", ids);
            setArray(OFF.labels, OFF.nLabels, allocI64(ids), ids.length);
        }
        else if (y !== undefined) {
            const ym = targetMatrix(y, X.rows);
            const yv = makeMatrixView(ym.data, ym.rows, ym.cols);
            allocs.push({ ptr: yv.viewPtr, free: yv.free });
            m.setValue(struct + OFF.Y, yv.viewPtr, "i32");
        }
        if (inputs.sampleWeight)
            setArray(OFF.sampleWeight, OFF.nSampleWeight, allocF64(inputs.sampleWeight), inputs.sampleWeight.length);
        if (inputs.groups)
            setArray(OFF.groups, OFF.nGroups, allocI64(inputs.groups), inputs.groups.length);
        if (inputs.featureGroups)
            setArray(OFF.featureGroups, OFF.nFeatureGroups, allocI64(inputs.featureGroups), inputs.featureGroups.length);
        if (inputs.blocks)
            setArray(OFF.blocks, OFF.nBlocks, allocI64(inputs.blocks), inputs.blocks.length);
        if (inputs.axis)
            setArray(OFF.axis, OFF.nAxis, allocF64(inputs.axis), inputs.axis.length);
        if (inputs.foldIds)
            setArray(OFF.foldIds, OFF.nFoldIds, allocI64(inputs.foldIds), inputs.foldIds.length);
        if (inputs.XTarget) {
            const tv = makeMatrixView(inputs.XTarget.data, inputs.XTarget.rows, inputs.XTarget.cols);
            allocs.push({ ptr: tv.viewPtr, free: tv.free });
            m.setValue(struct + OFF.XTarget, tv.viewPtr, "i32");
        }
        return fn(struct, hold);
    }
    finally {
        allocs.forEach((a) => a.free());
        m._free(struct);
    }
}
/** Parameters of one catalog method (estimator or procedure). */
export class NativeMethod {
    /** Explicit parameter values (unset ones take the native default). */
    params = {};
    /** Registers a generated class so fromN4me() and methodClass() find it. */
    static register(methodId, cls) {
        registry.set(methodId, cls);
    }
}
/** The native manifest: every method's roles, node kinds, fit inputs and typed parameters. */
export function manifest() {
    const m = getModule();
    const sizePtr = m._malloc(4);
    try {
        checkStatus(m.ccall("n4m_method_manifest_json", "number", ["number", "number", "number"], [0, 0, sizePtr]));
        const size = m.getValue(sizePtr, "i32");
        const buf = m._malloc(Math.max(1, size));
        try {
            checkStatus(m.ccall("n4m_method_manifest_json", "number", ["number", "number", "number"], [buf, size, sizePtr]));
            return JSON.parse(new TextDecoder().decode(m.HEAPU8.subarray(buf, buf + size)));
        }
        finally {
            m._free(buf);
        }
    }
    finally {
        m._free(sizePtr);
    }
}
/** The generated class of a catalog method id. */
export function methodClass(methodId) {
    const cls = registry.get(methodId);
    if (cls === undefined)
        throw new Error(`no n4m role class for '${methodId}'`);
    return cls;
}
/** Base of every generated estimator: parameters, fit and N4ME state. */
export class NativeEstimator extends NativeMethod {
    /** True when the fit target is class labels (classifiers). */
    labelTarget = false;
    ptr = 0;
    get fitted() {
        return this.ptr !== 0;
    }
    /**
     * Fit on row-major X and the target: responses for a regressor (a vector
     * or a row-major matrix, one row per row of X), integer class ids for a
     * classifier (one per row). Returns this. The fitted state is replaced
     * only when the fit succeeds: a failed refit leaves the previous one.
     */
    fit(X, y, inputs = {}) {
        const m = getModule();
        const est = withFitInputs(X, y, this.labelTarget, inputs, (struct, hold) => withContext((ctx) => {
            const params = nativeParams(ctx, this);
            const out = m._malloc(4);
            try {
                m.setValue(out, 0, "i32");
                const idPtr = hold(cString(this.methodId));
                checkStatus(m.ccall("n4m_estimator_create", "number", ["number", "number", "number", "number"], [ctx, idPtr, params, out]), ctx);
                const handle = m.getValue(out, "i32");
                const status = m.ccall("n4m_estimator_fit", "number", ["number", "number", "number"], [ctx, handle, struct]);
                if (status !== 0) {
                    m.ccall("n4m_estimator_destroy", null, ["number"], [handle]);
                    checkStatus(status, ctx);
                }
                return handle;
            }
            finally {
                m._free(out);
                m.ccall("n4m_params_destroy", null, ["number"], [params]);
            }
        }));
        this.dispose();
        this.ptr = est;
        return this;
    }
    /** True when the fitted state embeds training rows (kernel PLS, GPR-PLS, LW-PLS, ...). */
    containsTrainingRows() {
        const m = getModule();
        const out = m._malloc(4);
        try {
            checkStatus(m.ccall("n4m_estimator_contains_training_rows", "number", ["number", "number"], [this.handle(), out]));
            return m.getValue(out, "i32") !== 0;
        }
        finally {
            m._free(out);
        }
    }
    /**
     * Named diagnostics of the last fit (n4m_estimator_fit_result), for
     * example the training loss history of a neural regressor; null when the
     * method keeps none, including after fromN4me (diagnostics are not state).
     */
    fitResult() {
        const m = getModule();
        const handle = this.handle();
        const out = m._malloc(8);
        try {
            m.setValue(out, 0, "i32");
            const status = m.ccall("n4m_estimator_fit_result", "number", ["number", "number"], [handle, out]);
            if (status === 10)
                return null; // N4M_ERR_UNSUPPORTED: no fit diagnostics
            checkStatus(status);
            const result = m.getValue(out, "i32");
            checkStatus(m.ccall("n4m_method_result_entry_count", "number", ["number", "number"], [result, out]));
            const count = m.getValue(out, "i32");
            const entries = {};
            for (let i = 0; i < count; ++i) {
                checkStatus(m.ccall("n4m_method_result_entry", "number", ["number", "number", "number", "number"], [result, i, out, out + 4]));
                const name = m.UTF8ToString(m.getValue(out, "i32"));
                entries[name] = readEntry(result, name, m.getValue(out + 4, "i32"));
            }
            return entries;
        }
        finally {
            m._free(out);
        }
    }
    /**
     * Portable fitted state (N4ME bytes), readable by every n4m binding. A
     * state that embeds training rows (containsTrainingRows()) is refused
     * unless `allowTrainingRows` is set: sharing the export shares them.
     */
    toN4me(options = {}) {
        const m = getModule();
        const handle = this.handle();
        const flags = options.allowTrainingRows === true ? 1 : 0;
        return withContext((ctx) => {
            const sizePtr = m._malloc(4);
            try {
                checkStatus(m.ccall("n4m_estimator_export_size", "number", ["number", "number", "number", "number"], [ctx, handle, flags, sizePtr]), ctx);
                const size = m.getValue(sizePtr, "i32");
                const buf = m._malloc(Math.max(1, size));
                try {
                    checkStatus(m.ccall("n4m_estimator_export_to_buffer", "number", ["number", "number", "number", "number", "number", "number"], [ctx, handle, flags, buf, size, sizePtr]), ctx);
                    return m.HEAPU8.slice(buf, buf + m.getValue(sizePtr, "i32"));
                }
                finally {
                    m._free(buf);
                }
            }
            finally {
                m._free(sizePtr);
            }
        });
    }
    /** Rebuilds a fitted estimator of the class registered for its method. */
    static fromN4me(payload) {
        const m = getModule();
        const data = m._malloc(Math.max(1, payload.byteLength));
        const out = m._malloc(4);
        try {
            m.HEAPU8.set(payload, data);
            m.setValue(out, 0, "i32");
            const handle = withContext((ctx) => {
                checkStatus(m.ccall("n4m_estimator_import_from_buffer", "number", ["number", "number", "number", "number"], [ctx, data, payload.byteLength, out]), ctx);
                return m.getValue(out, "i32");
            });
            const cls = registry.get(NativeEstimator.methodIdOf(handle));
            if (cls === undefined || !(cls.prototype instanceof NativeEstimator)) {
                m.ccall("n4m_estimator_destroy", null, ["number"], [handle]);
                throw new Error("no JS class registered for this N4ME method");
            }
            const est = new cls();
            est.ptr = handle;
            return est;
        }
        finally {
            m._free(data);
            m._free(out);
        }
    }
    /**
     * Affine map predict(X) = X B + b of a fitted estimator declaring the
     * native N4M_CAP_AFFINE capability (PLS family, Ridge, ...), also after
     * fromN4me(). Throws the native UNSUPPORTED refusal otherwise.
     */
    affineCoefficients() {
        const m = getModule();
        const handle = this.handle();
        const p = this.dim("n4m_estimator_n_features_in", handle);
        const q = this.dim("n4m_estimator_n_outputs", handle);
        const coef = m._malloc(Math.max(1, p * q) * 8);
        const icpt = m._malloc(Math.max(1, q) * 8);
        try {
            withContext((ctx) => checkStatus(m.ccall("n4m_estimator_affine_coefficients_v1", "number", ["number", "number", "number", "i64", "number", "i64"], [ctx, handle, coef, BigInt(p * q), icpt, BigInt(q)]), ctx));
            return {
                coefficients: { data: m.HEAPF64.slice(coef / 8, coef / 8 + p * q), rows: p, cols: q },
                intercept: m.HEAPF64.slice(icpt / 8, icpt / 8 + q),
            };
        }
        finally {
            m._free(coef);
            m._free(icpt);
        }
    }
    /** W, R, P, Q, x mean / scale of a fitted latent-variable model; native refusal otherwise. */
    latentStructure() {
        const m = getModule();
        const handle = this.handle();
        const p = this.dim("n4m_estimator_n_features_in", handle);
        const q = this.dim("n4m_estimator_n_outputs", handle);
        const aPtr = m._malloc(8);
        const allocs = [aPtr];
        try {
            return withContext((ctx) => {
                const call = (bufs, pa, qa) => checkStatus(m.ccall("n4m_estimator_latent_structure_v1", "number", ["number", "number", "number", "number", "number", "number", "i64", "number", "i64",
                    "number", "number", "i64"], [ctx, handle, aPtr, bufs[0], bufs[1], bufs[2], BigInt(pa), bufs[3], BigInt(qa),
                    bufs[4], bufs[5], BigInt(p)]), ctx);
                call([0, 0, 0, 0, 0, 0], 0, 0);
                const a = readI64(aPtr);
                const sizes = [p * a, p * a, p * a, q * a, p, p];
                const bufs = sizes.map((n) => {
                    const ptr = m._malloc(Math.max(1, n) * 8);
                    allocs.push(ptr);
                    return ptr;
                });
                call(bufs, p * a, q * a);
                const read = (i) => m.HEAPF64.slice(bufs[i] / 8, bufs[i] / 8 + sizes[i]);
                return {
                    nComponents: a,
                    xWeights: { data: read(0), rows: p, cols: a },
                    xRotations: { data: read(1), rows: p, cols: a },
                    xLoadings: { data: read(2), rows: p, cols: a },
                    yLoadings: { data: read(3), rows: q, cols: a },
                    xMean: read(4),
                    xScale: read(5),
                };
            });
        }
        finally {
            allocs.forEach((ptr) => m._free(ptr));
        }
    }
    /**
     * Importance profile (nFeaturesIn values) on rows X given in this
     * estimator's input space (normally the preprocessed training rows):
     * VIP, target-projection selectivity ratio or standardized coefficients
     * (ddof = 1), computed natively.
     */
    modelImportance(X, options) {
        const kind = IMPORTANCE_KINDS[options?.kind];
        if (kind === undefined)
            throw new Error(`unknown importance kind ${String(options?.kind)}`);
        const outputIndex = options.outputIndex ?? 0;
        if (!Number.isSafeInteger(outputIndex) || outputIndex < 0) {
            throw new Error("outputIndex must be a nonnegative integer");
        }
        checkMatrix("X", X);
        const m = getModule();
        const handle = this.handle();
        const p = this.dim("n4m_estimator_n_features_in", handle);
        const xv = makeMatrixView(X.data, X.rows, X.cols);
        const out = m._malloc(Math.max(1, p) * 8);
        try {
            withContext((ctx) => checkStatus(m.ccall("n4m_estimator_importance_v1", "number", ["number", "number", "number", "number", "i64", "number", "i64"], [ctx, handle, xv.viewPtr, kind, BigInt(outputIndex), out, BigInt(p)]), ctx));
            return m.HEAPF64.slice(out / 8, out / 8 + p);
        }
        finally {
            xv.free();
            m._free(out);
        }
    }
    dim(symbol, handle) {
        const m = getModule();
        const ptr = m._malloc(8);
        try {
            checkStatus(m.ccall(symbol, "number", ["number", "number"], [handle, ptr]));
            return readI64(ptr);
        }
        finally {
            m._free(ptr);
        }
    }
    /** Releases the native estimator. */
    dispose() {
        if (this.ptr !== 0) {
            getModule().ccall("n4m_estimator_destroy", null, ["number"], [this.ptr]);
            this.ptr = 0;
        }
    }
    predictMatrix(X) {
        return this.matrixOp("n4m_estimator_predict", "n4m_estimator_n_outputs", X);
    }
    transformMatrix(X) {
        return this.matrixOp("n4m_estimator_transform", "n4m_estimator_transform_cols", X);
    }
    decisionMatrix(X) {
        return this.matrixOp("n4m_estimator_decision_function", "n4m_estimator_n_outputs", X);
    }
    probaMatrix(X) {
        return this.matrixOp("n4m_estimator_predict_proba", "n4m_estimator_n_outputs", X);
    }
    labelArray(X) {
        const m = getModule();
        const handle = this.handle();
        const xv = makeMatrixView(X.data, X.rows, X.cols);
        const buf = m._malloc(Math.max(1, X.rows) * 8);
        try {
            withContext((ctx) => checkStatus(m.ccall("n4m_estimator_predict_labels", "number", ["number", "number", "number", "number", "i64"], [ctx, handle, xv.viewPtr, buf, BigInt(X.rows)]), ctx));
            return Array.from({ length: X.rows }, (_, i) => readI64(buf + 8 * i));
        }
        finally {
            xv.free();
            m._free(buf);
        }
    }
    maskArray(X, y) {
        const m = getModule();
        const handle = this.handle();
        checkMatrix("X", X);
        if (y !== undefined)
            checkLength("y", y, X.rows, "one per row of X");
        const xv = makeMatrixView(X.data, X.rows, X.cols);
        const yv = y === undefined ? undefined
            : makeMatrixView(Float64Array.from(y), X.rows, 1);
        const buf = m._malloc(Math.max(1, X.rows));
        try {
            withContext((ctx) => checkStatus(m.ccall("n4m_estimator_apply_mask", "number", ["number", "number", "number", "number", "number", "i64"], [ctx, handle, xv.viewPtr, yv ? yv.viewPtr : 0, buf, BigInt(X.rows)]), ctx));
            return Array.from(m.HEAPU8.subarray(buf, buf + X.rows), (v) => v !== 0);
        }
        finally {
            xv.free();
            yv?.free();
            m._free(buf);
        }
    }
    classArray() {
        return this.indexArray("n4m_estimator_classes");
    }
    selectedIndexArray() {
        return this.indexArray("n4m_estimator_selected_indices");
    }
    /** Reads a (handle, out, capacity, out_count) integer list. */
    indexArray(symbol) {
        const m = getModule();
        const handle = this.handle();
        const countPtr = m._malloc(8);
        try {
            checkStatus(m.ccall(symbol, "number", ["number", "number", "i64", "number"], [handle, 0, BigInt(0), countPtr]));
            const count = readI64(countPtr);
            const buf = m._malloc(Math.max(1, count) * 8);
            try {
                checkStatus(m.ccall(symbol, "number", ["number", "number", "i64", "number"], [handle, buf, BigInt(count), countPtr]));
                return Array.from({ length: count }, (_, i) => readI64(buf + 8 * i));
            }
            finally {
                m._free(buf);
            }
        }
        finally {
            m._free(countPtr);
        }
    }
    matrixOp(symbol, widthSymbol, X) {
        const m = getModule();
        const handle = this.handle();
        const widthPtr = m._malloc(8);
        try {
            checkStatus(m.ccall(widthSymbol, "number", ["number", "number"], [handle, widthPtr]));
            const cols = readI64(widthPtr);
            const xv = makeMatrixView(X.data, X.rows, X.cols);
            const ov = makeMatrixView(new Float64Array(X.rows * cols), X.rows, cols);
            try {
                withContext((ctx) => checkStatus(m.ccall(symbol, "number", ["number", "number", "number", "number"], [ctx, handle, xv.viewPtr, ov.viewPtr]), ctx));
                return { data: m.HEAPF64.slice(ov.dataPtr / 8, ov.dataPtr / 8 + X.rows * cols), rows: X.rows, cols };
            }
            finally {
                xv.free();
                ov.free();
            }
        }
        finally {
            m._free(widthPtr);
        }
    }
    handle() {
        if (this.ptr === 0)
            throw new Error(`${this.methodId} is not fitted`);
        return this.ptr;
    }
    static methodIdOf(handle) {
        const m = getModule();
        const indexPtr = m._malloc(4);
        const capsPtr = m._malloc(8);
        const info = m._malloc(72);
        try {
            checkStatus(m.ccall("n4m_estimator_info", "number", ["number", "number", "number"], [handle, indexPtr, capsPtr]));
            m.HEAPU8.fill(0, info, info + 72);
            m.setValue(info, 72, "i32");
            checkStatus(m.ccall("n4m_method_info_v1", "number", ["number", "number"], [m.getValue(indexPtr, "i32"), info]));
            return m.UTF8ToString(m.getValue(info + 8, "i32"));
        }
        finally {
            m._free(indexPtr);
            m._free(capsPtr);
            m._free(info);
        }
    }
}
const IMPORTANCE_KINDS = {
    vip: 0, selectivity_ratio: 1, standardized_coefficients: 2,
};
/** Base of the generated procedures: one native run, no fitted state. */
export class NativeProcedure extends NativeMethod {
    runRaw(X, y, inputs, read) {
        const m = getModule();
        const result = withFitInputs(X, y, false, inputs, (struct, hold) => withContext((ctx) => {
            const indexPtr = m._malloc(4);
            const out = m._malloc(4);
            const params = nativeParams(ctx, this);
            try {
                checkStatus(m.ccall("n4m_method_find", "number", ["number", "number"], [hold(cString(this.methodId)), indexPtr]));
                m.setValue(out, 0, "i32");
                checkStatus(m.ccall("n4m_procedure_run", "number", ["number", "number", "number", "number", "number"], [ctx, m.getValue(indexPtr, "i32"), params, struct, out]), ctx);
                return m.getValue(out, "i32");
            }
            finally {
                m.ccall("n4m_params_destroy", null, ["number"], [params]);
                m._free(indexPtr);
                m._free(out);
            }
        }));
        try {
            return read(result);
        }
        finally {
            m.ccall("n4m_method_result_destroy", null, ["number"], [result]);
        }
    }
    splitFolds(X, y, groups) {
        return this.runRaw(X, y, groups ? { groups } : {}, (result) => {
            const m = getModule();
            const scratch = m._malloc(32);
            try {
                checkStatus(m.ccall("n4m_method_result_get_n_folds", "number", ["number", "number"], [result, scratch]));
                const n = m.getValue(scratch, "i32");
                const folds = [];
                for (let f = 0; f < n; ++f) {
                    checkStatus(m.ccall("n4m_method_result_get_fold", "number", ["number", "number", "number", "number", "number", "number"], [result, f, scratch, scratch + 8, scratch + 16, scratch + 24]));
                    const take = (ptrAt, lenAt) => {
                        const base = m.getValue(ptrAt, "i32");
                        return Array.from({ length: readI64(lenAt) }, (_, i) => readI64(base + 8 * i));
                    };
                    folds.push({ train: take(scratch, scratch + 8), test: take(scratch + 16, scratch + 24) });
                }
                return folds;
            }
            finally {
                m._free(scratch);
            }
        });
    }
    augmentMatrix(X, axis) {
        return this.runRaw(X, undefined, axis ? { axis } : {}, (result) => readEntry(result, "X", 0));
    }
    augmentWithTargets(X, y, axis) {
        return this.runRaw(X, y, axis ? { axis } : {}, (result) => ({
            X: readEntry(result, "X", 0),
            Y: readEntry(result, "Y", 0),
        }));
    }
    runOutputs(X, y, inputs = {}) {
        return this.runRaw(X, y, inputs, (result) => {
            const m = getModule();
            const scratch = m._malloc(8);
            try {
                checkStatus(m.ccall("n4m_method_result_entry_count", "number", ["number", "number"], [result, scratch]));
                const count = m.getValue(scratch, "i32");
                const out = {};
                for (let i = 0; i < count; ++i) {
                    checkStatus(m.ccall("n4m_method_result_entry", "number", ["number", "number", "number", "number"], [result, i, scratch, scratch + 4]));
                    const name = m.UTF8ToString(m.getValue(scratch, "i32"));
                    out[name] = readEntry(result, name, m.getValue(scratch + 4, "i32"));
                }
                return out;
            }
            finally {
                m._free(scratch);
            }
        });
    }
}
/** One named result entry (kinds of n4m_method_result_entry_kind_t). */
function readEntry(result, name, kind) {
    const m = getModule();
    const key = cString(name);
    const scratch = m._malloc(24);
    try {
        if (kind === 0) {
            checkStatus(m.ccall("n4m_method_result_get_double_matrix", "number", ["number", "number", "number", "number", "number"], [result, key.ptr, scratch, scratch + 8, scratch + 16]));
            const rows = readI64(scratch + 8);
            const cols = readI64(scratch + 16);
            const data = m.getValue(scratch, "i32") / 8;
            return { data: m.HEAPF64.slice(data, data + rows * cols), rows, cols };
        }
        if (kind === 3) {
            checkStatus(m.ccall("n4m_method_result_get_scalar", "number", ["number", "number", "number"], [result, key.ptr, scratch]));
            return m.getValue(scratch, "double");
        }
        if (kind === 1) {
            checkStatus(m.ccall("n4m_method_result_get_int_vector", "number", ["number", "number", "number", "number"], [result, key.ptr, scratch, scratch + 8]));
            const base = m.getValue(scratch, "i32");
            return Array.from({ length: m.getValue(scratch + 8, "i32") }, (_, i) => m.getValue(base + 4 * i, "i32"));
        }
        checkStatus(m.ccall("n4m_method_result_get_int64_vector", "number", ["number", "number", "number", "number"], [result, key.ptr, scratch, scratch + 8]));
        const base = m.getValue(scratch, "i32");
        return Array.from({ length: readI64(scratch + 8) }, (_, i) => readI64(base + 8 * i));
    }
    finally {
        key.free();
        m._free(scratch);
    }
}
