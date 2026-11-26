// SPDX-License-Identifier: CECILL-2.1 OR AGPL-3.0-or-later
import { checkStatus, getModule, makeMatrixView } from "./ffi.js";
const METHODS = {
    average_rank: { code: 0, variant: "average_rank" }, elo: { code: 1, variant: "elo" },
    bradley_terry: { code: 2, variant: "bradley_terry" },
    trueskill: { code: 3, variant: "trueskill_1v1" },
    trueskill_1v1: { code: 3, variant: "trueskill_1v1" },
    openskill: { code: 4, variant: "weng_lin_bradley_terry" },
    weng_lin_bradley_terry: { code: 4, variant: "weng_lin_bradley_terry" },
};
const DOUBLE_KEYS = ["tie_tolerance", "initial_mean", "k_factor", "elo_scale", "prior_sigma",
    "beta", "dynamics", "draw_margin", "regularization", "convergence_tolerance"];
const nullable = (value) => Number.isFinite(value) ? value : null;
/** Rank candidates in columns across comparable blocks in rows. The required
 * row-major presence mask contains 0 or 1; masked scores may be nonfinite.
 * Bayesian uncertainty is an approximate posterior SD, not a standard error. */
export function rank(scores, present, options = {}) {
    const allowed = new Set(["method", "higher_is_better", "max_iterations", ...DOUBLE_KEYS]);
    for (const key of Object.keys(options)) {
        if (!allowed.has(key))
            throw new Error(`unknown ranking option ${key}`);
    }
    const method = options.method ?? "average_rank";
    if (!Object.prototype.hasOwnProperty.call(METHODS, method))
        throw new Error(`unknown ranking method ${method}`);
    const size = scores.rows * scores.cols;
    if (!Number.isSafeInteger(scores.rows) || !Number.isSafeInteger(scores.cols) ||
        scores.rows < 1 || scores.cols < 1 || !Number.isSafeInteger(size) ||
        size > 0x7fffffff || scores.cols > Math.floor(0x7fffffff / 80) ||
        scores.data.length !== size || !(present instanceof Uint8Array) || present.length !== size ||
        !present.every(value => value === 0 || value === 1)) {
        throw new Error("scores and 0/1 presence mask must have matching nonempty dimensions");
    }
    if (options.higher_is_better !== undefined && typeof options.higher_is_better !== "boolean") {
        throw new Error("higher_is_better must be boolean");
    }
    if (options.max_iterations !== undefined && !Number.isSafeInteger(options.max_iterations)) {
        throw new Error("max_iterations must be a safe integer");
    }
    for (const key of DOUBLE_KEYS) {
        if (options[key] !== undefined && !Number.isFinite(options[key]))
            throw new Error(`${key} must be finite`);
    }
    const m = getModule();
    const view = makeMatrixView(scores.data, scores.rows, scores.cols);
    const allocations = [];
    const alloc = (bytes) => {
        const ptr = m._malloc(bytes);
        if (!ptr)
            throw new Error("ranking allocation failed");
        allocations.push(ptr);
        return ptr;
    };
    try {
        const opt = alloc(104), mask = alloc(size), output = alloc(scores.cols * 10 * 8), summary = alloc(72);
        checkStatus(m.ccall("n4m_ranking_options_init_v1", "number", ["number"], [opt]));
        m.HEAPU8.set(present, mask);
        const data = new DataView(m.HEAPU8.buffer);
        data.setInt32(opt + 8, METHODS[method].code, true);
        if (options.higher_is_better !== undefined)
            data.setInt32(opt + 12, options.higher_is_better ? 1 : 0, true);
        DOUBLE_KEYS.forEach((key, i) => {
            const value = options[key];
            if (value !== undefined)
                data.setFloat64(opt + 16 + 8 * i, value, true);
        });
        if (options.max_iterations !== undefined)
            data.setBigInt64(opt + 96, BigInt(options.max_iterations), true);
        const resolved = { higher_is_better: data.getInt32(opt + 12, true) !== 0,
            ...Object.fromEntries(DOUBLE_KEYS.map((key, i) => [key, data.getFloat64(opt + 16 + 8 * i, true)])),
            max_iterations: Number(data.getBigInt64(opt + 96, true)) };
        checkStatus(m.ccall("n4m_ranking_compute_v1", "number", ["number", "number", "number", "number", "i64", "number", "i64"], [view.viewPtr, mask, opt, output, BigInt(scores.cols * 10), summary, 9n]));
        const values = m.HEAPF64;
        const candidates = Array.from({ length: scores.cols }, (_, i) => {
            const p = output / 8 + i * 10;
            return { index: i, score: nullable(values[p]), uncertainty: nullable(values[p + 1]),
                average_rank: nullable(values[p + 2]), observations: values[p + 3], matches: values[p + 4],
                component: values[p + 5], wins: values[p + 6], ties: values[p + 7], losses: values[p + 8],
                rank: nullable(values[p + 9]) };
        });
        const s = summary / 8;
        const warnings = [
            "Mean ranks use all observed blocks; Friedman uses complete blocks only.",
            "Friedman p-value uses a chi-square approximation and assumes independent blocks.",
        ];
        if (METHODS[method].code >= 3)
            warnings.push("Pairwise replay within each block is a composite approximation; posterior SD does not account for dependence between comparisons or repeated folds.");
        if (METHODS[method].code === 0 && values[s + 1] < values[s])
            warnings.push("Incomplete design: mean ranks are descriptive and average_rank global ranks are unavailable; use bradley_terry or trueskill for incomplete comparisons.");
        if (values[s + 3] > 1)
            warnings.push("Disconnected comparison graph: global ranks are unavailable.");
        if (values[s + 6] === 0)
            warnings.push("Bradley-Terry did not converge; global ranks are unavailable.");
        return { schema_version: "n4m.ranking/1", method, variant: METHODS[method].variant, options: resolved,
            score_direction: method === "average_rank" ? "lower_is_better" : "higher_is_better",
            uncertainty_kind: METHODS[method].code >= 3 ? "approximate_posterior_sd" : null,
            candidates, coverage: { blocks: values[s], complete_blocks: values[s + 1],
                paired_comparisons: values[s + 2], connected_components: values[s + 3] },
            inference: { friedman_statistic: nullable(values[s + 4]), degrees_of_freedom: nullable(values[s + 5]),
                friedman_p_value: nullable(values[s + 8]),
                p_value_method: "chi_square_approximation",
                reason: "chi_square_approximation_on_complete_blocks; host_must_assess_independence" },
            warnings,
            convergence: { converged: values[s + 6] !== 0, iterations: values[s + 7] } };
    }
    finally {
        allocations.forEach(ptr => m._free(ptr));
        view.free();
    }
}
