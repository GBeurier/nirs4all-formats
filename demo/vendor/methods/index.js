// SPDX-License-Identifier: CECILL-2.1 OR AGPL-3.0-or-later
//
// Public TypeScript API for the @nirs4all/methods binding — a
// non-idiomatic function library over libn4m (raw typed arrays in/out). See
// INPUT_CONTRACT.md and examples/consume.mjs.
//
// Example:
//   import * as n4m from "@nirs4all/methods";
//   await n4m.loadModule();
//   const model = n4m.fitPls({ data: X, rows, cols }, { data: y, rows, cols: 1 }, 3);
//   const preds = n4m.predictPls(model, { data: X, rows, cols });
import { getModule } from "./ffi.js";
export { SpectralEncoder } from "./spectralEncoding.js";
export { Optimizer } from "./optimization.js";
export { loadModule, getModule, makeMatrixView, readArrayView } from "./ffi.js";
export { Context } from "./context.js";
export { preflightPca, preflightRepeatability, preflightSummary, preflightNearestNeighbors, preflightLocalContrast, preflightNoiseProxy, preflightSnv, preflightPairsWithinTolerance, preflightFactorAssociation, preflightHolmAdjust, preflightAdjustedRandIndex, statsCorrelationTest, statsBhAdjust, statsFactorAssociationExact, statsCorrelationExact, statsPermanova, statsPermdisp, statsPermanovaFreedmanLane, statsEnergyDistance, statsPcaMonitor, statsVarianceComponentsReml, statsVarianceComponentsIntervals, statsGroupMeans, statsRegressionScores, clusterHdbscan, clusterJaccardMatrix, linearSumAssignment, randomPermutations, randomMultivariateNormal } from "./preflightDiagnostics.js";
export { Config } from "./config.js";
export { Model, fitPls, predictPls, fitModel, predictModel, fitAom, fitAomChain, fitPop, fitAomRidge, fitAomStack, computeSplit, computeSplitIndices } from "./model.js";
export { ppCreate, ppFit, ppTransform, ppGetState, ppSetState, ppDestroy, } from "./preprocessing.js";
export { MethodResult } from "./methodResult.js";
export { NativeModel } from "./nativeModel.js";
export { NativeEstimator, NativeMethod, NativeProcedure, manifest, methodClass } from "./estimatorRoles.js";
export * from "./estimatorRolesGenerated.js";
export { RolePipeline } from "./rolePipeline.js";
export { MultimodalPipeline, MultimodalClassifierPipeline } from "./multimodalPipeline.js";
export { splitNative } from "./nativeSplitter.js";
export { augmentNative } from "./nativeAugmentation.js";
export { NativePreprocessingPipeline, PipelineOperatorKind, } from "./nativePreprocessingPipeline.js";
export { selectSpa, selectVariables, selectorMethods } from "./selection.js";
export { inspectN4mm, SERIALIZED_MODEL_INFO_SCHEMA_V1, SERIALIZED_MODEL_CAPABILITY_PREDICT, SERIALIZED_MODEL_CAPABILITY_TRANSFORM, SERIALIZED_MODEL_CAPABILITY_AFFINE, SERIALIZED_MODEL_CAPABILITY_PIPELINE, PipelineFingerprintAlgorithm, PipelineSemanticProfile, SerializedSavitzkyGolayMode, SerializedPipelineOperatorKind, } from "./serialization.js";
export { Status, Dtype, Algorithm, Solver, Deflation, ModelArray, N4mError, } from "./types.js";
/** ABI / project version reported by the loaded WASM module. */
export function version() {
    const m = getModule();
    const ptr = m.ccall("n4m_get_version_string", "number", [], []);
    return ptr === 0 ? "" : m.UTF8ToString(ptr);
}
/** ABI MAJOR.MINOR.PATCH triple. */
export function abiVersion() {
    const m = getModule();
    return [
        m.ccall("n4m_get_abi_version_major", "number", [], []),
        m.ccall("n4m_get_abi_version_minor", "number", [], []),
        m.ccall("n4m_get_abi_version_patch", "number", [], []),
    ];
}
export { rank } from "./ranking.js";
export { mixedEffects, fanova, normalizeLoss } from "./componentAnalysis.js";
export { explainModel, linearShap, treeShap, groupsFixedWidth, groupsFromRanges, summarizeAttributions, perturbRows, modelImportance, aggregateProfile, compareProfiles } from "./explain.js";
