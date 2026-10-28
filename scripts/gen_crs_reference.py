#!/usr/bin/env python3
"""Generate crates/engine/tests/fixtures/crs/reference.json from PROJ's `cs2cs` (docs/CRS_PLAN.md RD-1, R1).

For every CRS of the test zoo: the coordinates of a fixed set of WGS 84 points (best operation, cs2cs default),
and direct A->B results for a set of pairs. All values are x = east / lon, y = north / lat, whatever the CRS's
authority axis order. A point PROJ cannot project is null. The PROJ version is recorded.
Run: python3 scripts/gen_crs_reference.py   (needs cs2cs and projinfo on PATH)
"""
import json, os, subprocess

ZOO = {
    "4326": "EPSG:4326", "4269": "EPSG:4269", "4267": "EPSG:4267", "3857": "EPSG:3857",
    "32615": "EPSG:32615", "26915": "EPSG:26915", "32756": "EPSG:32756", "5070": "EPSG:5070",
    "2277": "EPSG:2277", "32139": "EPSG:32139",
    "ft_utm15": "+proj=utm +zone=15 +datum=NAD83 +units=ft +no_defs",   # international foot
    "3031": "EPSG:3031", "3413": "EPSG:3413", "3035": "EPSG:3035", "2193": "EPSG:2193",
    "32660": "EPSG:32660", "32601": "EPSG:32601", "4807": "EPSG:4807",
    "aeqd": "+proj=aeqd +lat_0=40 +lon_0=-100 +datum=WGS84 +units=m +no_defs",
    # An ESRI-flavoured WKT (what a shapefile .prj holds): the same CRS as EPSG:26915 under other names.
    "esri_26915": 'PROJCS["NAD_1983_UTM_Zone_15N",GEOGCS["GCS_North_American_1983",DATUM["D_North_American_1983",SPHEROID["GRS_1980",6378137.0,298.257222101]],PRIMEM["Greenwich",0.0],UNIT["Degree",0.0174532925199433]],PROJECTION["Transverse_Mercator"],PARAMETER["False_Easting",500000.0],PARAMETER["False_Northing",0.0],PARAMETER["Central_Meridian",-93.0],PARAMETER["Scale_Factor",0.9996],PARAMETER["Latitude_Of_Origin",0.0],UNIT["Meter",1.0]]',
}
# WGS 84 lon, lat: anchors, mid-latitudes, the southern hemisphere, near the poles, the antimeridian.
POINTS = [
    (-93, 0), (-97.5, 35.2), (-100, 40), (-70, 44), (10, 52), (151.2, -33.9), (174.8, -41.3), (178.5, -17.8),
    (-179.5, -17.8), (-147, 64), (-122.3, 47.6), (-93, 60), (2.35, 48.85), (139.7, 35.7), (-58.4, -34.6),
    (28, -26), (0, 85), (0, -71), (0, -89), (180, 0), (-96, 23), (-95.2, 29.8),
]
PAIRS = [("4267", "4269"), ("4326", "4267"), ("4326", "4269"), ("26915", "32615"), ("32615", "3857"), ("26915", "5070"),
         ("2277", "32139"), ("3035", "4326"), ("2193", "4326"), ("32660", "32601"), ("4807", "4326"), ("5070", "3857"),
         ("3031", "3413"), ("26915", "ft_utm15"), ("esri_26915", "26915"), ("4326", "aeqd")]


def run(args, text):
    p = subprocess.run(args, input=text, capture_output=True, text=True)
    return p.stdout.strip().splitlines()


def swapped(defn):
    """True when the CRS's authority axis order is not east/north (first axis is a northing or a latitude)."""
    out = subprocess.run(["projinfo", "-q", "-o", "PROJJSON", defn], capture_output=True, text=True).stdout
    try:
        axes = json.loads(out)["coordinate_system"]["axis"]
    except Exception:
        return False
    name = axes[0]["name"].lower()
    return "northing" in name or "latitude" in name  # 3031's first axis is "Easting" although it points north


def num(s):
    try:
        v = float(s)
    except ValueError:
        return None
    return v if abs(v) < 1e15 else None


def legacy_args(src, dst):
    """cs2cs's legacy `+to` syntax, used when the target is a bare PROJ string (its CRS mode mishandles those, e.g. +units=ft)."""
    s = {"OGC:CRS84": "+proj=longlat +datum=WGS84 +no_defs"}.get(src)
    if s is None and src.startswith("EPSG:"):
        s = f"+init=epsg:{src[5:]}"
    return (s.split() + ["+to"] + dst.split()) if s else None


def convert(src, dst, pts, src_swap, dst_swap):
    legacy = legacy_args(src, dst) if dst.startswith("+") else None
    if legacy:
        src_swap = False  # +init=epsg:… is always east/north, whatever the authority's axis order
    lines = "\n".join((f"{y!r} {x!r}" if src_swap else f"{x!r} {y!r}") for x, y in pts)
    out = run(["cs2cs", "-d", "12", "-f", "%.9f"] + legacy, lines + "\n") if legacy else run(["cs2cs", "-d", "12", "-f", "%.9f", src, dst], lines + "\n")
    res = []
    for line in out:
        t = line.replace("\t", " ").split()
        a, b = (num(t[0]), num(t[1])) if len(t) >= 2 else (None, None)
        if a is None or b is None:
            res.append(None)
        else:
            res.append([b, a] if dst_swap else [a, b])
    return res


def main():
    version = subprocess.run(["cs2cs"], capture_output=True, text=True).stderr.splitlines()[0].replace("Rel. ", "").split(",")[0]
    swap = {k: swapped(v) for k, v in ZOO.items()}
    geo = {k: convert("OGC:CRS84", v, POINTS, False, swap[k]) for k, v in ZOO.items()}
    pairs = []
    for a, b in PAIRS:
        ins = [p for p in geo[a] if p is not None]
        pairs.append({"from": a, "to": b, "in": ins, "out": convert(ZOO[a], ZOO[b], ins, swap[a], swap[b])})
    out = {"proj_version": version, "crs": ZOO, "axis_swapped": swap, "points": POINTS, "geo": geo, "pairs": pairs}
    path = os.path.join(os.path.dirname(__file__), "..", "crates", "engine", "tests", "fixtures", "crs", "reference.json")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        json.dump(out, f, indent=1)
        f.write("\n")
    n = sum(1 for k in geo for v in geo[k] if v is not None) + sum(1 for p in pairs for v in p["out"] if v is not None)
    print(f"PROJ {version}: {len(ZOO)} CRSs, {len(POINTS)} points, {len(pairs)} pairs, {n} reference values -> {os.path.normpath(path)}")


if __name__ == "__main__":
    main()
