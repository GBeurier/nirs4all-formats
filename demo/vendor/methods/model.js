// SPDX-License-Identifier: CECILL-2.1 OR AGPL-3.0-or-later
//
// PLS regression model wrapper. Internally uses the `n4m_wasm_pls_fit`
// helper which takes raw double pointers and works around an Emscripten
// 5.0.7 codegen issue for matrix-view-pointer args (see README "Status").
import { checkStatus, getModule } from "./ffi.js";
function _malloc_f64(M, len) {
    const ptr = M._malloc(len * 8);
    return { ptr, len };
}
function _copy_in(M, buf, ptr) {
    if (buf.length === 0)
        return;
    M.HEAPF64.set(buf, ptr >>> 3);
}
function _read_out(M, ptr, len) {
    return new Float64Array(M.HEAPU8.buffer, ptr, len).slice();
}
/** Fit a SIMPLS PLS-regression model on (X, Y).
 *
 * @param X row-major (n × p) input matrix.
 * @param Y row-major (n × q) target matrix.
 * @param n_components number of latent components.
 */
export function fitPls(X, Y, n_components) {
    if (X.rows !== Y.rows) {
        throw new Error(`X.rows (${X.rows}) must equal Y.rows (${Y.rows})`);
    }
    const M = getModule();
    const n = X.rows, p = X.cols, q = Y.cols;
    const xBuf = _malloc_f64(M, n * p);
    const yBuf = _malloc_f64(M, n * q);
    const coefsBuf = _malloc_f64(M, p * q);
    const xmBuf = _malloc_f64(M, p);
    const ymBuf = _malloc_f64(M, q);
    try {
        _copy_in(M, X.data, xBuf.ptr);
        _copy_in(M, Y.data, yBuf.ptr);
        // Uses the public ABI helper (1.13+): raw double pointers
        // + ints, no matrix-view structs in the JS↔WASM boundary.
        const status = M.ccall("n4m_estimators_pls_fit", "number", ["number", "number", "number", "number", "number",
            "number", "number", "number", "number", "number"], [xBuf.ptr, yBuf.ptr, n, p, q, n_components,
            coefsBuf.ptr, xmBuf.ptr, ymBuf.ptr, 0]);
        checkStatus(status);
        return {
            coefficients: _read_out(M, coefsBuf.ptr, p * q),
            xMean: _read_out(M, xmBuf.ptr, p),
            yMean: _read_out(M, ymBuf.ptr, q),
            n_features: p,
            n_targets: q,
        };
    }
    finally {
        M._free(xBuf.ptr);
        M._free(yBuf.ptr);
        M._free(coefsBuf.ptr);
        M._free(xmBuf.ptr);
        M._free(ymBuf.ptr);
    }
}
/** Predict from a fitted PlsModel for new X (row-major n_new × p). */
export function predictPls(model, X_new) {
    if (X_new.cols !== model.n_features) {
        throw new Error(`X_new.cols (${X_new.cols}) must equal n_features (` +
            `${model.n_features})`);
    }
    const M = getModule();
    const n_new = X_new.rows, p = model.n_features, q = model.n_targets;
    const xBuf = _malloc_f64(M, n_new * p);
    const coefsBuf = _malloc_f64(M, p * q);
    const xmBuf = _malloc_f64(M, p);
    const ymBuf = _malloc_f64(M, q);
    const predsBuf = _malloc_f64(M, n_new * q);
    try {
        _copy_in(M, X_new.data, xBuf.ptr);
        _copy_in(M, model.coefficients, coefsBuf.ptr);
        _copy_in(M, model.xMean, xmBuf.ptr);
        _copy_in(M, model.yMean, ymBuf.ptr);
        const status = M.ccall("n4m_wasm_pls_predict_from_coeffs", "number", ["number", "number", "number", "number",
            "number", "number", "number", "number"], [xBuf.ptr, n_new, p, q,
            coefsBuf.ptr, xmBuf.ptr, ymBuf.ptr, predsBuf.ptr]);
        checkStatus(status);
        return {
            data: _read_out(M, predsBuf.ptr, n_new * q),
            rows: n_new,
            cols: q,
        };
    }
    finally {
        M._free(xBuf.ptr);
        M._free(coefsBuf.ptr);
        M._free(xmBuf.ptr);
        M._free(ymBuf.ptr);
        M._free(predsBuf.ptr);
    }
}
/** Fit any coefficient-based libn4m model by its catalog `type` token.
 *
 * Tier A (PLS / PLSRegression / PCR / PLSCanonical / PLSSVD / PLSDA) routes
 * through the algorithm-enum model API; Tier B (Ridge, RidgePLS, CPPLS, ...)
 * through the matching standalone fit. The `params` vector is the documented
 * positional contract per model (see the nirs4all-web browser catalog). Unknown or
 * non-coefficient tokens throw (the C side returns N4M_ERR_NOT_IMPLEMENTED).
 *
 * @param model catalog `type` token, e.g. `'Ridge'`.
 * @param X row-major (n × p) input matrix.
 * @param Y row-major (n × q) target matrix.
 * @param n_components number of latent components (used by the PLS family).
 * @param params positional hyper-parameter vector for the model. For
 *   GroupSparsePLS this is `[group_lambda, ...group_assignment]`, with exactly
 *   one non-negative integer group ID per input feature; no implicit grouping
 *   is applied.
 */
export function fitModel(model, X, Y, n_components, params = []) {
    if (X.rows !== Y.rows) {
        throw new Error(`X.rows (${X.rows}) must equal Y.rows (${Y.rows})`);
    }
    const M = getModule();
    const n = X.rows, p = X.cols, q = Y.cols;
    const xBuf = _malloc_f64(M, n * p);
    const yBuf = _malloc_f64(M, n * q);
    const coefsBuf = _malloc_f64(M, p * q);
    const xmBuf = _malloc_f64(M, p);
    const ymBuf = _malloc_f64(M, q);
    const interBuf = _malloc_f64(M, q);
    const hasInterBuf = M._malloc(4); // int32 flag: 1 iff a genuine intercept
    const pPtr = params.length > 0 ? _malloc_f64(M, params.length) : { ptr: 0, len: 0 };
    try {
        _copy_in(M, X.data, xBuf.ptr);
        _copy_in(M, Y.data, yBuf.ptr);
        if (params.length > 0)
            _copy_in(M, Float64Array.from(params), pPtr.ptr);
        const status = M.ccall("n4m_wasm_model_fit", "number", ["string", "number", "number", "number", "number",
            "number", "number", "number", "number",
            "number", "number", "number", "number", "number", "number"], [model, pPtr.ptr, params.length, xBuf.ptr, yBuf.ptr,
            n, p, q, n_components,
            coefsBuf.ptr, xmBuf.ptr, ymBuf.ptr, interBuf.ptr,
            hasInterBuf, 0]);
        checkStatus(status);
        // Only models with a genuine affine intercept (Ridge and MBPLS) report
        // has_intercept=1; the PLS/PCR family and the PLS-based Tier-B fits
        // predict via the centred form and carry no intercept (kept null so a
        // caller never adds a misleading zero/y_mean term to x.B).
        const hasIntercept = M.HEAP32[hasInterBuf >> 2] === 1;
        return {
            coefficients: _read_out(M, coefsBuf.ptr, p * q),
            xMean: _read_out(M, xmBuf.ptr, p),
            yMean: _read_out(M, ymBuf.ptr, q),
            intercept: hasIntercept ? _read_out(M, interBuf.ptr, q) : null,
            n_features: p,
            n_targets: q,
        };
    }
    finally {
        M._free(xBuf.ptr);
        M._free(yBuf.ptr);
        M._free(coefsBuf.ptr);
        M._free(xmBuf.ptr);
        M._free(ymBuf.ptr);
        M._free(interBuf.ptr);
        M._free(hasInterBuf);
        if (pPtr.ptr !== 0)
            M._free(pPtr.ptr);
    }
}
/** Predict from a fitted {@link FittedModel} for new X (row-major n_new × p). */
export function predictModel(model, X_new) {
    if (X_new.cols !== model.n_features) {
        throw new Error(`X_new.cols (${X_new.cols}) must equal n_features (` +
            `${model.n_features})`);
    }
    const M = getModule();
    const n_new = X_new.rows, p = model.n_features, q = model.n_targets;
    const xBuf = _malloc_f64(M, n_new * p);
    const coefsBuf = _malloc_f64(M, p * q);
    const xmBuf = _malloc_f64(M, p);
    const ymBuf = _malloc_f64(M, q);
    const predsBuf = _malloc_f64(M, n_new * q);
    // AOM-PLS (and any future affine model) carries input-space coefficients +
    // a genuine intercept and zero means — it predicts on RAW X via the
    // explicit-intercept form  pred = intercept + x.B. Every centred model
    // (PLS family + centred Tier-B fits) carries intercept = null
    // and predicts via  pred = y_mean + (x - x_mean).B. The C helper picks the
    // form from whether the intercept pointer is non-NULL.
    const useIntercept = model.intercept !== null;
    const interBuf = useIntercept ? _malloc_f64(M, q) : { ptr: 0, len: 0 };
    try {
        _copy_in(M, X_new.data, xBuf.ptr);
        _copy_in(M, model.coefficients, coefsBuf.ptr);
        _copy_in(M, model.xMean, xmBuf.ptr);
        _copy_in(M, model.yMean, ymBuf.ptr);
        if (useIntercept)
            _copy_in(M, model.intercept, interBuf.ptr);
        const status = M.ccall("n4m_wasm_model_predict_from_coeffs", "number", ["number", "number", "number", "number",
            "number", "number", "number", "number", "number"], [coefsBuf.ptr, xmBuf.ptr, ymBuf.ptr, interBuf.ptr,
            xBuf.ptr, n_new, p, q, predsBuf.ptr]);
        checkStatus(status);
        return {
            data: _read_out(M, predsBuf.ptr, n_new * q),
            rows: n_new,
            cols: q,
        };
    }
    finally {
        M._free(xBuf.ptr);
        M._free(coefsBuf.ptr);
        M._free(xmBuf.ptr);
        M._free(ymBuf.ptr);
        M._free(predsBuf.ptr);
        if (interBuf.ptr !== 0)
            M._free(interBuf.ptr);
    }
}
/** Fit one PLS or Ridge head with the configurable native AOM chain sweep.
 * The exact same descriptor can be evaluated by an external HPO implementation,
 * making search-space parity explicit and auditable. */
export function fitAomChain(X, Y, descriptor, options) {
    if (X.rows !== Y.rows)
        throw new Error(`X.rows (${X.rows}) must equal Y.rows (${Y.rows})`);
    const { chainOffsets, operatorKinds, parameterOffsets, parameters } = descriptor;
    if (chainOffsets.length < 2 || operatorKinds.length < 1 || parameterOffsets.length !== operatorKinds.length + 1) {
        throw new Error("Invalid AOM chain descriptor");
    }
    const ridgeLambdas = options.ridgeLambdas ?? [];
    const plsComponents = options.plsComponents ?? [];
    if (options.head === "ridge" && ridgeLambdas.length < 1)
        throw new Error("Ridge AOM requires ridgeLambdas");
    if (options.head === "pls" && plsComponents.length < 1)
        throw new Error("PLS AOM requires plsComponents");
    const M = getModule();
    const n = X.rows, p = X.cols, q = Y.cols;
    const xBuf = _malloc_f64(M, n * p), yBuf = _malloc_f64(M, n * q);
    const chainBuf = M._malloc(chainOffsets.length * 4);
    const kindsBuf = M._malloc(operatorKinds.length * 4);
    const offsetsBuf = M._malloc(parameterOffsets.length * 4);
    const paramsBuf = parameters.length > 0 ? _malloc_f64(M, parameters.length).ptr : 0;
    const ridgeBuf = ridgeLambdas.length > 0 ? _malloc_f64(M, ridgeLambdas.length).ptr : 0;
    const plsBuf = plsComponents.length > 0 ? M._malloc(plsComponents.length * 4) : 0;
    const coefsBuf = _malloc_f64(M, p * q), interBuf = _malloc_f64(M, q);
    const selectedBuf = M._malloc(4), parameterBuf = _malloc_f64(M, 1), scoreBuf = _malloc_f64(M, 1);
    try {
        _copy_in(M, X.data, xBuf.ptr);
        _copy_in(M, Y.data, yBuf.ptr);
        M.HEAP32.set(Int32Array.from(chainOffsets), chainBuf >> 2);
        M.HEAP32.set(Int32Array.from(operatorKinds), kindsBuf >> 2);
        M.HEAP32.set(Int32Array.from(parameterOffsets), offsetsBuf >> 2);
        if (paramsBuf !== 0)
            M.HEAPF64.set(Float64Array.from(parameters), paramsBuf >>> 3);
        if (ridgeBuf !== 0)
            M.HEAPF64.set(Float64Array.from(ridgeLambdas), ridgeBuf >>> 3);
        if (plsBuf !== 0)
            M.HEAP32.set(Int32Array.from(plsComponents), plsBuf >> 2);
        const status = M.ccall("n4m_wasm_aom_chain_fit", "number", Array(25).fill("number"), [xBuf.ptr, yBuf.ptr, n, p, q, options.nFolds ?? 5,
            chainBuf, chainOffsets.length, kindsBuf, operatorKinds.length,
            offsetsBuf, parameterOffsets.length, paramsBuf, parameters.length,
            ridgeBuf, ridgeLambdas.length, plsBuf, plsComponents.length,
            options.head === "ridge" ? 1 : 2,
            options.momentPolicy === "materialized" ? 1 : options.momentPolicy === "moments" ? 2 : 0,
            coefsBuf.ptr, interBuf.ptr, selectedBuf, parameterBuf.ptr, scoreBuf.ptr]);
        checkStatus(status);
        return {
            coefficients: _read_out(M, coefsBuf.ptr, p * q),
            xMean: new Float64Array(p), yMean: new Float64Array(q),
            intercept: _read_out(M, interBuf.ptr, q),
            n_features: p, n_targets: q,
            selectedChain: M.HEAP32[selectedBuf >> 2] ?? -1,
            selectedParameter: _read_out(M, parameterBuf.ptr, 1)[0] ?? NaN,
            score: _read_out(M, scoreBuf.ptr, 1)[0] ?? NaN,
        };
    }
    finally {
        [xBuf.ptr, yBuf.ptr, chainBuf, kindsBuf, offsetsBuf, paramsBuf, ridgeBuf, plsBuf,
            coefsBuf.ptr, interBuf.ptr, selectedBuf, parameterBuf.ptr, scoreBuf.ptr]
            .filter((ptr) => ptr !== 0).forEach((ptr) => M._free(ptr));
    }
}
/** Fit AOM-PLS (operator-adaptive PLS) on (X, Y).
 *
 * Screens a bank of strict-linear preprocessing operators by internal k-fold CV
 * and fits SIMPLS on the winner, returning INPUT-SPACE coefficients so the model
 * predicts on RAW X — it is therefore used WITHOUT preceding preprocessing steps
 * (the screen does the preprocessing internally). Numerics are 100% libn4m
 * (`n4m_model_selection_aom_pls_select`); this only builds the bank + validation plan.
 *
 * @param X row-major (n × p) input matrix.
 * @param Y row-major (n × q) target matrix.
 * @param maxComponents max latent components for the internal SIMPLS fits.
 * @param nFolds internal-CV fold count for the operator screen.
 * @param seed reserved (the contiguous-fold partition is deterministic).
 * @param operatorKinds optional `n4m_operator_kind_t` bank override; when
 *   omitted a default strict bank (identity / detrend / SG smooth / SG
 *   derivative / finite-difference) is screened.
 */
export function fitAom(X, Y, maxComponents, nFolds = 5, seed = 0, operatorKinds = []) {
    if (X.rows !== Y.rows) {
        throw new Error(`X.rows (${X.rows}) must equal Y.rows (${Y.rows})`);
    }
    const M = getModule();
    const n = X.rows, p = X.cols, q = Y.cols;
    const xBuf = _malloc_f64(M, n * p);
    const yBuf = _malloc_f64(M, n * q);
    const coefsBuf = _malloc_f64(M, p * q);
    const interBuf = _malloc_f64(M, q);
    const selBuf = M._malloc(4); // int32 selected operator index
    const scoreBuf = _malloc_f64(M, 1);
    const opsPtr = operatorKinds.length > 0
        ? M._malloc(operatorKinds.length * 4)
        : 0;
    try {
        _copy_in(M, X.data, xBuf.ptr);
        _copy_in(M, Y.data, yBuf.ptr);
        if (opsPtr !== 0)
            M.HEAP32.set(Int32Array.from(operatorKinds), opsPtr >> 2);
        const status = M.ccall("n4m_wasm_aom_fit", "number", ["number", "number", "number", "number", "number",
            "number", "number", "number", "number", "number",
            "number", "number", "number", "number"], [xBuf.ptr, yBuf.ptr, n, p, q,
            maxComponents, nFolds, seed, opsPtr, operatorKinds.length,
            coefsBuf.ptr, interBuf.ptr, selBuf, scoreBuf.ptr]);
        checkStatus(status);
        return {
            coefficients: _read_out(M, coefsBuf.ptr, p * q),
            // zero means — AOM predicts on RAW X via the affine intercept form.
            xMean: new Float64Array(p),
            yMean: new Float64Array(q),
            intercept: _read_out(M, interBuf.ptr, q),
            n_features: p,
            n_targets: q,
            selectedOperator: M.HEAP32[selBuf >> 2] ?? -1,
            score: _read_out(M, scoreBuf.ptr, 1)[0] ?? NaN,
        };
    }
    finally {
        M._free(xBuf.ptr);
        M._free(yBuf.ptr);
        M._free(coefsBuf.ptr);
        M._free(interBuf.ptr);
        M._free(selBuf);
        M._free(scoreBuf.ptr);
        if (opsPtr !== 0)
            M._free(opsPtr);
    }
}
/** Fit POP-PLS (per-component operator-adaptive PLS) on (X, Y).
 *
 * Like AOM-PLS but picks one strict-linear operator PER latent component
 * (`n4m_model_selection_pop_pls_select`) rather than one for the whole model, then
 * returns INPUT-SPACE coefficients so it predicts on RAW X via the same affine
 * intercept path — so it is used WITHOUT preceding preprocessing steps (the
 * screen does the preprocessing internally). Numerics are 100% libn4m; this
 * only builds the bank + validation plan.
 *
 * @param X row-major (n × p) input matrix.
 * @param Y row-major (n × q) target matrix.
 * @param maxComponents max latent components for the internal SIMPLS fits.
 * @param nFolds internal-CV fold count for the operator screen.
 * @param seed reserved (the contiguous-fold partition is deterministic).
 * @param operatorKinds optional `n4m_operator_kind_t` bank override; when
 *   omitted a default strict bank (identity / detrend / SG smooth / SG
 *   derivative / finite-difference) is screened.
 */
export function fitPop(X, Y, maxComponents, nFolds = 5, seed = 0, operatorKinds = []) {
    if (X.rows !== Y.rows) {
        throw new Error(`X.rows (${X.rows}) must equal Y.rows (${Y.rows})`);
    }
    const M = getModule();
    const n = X.rows, p = X.cols, q = Y.cols;
    // The selector clamps max_components to min(maxComponents, p, n-1); allocate
    // the per-component op buffer to the un-clamped request (always >= clamp).
    const maxComp = Math.max(1, maxComponents);
    const xBuf = _malloc_f64(M, n * p);
    const yBuf = _malloc_f64(M, n * q);
    const coefsBuf = _malloc_f64(M, p * q);
    const interBuf = _malloc_f64(M, q);
    const opsOutBuf = M._malloc(maxComp * 4); // int32 per-component op indices
    const nSelBuf = M._malloc(4); // int32 selected component count
    const scoreBuf = _malloc_f64(M, 1);
    const opsPtr = operatorKinds.length > 0
        ? M._malloc(operatorKinds.length * 4)
        : 0;
    try {
        _copy_in(M, X.data, xBuf.ptr);
        _copy_in(M, Y.data, yBuf.ptr);
        if (opsPtr !== 0)
            M.HEAP32.set(Int32Array.from(operatorKinds), opsPtr >> 2);
        const status = M.ccall("n4m_wasm_pop_fit", "number", ["number", "number", "number", "number", "number",
            "number", "number", "number", "number", "number",
            "number", "number", "number", "number", "number"], [xBuf.ptr, yBuf.ptr, n, p, q,
            maxComp, nFolds, seed, opsPtr, operatorKinds.length,
            coefsBuf.ptr, interBuf.ptr, opsOutBuf, nSelBuf, scoreBuf.ptr]);
        checkStatus(status);
        const nSel = Math.max(0, M.HEAP32[nSelBuf >> 2] ?? 0);
        const selectedOperators = [];
        for (let k = 0; k < nSel; ++k) {
            selectedOperators.push(M.HEAP32[(opsOutBuf >> 2) + k] ?? -1);
        }
        return {
            coefficients: _read_out(M, coefsBuf.ptr, p * q),
            // zero means — POP predicts on RAW X via the affine intercept form.
            xMean: new Float64Array(p),
            yMean: new Float64Array(q),
            intercept: _read_out(M, interBuf.ptr, q),
            n_features: p,
            n_targets: q,
            selectedOperators,
            selectedComponents: nSel,
            score: _read_out(M, scoreBuf.ptr, 1)[0] ?? NaN,
        };
    }
    finally {
        M._free(xBuf.ptr);
        M._free(yBuf.ptr);
        M._free(coefsBuf.ptr);
        M._free(interBuf.ptr);
        M._free(opsOutBuf);
        M._free(nSelBuf);
        M._free(scoreBuf.ptr);
        if (opsPtr !== 0)
            M._free(opsPtr);
    }
}
/** Fit the AOM Ridge simplex blender (n4m_ensemble_aom_ridge_blender_fit): builds
 *  a strict-linear chain bank internally, OOF-blends (chain, λ) Ridge candidates
 *  over `cv` contiguous folds, and returns the weighted final INPUT-SPACE
 *  coefficients + intercept — so it predicts on RAW X via the affine form
 *  y = intercept + X.B (used WITHOUT preceding preprocessing). */
export function fitAomRidge(X, Y, opts = {}) {
    if (X.rows !== Y.rows)
        throw new Error(`X.rows (${X.rows}) must equal Y.rows (${Y.rows})`);
    const M = getModule();
    const n = X.rows, p = X.cols, q = Y.cols;
    const profile = opts.profile ?? 0;
    const cv = opts.cv ?? 5;
    const regularizer = opts.regularizer ?? 0.01;
    const lambdas = opts.ridgeLambdas ?? [];
    const xBuf = _malloc_f64(M, n * p);
    const yBuf = _malloc_f64(M, n * q);
    const coefsBuf = _malloc_f64(M, p * q);
    const interBuf = _malloc_f64(M, q);
    const lamBuf = lambdas.length > 0 ? _malloc_f64(M, lambdas.length) : { ptr: 0 };
    try {
        _copy_in(M, X.data, xBuf.ptr);
        _copy_in(M, Y.data, yBuf.ptr);
        if (lamBuf.ptr !== 0)
            _copy_in(M, Float64Array.from(lambdas), lamBuf.ptr);
        const status = M.ccall("n4m_wasm_aom_ridge_fit", "number", ["number", "number", "number", "number", "number",
            "number", "number", "number", "number", "number",
            "number", "number"], [xBuf.ptr, yBuf.ptr, n, p, q,
            profile, cv, lamBuf.ptr, lambdas.length, regularizer,
            coefsBuf.ptr, interBuf.ptr]);
        checkStatus(status);
        return {
            coefficients: _read_out(M, coefsBuf.ptr, p * q),
            xMean: new Float64Array(p),
            yMean: new Float64Array(q),
            intercept: _read_out(M, interBuf.ptr, q),
            n_features: p,
            n_targets: q,
        };
    }
    finally {
        M._free(xBuf.ptr);
        M._free(yBuf.ptr);
        M._free(coefsBuf.ptr);
        M._free(interBuf.ptr);
        if (lamBuf.ptr !== 0)
            M._free(lamBuf.ptr);
    }
}
/** Fit the AOM operator-PLS score stack with Ridge head
 *  (n4m_ensemble_aom_operator_pls_stack_fit). SINGLE-TARGET only (Y must be
 *  n × 1). Returns the stack folded into INPUT-SPACE coefficients + intercept,
 *  so it predicts on RAW X via the affine form (used WITHOUT preprocessing). */
export function fitAomStack(X, Y, opts = {}) {
    if (X.rows !== Y.rows)
        throw new Error(`X.rows (${X.rows}) must equal Y.rows (${Y.rows})`);
    if (Y.cols !== 1)
        throw new Error("fitAomStack is single-target: Y must have exactly 1 column");
    const M = getModule();
    const n = X.rows, p = X.cols, q = 1;
    const profile = opts.profile ?? 0;
    const cv = opts.cv ?? 5;
    const maxComponents = opts.maxComponents ?? 15;
    const stdPenalty = opts.stdPenalty ?? 0;
    const gapPenalty = opts.gapPenalty ?? 0;
    const alphas = opts.alphas ?? [];
    const xBuf = _malloc_f64(M, n * p);
    const yBuf = _malloc_f64(M, n * q);
    const coefsBuf = _malloc_f64(M, p * q);
    const interBuf = _malloc_f64(M, q);
    const alphaBuf = alphas.length > 0 ? _malloc_f64(M, alphas.length) : { ptr: 0 };
    try {
        _copy_in(M, X.data, xBuf.ptr);
        _copy_in(M, Y.data, yBuf.ptr);
        if (alphaBuf.ptr !== 0)
            _copy_in(M, Float64Array.from(alphas), alphaBuf.ptr);
        const status = M.ccall("n4m_wasm_aom_stack_fit", "number", ["number", "number", "number", "number", "number",
            "number", "number", "number", "number", "number",
            "number", "number", "number", "number"], [xBuf.ptr, yBuf.ptr, n, p, q,
            profile, cv, maxComponents, alphaBuf.ptr, alphas.length,
            stdPenalty, gapPenalty, coefsBuf.ptr, interBuf.ptr]);
        checkStatus(status);
        return {
            coefficients: _read_out(M, coefsBuf.ptr, p * q),
            xMean: new Float64Array(p),
            yMean: new Float64Array(q),
            intercept: _read_out(M, interBuf.ptr, q),
            n_features: p,
            n_targets: q,
        };
    }
    finally {
        M._free(xBuf.ptr);
        M._free(yBuf.ptr);
        M._free(coefsBuf.ptr);
        M._free(interBuf.ptr);
        if (alphaBuf.ptr !== 0)
            M._free(alphaBuf.ptr);
    }
}
const SPLIT_KIND_CODE = {
    KennardStone: 0,
    SPXY: 1,
    KMeans: 2,
    KBinsStratified: 3,
    DataTwinning: 4,
    SystematicCircular: 5,
};
/** Compute a single train/test split over the rows of X (and Y) via libn4m's
 * splitters, returning a `Uint8Array` mask of length n where 1 = test, 0 = train.
 *
 * Numerics are 100% libn4m (`n4m_wasm_split` → n4m_split_*). KennardStone and
 * SPXY are deterministic; KMeans and KBinsStratified use `opts.seed`. SPXY and
 * KBinsStratified need Y; KennardStone and KMeans use X only.
 *
 * @param kind splitter strategy.
 * @param X row-major (n × p) input matrix.
 * @param Y row-major (n × q) target matrix (required for SPXY / KBinsStratified).
 * @param opts split options (testSize / seed / maxIter / nBins / strategy).
 */
export function computeSplit(kind, X, Y, opts = {}) {
    const M = getModule();
    const n = X.rows, p = X.cols;
    const q = Y ? Y.cols : 1;
    const testSize = opts.testSize ?? 0.25;
    const seed = (opts.seed ?? 0) >>> 0;
    // p0/p1 generic int params: KMeans → maxIter; KBins → nBins, strategy.
    let p0 = 0, p1 = 0;
    if (kind === "KMeans")
        p0 = opts.maxIter ?? 100;
    if (kind === "KBinsStratified") {
        p0 = opts.nBins ?? 5;
        p1 = opts.strategy ?? 0;
    }
    const xBuf = _malloc_f64(M, n * p);
    const yBuf = Y ? _malloc_f64(M, n * q) : { ptr: 0, len: 0 };
    const maskBuf = M._malloc(n * 4); // int32[n]
    try {
        _copy_in(M, X.data, xBuf.ptr);
        if (Y)
            _copy_in(M, Y.data, yBuf.ptr);
        const status = M.ccall("n4m_wasm_split", "number", ["number", "number", "number", "number", "number",
            "number", "number", "number", "number", "number", "number"], [SPLIT_KIND_CODE[kind], testSize, seed, p0, p1,
            xBuf.ptr, yBuf.ptr, n, p, q, maskBuf]);
        checkStatus(status);
        const mask = new Uint8Array(n);
        for (let i = 0; i < n; i++)
            mask[i] = M.HEAP32[(maskBuf >> 2) + i] === 1 ? 1 : 0;
        return mask;
    }
    finally {
        M._free(xBuf.ptr);
        if (yBuf.ptr !== 0)
            M._free(yBuf.ptr);
        M._free(maskBuf);
    }
}
/** Compute a train/test split and return the ordered train/test indices from libn4m.
 *
 * Unlike {@link computeSplit}, this preserves splitter-specific ordering, which matters
 * for strict parity with the native Python binding.
 */
export function computeSplitIndices(kind, X, Y, opts = {}) {
    const M = getModule();
    const n = X.rows, p = X.cols;
    const q = Y ? Y.cols : 1;
    const testSize = opts.testSize ?? 0.25;
    const seed = (opts.seed ?? 0) >>> 0;
    let p0 = 0, p1 = 0;
    if (kind === "KMeans")
        p0 = opts.maxIter ?? 100;
    if (kind === "KBinsStratified") {
        p0 = opts.nBins ?? 5;
        p1 = opts.strategy ?? 0;
    }
    const xBuf = _malloc_f64(M, n * p);
    const yBuf = Y ? _malloc_f64(M, n * q) : { ptr: 0, len: 0 };
    const trainBuf = M._malloc(n * 4);
    const testBuf = M._malloc(n * 4);
    const nTrainBuf = M._malloc(4);
    const nTestBuf = M._malloc(4);
    try {
        _copy_in(M, X.data, xBuf.ptr);
        if (Y)
            _copy_in(M, Y.data, yBuf.ptr);
        const status = M.ccall("n4m_wasm_split_indices", "number", ["number", "number", "number", "number", "number",
            "number", "number", "number", "number", "number",
            "number", "number", "number", "number"], [SPLIT_KIND_CODE[kind], testSize, seed, p0, p1,
            xBuf.ptr, yBuf.ptr, n, p, q,
            trainBuf, nTrainBuf, testBuf, nTestBuf]);
        checkStatus(status);
        const nTrain = M.HEAP32[nTrainBuf >> 2] ?? 0;
        const nTest = M.HEAP32[nTestBuf >> 2] ?? 0;
        return {
            trainIndices: new Int32Array(M.HEAPU8.buffer, trainBuf, nTrain).slice(),
            testIndices: new Int32Array(M.HEAPU8.buffer, testBuf, nTest).slice(),
        };
    }
    finally {
        M._free(xBuf.ptr);
        if (yBuf.ptr !== 0)
            M._free(yBuf.ptr);
        M._free(trainBuf);
        M._free(testBuf);
        M._free(nTrainBuf);
        M._free(nTestBuf);
    }
}
/* Legacy class wrapper preserved for backwards compat with the
 * scaffold; not yet exposed via index.ts. */
export class Model {
    _data;
    constructor(data) { this._data = data; }
    static fit(_ctx, _cfg, X, Y, n_components = 3) {
        return new Model(fitPls(X, Y, n_components));
    }
    predict(_ctx, X_new) {
        return predictPls(this._data, X_new);
    }
    get coefficients() { return this._data.coefficients; }
    get xMean() { return this._data.xMean; }
    get yMean() { return this._data.yMean; }
    destroy() { }
}
