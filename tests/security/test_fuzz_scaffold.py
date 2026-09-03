import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
EXPECTED_SEEDS = {
    "00-jcamp-peak-assignments": (0, "samples/jcamp_dx/synthetic_peak_assignments.jdx"),
    "01-perkin-elmer-trailing": (1, "samples/perkin_elmer/synthetic_trailing.sp"),
    "02-msa-minimum-metadata": (2, "samples/msa_iso22029/minimum_metadata.msa"),
    "03-viavi-micronir": (3, "samples/viavi_micronir/synthetic_micronir.sam"),
    "04-animl-autoincrement": (4, "samples/animl/synthetic_nirs_autoincrement.animl"),
}


def test_fuzz_package_targets_public_registry_with_bounded_input() -> None:
    root_manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
    assert "fuzz" in root_manifest["workspace"]["exclude"]

    manifest = tomllib.loads((ROOT / "fuzz" / "Cargo.toml").read_text())
    assert manifest["package"]["publish"] is False
    assert manifest["package"]["metadata"]["cargo-fuzz"] is True
    dependency = manifest["dependencies"]["nirs4all-formats"]
    assert dependency == {
        "path": "../crates/nirs4all-formats",
        "default-features": False,
    }

    target = (ROOT / "fuzz" / "fuzz_targets" / "registry_open_bytes.rs").read_text()
    assert "const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;" in target
    assert "nirs4all_formats::open_bytes(name, payload)" in target
    assert "Path" not in target


def test_seed_corpus_is_small_and_derived_only_from_public_fixtures() -> None:
    corpus = ROOT / "fuzz" / "corpus" / "registry_open_bytes"
    assert {path.name for path in corpus.iterdir()} == set(EXPECTED_SEEDS)

    for name, (selector, relative_source) in EXPECTED_SEEDS.items():
        assert not relative_source.startswith("samples_local/")
        source = (ROOT / relative_source).read_bytes()
        assert len(source) <= 1024 * 1024
        assert (corpus / name).read_bytes() == bytes([selector]) + source


def test_fuzz_scaffold_does_not_install_or_download_tools() -> None:
    forbidden = ("cargo install", "rustup", "curl ", "wget ", "pip install")
    text_files = [
        ROOT / "fuzz" / "Cargo.toml",
        ROOT / "fuzz" / "README.md",
        ROOT / "fuzz" / "fuzz_targets" / "registry_open_bytes.rs",
        ROOT / "fuzz" / "scripts" / "build_seed_corpus.py",
    ]
    combined = "\n".join(path.read_text() for path in text_files).lower()
    assert not any(command in combined for command in forbidden)
