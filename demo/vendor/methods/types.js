// SPDX-License-Identifier: CECILL-2.1 OR AGPL-3.0-or-later
/** Mirror of the C enum n4m_status_t (cpp/include/n4m/n4m.h). */
export var Status;
(function (Status) {
    Status[Status["OK"] = 0] = "OK";
    Status[Status["ERR_INVALID_ARGUMENT"] = 1] = "ERR_INVALID_ARGUMENT";
    Status[Status["ERR_NULL_POINTER"] = 2] = "ERR_NULL_POINTER";
    Status[Status["ERR_SHAPE_MISMATCH"] = 3] = "ERR_SHAPE_MISMATCH";
    Status[Status["ERR_DTYPE_MISMATCH"] = 4] = "ERR_DTYPE_MISMATCH";
    Status[Status["ERR_STRIDE_INVALID"] = 5] = "ERR_STRIDE_INVALID";
    Status[Status["ERR_NOT_FITTED"] = 6] = "ERR_NOT_FITTED";
    Status[Status["ERR_NUMERICAL_FAILURE"] = 7] = "ERR_NUMERICAL_FAILURE";
    Status[Status["ERR_CONVERGENCE_FAILED"] = 8] = "ERR_CONVERGENCE_FAILED";
    Status[Status["ERR_OUT_OF_MEMORY"] = 9] = "ERR_OUT_OF_MEMORY";
    Status[Status["ERR_UNSUPPORTED"] = 10] = "ERR_UNSUPPORTED";
    Status[Status["ERR_NOT_IMPLEMENTED"] = 11] = "ERR_NOT_IMPLEMENTED";
    Status[Status["ERR_ABI_MISMATCH"] = 12] = "ERR_ABI_MISMATCH";
    Status[Status["ERR_IO"] = 13] = "ERR_IO";
    Status[Status["ERR_CORRUPT_BUFFER"] = 14] = "ERR_CORRUPT_BUFFER";
    Status[Status["ERR_VERSION_INCOMPATIBLE"] = 15] = "ERR_VERSION_INCOMPATIBLE";
    Status[Status["ERR_BACKEND_UNAVAILABLE"] = 16] = "ERR_BACKEND_UNAVAILABLE";
    Status[Status["ERR_CANCELLED"] = 17] = "ERR_CANCELLED";
    Status[Status["ERR_INTERNAL"] = 255] = "ERR_INTERNAL";
})(Status || (Status = {}));
/** Mirror of n4m_dtype_t (cpp/include/n4m/n4m.h §2). */
export var Dtype;
(function (Dtype) {
    Dtype[Dtype["UNKNOWN"] = 0] = "UNKNOWN";
    Dtype[Dtype["F64"] = 1] = "F64";
    Dtype[Dtype["F32"] = 2] = "F32";
    Dtype[Dtype["I32"] = 3] = "I32";
    Dtype[Dtype["I64"] = 4] = "I64";
})(Dtype || (Dtype = {}));
/** Mirror of n4m_algorithm_t. Values must match cpp/include/n4m/n4m.h. */
export var Algorithm;
(function (Algorithm) {
    Algorithm[Algorithm["PLS_REGRESSION"] = 0] = "PLS_REGRESSION";
    Algorithm[Algorithm["PLS_CANONICAL"] = 1] = "PLS_CANONICAL";
    Algorithm[Algorithm["PLS_SVD"] = 2] = "PLS_SVD";
    Algorithm[Algorithm["PLS_DA"] = 3] = "PLS_DA";
    Algorithm[Algorithm["OPLS"] = 4] = "OPLS";
    Algorithm[Algorithm["OPLS_DA"] = 5] = "OPLS_DA";
    Algorithm[Algorithm["SPARSE_PLS"] = 6] = "SPARSE_PLS";
    Algorithm[Algorithm["MB_PLS"] = 7] = "MB_PLS";
    Algorithm[Algorithm["LW_PLS"] = 8] = "LW_PLS";
    Algorithm[Algorithm["AOM_PLS"] = 9] = "AOM_PLS";
    Algorithm[Algorithm["PCR"] = 10] = "PCR";
    Algorithm[Algorithm["IMPORTED_LINEAR_PREDICTOR"] = 11] = "IMPORTED_LINEAR_PREDICTOR";
})(Algorithm || (Algorithm = {}));
/** Mirror of n4m_solver_t. */
export var Solver;
(function (Solver) {
    Solver[Solver["NIPALS"] = 0] = "NIPALS";
    Solver[Solver["SIMPLS"] = 1] = "SIMPLS";
    Solver[Solver["ORTHOGONAL_SCORES"] = 2] = "ORTHOGONAL_SCORES";
    Solver[Solver["KERNEL_ALGORITHM"] = 3] = "KERNEL_ALGORITHM";
    Solver[Solver["WIDE_KERNEL"] = 4] = "WIDE_KERNEL";
    Solver[Solver["SVD"] = 5] = "SVD";
    Solver[Solver["POWER"] = 6] = "POWER";
    Solver[Solver["RANDOMIZED_SVD"] = 7] = "RANDOMIZED_SVD";
})(Solver || (Solver = {}));
/** Mirror of n4m_model_array_t (n4m_model_get_array tags). */
export var ModelArray;
(function (ModelArray) {
    ModelArray[ModelArray["COEFFICIENTS"] = 0] = "COEFFICIENTS";
    ModelArray[ModelArray["INTERCEPT"] = 1] = "INTERCEPT";
    ModelArray[ModelArray["X_MEAN"] = 2] = "X_MEAN";
    ModelArray[ModelArray["X_SCALE"] = 3] = "X_SCALE";
    ModelArray[ModelArray["Y_MEAN"] = 4] = "Y_MEAN";
    ModelArray[ModelArray["Y_SCALE"] = 5] = "Y_SCALE";
    ModelArray[ModelArray["WEIGHTS_W"] = 6] = "WEIGHTS_W";
    ModelArray[ModelArray["LOADINGS_P"] = 7] = "LOADINGS_P";
    ModelArray[ModelArray["Y_LOADINGS_Q"] = 8] = "Y_LOADINGS_Q";
    ModelArray[ModelArray["ROTATIONS_R"] = 9] = "ROTATIONS_R";
    ModelArray[ModelArray["SCORES_T"] = 10] = "SCORES_T";
    ModelArray[ModelArray["Y_SCORES_U"] = 11] = "Y_SCORES_U";
    /** ABI 2.20: (1, 1) X-Y cross-covariance resolution of the components. */
    ModelArray[ModelArray["COMPONENT_RESOLUTION"] = 12] = "COMPONENT_RESOLUTION";
    /** ABI 2.20: (1, n_components), 1 for a numerically unresolved component. */
    ModelArray[ModelArray["UNRESOLVED_COMPONENTS"] = 13] = "UNRESOLVED_COMPONENTS";
})(ModelArray || (ModelArray = {}));
/** Mirror of n4m_deflation_t. */
export var Deflation;
(function (Deflation) {
    Deflation[Deflation["REGRESSION"] = 0] = "REGRESSION";
    Deflation[Deflation["CANONICAL"] = 1] = "CANONICAL";
    Deflation[Deflation["X_ONLY"] = 2] = "X_ONLY";
    Deflation[Deflation["XY"] = 3] = "XY";
    Deflation[Deflation["ORTHOGONAL"] = 4] = "ORTHOGONAL";
})(Deflation || (Deflation = {}));
export class N4mError extends Error {
    status;
    constructor(status, message) {
        super(`n4m error ${status}: ${message}`);
        this.name = "N4mError";
        this.status = status;
    }
}
