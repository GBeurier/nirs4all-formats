// SPDX-License-Identifier: CECILL-2.1 OR AGPL-3.0-or-later
import { Config } from "./config.js";
import { Context } from "./context.js";
import { checkStatus, getModule, makeMatrixView } from "./ffi.js";
import { MethodResult } from "./methodResult.js";
const i = (name, fallback) => [name, "int", fallback];
const d = (name, fallback) => [name, "double", fallback];
const u = (name, fallback = 0) => [name, "uint64", fallback];
const b = (name, fallback = 1) => [name, "bool", fallback];
const a = (name) => [name, "doubleArray", null];
/** Exact scalar ordering of the public feature_selection.h C ABI. */
const SELECTORS = {
    spa_select: { params: [i("top_k", 10)] },
    cars_select: { plan: true, params: [i("n_iterations", 50), i("min_features", 5)] },
    interval_select: { plan: true, params: [i("interval_width", 10), i("step", 1)] },
    stability_select: { plan: true, params: [i("top_k", 10)] },
    uve_select: { plan: true, params: [i("noise_features", -1), u("noise_seed")] },
    random_frog_select: { plan: true, params: [i("n_iterations", 100), i("initial_size", 30),
            i("min_size", -1), i("max_size", -1), i("top_k", 10), u("seed")] },
    scars_select: { plan: true, params: [i("n_iterations", 50), i("min_features", 5),
            d("sample_fraction", 0.8), u("seed")] },
    ga_select: { plan: true, params: [i("n_generations", 50), i("population_size", 50),
            i("min_features", -1), i("max_features", -1), d("mutation_rate", 0.01), u("seed")] },
    pso_select: { plan: true, params: [i("n_swarm", 30), i("n_iterations", 50),
            d("w", 0.729), d("c1", 1.494), d("c2", 1.494), d("v_max", 4), u("seed")] },
    vissa_select: { plan: true, params: [i("n_iterations", 20), i("n_submodels", 100),
            d("ratio_kept", 0.1), d("threshold", 0.5), d("floor_probability", 0.01), u("seed")] },
    shaving_select: { plan: true, params: [i("n_steps", 10), i("min_features", 5),
            d("shave_fraction", 0.1)] },
    bve_select: { plan: true, params: [i("n_steps", 10), i("min_features", 5)] },
    t2_select: { plan: true, params: [a("alpha_thresholds"), i("min_selected", -1)] },
    wvc_select: { config: false, params: [i("top_k", 10), b("normalize")] },
    wvc_threshold_select: { config: false, params: [b("normalize"), d("threshold", 0),
            d("threshold_factor", 1), i("min_selected", 1)] },
    emcuve_select: { plan: true, params: [i("noise_features", -1), u("noise_seed"),
            i("n_ensembles", 5), d("vote_threshold", 0.5)] },
    randomization_select: { params: [i("n_permutations", 100), u("randomization_seed"),
            d("alpha", 0.05)] },
    bipls_select: { plan: true, params: [i("interval_width", 10), i("min_intervals", 1)] },
    sipls_select: { plan: true, params: [i("interval_width", 10), i("combination_size", 2)] },
    rep_select: { plan: true, params: [i("n_steps", 10), i("min_features", 5),
            i("remove_count", 1)] },
    ipw_select: { plan: true, params: [i("n_iterations", 10), i("top_k", 10),
            d("damping", 0.5), d("weight_floor", 1e-6)] },
    st_select: { plan: true, params: [a("thresholds"), i("min_selected", -1)] },
    iriv_select: { plan: true, params: [i("max_rounds", 20), u("seed")] },
    irf_select: { plan: true, params: [i("n_iterations", 100), i("window_size", 10),
            i("initial_intervals", 10), i("top_k", 5), u("seed")] },
    vip_spa_select: { params: [d("vip_threshold", 0.3), i("top_k", 10)] },
};
export const selectorMethods = Object.freeze(Object.keys(SELECTORS));
function resolvedParam(spec, params, X, nComponents) {
    const [name, kind, fallback] = spec;
    const raw = params[name] ?? (fallback === -1
        ? (name === "min_selected" || name === "min_size" || name === "min_features"
            ? nComponents : X.cols) : fallback);
    if (kind === "doubleArray") {
        if (!Array.isArray(raw) || raw.length === 0 ||
            !raw.every((value) => typeof value === "number" && Number.isFinite(value))) {
            throw new TypeError(`${name} must be a non-empty finite numeric array.`);
        }
        return raw;
    }
    if (kind === "bool") {
        if (typeof raw !== "boolean" && raw !== 0 && raw !== 1) {
            throw new TypeError(`${name} must be boolean.`);
        }
        return raw === true || raw === 1 ? 1 : 0;
    }
    if (kind === "uint64") {
        if (typeof raw === "string" && /^\d+$/.test(raw)) {
            const value = BigInt(raw);
            if (value <= 0xffffffffffffffffn)
                return value;
        }
        if (typeof raw === "number" && Number.isSafeInteger(raw) && raw >= 0)
            return BigInt(raw);
        throw new RangeError(`${name} must be a non-negative uint64 integer.`);
    }
    if (typeof raw !== "number" || !Number.isFinite(raw)) {
        throw new TypeError(`${name} must be finite numeric.`);
    }
    if (kind === "int" && (!Number.isInteger(raw) || raw < 1 || raw > 2147483647)) {
        throw new RangeError(`${name} must be a positive i32 integer.`);
    }
    return raw;
}
function allocInt64(values) {
    const m = getModule();
    const ptr = m._malloc(values.length * 8);
    const view = new DataView(m.HEAPU8.buffer, m.HEAPU8.byteOffset + ptr, values.length * 8);
    values.forEach((value, index) => view.setBigInt64(index * 8, BigInt(value), true));
    return ptr;
}
function createPlan(rows) {
    if (rows < 4)
        throw new RangeError("Selectors with validation plans require at least 4 training rows.");
    const m = getModule();
    const out = m._malloc(4);
    let plan = 0;
    try {
        checkStatus(m.ccall("n4m_validation_plan_create", "number", ["number"], [out]));
        plan = m.getValue(out, "i32") >>> 0;
        checkStatus(m.ccall("n4m_validation_plan_set_n_samples", "number", ["number", "i64"], [plan, BigInt(rows)]));
        const folds = Math.min(3, Math.floor(rows / 2));
        const foldSize = Math.floor(rows / folds);
        for (let fold = 0; fold < folds; fold += 1) {
            const start = fold * foldSize;
            const stop = fold === folds - 1 ? rows : start + foldSize;
            const train = Array.from({ length: rows }, (_, row) => row)
                .filter((row) => row < start || row >= stop);
            const test = Array.from({ length: stop - start }, (_, row) => start + row);
            const trainPtr = allocInt64(train);
            try {
                const testPtr = allocInt64(test);
                try {
                    checkStatus(m.ccall("n4m_validation_plan_add_fold", "number", ["number", "number", "i64", "number", "i64"], [plan, trainPtr, BigInt(train.length), testPtr, BigInt(test.length)]));
                }
                finally {
                    m._free(testPtr);
                }
            }
            finally {
                m._free(trainPtr);
            }
        }
        return plan;
    }
    catch (error) {
        if (plan)
            m.ccall("n4m_validation_plan_destroy", null, ["number"], [plan]);
        throw error;
    }
    finally {
        m._free(out);
    }
}
/** Run one of the 25 native selectors, preserving its ranked int64 indices. */
export function selectVariables(method, X, Y, nComponents = 2, methodParams = {}) {
    if (!Object.prototype.hasOwnProperty.call(SELECTORS, method)) {
        throw new RangeError(`Unsupported selector '${method}'.`);
    }
    const spec = SELECTORS[method];
    if (!methodParams || typeof methodParams !== "object" || Array.isArray(methodParams)) {
        throw new TypeError("Selector methodParams must be a mapping.");
    }
    const names = new Set(spec.params.map(([name]) => name));
    for (const key of Object.keys(methodParams)) {
        if (!names.has(key))
            throw new TypeError(`Unsupported ${method} parameter '${key}'.`);
    }
    if (X.rows !== Y.rows || Y.cols !== 1 || X.rows < 2 || X.cols < 1) {
        throw new RangeError("Selector requires matching X/Y rows and one target column.");
    }
    if (!Number.isInteger(nComponents) || nComponents < 1 ||
        nComponents > Math.min(X.cols, X.rows - 1)) {
        throw new RangeError("Selector nComponents exceed the available training rank.");
    }
    const values = spec.params.map((param) => resolvedParam(param, methodParams, X, nComponents));
    const m = getModule();
    const context = Context.create();
    let config = null;
    let plan = 0;
    const allocations = [];
    try {
        if (spec.config !== false) {
            config = Config.create();
            config.setNComponents(nComponents);
        }
        if (spec.plan)
            plan = createPlan(X.rows);
        const xv = makeMatrixView(X.data, X.rows, X.cols);
        try {
            const yv = makeMatrixView(Y.data, Y.rows, Y.cols);
            try {
                const out = m._malloc(4);
                allocations.push(out);
                m.setValue(out, 0, "i32");
                const argTypes = ["number"];
                const args = [context.handle];
                if (config) {
                    argTypes.push("number");
                    args.push(config.handle);
                }
                argTypes.push("number", "number");
                args.push(xv.viewPtr, yv.viewPtr);
                if (plan) {
                    argTypes.push("number");
                    args.push(plan);
                }
                if (!config) {
                    argTypes.push("number");
                    args.push(nComponents);
                }
                values.forEach((value) => {
                    if (Array.isArray(value)) {
                        const ptr = m._malloc(value.length * 8);
                        allocations.push(ptr);
                        m.HEAPF64.set(value, ptr >>> 3);
                        argTypes.push("number", "i64");
                        args.push(ptr, BigInt(value.length));
                    }
                    else if (typeof value === "bigint") {
                        argTypes.push("i64");
                        args.push(value);
                    }
                    else {
                        argTypes.push("number");
                        args.push(value);
                    }
                });
                argTypes.push("number");
                args.push(out);
                checkStatus(m.ccall(`n4m_feature_selection_${method}`, "number", argTypes, args), context.handle);
                const result = new MethodResult(m.getValue(out, "i32") >>> 0);
                try {
                    return result.vectorInt64("selected_indices");
                }
                finally {
                    result.destroy();
                }
            }
            finally {
                yv.free();
            }
        }
        finally {
            xv.free();
        }
    }
    finally {
        allocations.forEach((ptr) => m._free(ptr));
        if (plan)
            m.ccall("n4m_validation_plan_destroy", null, ["number"], [plan]);
        config?.destroy();
        context.destroy();
    }
}
/** Compatibility convenience wrapper for the native SPA selector. */
export function selectSpa(X, Y, topK, nComponents = 2) {
    if (!Number.isSafeInteger(topK) || topK < 1 || topK > X.cols) {
        throw new RangeError("SPA topK must be between 1 and X.cols.");
    }
    return selectVariables("spa_select", X, Y, nComponents, { top_k: topK });
}
