// SPDX-License-Identifier: CECILL-2.1 OR AGPL-3.0-or-later
// Native role pipeline (ABI 2.14): a trained linear recipe of catalog
// estimators (sample filters, transformers / selectors, one regressor or
// classifier). Recipe validation, fit-input routing, the feature-name check
// and the per-step N4ME states are native (n4m_role_pipeline_*); this file
// marshals only. Class label tables stay here, as the other bindings do, and
// follow the shared label contract (docs/abi/estimator_roles_design.md).
import { checkStatus, getModule, makeMatrixView } from "./ffi.js";
import { cString, methodClass, nativeParams, readI64, withContext, withFitInputs, } from "./estimatorRoles.js";
const ROLES = {
    1: "transformer", 2: "regressor", 4: "classifier", 8: "selector", 16: "sample_filter",
};
const EXPORT_ALLOW_TRAINING_ROWS = 1;
// n4m_role_pipeline_step_info_v1_t on wasm32 (pointers 4 bytes, int64 8-aligned),
// with the ABI 2.24 affine / column_aligned fields at 40 / 44.
const STEP_INFO_SIZE = 48;
function parseStep(step) {
    const id = (s) => (s.startsWith("n4m:") ? s.slice(4) : s);
    if (typeof step === "string")
        return { methodId: id(step), params: {} };
    if (Array.isArray(step))
        return { methodId: id(step[0]), params: { ...step[1] } };
    const name = "methodId" in step ? step.methodId : step.class;
    return { methodId: id(name), params: { ...(step.params ?? {}) } };
}
/** A NUL-terminated string array; `n` pointers after the array head. */
function cStrings(values, allocs) {
    const m = getModule();
    const head = m._malloc(Math.max(1, values.length) * 4);
    allocs.push({ ptr: head, free: () => m._free(head) });
    values.forEach((v, i) => {
        const s = cString(v);
        allocs.push(s);
        m.setValue(head + 4 * i, s.ptr, "i32");
    });
    return head;
}
/** Column names become C strings: a NUL would silently truncate one, so it is refused. */
function checkNames(names) {
    names.forEach((name, i) => {
        if (name.includes("\0")) {
            throw new Error(`feature name ${i} must not contain NUL: ${JSON.stringify(name)}`);
        }
    });
}
/**
 * Class ids for the core and the label table (shared label contract).
 * Integer labels are the ids and must be safe integers (so they fit int64
 * exactly); strings and non-integer numbers become the table of sorted unique
 * labels, the ids their positions. Missing (null / undefined), non-finite and
 * boolean labels, and a mix of strings and numbers, are refused.
 */
function encodeLabels(y) {
    const labels = Array.from(y);
    for (const v of labels) {
        if (v === null || v === undefined) {
            throw new TypeError("class labels must not be missing (null or undefined)");
        }
        if (typeof v === "number" && !Number.isFinite(v)) {
            throw new RangeError("class labels must be finite (no NaN or infinity)");
        }
        if (typeof v !== "number" && typeof v !== "string") {
            throw new TypeError(`class labels must be integers, finite numbers or strings; got ${String(v)}`);
        }
    }
    const numbers = labels.every((v) => typeof v === "number");
    if (numbers && labels.every((v) => Number.isInteger(v))) {
        const unsafe = labels.find((v) => !Number.isSafeInteger(v));
        if (unsafe !== undefined) {
            throw new RangeError(`class labels must fit int64 exactly (safe integers); got ${unsafe}`);
        }
        return { ids: labels };
    }
    if (!numbers && !labels.every((v) => typeof v === "string")) {
        throw new TypeError("class labels must be all numbers or all strings");
    }
    const names = [...new Set(labels)];
    names.sort(numbers ? (a, b) => a - b : undefined);
    const index = new Map(names.map((name, i) => [name, i]));
    return { ids: labels.map((v) => index.get(v)), names };
}
/**
 * An imported label table (class_names) checked against the shared label
 * contract: a non-empty list of unique strings or finite numbers (not both,
 * no boolean) with an entry for every class id of the fitted state.
 */
function labelTable(table, ids) {
    if (!Array.isArray(table))
        throw new TypeError("class_names must be a list");
    if (table.length === 0)
        throw new RangeError("class_names must not be empty");
    table.forEach((v, i) => {
        if (v === null || v === undefined)
            throw new TypeError(`class_names has a missing entry at ${i}`);
        if (typeof v !== "string" && typeof v !== "number") {
            throw new TypeError(`class_names entries must be strings or numbers; entry ${i} is ${String(v)}`);
        }
        if (typeof v === "number" && !Number.isFinite(v)) {
            throw new RangeError(`class_names has a non-finite entry at ${i}: ${v}`);
        }
    });
    if (new Set(table.map((v) => typeof v)).size > 1)
        throw new TypeError("class_names mixes strings and numbers");
    const inexact = table.find((v) => typeof v === "number" && Number.isInteger(v) && Math.abs(v) > 2 ** 53);
    if (inexact !== undefined) {
        throw new RangeError(`numeric class_names must be exactly representable as float64; ${inexact} is beyond ±2^53`);
    }
    const seen = new Set();
    for (const v of table) {
        if (seen.has(v))
            throw new RangeError(`class_names has duplicate label ${JSON.stringify(v)}`);
        seen.add(v);
    }
    const outside = ids.find((id) => id < 0 || id >= table.length);
    if (outside !== undefined) {
        throw new RangeError(`class id ${outside} has no entry in class_names (${table.length} entries)`);
    }
    return [...table];
}
/** Native trained recipe of role steps, portable as N4ME states. */
export class RolePipeline {
    steps;
    ptr = 0;
    names;
    classNames;
    constructor(steps) {
        this.steps = [...steps];
    }
    /** An unfitted pipeline; the recipe is validated natively. */
    static fromSteps(steps) {
        const pipeline = new RolePipeline(steps);
        getModule().ccall("n4m_role_pipeline_destroy", null, ["number"], [pipeline.create()]);
        return pipeline;
    }
    /**
     * A fitted pipeline rebuilt from one N4ME state per stateful step. The
     * native import refuses states that contradict the recipe (count, method,
     * parameters, role, widths). classNames restores the label table of a
     * classifier trained on strings or non-integer numbers; it is refused
     * unless it is a non-empty list of unique strings or finite numbers with
     * an entry for every class id of the fitted state.
     */
    static fromStates(steps, states, options = {}) {
        const m = getModule();
        const pipeline = new RolePipeline(steps);
        const handle = pipeline.create();
        const payloads = states.map((s) => (s instanceof Uint8Array ? s : s.n4me));
        const allocs = [];
        let classNames;
        try {
            const buffers = m._malloc(Math.max(1, payloads.length) * 4);
            allocs.push({ ptr: buffers, free: () => m._free(buffers) });
            const sizes = m._malloc(Math.max(1, payloads.length) * 4);
            allocs.push({ ptr: sizes, free: () => m._free(sizes) });
            payloads.forEach((payload, i) => {
                const data = m._malloc(Math.max(1, payload.byteLength));
                allocs.push({ ptr: data, free: () => m._free(data) });
                m.HEAPU8.set(payload, data);
                m.setValue(buffers + 4 * i, data, "i32");
                m.setValue(sizes + 4 * i, payload.byteLength, "i32");
            });
            withContext((ctx) => {
                RolePipeline.setFeatureNames(ctx, handle, options.featureNames);
                checkStatus(m.ccall("n4m_role_pipeline_import_states", "number", ["number", "number", "number", "number", "number"], [ctx, handle, payloads.length, buffers, sizes]), ctx);
            });
            if (options.classNames !== undefined) {
                if (RolePipeline.stepInfo(handle, steps.length - 1).role !== "classifier") {
                    throw new TypeError("class_names are given but the final step is not a classifier");
                }
                classNames = labelTable(options.classNames, RolePipeline.classIds(handle));
            }
        }
        catch (error) {
            m.ccall("n4m_role_pipeline_destroy", null, ["number"], [handle]);
            throw error;
        }
        finally {
            allocs.forEach((a) => a.free());
        }
        pipeline.publish(handle, options.featureNames, classNames);
        return pipeline;
    }
    get fitted() {
        return this.ptr !== 0;
    }
    /** Input column names stored at fit or import (undefined: positional). */
    get featureNames() {
        return this.names === undefined ? undefined : [...this.names];
    }
    /**
     * Fits every step natively. y holds responses for a final regressor (a
     * vector or a row-major matrix) or class labels for a final classifier;
     * featureNames are stored and checked at every later call. Returns this;
     * on failure the previous fit is kept.
     */
    fit(X, y, inputs = {}) {
        const m = getModule();
        const handle = this.create();
        let classNames;
        try {
            const classifier = RolePipeline.stepInfo(handle, this.steps.length - 1).role === "classifier";
            let target = y;
            if (classifier && y !== undefined) {
                const encoded = encodeLabels(y);
                classNames = encoded.names;
                target = encoded.ids;
            }
            const { featureNames, ...fitInputs } = inputs;
            withFitInputs(X, target, classifier, fitInputs, (struct) => withContext((ctx) => {
                RolePipeline.setFeatureNames(ctx, handle, featureNames);
                checkStatus(m.ccall("n4m_role_pipeline_fit", "number", ["number", "number", "number"], [ctx, handle, struct]), ctx);
            }));
            this.publish(handle, featureNames, classNames);
        }
        catch (error) {
            if (this.ptr !== handle)
                m.ccall("n4m_role_pipeline_destroy", null, ["number"], [handle]);
            throw error;
        }
        return this;
    }
    /** Predictions of a final regressor. */
    predict(X, featureNames) {
        return this.matrixOp("n4m_role_pipeline_predict", "n4m_role_pipeline_n_outputs", X, featureNames);
    }
    /** Rows after the transformers and selectors (the final step's input). */
    transform(X, featureNames) {
        return this.matrixOp("n4m_role_pipeline_transform", "n4m_role_pipeline_transform_cols", X, featureNames);
    }
    /** Class scores of a final classifier, one column per class in classes() order. */
    decisionFunction(X, featureNames) {
        return this.matrixOp("n4m_role_pipeline_decision_function", "n4m_role_pipeline_n_outputs", X, featureNames);
    }
    /** Class probabilities, for final classifiers that define them. */
    predictProba(X, featureNames) {
        return this.matrixOp("n4m_role_pipeline_predict_proba", "n4m_role_pipeline_n_outputs", X, featureNames);
    }
    /** Class labels of a final classifier (names when trained on names). */
    predictLabels(X, featureNames) {
        const m = getModule();
        const handle = this.handle();
        const xv = makeMatrixView(X.data, X.rows, X.cols);
        const buf = m._malloc(Math.max(1, X.rows) * 8);
        const allocs = [];
        try {
            withContext((ctx) => {
                this.checkFeatures(ctx, X, featureNames, allocs);
                checkStatus(m.ccall("n4m_role_pipeline_predict_labels", "number", ["number", "number", "number", "number", "i64"], [ctx, handle, xv.viewPtr, buf, BigInt(X.rows)]), ctx);
            });
            return Array.from({ length: X.rows }, (_, i) => this.label(readI64(buf + 8 * i)));
        }
        finally {
            xv.free();
            m._free(buf);
            allocs.forEach((a) => a.free());
        }
    }
    /**
     * Label table of a classifier fitted on names (index = class id), else
     * undefined. Unlike classes() it keeps labels whose rows a sample filter
     * removed, so an exported pipeline can restore every name.
     */
    labelNames() {
        this.handle();
        return this.classNames === undefined ? undefined : [...this.classNames];
    }
    /** Fitted classes, ascending ids (names when trained on names). */
    classes() {
        return RolePipeline.classIds(this.handle()).map((id) => this.label(id));
    }
    /** Native class ids of the final classifier (n4m_role_pipeline_classes). */
    static classIds(handle) {
        const m = getModule();
        const countPtr = m._malloc(8);
        try {
            checkStatus(m.ccall("n4m_role_pipeline_classes", "number", ["number", "number", "i64", "number"], [handle, 0, BigInt(0), countPtr]));
            const count = readI64(countPtr);
            const buf = m._malloc(Math.max(1, count) * 8);
            try {
                checkStatus(m.ccall("n4m_role_pipeline_classes", "number", ["number", "number", "i64", "number"], [handle, buf, BigInt(count), countPtr]));
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
    /** Per step: method, role played, state index, fitted widths, training rows, affine / column-aligned traits. */
    stepsInfo() {
        const handle = this.handle();
        return this.steps.map((_, i) => RolePipeline.stepInfo(handle, i));
    }
    /** One step of stepsInfo(). */
    stepInfo(step) {
        if (!Number.isSafeInteger(step) || step < 0 || step >= this.steps.length) {
            throw new RangeError(`step ${step} is outside [0, ${this.steps.length})`);
        }
        return RolePipeline.stepInfo(this.handle(), step);
    }
    /**
     * Exact raw-input affine map predict(X) = X B_raw + b_raw of a fitted
     * pipeline whose transformers / selectors are all affine and whose final
     * model is affine (B_raw = A' B composed natively). The native refusal
     * names the first non-affine step ("step <i> (<method id>): ...").
     */
    inputAffineCoefficients() {
        const m = getModule();
        const handle = this.handle();
        const dims = m._malloc(16);
        try {
            checkStatus(m.ccall("n4m_role_pipeline_n_features_in", "number", ["number", "number"], [handle, dims]));
            checkStatus(m.ccall("n4m_role_pipeline_n_outputs", "number", ["number", "number"], [handle, dims + 8]));
            const p = readI64(dims), q = readI64(dims + 8);
            const coef = m._malloc(Math.max(1, p * q) * 8);
            const icpt = m._malloc(Math.max(1, q) * 8);
            try {
                withContext((ctx) => checkStatus(m.ccall("n4m_role_pipeline_input_affine_coefficients_v1", "number", ["number", "number", "number", "i64", "number", "i64"], [ctx, handle, coef, BigInt(p * q), icpt, BigInt(q)]), ctx));
                return {
                    coefficients: { data: m.HEAPF64.slice(coef / 8, coef / 8 + p * q), rows: p, cols: q },
                    intercept: m.HEAPF64.slice(icpt / 8, icpt / 8 + q),
                    nFeaturesIn: p,
                };
            }
            finally {
                m._free(coef);
                m._free(icpt);
            }
        }
        finally {
            m._free(dims);
        }
    }
    /**
     * One N4ME state per stateful step. A state that embeds training rows is
     * refused unless allowTrainingRows is set.
     */
    exportStates(options = {}) {
        const m = getModule();
        const handle = this.handle();
        const flags = options.allowTrainingRows ? EXPORT_ALLOW_TRAINING_ROWS : 0;
        return withContext((ctx) => this.stepsInfo().filter((s) => s.stateIndex >= 0).map((step) => {
            const sizePtr = m._malloc(4);
            try {
                checkStatus(m.ccall("n4m_role_pipeline_export_state_size", "number", ["number", "number", "number", "number", "number"], [ctx, handle, step.stateIndex, flags, sizePtr]), ctx);
                const size = m.getValue(sizePtr, "i32");
                const buf = m._malloc(Math.max(1, size));
                try {
                    checkStatus(m.ccall("n4m_role_pipeline_export_state_to_buffer", "number", ["number", "number", "number", "number", "number", "number", "number"], [ctx, handle, step.stateIndex, flags, buf, size, sizePtr]), ctx);
                    return {
                        methodId: step.methodId,
                        n4me: m.HEAPU8.slice(buf, buf + m.getValue(sizePtr, "i32")),
                        containsTrainingRows: step.containsTrainingRows,
                    };
                }
                finally {
                    m._free(buf);
                }
            }
            finally {
                m._free(sizePtr);
            }
        }));
    }
    /** Releases the native pipeline. */
    dispose() {
        if (this.ptr !== 0) {
            getModule().ccall("n4m_role_pipeline_destroy", null, ["number"], [this.ptr]);
            this.ptr = 0;
        }
    }
    create() {
        const m = getModule();
        const parsed = this.steps.map(parseStep);
        const allocs = [];
        const params = [];
        return withContext((ctx) => {
            try {
                for (const step of parsed) {
                    const method = new (methodClass(step.methodId))();
                    method.params = step.params;
                    params.push(nativeParams(ctx, method));
                }
                const ids = cStrings(parsed.map((s) => s.methodId), allocs);
                const paramsPtr = m._malloc(Math.max(1, params.length) * 4);
                allocs.push({ ptr: paramsPtr, free: () => m._free(paramsPtr) });
                params.forEach((p, i) => m.setValue(paramsPtr + 4 * i, p, "i32"));
                const out = m._malloc(4);
                allocs.push({ ptr: out, free: () => m._free(out) });
                m.setValue(out, 0, "i32");
                checkStatus(m.ccall("n4m_role_pipeline_create", "number", ["number", "number", "number", "number", "number"], [ctx, parsed.length, ids, paramsPtr, out]), ctx);
                return m.getValue(out, "i32");
            }
            finally {
                params.forEach((p) => m.ccall("n4m_params_destroy", null, ["number"], [p]));
                allocs.forEach((a) => a.free());
            }
        });
    }
    publish(handle, featureNames, classNames) {
        this.dispose();
        this.ptr = handle;
        this.names = featureNames === undefined ? undefined : [...featureNames];
        this.classNames = classNames === undefined ? undefined : [...classNames];
    }
    handle() {
        if (this.ptr === 0)
            throw new Error("RolePipeline is not fitted");
        return this.ptr;
    }
    label(id) {
        return this.classNames === undefined ? id : this.classNames[id];
    }
    static setFeatureNames(ctx, handle, names) {
        if (names === undefined)
            return;
        checkNames(names);
        const allocs = [];
        try {
            checkStatus(getModule().ccall("n4m_role_pipeline_set_feature_names", "number", ["number", "number", "number", "i64"], [ctx, handle, cStrings(names, allocs), BigInt(names.length)]), ctx);
        }
        finally {
            allocs.forEach((a) => a.free());
        }
    }
    /** Native width and feature-name check of X (names optional: positional). */
    checkFeatures(ctx, X, names, allocs) {
        if (names !== undefined && names.length !== X.cols) {
            throw new RangeError(`${names.length} feature names for ${X.cols} columns`);
        }
        if (names !== undefined)
            checkNames(names);
        const namesPtr = names === undefined ? 0 : cStrings(names, allocs);
        checkStatus(getModule().ccall("n4m_role_pipeline_check_features", "number", ["number", "number", "i64", "number"], [ctx, this.handle(), BigInt(X.cols), namesPtr]), ctx);
    }
    static stepInfo(handle, step) {
        const m = getModule();
        const info = m._malloc(STEP_INFO_SIZE);
        try {
            m.HEAPU8.fill(0, info, info + STEP_INFO_SIZE);
            m.setValue(info, STEP_INFO_SIZE, "i32");
            checkStatus(m.ccall("n4m_role_pipeline_step_info_v1", "number", ["number", "number", "number"], [handle, step, info]));
            return {
                methodId: m.UTF8ToString(m.getValue(info + 8, "i32")),
                role: ROLES[m.getValue(info + 12, "i32")],
                stateIndex: m.getValue(info + 16, "i32"),
                containsTrainingRows: m.getValue(info + 20, "i32") !== 0,
                nFeaturesIn: readI64(info + 24),
                nFeaturesOut: readI64(info + 32),
                affine: m.getValue(info + 40, "i32") !== 0,
                columnAligned: m.getValue(info + 44, "i32") !== 0,
            };
        }
        finally {
            m._free(info);
        }
    }
    matrixOp(symbol, widthSymbol, X, featureNames) {
        const m = getModule();
        const handle = this.handle();
        const widthPtr = m._malloc(8);
        const allocs = [];
        try {
            checkStatus(m.ccall(widthSymbol, "number", ["number", "number"], [handle, widthPtr]));
            const cols = readI64(widthPtr);
            const xv = makeMatrixView(X.data, X.rows, X.cols);
            const ov = makeMatrixView(new Float64Array(X.rows * cols), X.rows, cols);
            try {
                withContext((ctx) => {
                    this.checkFeatures(ctx, X, featureNames, allocs);
                    checkStatus(m.ccall(symbol, "number", ["number", "number", "number", "number"], [ctx, handle, xv.viewPtr, ov.viewPtr]), ctx);
                });
                return { data: m.HEAPF64.slice(ov.dataPtr / 8, ov.dataPtr / 8 + X.rows * cols), rows: X.rows, cols };
            }
            finally {
                xv.free();
                ov.free();
            }
        }
        finally {
            m._free(widthPtr);
            allocs.forEach((a) => a.free());
        }
    }
}
