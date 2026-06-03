#!/usr/bin/env python3
"""Generate demo/formats.json from the authoritative docs/FORMAT_MATRIX.md table.

The demo's "supported formats" table is derived from the project's own matrix so
it never drifts by hand. Run it from build.sh / CI before assembling the page.
"""
import json
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
MATRIX = HERE.parent / "docs" / "FORMAT_MATRIX.md"
OUT = HERE / "formats.json"


def to_int(s):
    s = (s or "").strip()
    return int(s) if s.isdigit() else 0


def status_of(coverage):
    c = coverage.lower()
    if "non viable" in c:
        return "sourcing"            # parser blocked — needs real samples/specs
    if "hors-scope" in c or "hors scope" in c:
        return "outscope"
    if "utile incomplet" in c:
        return "partial"             # parser exists but incomplete
    if "adjacent" in c:
        return "adjacent"            # neighbouring domain (Raman/MS/AFM), reads or disambiguates
    if "diffusable" in c and ("cibl" in c):
        return "targeted"            # publishable if the scope is stated
    if "diffusable" in c:
        return "supported"           # main active variants covered
    return "partial"


def main():
    if not MATRIX.exists():
        sys.exit(f"matrix not found: {MATRIX}")
    lines = MATRIX.read_text().splitlines()
    try:
        hi = next(i for i, l in enumerate(lines) if l.startswith("|") and "Couverture NIRS" in l)
    except StopIteration:
        sys.exit("could not find the matrix header row")
    headers = [h.strip() for h in lines[hi].strip().strip("|").split("|")]
    idx = {h: i for i, h in enumerate(headers)}

    def col(cells, name):
        return cells[idx[name]].strip() if name in idx and idx[name] < len(cells) else ""

    formats = []
    for l in lines[hi + 2:]:
        if not l.startswith("|"):
            break
        cells = [c.strip() for c in l.strip().strip("|").split("|")]
        if len(cells) != len(headers):
            continue
        coverage = col(cells, "Couverture NIRS")
        formats.append({
            "name": col(cells, "Nom"),
            "vendor": col(cells, "Vendeur"),
            "ext": col(cells, "Extensions"),
            "variants": to_int(col(cells, "Variants")),
            "ok": to_int(col(cells, "Validés")),
            "partial": to_int(col(cells, "Partiels")),
            "planned": to_int(col(cells, "Planifiés")),
            "blocked": to_int(col(cells, "Bloqués")),
            "priority": (col(cells, "Priorité").split() or [""])[0],
            "status": status_of(coverage),
        })

    order = {"supported": 0, "targeted": 1, "partial": 2, "adjacent": 3, "sourcing": 4, "outscope": 5}
    formats.sort(key=lambda f: (order.get(f["status"], 9), -f["ok"], f["name"].lower()))

    summary = {"families": len(formats)}
    for f in formats:
        summary[f["status"]] = summary.get(f["status"], 0) + 1
    summary["validated_variants"] = sum(f["ok"] for f in formats)
    summary["partial_variants"] = sum(f["partial"] for f in formats)

    OUT.write_text(json.dumps({"summary": summary, "formats": formats}, ensure_ascii=False, indent=1))
    print(f"wrote {OUT.relative_to(HERE.parent)} — {summary['families']} families, "
          f"{summary['validated_variants']} validated variants")


if __name__ == "__main__":
    main()
