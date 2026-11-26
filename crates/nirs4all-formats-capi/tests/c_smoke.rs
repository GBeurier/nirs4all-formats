//! Compile `tests/smoke.c` against the committed header and the cdylib of
//! this build with the system C compiler (`$CC`, default `cc`), then run it.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// `target/<profile>`, where cargo places the cdylib next to `deps/`.
fn profile_dir() -> PathBuf {
    let exe = std::env::current_exe().expect("test executable path");
    exe.parent()
        .and_then(Path::parent)
        .expect("test executable lives in target/<profile>/deps")
        .to_path_buf()
}

#[test]
fn c_program_links_and_runs() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_dir = profile_dir();
    let library = if cfg!(target_os = "macos") {
        "libnirs4all_formats_capi.dylib"
    } else {
        "libnirs4all_formats_capi.so"
    };
    if !lib_dir.join(library).exists() {
        // `cargo test` does not always emit the cdylib: build it in the same profile.
        let mut build = Command::new(env!("CARGO"));
        build.args(["build", "-p", "nirs4all-formats-capi"]);
        if lib_dir.file_name().is_some_and(|name| name == "release") {
            build.arg("--release");
        }
        assert!(build.status().expect("run cargo build").success());
    }
    assert!(
        lib_dir.join(library).exists(),
        "{library} not found in {}",
        lib_dir.display()
    );
    let work = std::env::temp_dir().join(format!("n4fmt-c-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&work).unwrap();
    let binary = work.join("smoke");
    let compiler = std::env::var("CC").unwrap_or_else(|_| "cc".to_string());
    let status = Command::new(&compiler)
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
        .arg("-I")
        .arg(crate_dir.join("include"))
        .arg(crate_dir.join("tests/smoke.c"))
        .arg("-L")
        .arg(&lib_dir)
        .arg(format!("-Wl,-rpath,{}", lib_dir.display()))
        .arg("-lnirs4all_formats_capi")
        .arg("-o")
        .arg(&binary)
        .status()
        .unwrap_or_else(|error| panic!("cannot run the C compiler '{compiler}': {error}"));
    assert!(status.success(), "the C smoke test does not compile");

    let sample = crate_dir.join("../../samples/jcamp_dx/TESTSPEC.DX");
    let output = Command::new(&binary)
        .arg(&sample)
        .arg(&work)
        .output()
        .expect("run the C smoke test");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stdout}\n{stderr}");
    assert!(stdout.contains("all checks passed"), "{stdout}");
    assert!(work.join("smoke.jdx").exists());
    std::fs::remove_dir_all(&work).unwrap();
}
