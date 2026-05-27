project = "nirs4all-formats"
author = "G. Beurier"
copyright = "2026, G. Beurier"

extensions = ["myst_parser"]
source_suffix = {
    ".rst": "restructuredtext",
    ".md": "markdown",
}
master_doc = "index"
html_theme = "furo"

# Internal planning/design artifact (about renaming this repo + building the new
# nirs4all-io bridge); it uses illustrative pseudo-JSON/TOML fences that no strict
# Pygments lexer accepts, and it is not part of the published formats docs.
exclude_patterns = ["_build", "Thumbs.db", ".DS_Store", "REDESIGN_FORMATS_AND_IO.md"]
