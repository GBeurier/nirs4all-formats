//! Export presets for third-party software.
//!
//! A preset names a format the software documents as importable. It is not a
//! claim that the software accepts our files: `tested` stays false until an
//! import with a named version is recorded.

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct SoftwareProfile {
    pub id: &'static str,
    pub label: &'static str,
    pub format: &'static str,
    pub extension: &'static str,
    /// Other formats the same software documents.
    pub alternatives: &'static [&'static str],
    /// Where the import capability is described.
    pub evidence: &'static str,
    /// Whether an import of our files was verified in the software.
    pub tested: bool,
    pub notes: &'static str,
}

const SOFTWARE: &[SoftwareProfile] = &[
    SoftwareProfile {
        id: "opus",
        label: "Bruker OPUS",
        format: "jcamp-dx",
        extension: "dx",
        alternatives: &["galactic-spc", "bruker-dpt"],
        evidence: "OPUS 5 reference manual, sections 2.1 and 3.11",
        tested: false,
        notes: "",
    },
    SoftwareProfile {
        id: "omnic",
        label: "Thermo OMNIC Paradigm",
        format: "jcamp-dx",
        extension: "jdx",
        alternatives: &["galactic-spc"],
        evidence: "OMNIC Paradigm user guide, Import and export data",
        tested: false,
        notes: "",
    },
    SoftwareProfile {
        id: "perkinelmer-spectrum",
        label: "PerkinElmer Spectrum",
        format: "jcamp-dx",
        extension: "dx",
        alternatives: &["galactic-spc"],
        evidence: "PerkinElmer Spectrum user's guide, File Open",
        tested: false,
        notes: "",
    },
    SoftwareProfile {
        id: "nircal",
        label: "BUCHI NIRCal",
        format: "jcamp-dx",
        extension: "dx",
        alternatives: &["galactic-spc"],
        evidence: "NIRCal 5.5 manual, section 3.15.2 (Spectra Converter)",
        tested: false,
        notes: "Import through the Spectra Converter.",
    },
    SoftwareProfile {
        id: "grams",
        label: "Thermo GRAMS",
        format: "galactic-spc",
        extension: "spc",
        alternatives: &["jcamp-dx"],
        evidence: "SPC is the native GRAMS format",
        tested: false,
        notes: "",
    },
    SoftwareProfile {
        id: "unscrambler",
        label: "The Unscrambler X",
        format: "xlsx",
        extension: "xlsx",
        alternatives: &["jcamp-dx", "galactic-spc", "csv"],
        evidence: "The Unscrambler X 10.3 user manual, Import Data",
        tested: false,
        notes: "The workbook carries reference values next to the spectra.",
    },
    SoftwareProfile {
        id: "simca",
        label: "Sartorius SIMCA",
        format: "xlsx",
        extension: "xlsx",
        alternatives: &["csv", "jcamp-dx", "galactic-spc"],
        evidence: "Sartorius application note on spectroscopic data in SIMCA",
        tested: false,
        notes: "The workbook carries reference values next to the spectra.",
    },
    SoftwareProfile {
        id: "solo",
        label: "Eigenvector PLS_Toolbox / Solo",
        format: "xlsx",
        extension: "xlsx",
        alternatives: &["csv", "galactic-spc", "jcamp-dx"],
        evidence: "PLS_Toolbox documentation 7.5, Data Importing Formats",
        tested: false,
        notes: "The workbook carries reference values next to the spectra.",
    },
    SoftwareProfile {
        id: "pirouette",
        label: "Infometrix Pirouette",
        format: "galactic-spc",
        extension: "spc",
        alternatives: &["jcamp-dx"],
        evidence: "Pirouette 5.0 manual, chapter 14",
        tested: false,
        notes: "Pirouette reads JCAMP-DX up to version 4.24.",
    },
    SoftwareProfile {
        id: "spectragryph",
        label: "Spectragryph",
        format: "galactic-spc",
        extension: "spc",
        alternatives: &["jcamp-dx", "xy-text"],
        evidence: "Spectragryph feature list, Data import",
        tested: false,
        notes: "",
    },
    SoftwareProfile {
        id: "winisi",
        label: "FOSS WinISI",
        format: "jcamp-dx",
        extension: "dx",
        alternatives: &[],
        evidence: "User forum only; not confirmed by FOSS documentation",
        tested: false,
        notes: "Users report that WinISI expects 700 or 1050 points per spectrum; axes are never resampled here.",
    },
];

/// Software export presets.
pub fn software_catalog() -> &'static [SoftwareProfile] {
    SOFTWARE
}
