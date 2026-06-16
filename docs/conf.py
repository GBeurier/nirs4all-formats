project = "nirs4all-formats"
author = "G. Beurier"
copyright = "2026, G. Beurier"

extensions = [
    "myst_parser",
    "sphinx_design",
    "sphinx_copybutton",
    "sphinx.ext.autosectionlabel",
    "sphinx.ext.mathjax",
    "sphinxext.opengraph",
]

myst_enable_extensions = [
    "colon_fence",
    "deflist",
    "fieldlist",
    "substitution",
    "tasklist",
    "attrs_inline",
    "dollarmath",
]
myst_heading_anchors = 3

autosectionlabel_prefix_document = True

source_suffix = {
    ".rst": "restructuredtext",
    ".md": "markdown",
}
root_doc = "index"
html_theme = "furo"

# Excluded: internal planning / status / inventory / reverse-engineering artifacts.
# These are tracked in-repo for maintainers but are not part of the published,
# end-user documentation site. Keeping them out of the build avoids "document
# isn't in any toctree" warnings under -W. REDESIGN_FORMATS_AND_IO.md also uses
# illustrative pseudo-JSON/TOML fences that no strict Pygments lexer accepts.
exclude_patterns = [
    "_build",
    "Thumbs.db",
    ".DS_Store",
    "REDESIGN_FORMATS_AND_IO.md",
    "PLAN.md",
    "DIRECTIONS.md",
    "ROADMAP.md",
    "STATUS.md",
    "FORMATS.md",
    "FORMAT_MATRIX.md",
    "IMPLEMENTATION_DASHBOARD.md",
    "FORMAT_GAPS.md",
    "MISSING_SAMPLES.md",
    "Formats_extract.md",
    "REVERSE_ENGINEERING.md",
    "FIXTURE_GOVERNANCE.md",
    "CI_MATRIX.md",
    "dev",
    "reviews",
]

html_static_path = ["_static"]
html_favicon = "_static/brand/favicon.ico"

html_theme_options = {
    "light_logo": "brand/horizontal.svg",
    "dark_logo": "brand/horizontal-dark.svg",
}

# Open Graph / social cards
ogp_site_url = "https://nirs4all-formats.readthedocs.io/en/latest/"
ogp_image = "https://nirs4all-formats.readthedocs.io/en/latest/_static/brand/og.png"
