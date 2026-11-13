import json, numpy as np, itertools
import os
p = os.path.join(os.path.dirname(os.path.abspath(__file__)), "columbus.geojson")
fc = json.load(open(p))
feats = fc["features"]
n = len(feats)
y = np.array([f["properties"]["CRIME"] for f in feats]); inc = np.array([f["properties"]["INC"] for f in feats]); hov = np.array([f["properties"]["HOVAL"] for f in feats])
def verts(f, nd):
    g = f["geometry"]; polys = g["coordinates"] if g["type"] == "MultiPolygon" else [g["coordinates"]]
    return {(round(x, nd), round(yy, nd)) for poly in polys for ring in poly for x, yy in ring}
def build(minshared, nd):
    V = [verts(f, nd) for f in feats]
    A = np.zeros((n, n))
    for i, j in itertools.combinations(range(n), 2):
        if len(V[i] & V[j]) >= minshared:
            A[i, j] = A[j, i] = 1
    return A
X = np.column_stack([np.ones(n), inc, hov])
def ml(A):
    W = A / A.sum(1, keepdims=True)
    I = np.eye(n)
    ev = np.linalg.eigvals(W).real
    z = y - y.mean(); I_m = n / W.sum() * (z @ W @ z) / (z @ z)
    def ols(Xm, ym):
        b = np.linalg.lstsq(Xm, ym, rcond=None)[0]; e = ym - Xm @ b; return b, e
    def ll_lag(r):
        b, e = ols(X, y - r * (W @ y)); s2 = e @ e / n
        return -n / 2 * np.log(2 * np.pi * s2) + np.linalg.slogdet(I - r * W)[1] - n / 2
    def ll_err(l):
        b, e = ols(X - l * (W @ X), y - l * (W @ y)); s2 = e @ e / n
        return -n / 2 * np.log(2 * np.pi * s2) + np.linalg.slogdet(I - l * W)[1] - n / 2
    out = {}
    for name, f in (("lag", ll_lag), ("err", ll_err)):
        grid = np.linspace(-0.99, 0.99, 3961); v = [f(g) for g in grid]; k = int(np.argmax(v))
        lo, hi = grid[max(k - 1, 0)], grid[min(k + 1, len(grid) - 1)]
        for _ in range(60):
            a, b = lo + (hi - lo) / 3, hi - (hi - lo) / 3
            if f(a) < f(b): lo = a
            else: hi = b
        r = (lo + hi) / 2
        if name == "lag": bb, _ = ols(X, y - r * (W @ y))
        else: bb, _ = ols(X - r * (W @ X), y - r * (W @ y))
        out[name] = (r, bb)
    return A.sum() , I_m, out
for label, ms, nd in (("queen (>=1 shared vertex)", 1, 6), ("rook (>=2 shared vertices)", 2, 6)):
    links, mI, out = ml(build(ms, nd))
    print(label, "links", int(links), "Moran", round(mI, 5))
    for k, (r, b) in out.items():
        print("  ", k, round(r, 5), np.round(b, 5))
