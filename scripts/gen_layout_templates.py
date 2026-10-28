#!/usr/bin/env python3
"""Write the bundled layout templates (crates/engine/assets/layout_templates/*.json).

One map page per size and orientation: title, map frame, legend (top right), north arrow (top left),
scale bar with its 1:N label (bottom left) and the sources line under the frame. Map frames carry no
coordinate system or centre: "New Layout from template" fills them from the map view.
Run: python3 scripts/gen_layout_templates.py   (then commit the changed files)
"""
import json, os

SIZES = {"Letter": (215.9, 279.4), "Legal": (215.9, 355.6), "Tabloid": (279.4, 431.8), "A4": (210, 297), "A3": (297, 420)}
M = 12.7
TITLE_H = 12
GAP = 2
SOURCES_H = 8
INSET = 3


def r(v):
    return round(v, 1)


def base(i, x, y, w, h):
    return {"id": i, "x": r(x), "y": r(y), "w": r(w), "h": r(h), "rotation": 0, "locked": False, "visible": True}


def template(size, orientation):
    w0, h0 = SIZES[size]
    w, h = (h0, w0) if orientation == "landscape" else (w0, h0)
    fx, fy = M, M + TITLE_H + GAP
    fw = w - 2 * M
    fh = h - fy - M - SOURCES_H - 1.5
    legend_w, legend_h = (46, 80) if fh > 120 else (40, min(60, fh - 2 * INSET))
    els = [
        {"kind": "text", **base("t1", M, M, fw, TITLE_H), "text": "{layout_name}", "font": "sans-bold", "size": 18, "align": "center", "color": "#000000"},
        {"kind": "map_frame", **base("f1", fx, fy, fw, fh), "crs": "", "center": [0, 0], "scale": 1000000, "layers": "map", "basemap": "none", "graticule": None, "grid": None, "border": {"width": 0.35, "color": "#000000"}},
        {"kind": "legend", **base("lg1", fx + fw - legend_w - INSET, fy + INSET, legend_w, legend_h), "frame": "f1", "title": "Legend", "columns": 1},
        {"kind": "north_arrow", **base("na1", fx + INSET, fy + INSET, 12, 18), "frame": "f1", "style": "simple"},
        {"kind": "scale_bar", **base("sb1", fx + INSET, fy + fh - 14 - INSET, 55, 14), "frame": "f1", "units": "km", "segments": 4, "ratio": True},
        {"kind": "sources", **base("src1", fx, fy + fh + 1.5, fw, SOURCES_H), "frame": "f1", "size": 6},
    ]
    return {
        "id": f"template-{size}-{orientation}",
        "name": f"Map — {size} {orientation}",
        "version": 1,
        "body": "earth",
        "page": {"width": r(w), "height": r(h), "units": "mm", "margin": M, "background": "#ffffff"},
        "elements": els,
        "series": None,
    }


if __name__ == "__main__":
    out = os.path.join(os.path.dirname(__file__), "..", "crates", "engine", "assets", "layout_templates")
    os.makedirs(out, exist_ok=True)
    for size in SIZES:
        for o in ("landscape", "portrait"):
            with open(os.path.join(out, f"{size.lower()}-{o}.json"), "w") as f:
                json.dump(template(size, o), f, indent=2, ensure_ascii=False)
                f.write("\n")
    print("wrote", len(SIZES) * 2, "templates to", os.path.normpath(out))
