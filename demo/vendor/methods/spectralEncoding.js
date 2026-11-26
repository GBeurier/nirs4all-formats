// SPDX-License-Identifier: CECILL-2.1 OR AGPL-3.0-or-later
import { checkStatus, getModule } from "./ffi.js";
/** Fitted native LVSE/GCU. Call dispose() to release its WASM handle. */
export class SpectralEncoder {
    ptr = 0;
    features = 0;
    outputs = 0;
    constructor(options) {
        if (options.kind !== "lvse" && options.kind !== "gcu")
            throw new Error("Unknown encoder kind");
        const width = options.width ?? 64;
        const rank = options.rank ?? (options.kind === "lvse" ? 4 : 16);
        const iterations = options.maxIter ?? 60;
        for (const v of [width, rank, iterations]) {
            if (!Number.isInteger(v) || v < 1 || v > 2147483647)
                throw new Error("Invalid encoder integer parameter");
        }
        const m = getModule(), out = m._malloc(4);
        try {
            checkStatus(m.ccall("n4m_decomposition_spectral_create", "number", Array(9).fill("number"), [options.kind === "lvse" ? 0 : 1, width, rank,
                options.overlap ?? 0, Number(options.standardize ?? true),
                Number(options.snv ?? false), iterations, options.tolerance ?? 1e-3, out]));
            this.ptr = m.HEAP32[out / 4];
        }
        finally {
            m._free(out);
        }
    }
    check(X) {
        if (!this.ptr)
            throw new Error("Encoder is disposed");
        if (!Number.isInteger(X.rows) || !Number.isInteger(X.cols) || X.rows < 1 || X.cols < 1
            || X.rows > 2147483647 || X.cols > 2147483647 || X.data.length !== X.rows * X.cols)
            throw new Error("Invalid spectral matrix dimensions");
    }
    fit(X) {
        this.check(X);
        const m = getModule(), xp = m._malloc(X.data.length * 8), count = m._malloc(8);
        try {
            m.HEAPF64.set(X.data, xp / 8);
            checkStatus(m.ccall("n4m_wasm_spectral_fit", "number", Array(4).fill("number"), [this.ptr, xp, X.rows, X.cols]));
            checkStatus(m.ccall("n4m_decomposition_spectral_output_cols", "number", ["number", "number"], [this.ptr, count]));
            const view = new DataView(m.HEAPU8.buffer);
            this.outputs = Number(view.getBigInt64(count, true));
            this.features = X.cols;
            return this;
        }
        finally {
            m._free(xp);
            m._free(count);
        }
    }
    transform(X) {
        this.check(X);
        if (!this.outputs || X.cols !== this.features)
            throw new Error("Unfitted encoder or feature mismatch");
        const m = getModule(), xp = m._malloc(X.data.length * 8), out = m._malloc(X.rows * this.outputs * 8);
        try {
            m.HEAPF64.set(X.data, xp / 8);
            checkStatus(m.ccall("n4m_wasm_spectral_transform", "number", Array(6).fill("number"), [this.ptr, xp, X.rows, X.cols, out, this.outputs]));
            return { data: Float64Array.from(m.HEAPF64.subarray(out / 8, out / 8 + X.rows * this.outputs)),
                rows: X.rows, cols: this.outputs };
        }
        finally {
            m._free(xp);
            m._free(out);
        }
    }
    /** LVSE without SNV: transform(X) = X @ operator.T + offset. */
    exportAffine() {
        if (!this.ptr || !this.outputs)
            throw new Error("Encoder is not fitted");
        const m = getModule(), size = this.outputs * this.features;
        const op = m._malloc(size * 8), offset = m._malloc(this.outputs * 8);
        try {
            checkStatus(m.ccall("n4m_wasm_spectral_affine", "number", Array(5).fill("number"), [this.ptr, op, offset, this.features, this.outputs]));
            return { operator: { data: Float64Array.from(m.HEAPF64.subarray(op / 8, op / 8 + size)),
                    rows: this.outputs, cols: this.features },
                offset: Float64Array.from(m.HEAPF64.subarray(offset / 8, offset / 8 + this.outputs)) };
        }
        finally {
            m._free(op);
            m._free(offset);
        }
    }
    dispose() {
        if (this.ptr) {
            getModule().ccall("n4m_decomposition_spectral_destroy", null, ["number"], [this.ptr]);
            this.ptr = 0;
        }
    }
}
