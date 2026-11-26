// SPDX-License-Identifier: CECILL-2.1 OR AGPL-3.0-or-later
import { checkStatus, getModule } from "./ffi.js";
import { N4mError, Status } from "./types.js";
/** RAII wrapper around n4m_context_t. */
export class Context {
    _ptr;
    constructor(ptr) {
        this._ptr = ptr;
    }
    /** Create a new context. Throws N4mError on failure. */
    static create() {
        const m = getModule();
        const out = m._malloc(4);
        if (out === 0)
            throw new N4mError(Status.ERR_OUT_OF_MEMORY, "context allocation failed");
        try {
            const status = m.ccall("n4m_context_create", "number", ["number"], [out]);
            checkStatus(status);
            const ptr = m.getValue(out, "i32");
            return new Context(ptr);
        }
        finally {
            m._free(out);
        }
    }
    /** Returns the raw `n4m_context_t*` pointer (handle). */
    get handle() {
        return this._ptr;
    }
    /** Free the context. Safe to call multiple times. */
    destroy() {
        if (this._ptr === 0)
            return;
        getModule().ccall("n4m_context_destroy", null, ["number"], [this._ptr]);
        this._ptr = 0;
    }
    /** Set the RNG seed used by stochastic algorithms.
     *
     *  n4m_context_set_seed takes a uint64_t. Under WASM_BIGINT=1 the i64 slot
     *  must be marshalled as 'i64' with a BigInt — passing a JS number loses
     *  precision above 2^53 and throws "Cannot convert N to a BigInt" on emsdk
     *  >= 5.0.7 (same class of bug as the matrix-view dims; see ffi.ts). */
    setSeed(seed) {
        const status = getModule().ccall("n4m_context_set_seed", "number", ["number", "i64"], [this._ptr, BigInt(seed)]);
        checkStatus(status, this._ptr);
    }
}
