//! Build script: regenerate the C ABI header into `include/nirs4all_formats.h`.
//!
//! Re-runs whenever the crate source or the workspace-level `cbindgen.toml`
//! changes. The generated header is committed alongside the crate so
//! downstream packagers can pin a stable copy without invoking cargo.

use std::path::PathBuf;

fn main() {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_dir = crate_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root is two levels above the crate manifest");
    let config_path = workspace_dir.join("cbindgen.toml");
    let output_path = crate_dir.join("include").join("nirs4all_formats.h");

    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed={}", config_path.display());

    if !config_path.exists() {
        eprintln!(
            "warning: cbindgen.toml not found at {}; skipping header regeneration",
            config_path.display()
        );
        return;
    }

    let config = match cbindgen::Config::from_file(&config_path) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("warning: failed to load cbindgen.toml: {err}; skipping header regeneration");
            return;
        }
    };

    match cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(config)
        .generate()
    {
        Ok(bindings) => {
            std::fs::create_dir_all(output_path.parent().expect("include dir parent"))
                .expect("create include directory");
            bindings.write_to_file(&output_path);
        }
        Err(err) => {
            eprintln!("warning: cbindgen failed to generate header: {err}");
        }
    }
}
