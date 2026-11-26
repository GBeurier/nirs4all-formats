// SPDX-License-Identifier: CECILL-2.1 OR AGPL-3.0-or-later
// Fitted preprocessing and N4MP portability; all numerics remain in libn4m.
import { checkStatus, getModule, makeMatrixView } from "./ffi.js";
/** The 15 pipeline kinds currently implemented by the native core. */
export var PipelineOperatorKind;
(function (PipelineOperatorKind) {
    PipelineOperatorKind[PipelineOperatorKind["IDENTITY"] = 0] = "IDENTITY";
    PipelineOperatorKind[PipelineOperatorKind["CENTER"] = 1] = "CENTER";
    PipelineOperatorKind[PipelineOperatorKind["AUTOSCALE"] = 2] = "AUTOSCALE";
    PipelineOperatorKind[PipelineOperatorKind["PARETO_SCALE"] = 3] = "PARETO_SCALE";
    PipelineOperatorKind[PipelineOperatorKind["SNV"] = 4] = "SNV";
    PipelineOperatorKind[PipelineOperatorKind["MSC"] = 5] = "MSC";
    PipelineOperatorKind[PipelineOperatorKind["EMSC"] = 6] = "EMSC";
    PipelineOperatorKind[PipelineOperatorKind["DETREND_POLY"] = 7] = "DETREND_POLY";
    PipelineOperatorKind[PipelineOperatorKind["SAVGOL_SMOOTH"] = 8] = "SAVGOL_SMOOTH";
    PipelineOperatorKind[PipelineOperatorKind["SAVGOL_DERIVATIVE"] = 9] = "SAVGOL_DERIVATIVE";
    PipelineOperatorKind[PipelineOperatorKind["NORRIS_WILLIAMS"] = 10] = "NORRIS_WILLIAMS";
    PipelineOperatorKind[PipelineOperatorKind["ASLS_BASELINE"] = 11] = "ASLS_BASELINE";
    PipelineOperatorKind[PipelineOperatorKind["OSC"] = 12] = "OSC";
    PipelineOperatorKind[PipelineOperatorKind["EPO"] = 13] = "EPO";
    PipelineOperatorKind[PipelineOperatorKind["WAVELET_DENOISE"] = 14] = "WAVELET_DENOISE";
})(PipelineOperatorKind || (PipelineOperatorKind = {}));
function validateMatrix(X, label) {
    if (!Number.isSafeInteger(X.rows) || !Number.isSafeInteger(X.cols) ||
        X.rows < 1 || X.cols < 1 || X.data.length !== X.rows * X.cols ||
        !X.data.every(Number.isFinite)) {
        throw new Error(`${label} must be a nonempty finite row-major matrix`);
    }
}
function validateSteps(steps) {
    if (steps.length < 1 || steps.length > 256) {
        throw new Error("native pipeline requires 1–256 ordered steps");
    }
    for (const step of steps) {
        if (!Number.isInteger(step.kind) || step.kind < 0 || step.kind > 14 ||
            !Array.isArray(step.params) || step.params.length > 256 ||
            !step.params.every(Number.isFinite)) {
            throw new Error("invalid or unsupported native pipeline step");
        }
    }
}
function readPlan(ptr) {
    const m = getModule();
    const info = m._malloc(12); // int64_t feature width, int32_t operator count
    try {
        checkStatus(m.ccall("n4m_pipeline_get_info", "number", ["number", "number", "number"], [ptr, info, info + 8]));
        const features = Number(m.getValue(info, "i64"));
        const count = m.getValue(info + 8, "i32");
        if (!Number.isSafeInteger(features) || features < 1 || count < 1 || count > 256) {
            throw new Error("invalid native pipeline metadata");
        }
        const kindPtr = m._malloc(4);
        const countPtr = m._malloc(4);
        try {
            const steps = [];
            for (let index = 0; index < count; ++index) {
                checkStatus(m.ccall("n4m_pipeline_get_operator", "number", ["number", "number", "number", "number", "number", "number"], [ptr, index, kindPtr, 0, 0, countPtr]));
                const kind = m.getValue(kindPtr, "i32");
                const nParams = m.getValue(countPtr, "i32");
                if (kind < 0 || kind > 14 || nParams < 0 || nParams > 256) {
                    throw new Error("invalid native pipeline operator metadata");
                }
                const paramsPtr = m._malloc(Math.max(1, nParams) * 8);
                try {
                    if (nParams > 0) {
                        checkStatus(m.ccall("n4m_pipeline_get_operator", "number", ["number", "number", "number", "number", "number", "number"], [ptr, index, kindPtr, paramsPtr, nParams, countPtr]));
                    }
                    steps.push({ kind, params: Array.from(m.HEAPF64.subarray(paramsPtr / 8, paramsPtr / 8 + nParams)) });
                }
                finally {
                    m._free(paramsPtr);
                }
            }
            return { features, steps };
        }
        finally {
            m._free(kindPtr);
            m._free(countPtr);
        }
    }
    finally {
        m._free(info);
    }
}
function samePlan(a, b) {
    return a.length === b.length && a.every((step, index) => {
        const other = b[index];
        return other !== undefined && step.kind === other.kind &&
            step.params.length === other.params.length &&
            step.params.every((value, i) => Object.is(value, other.params[i]));
    });
}
/** Owning JS façade for an ordered, fitted C ABI preprocessing pipeline. */
export class NativePreprocessingPipeline {
    _ptr;
    _ctx;
    nFeatures;
    steps;
    constructor(ctx, ptr, features, steps) {
        this._ctx = ctx;
        this._ptr = ptr;
        this.nFeatures = features;
        this.steps = steps.map(step => Object.freeze({
            kind: step.kind, params: Object.freeze([...step.params]),
        }));
        Object.freeze(this.steps);
    }
    /** Fit an ordered recipe. OSC and EPO require Y; others may omit it. */
    static fit(ctx, steps, X, Y) {
        validateSteps(steps);
        validateMatrix(X, "X");
        if (Y !== undefined) {
            validateMatrix(Y, "Y");
            if (Y.rows !== X.rows)
                throw new Error("Y rows must match X rows");
        }
        const m = getModule();
        const outPtr = m._malloc(4);
        let ptr = 0;
        try {
            m.setValue(outPtr, 0, "i32");
            checkStatus(m.ccall("n4m_pipeline_create", "number", ["number"], [outPtr]));
            ptr = m.getValue(outPtr, "i32");
            for (const step of steps) {
                const params = m._malloc(Math.max(1, step.params.length) * 8);
                try {
                    m.HEAPF64.set(step.params, params / 8);
                    checkStatus(m.ccall("n4m_pipeline_add_operator", "number", ["number", "number", "number", "number"], [ptr, step.kind, step.params.length ? params : 0,
                        step.params.length]));
                }
                finally {
                    m._free(params);
                }
            }
            const xView = makeMatrixView(X.data, X.rows, X.cols);
            let yView;
            try {
                yView = Y === undefined ? undefined :
                    makeMatrixView(Y.data, Y.rows, Y.cols);
                checkStatus(m.ccall("n4m_pipeline_fit", "number", ["number", "number", "number", "number"], [ctx.handle, ptr, xView.viewPtr, yView?.viewPtr ?? 0]), ctx.handle);
            }
            finally {
                yView?.free();
                xView.free();
            }
            const plan = readPlan(ptr);
            if (!samePlan(plan.steps, steps)) {
                throw new Error("native fitted plan differs from requested recipe");
            }
            const result = new NativePreprocessingPipeline(ctx, ptr, plan.features, plan.steps);
            ptr = 0;
            return result;
        }
        finally {
            if (ptr !== 0)
                m.ccall("n4m_pipeline_destroy", null, ["number"], [ptr]);
            m._free(outPtr);
        }
    }
    /** Import fitted N4MP bytes; optionally attest against an external recipe. */
    static fromBytes(ctx, bytes, expectedSteps) {
        if (!(bytes instanceof Uint8Array) || bytes.length === 0 ||
            bytes.length > 64 * 1024 * 1024) {
            throw new Error("N4MP payload must be a Uint8Array of at most 64 MiB");
        }
        if (expectedSteps !== undefined)
            validateSteps(expectedSteps);
        const m = getModule();
        const data = m._malloc(bytes.length);
        const outPtr = m._malloc(4);
        let ptr = 0;
        try {
            m.HEAPU8.set(bytes, data);
            m.setValue(outPtr, 0, "i32");
            checkStatus(m.ccall("n4m_pipeline_import_from_buffer", "number", ["number", "number", "number", "number"], [ctx.handle, data, bytes.length, outPtr]), ctx.handle);
            ptr = m.getValue(outPtr, "i32");
            const plan = readPlan(ptr);
            if (expectedSteps !== undefined && !samePlan(plan.steps, expectedSteps)) {
                throw new Error("N4MP ordered plan does not match expected recipe");
            }
            const result = new NativePreprocessingPipeline(ctx, ptr, plan.features, plan.steps);
            ptr = 0;
            return result;
        }
        finally {
            if (ptr !== 0)
                m.ccall("n4m_pipeline_destroy", null, ["number"], [ptr]);
            m._free(data);
            m._free(outPtr);
        }
    }
    /** Transform new rows using only native fitted state. */
    transform(X) {
        if (this._ptr === 0)
            throw new Error("native pipeline has been destroyed");
        validateMatrix(X, "X");
        if (X.cols !== this.nFeatures)
            throw new Error("X feature width must match fit");
        const m = getModule();
        const input = makeMatrixView(X.data, X.rows, X.cols);
        try {
            const output = makeMatrixView(new Float64Array(X.data.length), X.rows, X.cols);
            try {
                checkStatus(m.ccall("n4m_pipeline_transform", "number", ["number", "number", "number", "number"], [this._ctx.handle, this._ptr, input.viewPtr, output.viewPtr]), this._ctx.handle);
                return { data: Float64Array.from(m.HEAPF64.subarray(output.dataPtr / 8, output.dataPtr / 8 + X.data.length)),
                    rows: X.rows, cols: X.cols };
            }
            finally {
                output.free();
            }
        }
        finally {
            input.free();
        }
    }
    /** Export the native fitted state and ordered recipe in N4MP format. */
    toBytes() {
        if (this._ptr === 0)
            throw new Error("native pipeline has been destroyed");
        const m = getModule();
        const sizePtr = m._malloc(4); // WASM32 size_t
        const writtenPtr = m._malloc(4);
        try {
            checkStatus(m.ccall("n4m_pipeline_export_size", "number", ["number", "number"], [this._ptr, sizePtr]));
            const size = m.getValue(sizePtr, "i32");
            const data = m._malloc(Math.max(1, size));
            try {
                checkStatus(m.ccall("n4m_pipeline_export_to_buffer", "number", ["number", "number", "number", "number"], [this._ptr, data, size, writtenPtr]));
                return Uint8Array.from(m.HEAPU8.subarray(data, data + m.getValue(writtenPtr, "i32")));
            }
            finally {
                m._free(data);
            }
        }
        finally {
            m._free(sizePtr);
            m._free(writtenPtr);
        }
    }
    destroy() {
        if (this._ptr === 0)
            return;
        getModule().ccall("n4m_pipeline_destroy", null, ["number"], [this._ptr]);
        this._ptr = 0;
    }
}
