//! Network analysis core (Wave 1, package T5): a routable graph built from line features, snapping of stops onto it, Dijkstra's algorithm
//! (Dijkstra 1959, with a binary heap) and the paths and reachable stretches it finds. Written directly from the textbook algorithm, with no
//! graph library: the only graph algorithms needed are Dijkstra and connected components (union-find), a few dozen lines each.
//!
//! Graph building: every line feature (part) is one edge between its first and last vertex, and edges meet where their end points are within
//! the tolerance (in metres, geodesic). Lines are NOT noded: two lines that cross or touch in the middle without an end point there are not
//! connected, so the layer must be split at intersections (as TIGER, National Map and Overture road segments are). Everything is held in
//! longitude/latitude on WGS 84, so lengths are geodesic (Karney 2013) whatever the layer's coordinate system; results are transformed back.
//! Edge cost is the geodesic length in metres, or the travel time in minutes from a speed per edge. A one-way field limits the direction.
//! Stops are snapped to the nearest point of the nearest edge (not just to an end point); the edge is split there for the search, so a route
//! may start and end in the middle of a segment, and the snap distance is reported for every stop.

use crate::geomkit;
use anyhow::{anyhow, Result};
use geographiclib_rs::{Geodesic, InverseGeodesic};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap, HashMap};

pub struct Edge {
    pub a: usize,
    pub b: usize,
    /// Vertices (lon, lat) in the feature's digitised direction and the geodesic length (m) up to each.
    pub pts: Vec<(f64, f64)>,
    pub cum: Vec<f64>,
    pub len: f64,
    /// Cost of travelling the whole edge from a to b and from b to a (infinite when not allowed).
    pub cost_ab: f64,
    pub cost_ba: f64,
    /// Row number (1-based) of the feature in the input layer.
    pub rn: i64,
}

pub struct NetRow {
    pub rn: i64,
    pub wkt: String,
    pub speed: Option<f64>,
    pub oneway: Option<String>,
}

pub struct Network {
    pub nodes: Vec<(f64, f64)>,
    pub edges: Vec<Edge>,
    pub comp: Vec<usize>,
    pub ncomp: usize,
    /// Features that were not used, with the reason: (row number, reason).
    pub skipped: Vec<(i64, &'static str)>,
}

struct Dsu(Vec<usize>);
impl Dsu {
    fn find(&mut self, mut x: usize) -> usize {
        while self.0[x] != x {
            self.0[x] = self.0[self.0[x]];
            x = self.0[x];
        }
        x
    }
    fn union(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            self.0[a.max(b)] = a.min(b);
        }
    }
}

/// -1 reverse only, 0 both, 1 forward only.
fn oneway_code(v: &Option<String>) -> i8 {
    match v.as_deref().map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("ft" | "f" | "1" | "yes" | "y" | "true" | "forward" | "fwd") => 1,
        Some("tf" | "t" | "r" | "-1" | "reverse" | "rev" | "backward") => -1,
        _ => 0,
    }
}

/// `speed_ms` turns the speed value of a row into metres per second; None means cost = length.
pub fn build(rows: Vec<NetRow>, tol_m: f64, speed_ms: Option<f64>) -> Result<Network> {
    let geod = Geodesic::wgs84();
    let mut skipped = vec![];
    // (rn, polyline, speed, oneway) per part.
    let mut parts: Vec<(i64, Vec<(f64, f64)>, Option<f64>, i8)> = vec![];
    for r in rows {
        match geomkit::parse_wkt(&r.wkt) {
            Ok(ps) if !ps.is_empty() => {
                for p in ps {
                    if let Some(l) = p.into_iter().next().filter(|l| l.len() >= 2) {
                        parts.push((r.rn, l, r.speed, oneway_code(&r.oneway)));
                    } else {
                        skipped.push((r.rn, "a line with fewer than 2 points"));
                    }
                }
            }
            _ => skipped.push((r.rn, "an empty or unreadable geometry")),
        }
    }
    // End points, clustered within the tolerance.
    let ends: Vec<(f64, f64)> = parts.iter().flat_map(|p| [p.1[0], *p.1.last().unwrap()]).collect();
    let space = geomkit::Space::new(geomkit::metric("EPSG:4326")?, ends.clone());
    let grid = space.grid(tol_m.max(0.01));
    let mut dsu = Dsu((0..ends.len()).collect());
    for i in 0..ends.len() {
        if i % 4096 == 0 && crate::progress::run_cancelled() {
            anyhow::bail!("{}", crate::progress::CANCELLED);
        }
        for (j, _) in space.within(&grid, i, tol_m) {
            dsu.union(i, j);
        }
    }
    let mut node_of: HashMap<usize, usize> = HashMap::new();
    let mut nodes: Vec<(f64, f64)> = vec![];
    let end_node: Vec<usize> = (0..ends.len())
        .map(|i| {
            let root = dsu.find(i);
            *node_of.entry(root).or_insert_with(|| {
                nodes.push(ends[root]);
                nodes.len() - 1
            })
        })
        .collect();
    let mut edges = vec![];
    for (k, (rn, pts, speed, ow)) in parts.into_iter().enumerate() {
        let mut cum = vec![0.0];
        for w in pts.windows(2) {
            let d: f64 = geod.inverse(w[0].1, w[0].0, w[1].1, w[1].0);
            cum.push(cum.last().unwrap() + d);
        }
        let len = *cum.last().unwrap();
        if !(len > 0.0) {
            skipped.push((rn, "a line of zero length"));
            continue;
        }
        let cost = match speed_ms {
            None => len,
            Some(to_ms) => match speed.filter(|s| s.is_finite() && *s > 0.0) {
                Some(s) => len / (s * to_ms) / 60.0,
                None => {
                    skipped.push((rn, "no speed greater than 0 on the line"));
                    continue;
                }
            },
        };
        edges.push(Edge {
            a: end_node[2 * k],
            b: end_node[2 * k + 1],
            pts,
            cum,
            len,
            cost_ab: if ow >= 0 { cost } else { f64::INFINITY },
            cost_ba: if ow <= 0 { cost } else { f64::INFINITY },
            rn,
        });
    }
    let mut d = Dsu((0..nodes.len()).collect());
    for e in &edges {
        d.union(e.a, e.b);
    }
    let mut ids: BTreeMap<usize, usize> = BTreeMap::new();
    let comp: Vec<usize> = (0..nodes.len()).map(|i| { let r = d.find(i); let n = ids.len(); *ids.entry(r).or_insert(n) }).collect();
    let ncomp = ids.len();
    if edges.is_empty() {
        return Err(anyhow!("No line of the network layer can be used{}", if speed_ms.is_some() { " (check the speed field)" } else { "" }));
    }
    Ok(Network { nodes, edges, comp, ncomp, skipped })
}

/// Sizes (edges) of the components, largest first.
pub fn component_sizes(n: &Network) -> Vec<usize> {
    let mut s = vec![0usize; n.ncomp];
    for e in &n.edges {
        s[n.comp[e.a]] += 1;
    }
    s.sort_by(|a, b| b.cmp(a));
    s
}

/// A stop placed on the network.
#[derive(Clone, Debug)]
pub struct Snap {
    pub edge: usize,
    /// Fraction of the edge's length from its first vertex.
    pub frac: f64,
    pub at: (f64, f64),
    /// Geodesic distance from the stop to the snapped point (m).
    pub dist_m: f64,
}

pub struct SegIndex {
    cs: f64,
    cells: HashMap<(i64, i64), Vec<(u32, u32)>>,
}

impl SegIndex {
    pub fn new(net: &Network, cell_deg: f64) -> SegIndex {
        let cs = cell_deg.max(1e-5);
        let mut cells: HashMap<(i64, i64), Vec<(u32, u32)>> = HashMap::new();
        for (ei, e) in net.edges.iter().enumerate() {
            for (si, w) in e.pts.windows(2).enumerate() {
                let (x0, x1) = ((w[0].0.min(w[1].0) / cs).floor() as i64, (w[0].0.max(w[1].0) / cs).floor() as i64);
                let (y0, y1) = ((w[0].1.min(w[1].1) / cs).floor() as i64, (w[0].1.max(w[1].1) / cs).floor() as i64);
                for x in x0..=x1.min(x0 + 400) {
                    for y in y0..=y1.min(y0 + 400) {
                        cells.entry((x, y)).or_default().push((ei as u32, si as u32));
                    }
                }
            }
        }
        SegIndex { cs, cells }
    }

    /// The nearest point of the network within `max_m` of (lon, lat), or None.
    pub fn snap(&self, net: &Network, lon: f64, lat: f64, max_m: f64) -> Option<Snap> {
        let m_lat = 111_132.0;
        let m_lon = (111_320.0 * lat.to_radians().cos()).max(1.0);
        let (rx, ry) = ((max_m / m_lon / self.cs).ceil() as i64 + 1, (max_m / m_lat / self.cs).ceil() as i64 + 1);
        let (cx, cy) = ((lon / self.cs).floor() as i64, (lat / self.cs).floor() as i64);
        let mut best: Option<(f64, usize, usize, f64)> = None; // approx distance, edge, segment, t
        for x in cx - rx..=cx + rx {
            for y in cy - ry..=cy + ry {
                let Some(v) = self.cells.get(&(x, y)) else { continue };
                for &(ei, si) in v {
                    let e = &net.edges[ei as usize];
                    let (a, b) = (e.pts[si as usize], e.pts[si as usize + 1]);
                    let (ax, ay, bx, by) = ((a.0 - lon) * m_lon, (a.1 - lat) * m_lat, (b.0 - lon) * m_lon, (b.1 - lat) * m_lat);
                    let (dx, dy) = (bx - ax, by - ay);
                    let l2 = dx * dx + dy * dy;
                    let t = if l2 == 0.0 { 0.0 } else { ((-ax * dx - ay * dy) / l2).clamp(0.0, 1.0) };
                    let d = (ax + t * dx).hypot(ay + t * dy);
                    if best.map_or(true, |b| d < b.0) {
                        best = Some((d, ei as usize, si as usize, t));
                    }
                }
            }
        }
        let (_, ei, si, t) = best?;
        let e = &net.edges[ei];
        let (a, b) = (e.pts[si], e.pts[si + 1]);
        let at = (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
        let dist: f64 = Geodesic::wgs84().inverse(lat, lon, at.1, at.0);
        if dist > max_m {
            return None;
        }
        let along = e.cum[si] + t * (e.cum[si + 1] - e.cum[si]);
        Some(Snap { edge: ei, frac: (along / e.len).clamp(0.0, 1.0), at, dist_m: dist })
    }
}

#[derive(Clone)]
pub struct Arc {
    pub to: usize,
    pub cost: f64,
    pub edge: usize,
    /// Fractions of the edge covered, in the direction travelled (from f0 to f1).
    pub f0: f64,
    pub f1: f64,
}

pub struct Graph {
    pub adj: Vec<Vec<Arc>>,
}

/// The network with each stop's edge split at its snapped position. Stop `k` is node `net.nodes.len() + k`.
pub fn graph_with_stops(net: &Network, stops: &[Option<Snap>]) -> Graph {
    let mut adj: Vec<Vec<Arc>> = vec![vec![]; net.nodes.len() + stops.len()];
    let mut on_edge: BTreeMap<usize, Vec<(f64, usize)>> = BTreeMap::new();
    for (k, s) in stops.iter().enumerate() {
        if let Some(s) = s {
            on_edge.entry(s.edge).or_default().push((s.frac, net.nodes.len() + k));
        }
    }
    for (ei, e) in net.edges.iter().enumerate() {
        let mut chain: Vec<(f64, usize)> = vec![(0.0, e.a)];
        if let Some(v) = on_edge.get_mut(&ei) {
            v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            chain.extend(v.iter().cloned());
        }
        chain.push((1.0, e.b));
        for w in chain.windows(2) {
            let ((f0, u), (f1, v)) = (w[0], w[1]);
            if e.cost_ab.is_finite() {
                adj[u].push(Arc { to: v, cost: (f1 - f0) * e.cost_ab, edge: ei, f0, f1 });
            }
            if e.cost_ba.is_finite() {
                adj[v].push(Arc { to: u, cost: (f1 - f0) * e.cost_ba, edge: ei, f0: f1, f1: f0 });
            }
        }
    }
    Graph { adj }
}

#[derive(PartialEq)]
struct Item(f64, usize);
impl Eq for Item {}
impl Ord for Item {
    fn cmp(&self, o: &Self) -> Ordering {
        o.0.partial_cmp(&self.0).unwrap_or(Ordering::Equal).then(o.1.cmp(&self.1))
    }
}
impl PartialOrd for Item {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

pub struct Search {
    pub dist: Vec<f64>,
    /// For each node, (previous node, index of the arc in the previous node's list).
    pub prev: Vec<Option<(usize, usize)>>,
}

/// The same graph with every arc turned round (search it from a stop to find the cost of arriving there from everywhere).
pub fn reversed(g: &Graph) -> Graph {
    let mut adj: Vec<Vec<Arc>> = vec![vec![]; g.adj.len()];
    for (u, v) in g.adj.iter().enumerate() {
        for a in v {
            adj[a.to].push(Arc { to: u, cost: a.cost, edge: a.edge, f0: a.f1, f1: a.f0 });
        }
    }
    Graph { adj }
}

/// Dijkstra from `src`. Stops early once `need` of the nodes in `targets` are settled (`usize::MAX` = all of them); never expands past `cutoff`.
pub fn dijkstra(g: &Graph, src: usize, targets: &[usize], need: usize, cutoff: f64) -> Search {
    let n = g.adj.len();
    let mut dist = vec![f64::INFINITY; n];
    let mut prev = vec![None; n];
    let mut done = vec![false; n];
    let mut left: std::collections::HashSet<usize> = targets.iter().cloned().collect();
    let need = need.min(left.len());
    let mut found = 0usize;
    let mut heap = BinaryHeap::new();
    dist[src] = 0.0;
    heap.push(Item(0.0, src));
    while let Some(Item(d, u)) = heap.pop() {
        if done[u] {
            continue;
        }
        done[u] = true;
        if left.remove(&u) {
            found += 1;
            if found >= need {
                break;
            }
        }
        for (ai, a) in g.adj[u].iter().enumerate() {
            let nd = d + a.cost;
            if nd < dist[a.to] && nd <= cutoff {
                dist[a.to] = nd;
                prev[a.to] = Some((u, ai));
                heap.push(Item(nd, a.to));
            }
        }
    }
    Search { dist, prev }
}

/// Arcs along the path to `to` (in travel order).
pub fn path_arcs<'a>(g: &'a Graph, s: &Search, to: usize) -> Vec<&'a Arc> {
    let mut v = vec![];
    let mut cur = to;
    while let Some((p, ai)) = s.prev[cur] {
        v.push(&g.adj[p][ai]);
        cur = p;
    }
    v.reverse();
    v
}

/// Interpolated point at distance `d` (m) along an edge.
fn point_at(e: &Edge, d: f64) -> (f64, f64) {
    let k = e.cum.partition_point(|c| *c <= d).saturating_sub(1).min(e.pts.len() - 2);
    let span = e.cum[k + 1] - e.cum[k];
    let t = if span > 0.0 { ((d - e.cum[k]) / span).clamp(0.0, 1.0) } else { 0.0 };
    (e.pts[k].0 + t * (e.pts[k + 1].0 - e.pts[k].0), e.pts[k].1 + t * (e.pts[k + 1].1 - e.pts[k].1))
}

/// The part of an edge between two fractions, in the order of travel (f0 to f1).
pub fn slice(e: &Edge, f0: f64, f1: f64) -> Vec<(f64, f64)> {
    let (lo, hi) = (f0.min(f1) * e.len, f0.max(f1) * e.len);
    let mut v = vec![point_at(e, lo)];
    for (k, c) in e.cum.iter().enumerate() {
        if *c > lo && *c < hi {
            v.push(e.pts[k]);
        }
    }
    v.push(point_at(e, hi));
    if f0 > f1 {
        v.reverse();
    }
    v
}

/// The path to `to` as one polyline (lon, lat) and its length in metres.
pub fn path_geometry(net: &Network, g: &Graph, s: &Search, to: usize) -> (Vec<(f64, f64)>, f64) {
    let mut pts: Vec<(f64, f64)> = vec![];
    let mut len = 0.0;
    for a in path_arcs(g, s, to) {
        let e = &net.edges[a.edge];
        len += (a.f1 - a.f0).abs() * e.len;
        let part = slice(e, a.f0, a.f1);
        if pts.is_empty() {
            pts = part;
        } else {
            pts.extend(part.into_iter().skip(1));
        }
    }
    (pts, len)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small grid of roads near the equator, 0.001 degrees (about 111 m) apart:
    ///   A --- B --- C        A-B, B-C along the top, A-D and B-E and C-F down, D-E and E-F along the bottom.
    ///   |     |     |        B-E is one-way (B to E); G-H is a separate piece.
    ///   D --- E --- F
    fn rows() -> Vec<NetRow> {
        let l = |rn: i64, a: (f64, f64), b: (f64, f64), speed: f64, ow: Option<&str>| NetRow { rn, wkt: format!("LINESTRING({} {}, {} {})", a.0, a.1, b.0, b.1), speed: Some(speed), oneway: ow.map(String::from) };
        let (a, b, c, d, e, f) = ((0.0, 0.001), (0.001, 0.001), (0.002, 0.001), (0.0, 0.0), (0.001, 0.0), (0.002, 0.0));
        vec![l(1, a, b, 10.0, None), l(2, b, c, 10.0, None), l(3, a, d, 10.0, None), l(4, b, e, 10.0, Some("FT")), l(5, c, f, 10.0, None), l(6, d, e, 10.0, None), l(7, e, f, 10.0, None), l(8, (0.01, 0.0), (0.011, 0.0), 10.0, None)]
    }

    #[test]
    fn components_one_way_and_shortest_path_on_a_hand_graph() {
        let net = build(rows(), 1.0, None).unwrap();
        assert_eq!(net.edges.len(), 8);
        assert_eq!(net.ncomp, 2);
        assert_eq!(component_sizes(&net), vec![7, 1]);
        let unit: f64 = Geodesic::wgs84().inverse(0.0, 0.0, 0.0, 0.001);
        let g = graph_with_stops(&net, &[]);
        // From A (node 0) to F: A-B-C-F or A-D-E-F, both 3 edges; the one-way B-E cannot be used from E to B.
        let s = dijkstra(&g, 0, &[], usize::MAX, f64::INFINITY);
        let f_node = net.nodes.iter().position(|n| (n.0 - 0.002).abs() < 1e-9 && n.1.abs() < 1e-9).unwrap();
        assert!(s.dist[f_node] > 2.9 * unit && s.dist[f_node] < 4.1 * unit);
        let e_node = net.nodes.iter().position(|n| (n.0 - 0.001).abs() < 1e-9 && n.1.abs() < 1e-9).unwrap();
        let b_node = net.nodes.iter().position(|n| (n.0 - 0.001).abs() < 1e-9 && (n.1 - 0.001).abs() < 1e-9).unwrap();
        // Forward B to E: one edge. Backward E to B: must go around via D-A-B or F-C-B (more than 3 edges).
        let be: f64 = Geodesic::wgs84().inverse(0.001, 0.001, 0.0, 0.001);
        assert!((dijkstra(&g, b_node, &[], usize::MAX, f64::INFINITY).dist[e_node] - be).abs() < 1e-6);
        assert!(dijkstra(&g, e_node, &[], usize::MAX, f64::INFINITY).dist[b_node] > 2.9 * unit);
    }

    #[test]
    fn a_stop_in_the_middle_of_an_edge_splits_it() {
        let net = build(rows(), 1.0, None).unwrap();
        let idx = SegIndex::new(&net, 0.005);
        // A point 20 m north of the middle of A-B (edge 0): snap 20 m, halfway along.
        let s = idx.snap(&net, 0.0005, 0.001 + 20.0 / 110_574.0, 500.0).unwrap();
        assert_eq!(net.edges[s.edge].rn, 1);
        assert!((s.frac - 0.5).abs() < 1e-3 && (s.dist_m - 20.0).abs() < 0.2, "{s:?}");
        // Farther than the maximum snap distance: None.
        assert!(idx.snap(&net, 0.0005, 0.01, 500.0).is_none());
        // Two stops on the same edge are joined directly along it, not via the end points.
        let s1 = Snap { edge: s.edge, frac: 0.25, at: (0.0, 0.0), dist_m: 0.0 };
        let s2 = Snap { edge: s.edge, frac: 0.75, at: (0.0, 0.0), dist_m: 0.0 };
        let g = graph_with_stops(&net, &[Some(s1), Some(s2)]);
        let n0 = net.nodes.len();
        let r = dijkstra(&g, n0, &[n0 + 1], usize::MAX, f64::INFINITY);
        assert!((r.dist[n0 + 1] - 0.5 * net.edges[s.edge].len).abs() < 1e-6);
    }
}
