// SPDX-License-Identifier: CECILL-2.1 OR AGPL-3.0-or-later
/** Matrix-only adapters. Numerical fitting and normalization stay in the C ABI. */
import { checkStatus, getModule, makeMatrixView } from "./ffi.js";
const nullable = (v) => Number.isFinite(v) ? v : null;
class Arena {
    m = getModule();
    ptrs = [];
    views = [];
    alloc(bytes) {
        if (!Number.isSafeInteger(bytes) || bytes < 1 || bytes > 0x7fffffff)
            throw new RangeError("analysis allocation exceeds WASM limit");
        const p = this.m._malloc(bytes);
        if (!p)
            throw new Error("analysis allocation failed");
        this.ptrs.push(p);
        return p;
    }
    matrix(x) {
        if (!Number.isSafeInteger(x.rows) || !Number.isSafeInteger(x.cols) || x.rows < 1 || x.cols < 1 ||
            x.data.length !== x.rows * x.cols)
            throw new Error("invalid nonempty matrix");
        const v = makeMatrixView(x.data, x.rows, x.cols);
        this.views.push(v);
        return v.viewPtr;
    }
    free() { for (const v of this.views)
        v.free(); for (const p of this.ptrs)
        this.m._free(p); }
    read(p, n) {
        return Array.from(this.m.HEAPF64.subarray(p / 8, p / 8 + n), nullable);
    }
    options(init, size, fields, values) {
        const p = this.alloc(size);
        checkStatus(this.m.ccall(init, "number", ["number"], [p]));
        for (const [key, value] of Object.entries(values)) {
            if (value === undefined)
                continue;
            const field = fields.find(f => f[0] === key);
            if (!field)
                throw new Error(`unknown component analysis option ${key}`);
            const [, offset, type] = field;
            const v = key === "reml" && typeof value === "boolean" ? Number(value) : value;
            if (typeof v !== "number" || !Number.isFinite(v))
                throw new Error(`${key} must be finite`);
            const d = new DataView(this.m.HEAPU8.buffer);
            if (type === "f64")
                d.setFloat64(p + offset, v, true);
            else {
                if (!Number.isSafeInteger(v))
                    throw new Error(`${key} must be a safe integer`);
                if (type === "i32") {
                    if (v < -2147483648 || v > 2147483647)
                        throw new RangeError(`${key} exceeds int32 range`);
                    d.setInt32(p + offset, v, true);
                }
                else if (type === "u64") {
                    if (v < 0)
                        throw new RangeError(`${key} must be nonnegative`);
                    d.setBigUint64(p + offset, BigInt(v), true);
                }
                else
                    d.setBigInt64(p + offset, BigInt(v), true);
            }
        }
        return p;
    }
    fields(p, fields) {
        const d = new DataView(this.m.HEAPU8.buffer);
        return Object.fromEntries(fields.map(([key, offset, type]) => [key,
            type === "f64" ? nullable(d.getFloat64(p + offset, true)) : type === "i32" ? d.getInt32(p + offset, true) :
                Number(type === "u64" ? d.getBigUint64(p + offset, true) : d.getBigInt64(p + offset, true))]));
    }
}
const MIXED_OPTIONS = [["reml", 8, "i32"], ["max_evaluations", 16, "i64"],
    ["tolerance", 24, "f64"], ["rank_tolerance", 32, "f64"], ["boundary_tolerance", 40, "f64"]];
const MIXED_SUMMARY = [["fit_state", 8, "i32"], ["reason", 12, "i32"], ["converged", 16, "i32"],
    ["singular", 20, "i32"], ["boundary", 24, "i32"], ["optimizer_status", 28, "i32"],
    ["observations", 32, "i64"], ["fixed_effects", 40, "i64"], ["group_terms", 48, "i64"],
    ["design_rank", 56, "i64"], ["total_group_levels", 64, "i64"], ["evaluations", 72, "i64"],
    ["objective", 80, "f64"], ["log_likelihood", 88, "f64"], ["residual_df", 96, "f64"], ["condition_number", 104, "f64"]];
/** X includes any intercept explicitly. Group IDs preserve int64 with BigInt64Array. */
export function mixedEffects(X, y, groups, options = {}) {
    const a = new Arena();
    try {
        const opt = a.options("n4m_mixed_effects_options_init_v1", 48, MIXED_OPTIONS, { ...options });
        const base = { schema_version: "n4m.mixed_effects/1", options: a.fields(opt, MIXED_OPTIONS),
            uncertainty_kind: "conditional_plugin_standard_error",
            warnings: ["Gaussian random-intercept associations are not causal effects.", "Plug-in SE excludes variance-parameter uncertainty."] };
        if (!a.m.ccall("n4m_mixed_effects_backend_available_v1", "number", [], [])) {
            return { ...base, state: "unavailable", reason: "mixed_effects_backend_not_compiled", beta: null,
                covariance: null, standard_errors: null, variances: null, diagnostics: {} };
        }
        const xv = a.matrix(X), yv = a.matrix(y);
        if (!(groups.data instanceof BigInt64Array) || !Number.isSafeInteger(groups.rows) || !Number.isSafeInteger(groups.cols) ||
            groups.rows < 1 || groups.cols < 1 || groups.data.length !== groups.rows * groups.cols)
            throw new Error("invalid int64 group matrix");
        const gp = a.alloc(groups.data.byteLength), gv = a.alloc(48);
        a.m.HEAPU8.set(new Uint8Array(groups.data.buffer, groups.data.byteOffset, groups.data.byteLength), gp);
        checkStatus(a.m.ccall("n4m_matrix_view_init_rowmajor", "number", ["number", "number", "i64", "i64", "number"], [gv, gp, BigInt(groups.rows), BigInt(groups.cols), 4]));
        const p = X.cols, k = groups.cols, beta = a.alloc(p * 8), cov = a.alloc(p * p * 8), se = a.alloc(p * 8), variances = a.alloc((k + 1) * 8), summary = a.alloc(112);
        new DataView(a.m.HEAPU8.buffer).setBigUint64(summary, 112n, true);
        checkStatus(a.m.ccall("n4m_mixed_effects_fit_v1", "number", ["number", "number", "number", "number", "number", "i64", "number", "i64", "number", "i64", "number", "i64", "number"], [xv, yv, gv, opt, beta, BigInt(p), cov, BigInt(p * p), se, BigInt(p), variances, BigInt(k + 1), summary]));
        const diagnostics = a.fields(summary, MIXED_SUMMARY);
        const state = diagnostics.fit_state, reason = diagnostics.reason;
        diagnostics.fit_state = ["fitted", "fitted_boundary", "rank_deficient", "unidentifiable", "nonconverged", "numerical_refusal"][state];
        return { ...base, state: state <= 1 ? "ready" : "unavailable", reason: [null, "fixed_rank", "no_residual_df", "constant_response",
                "insufficient_group_levels", "confounded_variances", "zero_residual", "optimizer_budget", "optimizer_roundoff",
                "optimizer_failure", "nonfinite_result", "engine_rejected"][reason],
            beta: a.read(beta, p), standard_errors: a.read(se, p), covariance: Array.from({ length: p }, (_, i) => a.read(cov + i * p * 8, p)),
            variances: a.read(variances, k + 1), diagnostics };
    }
    finally {
        a.free();
    }
}
const FANOVA_OPTIONS = [["num_trees", 8, "i32"], ["max_depth", 12, "i32"], ["min_samples_leaf", 16, "i32"],
    ["max_features", 20, "i32"], ["seed", 24, "u64"], ["max_order", 32, "i32"], ["max_marginal_evaluations", 40, "i64"]];
const FANOVA_SUMMARY = [["fit_state", 8, "i32"], ["observations", 16, "i64"], ["features", 24, "i64"],
    ["trees", 32, "i64"], ["informative_trees", 40, "i64"], ["marginal_evaluations", 48, "i64"],
    ["total_variance_mean", 56, "f64"], ["remaining_fraction", 64, "f64"], ["train_rmse", 72, "f64"],
    ["validation_rmse", 80, "f64"], ["validation_r2", 88, "f64"]];
export function fanova(X, y, featureTypes, bounds, options = {}) {
    const a = new Arena();
    try {
        const { n_trees = 32, X_valid, y_valid, ...rest } = options;
        if ((X_valid === undefined) !== (y_valid === undefined))
            throw new Error("X_valid and y_valid are required together");
        const opt = a.options("n4m_fanova_options_init_v1", 48, FANOVA_OPTIONS, { num_trees: n_trees, ...rest });
        const resolved = a.fields(opt, FANOVA_OPTIONS);
        const base = { schema_version: "n4m.fanova/1", options: resolved, domain_measure: "uniform_product",
            aggregation: "mean_per_tree_normalized_components", uncertainty_kind: "tree_dispersion_not_confidence",
            warnings: ["Surrogate attribution under independent uniform domains is not causal.", "Tree dispersion is not a confidence interval."] };
        if (!a.m.ccall("n4m_fanova_backend_available_v1", "number", [], []))
            return { ...base, state: "unavailable",
                reason: "fanova_backend_not_compiled", feature_importance: [], interactions: [], diagnostics: {} };
        const xv = a.matrix(X), yv = a.matrix(y), bv = a.matrix(bounds), p = X.cols;
        if (!(featureTypes instanceof Uint32Array) || featureTypes.length !== p)
            throw new Error("one feature type is required per column");
        const types = a.alloc(p * 4);
        a.m.HEAPU8.set(new Uint8Array(featureTypes.buffer, featureTypes.byteOffset, featureTypes.byteLength), types);
        const vx = X_valid ? a.matrix(X_valid) : 0, vy = y_valid ? a.matrix(y_valid) : 0;
        const mains = a.alloc(p * 16), pairs = a.alloc(p * p * 16), summary = a.alloc(96);
        new DataView(a.m.HEAPU8.buffer).setBigUint64(summary, 96n, true);
        checkStatus(a.m.ccall("n4m_fanova_fit_v1", "number", ["number", "number", "number", "number", "number", "number", "number", "number", "i64", "number", "i64", "number"], [xv, yv, bv, types, vx, vy, opt, mains, BigInt(p * 2), pairs, BigInt(p * p * 2), summary]));
        const diagnostics = a.fields(summary, FANOVA_SUMMARY), state = diagnostics.fit_state;
        const feature_importance = Array.from({ length: p }, (_, i) => ({ index: i, fraction: a.read(mains + i * 16, 1)[0], tree_sd: a.read(mains + i * 16 + 8, 1)[0] }));
        const interactions = [];
        if (resolved.max_order === 2)
            for (let i = 0; i < p; i++)
                for (let j = i + 1; j < p; j++) {
                    const v = a.read(pairs + (i * p + j) * 16, 2);
                    interactions.push({ indices: [i, j], fraction: v[0], tree_sd: v[1] });
                }
        return { ...base, state: state === 0 ? "ready" : "unavailable", reason: [null, "zero_surrogate_variance", "zero_surrogate_variance", "numerical_refusal"][state],
            feature_importance, interactions, diagnostics };
    }
    finally {
        a.free();
    }
}
export function normalizeLoss(values, baselines) {
    if (values.length !== baselines.length || values.length < 1)
        throw new Error("values and baselines must have the same nonempty length");
    const a = new Arena();
    try {
        const x = a.alloc(values.byteLength), b = a.alloc(baselines.byteLength), out = a.alloc(values.byteLength);
        a.m.HEAPF64.set(values, x / 8);
        a.m.HEAPF64.set(baselines, b / 8);
        checkStatus(a.m.ccall("n4m_component_normalize_loss_v1", "number", ["number", "number", "i64", "number"], [x, b, BigInt(values.length), out]));
        return a.m.HEAPF64.slice(out / 8, out / 8 + values.length);
    }
    finally {
        a.free();
    }
}
