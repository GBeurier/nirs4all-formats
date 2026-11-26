// SPDX-License-Identifier: CECILL-2.1 OR AGPL-3.0-or-later
import { checkStatus, getModule } from "./ffi.js";
/** RAII wrapper around n4m_config_t. */
export class Config {
    _ptr;
    constructor(ptr) {
        this._ptr = ptr;
    }
    static create() {
        const m = getModule();
        const out = m._malloc(4);
        try {
            const status = m.ccall("n4m_config_create", "number", ["number"], [out]);
            checkStatus(status);
            return new Config(m.getValue(out, "i32"));
        }
        finally {
            m._free(out);
        }
    }
    get handle() {
        return this._ptr;
    }
    destroy() {
        if (this._ptr === 0)
            return;
        getModule().ccall("n4m_config_destroy", null, ["number"], [this._ptr]);
        this._ptr = 0;
    }
    setNComponents(k) {
        const s = getModule().ccall("n4m_config_set_n_components", "number", ["number", "number"], [this._ptr, k]);
        checkStatus(s);
    }
    setAlgorithm(a) {
        const s = getModule().ccall("n4m_config_set_algorithm", "number", ["number", "number"], [this._ptr, a]);
        checkStatus(s);
    }
    setSolver(solver) {
        const s = getModule().ccall("n4m_config_set_solver", "number", ["number", "number"], [this._ptr, solver]);
        checkStatus(s);
    }
    setDeflation(d) {
        const s = getModule().ccall("n4m_config_set_deflation", "number", ["number", "number"], [this._ptr, d]);
        checkStatus(s);
    }
    setCenterX(on) {
        const s = getModule().ccall("n4m_config_set_center_x", "number", ["number", "number"], [this._ptr, on ? 1 : 0]);
        checkStatus(s);
    }
    setCenterY(on) {
        const s = getModule().ccall("n4m_config_set_center_y", "number", ["number", "number"], [this._ptr, on ? 1 : 0]);
        checkStatus(s);
    }
    setStoreScores(on) {
        const s = getModule().ccall("n4m_config_set_store_scores", "number", ["number", "number"], [this._ptr, on ? 1 : 0]);
        checkStatus(s);
    }
}
