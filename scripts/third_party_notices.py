#!/usr/bin/env python3
"""Generate THIRD_PARTY_NOTICES.md: every open-source component GIS Studio
uses or ships, with its version, license and home page.

Sources of truth:
  - Rust crates:      `cargo metadata` (everything compiled into the app)
  - JavaScript:       package-lock.json (production packages bundled in the UI)
  - Native libraries: the dylibs in the built macOS app, mapped to their
                      Homebrew formulas (`brew info`), when the app is built
Run from the repository root:  python3 scripts/third_party_notices.py
"""
import glob
import json
import os
import shutil
import subprocess
import sys
from datetime import date

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
APP = os.path.join(ROOT, "target/release/bundle/macos/GIS Studio.app")
GPL_ONLY = ("GPL-2.0-only", "GPL-3.0-only", "GPL-2.0-or-later", "GPL-3.0-or-later")
# Homebrew lists a formula's overall license; for these, the library we ship
# is licensed differently from the formula's tools.
LIBRARY_LICENSE = {
    "libtool": "LGPL-2.1-or-later (libltdl; the libtool scripts are GPL-2.0-or-later)",
    "gettext": "LGPL-2.1-or-later (libintl; the gettext tools are GPL-3.0-or-later)",
    "xz": "0BSD (liblzma; some xz tools are GPL-2.0-or-later)",
    "unixodbc": "LGPL-2.1-or-later (libodbc)",
}


def cell(s):
    return (s or "—").replace("|", "\\|").replace("\n", " ")


def rust_crates():
    cargo = shutil.which("cargo") or "/opt/homebrew/opt/rustup/bin/cargo"
    meta = json.loads(subprocess.run([cargo, "metadata", "--format-version", "1"], cwd=ROOT, capture_output=True, text=True, check=True).stdout)
    ws = set(meta["workspace_members"])
    rows = {}
    for p in meta["packages"]:
        if p["id"] in ws:
            continue
        rows[(p["name"], p["version"])] = (p.get("license") or p.get("license_file") or "see crate", p.get("repository") or p.get("homepage") or f"https://crates.io/crates/{p['name']}")
    return sorted(rows.items())


def npm_packages():
    lock = json.load(open(os.path.join(ROOT, "package-lock.json")))
    rows = {}
    for path, p in lock.get("packages", {}).items():
        if not path or p.get("dev") or p.get("devOptional"):
            continue
        name = path.split("node_modules/")[-1]
        lic = p.get("license")
        if not lic:
            try:
                lic = json.load(open(os.path.join(ROOT, path, "package.json"))).get("license")
            except OSError:
                lic = None
        rows[(name, p.get("version", ""))] = (lic or "see package", f"https://www.npmjs.com/package/{name}")
    return sorted(rows.items())


def native_libs():
    fw = os.path.join(APP, "Contents/Frameworks")
    if not os.path.isdir(fw) or not shutil.which("brew"):
        return None
    owner = {}
    for lib in sorted(os.listdir(fw)):
        hits = glob.glob(f"/opt/homebrew/Cellar/*/*/lib/{lib}") + glob.glob(f"/opt/homebrew/Cellar/*/*/lib/*/{lib}")
        owner.setdefault(hits[0].split("/")[4] if hits else "(unknown)", []).append(lib)
    forms = [f for f in owner if not f.startswith("(")]
    info = json.loads(subprocess.run(["brew", "info", "--json=v2", *forms], capture_output=True, text=True).stdout or "{}")
    meta = {x["name"]: x for x in info.get("formulae", [])}
    rows = []
    for f, libs in sorted(owner.items()):
        m = meta.get(f, {})
        rows.append((f, m.get("versions", {}).get("stable", ""), LIBRARY_LICENSE.get(f) or m.get("license") or "unknown", m.get("homepage") or "", m.get("desc") or "", libs))
    return rows


def main():
    out = []
    w = out.append
    w("# Third-party notices")
    w("")
    w(f"GIS Studio is MIT OR Apache-2.0 licensed. It is built on the open-source software listed below, whose authors we thank. Generated {date.today().isoformat()} by `scripts/third_party_notices.py`; licenses are as declared by each package. Full license texts are in each package's source distribution (linked).")
    w("")

    libs = native_libs()
    gpl = [r for r in (libs or []) if any(g in r[2] for g in GPL_ONLY) and "LGPL" not in r[2].split(" OR ")[0] and not r[2].startswith(("MPL", "BSD", "MIT", "Apache", "LGPL", "0BSD", "(BSD"))]
    if gpl:
        w("## Licensing note for the macOS installer")
        w("")
        w("The bundled GDAL build (from Homebrew) links these libraries, which are licensed under the GPL only. Distributing the installer with them makes that binary distribution subject to the GPL (source must be offered). GIS Studio's own source code remains MIT OR Apache-2.0. A GDAL build without these optional drivers (PDF via poppler, HEIF/HEVC via x265, SpatiaLite topology via librttopo) avoids this.")
        w("")
        for f, ver, lic, home, desc, files in gpl:
            w(f"- **{f} {ver}** — {lic} — {desc} ({', '.join(files)})")
        w("")

    w("## Bundled data")
    w("")
    w("| Data | License / terms | Source |")
    w("|---|---|---|")
    w("| EPSG Geodetic Parameter Dataset (inside PROJ's proj.db) | EPSG Dataset Terms of Use, © IOGP | https://epsg.org/terms-of-use.html |")
    w("| PROJ data files (proj.db, grids) | MIT (PROJ); grids per their providers | https://proj.org/ |")
    w("| GDAL data files | MIT | https://gdal.org/ |")
    w("| DuckDB spatial extension | MIT | https://github.com/duckdb/duckdb-spatial |")
    w("| WhiteboxTools program (hydrology tools), © John Lindsay; built from release v2.4.0 and shipped as a separate program | MIT | https://github.com/jblindsay/whitebox-tools |")
    w("| World Magnetic Model 2025 coefficients (via `geomagnetism`) | Public domain (NOAA NCEI / British Geological Survey) | https://www.ncei.noaa.gov/products/world-magnetic-model |")
    w("| Noto Sans Regular, Bold, Italic (`crates/engine/assets/fonts/`, embedded in print layouts; license text in `OFL.txt` there) | SIL Open Font License 1.1, © The Noto Project Authors | https://github.com/notofonts/latin-greek-cyrillic |")
    w("| Extra CRS definitions (`crates/engine/assets/extra_crs.json`) | Derived from PROJ / EPSG (terms above) | https://proj.org/ |")
    w("")

    if libs is not None:
        w(f"## Native libraries bundled in the macOS app ({sum(len(r[5]) for r in libs)} files from {len(libs)} projects)")
        w("")
        w("| Project | Version | License | Home page | Files |")
        w("|---|---|---|---|---|")
        for f, ver, lic, home, desc, files in libs:
            w(f"| {cell(f)} | {cell(ver)} | {cell(lic)} | {cell(home)} | {cell(', '.join(files))} |")
        w("")
    else:
        w("## Native libraries")
        w("")
        w("Build the macOS app (`scripts/package-macos.sh`) and re-run this script to list the bundled native libraries.")
        w("")

    npm = npm_packages()
    w(f"## JavaScript packages bundled in the user interface ({len(npm)})")
    w("")
    w("| Package | Version | License |")
    w("|---|---|---|")
    for (name, ver), (lic, url) in npm:
        w(f"| [{cell(name)}]({url}) | {cell(ver)} | {cell(lic)} |")
    w("")

    crates = rust_crates()
    w(f"## Rust crates compiled into the app ({len(crates)})")
    w("")
    w("| Crate | Version | License | Repository |")
    w("|---|---|---|---|")
    for (name, ver), (lic, url) in crates:
        w(f"| {cell(name)} | {cell(ver)} | {cell(lic)} | {cell(url)} |")
    w("")

    path = os.path.join(ROOT, "THIRD_PARTY_NOTICES.md")
    open(path, "w").write("\n".join(out))
    print(f"Wrote {path}: {len(crates)} crates, {len(npm)} npm packages, {0 if libs is None else len(libs)} native projects, {len(gpl)} GPL-only")


if __name__ == "__main__":
    sys.exit(main())
