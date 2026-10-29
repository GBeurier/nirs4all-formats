//! Bundle the licensed security-backport sources in binary Python distributions.
use std::fs;
use std::path::Path;

fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir_all(target).expect("create bundled source directory");
    for entry in fs::read_dir(source).expect("read bundled source directory") {
        let entry = entry.expect("read bundled source entry");
        let destination = target.join(entry.file_name());
        if entry.file_type().expect("read source file type").is_dir() {
            copy_tree(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), destination).expect("copy licensed source");
        }
    }
}

fn main() {
    let manifest = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    for reader in ["hdf5-reader", "netcdf-reader"] {
        let source = manifest.join("../../vendor").join(reader);
        let target = output.join("reader-backports").join(reader);
        fs::create_dir_all(&target).expect("create reader license directory");
        for file in [
            "Cargo.toml",
            "UPSTREAM-Cargo.toml",
            "UPSTREAM-VCS.json",
            "README.md",
            "PROVENANCE.json",
            "LICENSE-MIT",
            "LICENSE-APACHE",
        ] {
            println!("cargo:rerun-if-changed={}", source.join(file).display());
            fs::copy(source.join(file), target.join(file)).expect("copy reader license/provenance");
        }
        println!("cargo:rerun-if-changed={}", source.join("src").display());
        copy_tree(&source.join("src"), &target.join("src"));
    }
}
