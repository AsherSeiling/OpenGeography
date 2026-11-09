//! Network analysis nodes (Wave 1, package T5): Shortest Route, Service Area, Closest Facility and OD Cost Matrix on any line layer (TIGER roads,
//! National Map transportation, Overture segments). The algorithms and their assumptions (end-point matching, no noding, geodesic lengths,
//! snapping into the middle of an edge, Dijkstra) are described in `network.rs`.

use super::centrography::sqlnum;
use super::*;
use crate::geomkit;
use crate::network as nw;

struct Cfg {
    time: bool,
    speed_to_ms: Option<f64>,
    tol: f64,
    max_snap: f64,
    speed_field: Option<String>,
    oneway_field: Option<String>,
}

impl Cfg {
    fn unit(&self) -> &'static str {
        if self.time { "min" } else { "m" }
    }
}

fn net_params() -> Vec<Param> {
    vec![
        Param::new("cost", "Cost", ParamKind::Enum).default("length").options(&[("length", "Length (metres, geodesic)"), ("time", "Travel time (minutes, from a speed field)")]),
        Param::new("speed_field", "Speed field (for travel time)", ParamKind::Field).field_of("net").optional(),
        Param::new("speed_unit", "Speed unit", ParamKind::Enum).default("kmh").options(&[("kmh", "km per hour"), ("mph", "miles per hour"), ("ms", "metres per second")]),
        Param::new("oneway_field", "One-way field (FT, TF or empty = both ways)", ParamKind::Field).field_of("net").optional(),
        Param::new("tolerance", "Join line ends within (metres)", ParamKind::Number).default(1),
        Param::new("max_snap", "Snap stops within (metres)", ParamKind::Number).default(500),
    ]
}

macro_rules! net_help {
    () => {
        "The network is any line layer whose lines are split at intersections (TIGER, National Map and Overture road segments are); lines are joined where their end points are within the tolerance, and a line that crosses another without an end point there is not connected to it. A one-way field marks the direction: FT (also F, 1, yes, true) = only the way the line is drawn, TF (also R, -1) = only against it, anything else = both ways. Stops are snapped to the nearest point of the nearest line within the snap distance and every snap distance is reported; a stop farther away, or on a piece of the network not connected to the others, is reported, not dropped silently. Lengths are geodesic on the WGS 84 ellipsoid whatever the coordinate system of the layers; travel time is length divided by the speed on each line."
    };
}

fn cfg(p: &Params) -> Result<Cfg> {
    let time = p.str("cost").as_deref() == Some("time");
    let speed_field = p.str("speed_field");
    if time && speed_field.is_none() {
        bail!("Travel time needs a speed field. Choose one, or use length as the cost.");
    }
    let speed_to_ms = time.then(|| match p.str("speed_unit").as_deref() { Some("mph") => 0.44704, Some("ms") => 1.0, _ => 1.0 / 3.6 });
    let (tol, max_snap) = (p.f64("tolerance").unwrap_or(1.0), p.f64("max_snap").unwrap_or(500.0));
    if !(tol.is_finite() && tol >= 0.0) {
        bail!("'Join line ends within' must be a number of metres, 0 or more");
    }
    if !(max_snap.is_finite() && max_snap > 0.0) {
        bail!("'Snap stops within' must be a number of metres greater than 0");
    }
    Ok(Cfg { time, speed_to_ms, tol, max_snap, speed_field, oneway_field: p.str("oneway_field") })
}

fn read_net(ctx: &Ctx, rel: &Rel, c: &Cfg) -> Result<nw::Network> {
    need_crs(rel)?;
    if !rel.has_geom || ctx.dimension(rel)? != 1 {
        bail!("The network must be a line layer");
    }
    let g = ctx.geom_in(rel, "a", "EPSG:4326")?;
    let sp = c.speed_field.as_ref().map(|f| format!("TRY_CAST(a.{}::VARCHAR AS DOUBLE)", ident(f))).unwrap_or_else(|| "NULL".into());
    let ow = c.oneway_field.as_ref().map(|f| format!("a.{}::VARCHAR", ident(f))).unwrap_or_else(|| "NULL".into());
    let mut stmt = ctx.conn.prepare(&format!(
        "SELECT rn, ST_AsText(g), sp, ow FROM (SELECT a.rn, {g} AS g, {sp} AS sp, {ow} AS ow FROM (SELECT row_number() OVER () AS rn, * FROM {}) a WHERE a.geom IS NOT NULL AND NOT ST_IsEmpty(a.geom)) WHERE g IS NOT NULL",
        ident(&rel.table)
    ))?;
    let rows: Vec<nw::NetRow> = stmt
        .query_map([], |r| Ok(nw::NetRow { rn: r.get(0)?, wkt: r.get(1)?, speed: r.get(2)?, oneway: r.get(3)? }))?
        .collect::<Result<_, _>>()?;
    let net = nw::build(rows, c.tol, c.speed_to_ms)?;
    let mut notes = vec![];
    let mut by_reason: std::collections::BTreeMap<&str, Vec<i64>> = Default::default();
    for (rn, why) in &net.skipped {
        by_reason.entry(why).or_default().push(*rn);
    }
    for (why, ids) in &by_reason {
        notes.push(format!("{} feature(s) were not used because of {why} (row numbers: {}{}).", ids.len(), ids.iter().take(10).map(|i| i.to_string()).collect::<Vec<_>>().join(", "), if ids.len() > 10 { ", …" } else { "" }));
    }
    let sizes = nw::component_sizes(&net);
    notes.push(format!("Network: {} segments joined at {} nodes into {} connected piece(s){}.", net.edges.len(), net.nodes.len(), net.ncomp, if sizes.len() > 1 { format!(" (segments per piece: {}{})", sizes.iter().take(5).map(|s| s.to_string()).collect::<Vec<_>>().join(", "), if sizes.len() > 5 { ", …" } else { "" }) } else { String::new() }));
    if net.ncomp > 1 {
        notes.push("Stops on different pieces cannot reach each other; those pairs are reported as unreachable. Check that the lines are split and joined where they should be (the tolerance for joining line ends is a setting).".into());
    }
    for n in notes {
        ctx.add_note(format!("Network analysis: {n}"));
    }
    Ok(net)
}

struct Stop {
    label: String,
    lon: f64,
    lat: f64,
}

fn read_stops(ctx: &Ctx, rel: &Rel, id_field: Option<&str>, what: &str) -> Result<Vec<Stop>> {
    need_crs(rel)?;
    if !rel.has_geom {
        bail!("{what} must be a layer with geometry");
    }
    let g = ctx.geom_in(rel, "a", "EPSG:4326")?;
    let id = id_field.map(|f| format!("a.{}::VARCHAR", ident(f))).unwrap_or_else(|| "CAST(a.rn AS VARCHAR)".into());
    let mut stmt = ctx.conn.prepare(&format!(
        "SELECT lbl, ST_X(c), ST_Y(c) FROM (SELECT {id} AS lbl, ST_Centroid({g}) AS c FROM (SELECT row_number() OVER () AS rn, * FROM {}) a WHERE a.geom IS NOT NULL AND NOT ST_IsEmpty(a.geom)) WHERE c IS NOT NULL",
        ident(&rel.table)
    ))?;
    let v: Vec<Stop> = stmt.query_map([], |r| Ok(Stop { label: r.get::<_, Option<String>>(0)?.unwrap_or_default(), lon: r.get(1)?, lat: r.get(2)? }))?.collect::<Result<_, _>>()?;
    let total: i64 = ctx.conn.query_row(&format!("SELECT count(*) FROM {}", ident(&rel.table)), [], |r| r.get(0))?;
    if total > v.len() as i64 {
        ctx.add_note(format!("Network analysis: {} feature(s) of {what} had no usable position and were left out.", total - v.len() as i64));
    }
    if v.is_empty() {
        bail!("{what} has no features with a position");
    }
    Ok(v)
}

fn snap_all(ctx: &Ctx, net: &nw::Network, stops: &[Stop], max: f64, what: &str) -> Vec<Option<nw::Snap>> {
    let idx = nw::SegIndex::new(net, (max / 111_000.0).clamp(1e-4, 0.5));
    let snaps: Vec<Option<nw::Snap>> = stops.iter().map(|s| idx.snap(net, s.lon, s.lat, max)).collect();
    let far: Vec<&str> = stops.iter().zip(&snaps).filter(|(_, s)| s.is_none()).map(|(s, _)| s.label.as_str()).collect();
    if !far.is_empty() {
        ctx.add_note(format!("Network analysis: {} of {} {what} are more than {max} m from any line and were not snapped (status 'not on the network'): {}{}.", far.len(), stops.len(), far.iter().take(10).cloned().collect::<Vec<_>>().join(", "), if far.len() > 10 { ", …" } else { "" }));
    }
    let mut sizes = vec![0usize; net.ncomp];
    for e in &net.edges {
        sizes[net.comp[e.a]] += 1;
    }
    if net.ncomp > 1 {
        let main = (0..net.ncomp).max_by_key(|c| sizes[*c]).unwrap();
        let off: Vec<&str> = stops.iter().zip(&snaps).filter(|(_, s)| s.as_ref().is_some_and(|s| net.comp[net.edges[s.edge].a] != main)).map(|(s, _)| s.label.as_str()).collect();
        if !off.is_empty() {
            ctx.add_note(format!("Network analysis: {} of {} {what} lie on a piece of the network that is not connected to the largest piece: {}{}.", off.len(), stops.len(), off.iter().take(10).cloned().collect::<Vec<_>>().join(", "), if off.len() > 10 { ", …" } else { "" }));
        }
    }
    snaps
}

fn line_wkt(t: &geomkit::Tx, pts: &[(f64, f64)]) -> Option<String> {
    let v: Option<Vec<(f64, f64)>> = pts.iter().map(|(x, y)| t.pt(*x, *y)).collect();
    let v = v?;
    (v.len() >= 2).then(|| format!("LINESTRING({})", v.iter().map(|(x, y)| format!("{x:?} {y:?}")).collect::<Vec<_>>().join(", ")))
}

fn geom_sql(w: Option<String>) -> String {
    w.map(|w| format!("ST_GeomFromText('{w}')")).unwrap_or_else(|| "CAST(NULL AS GEOMETRY)".into())
}

fn stop_port(id: &'static str, label: &'static str) -> Port {
    Port { id, label, kind: PortKind::Vector }
}
fn net_port() -> Port {
    stop_port("net", "Network (lines)")
}

fn id_param(id: &'static str, label: &'static str, port: &'static str) -> Param {
    Param::new(id, label, ParamKind::Field).field_of(port).optional()
}

// ---------------------------------------------------------------------------------------------------------------------------------

pub struct ShortestRoute;
impl Tool for ShortestRoute {
    fn spec(&self) -> ToolSpec {
        let mut params = vec![id_param("stop_id", "Stop name field (empty: row number)", "stops")];
        params.extend(net_params());
        ToolSpec { id: "shortest_route", name: "Shortest Route", category: "Analysis › Network", role: Role::Tool,
            description: concat!("The least-cost route through the stops in the order of the stop layer, one line per leg (stop 1 to 2, 2 to 3, …) with its cost, its length in metres and the distance each stop was snapped. Cost is length in metres or travel time in minutes (Dijkstra's algorithm). A leg that cannot be travelled (disconnected pieces, one-ways, a stop off the network) is kept as a row with an empty line and a status that says why. ", net_help!()),
            inputs: vec![net_port(), stop_port("stops", "Stops (in order)")], output: Some(PortKind::Vector), params }
    }
    fn output_fields(&self) -> OutFields {
        OutFields::from(&[]).add(NewField::new("leg", "INTEGER")).add(NewField::new("from_stop", "VARCHAR")).add(NewField::new("to_stop", "VARCHAR")).add(NewField::new("cost", "DOUBLE")).add(NewField::new("cost_unit", "VARCHAR")).add(NewField::new("length_m", "DOUBLE")).add(NewField::new("from_snap_m", "DOUBLE")).add(NewField::new("to_snap_m", "DOUBLE")).add(NewField::new("status", "VARCHAR"))
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let (nr, sr) = (&inputs["net"], &inputs["stops"]);
        let c = cfg(p)?;
        let net = read_net(ctx, nr, &c)?;
        let stops = read_stops(ctx, sr, p.str("stop_id").as_deref(), "the stops")?;
        if stops.len() < 2 {
            bail!("Shortest Route needs at least 2 stops");
        }
        let snaps = snap_all(ctx, &net, &stops, c.max_snap, "stops");
        let g = nw::graph_with_stops(&net, &snaps);
        let back = geomkit::tx("EPSG:4326", need_crs(nr)?)?;
        let n0 = net.nodes.len();
        let mut rows = vec![];
        let task = crate::progress::Task::start("Shortest route").cancellable();
        for leg in 0..stops.len() - 1 {
            task.check()?;
            let (a, b) = (leg, leg + 1);
            let (sa, sb) = (snaps[a].as_ref().map(|s| s.dist_m), snaps[b].as_ref().map(|s| s.dist_m));
            let (status, cost, len, geom) = if snaps[a].is_none() || snaps[b].is_none() {
                ("not on the network", f64::NAN, f64::NAN, None)
            } else {
                let s = nw::dijkstra(&g, n0 + a, &[n0 + b], usize::MAX, f64::INFINITY);
                if s.dist[n0 + b].is_finite() {
                    let (pts, len) = nw::path_geometry(&net, &g, &s, n0 + b);
                    ("ok", s.dist[n0 + b], len, line_wkt(&back, &pts))
                } else {
                    ("unreachable", f64::NAN, f64::NAN, None)
                }
            };
            rows.push(format!("({}, {}, {}, {}, '{}', {}, {}, {}, '{status}', {})", leg + 1, lit(&stops[a].label), lit(&stops[b].label), sqlnum(cost), c.unit(), sqlnum(len), sqlnum(sa.unwrap_or(f64::NAN)), sqlnum(sb.unwrap_or(f64::NAN)), geom_sql(geom)));
        }
        let bad = rows.iter().filter(|r| !r.contains("'ok'")).count();
        if bad > 0 {
            ctx.add_note(format!("Shortest Route: {bad} of {} legs could not be routed; they are in the result with an empty line and a status.", rows.len()));
        }
        Ok(Output::Select {
            sql: format!("SELECT * FROM (VALUES {}) t(leg, from_stop, to_stop, cost, cost_unit, length_m, from_snap_m, to_snap_m, status, geom)", rows.join(", ")),
            crs: nr.crs.clone(),
            view: false,
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------

pub struct ServiceArea;
impl Tool for ServiceArea {
    fn spec(&self) -> ToolSpec {
        let mut params = vec![
            id_param("facility_id", "Facility name field (empty: row number)", "facilities"),
            Param::new("cutoff", "Cutoff (metres, or minutes for travel time)", ParamKind::Number).default(1000),
            Param::new("output", "Result", ParamKind::Enum).default("lines").options(&[("lines", "Reachable streets (lines)"), ("polygon", "Area (polygon)")]),
            Param::new("buffer", "Polygon: buffer around the streets (metres)", ParamKind::Number).default(100),
        ];
        params.extend(net_params());
        ToolSpec { id: "service_area", name: "Service Area", category: "Analysis › Network", role: Role::Tool,
            description: concat!("Everything reachable from each facility within a cost cutoff (Dijkstra's algorithm, in the direction of travel away from the facility). The result is the reachable stretches of the network as lines, with the part of a line reached when the cutoff falls part-way along it, or, as a polygon per facility: the reachable lines buffered by the stated distance (in metres, computed in the local UTM zone of the network) and merged, so the polygon includes that buffer around the streets and is not a trimmed or convex outline. The cutoff is in metres for length and minutes for travel time. ", net_help!()),
            inputs: vec![net_port(), stop_port("facilities", "Facilities (points)")], output: Some(PortKind::Vector), params }
    }
    fn output_fields(&self) -> OutFields {
        OutFields::from(&[]).add(NewField::new("facility", "VARCHAR")).add(NewField::new("edge_row", "BIGINT")).add(NewField::new("fraction_reached", "DOUBLE")).add(NewField::new("cutoff", "DOUBLE")).add(NewField::new("cost_unit", "VARCHAR"))
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let (nr, fr) = (&inputs["net"], &inputs["facilities"]);
        let c = cfg(p)?;
        let cutoff = p.f64("cutoff").unwrap_or(f64::NAN);
        if !(cutoff.is_finite() && cutoff > 0.0) {
            bail!("'Cutoff' must be a number greater than 0 ({})", if c.time { "minutes" } else { "metres" });
        }
        let polygon = p.str("output").as_deref() == Some("polygon");
        let buf = p.f64("buffer").unwrap_or(100.0);
        if polygon && !(buf.is_finite() && buf > 0.0) {
            bail!("'Polygon: buffer around the streets' must be a number of metres greater than 0");
        }
        let net = read_net(ctx, nr, &c)?;
        let fac = read_stops(ctx, fr, p.str("facility_id").as_deref(), "the facilities")?;
        let snaps = snap_all(ctx, &net, &fac, c.max_snap, "facilities");
        let g = nw::graph_with_stops(&net, &snaps);
        let crs = need_crs(nr)?;
        let back = geomkit::tx("EPSG:4326", crs)?;
        let n0 = net.nodes.len();
        let task = crate::progress::Task::start("Service area").cancellable();
        let mut rows = vec![];
        for (k, f) in fac.iter().enumerate() {
            task.check()?;
            task.set(k as f64 / fac.len() as f64, None::<String>);
            if snaps[k].is_none() {
                continue;
            }
            let s = nw::dijkstra(&g, n0 + k, &[], usize::MAX, cutoff);
            // Covered fractions per edge from every arc that starts within the cutoff.
            let mut cov: std::collections::BTreeMap<usize, Vec<(f64, f64)>> = Default::default();
            for (u, arcs) in g.adj.iter().enumerate() {
                if !s.dist[u].is_finite() {
                    continue;
                }
                for a in arcs {
                    if a.cost <= 0.0 {
                        continue;
                    }
                    let reach = ((cutoff - s.dist[u]) / a.cost).min(1.0);
                    if reach <= 0.0 {
                        continue;
                    }
                    let end = a.f0 + (a.f1 - a.f0) * reach;
                    cov.entry(a.edge).or_default().push((a.f0.min(end), a.f0.max(end)));
                }
            }
            for (ei, mut iv) in cov {
                iv.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());
                let mut merged: Vec<(f64, f64)> = vec![];
                for (lo, hi) in iv {
                    match merged.last_mut() {
                        Some(l) if lo <= l.1 + 1e-9 => l.1 = l.1.max(hi),
                        _ => merged.push((lo, hi)),
                    }
                }
                for (lo, hi) in merged {
                    if hi - lo < 1e-9 {
                        continue;
                    }
                    let e = &net.edges[ei];
                    if let Some(w) = line_wkt(&back, &nw::slice(e, lo, hi)) {
                        rows.push(format!("({}, {}, {}, {}, '{}', ST_GeomFromText('{w}'))", lit(&f.label), e.rn, sqlnum(hi - lo), sqlnum(cutoff), c.unit()));
                    }
                }
            }
        }
        if rows.is_empty() {
            bail!("Nothing is reachable: no facility could be snapped to the network, or the cutoff is shorter than the way to the nearest line");
        }
        let lines = format!("SELECT * FROM (VALUES {}) t(facility, edge_row, fraction_reached, cutoff, cost_unit, geom)", rows.join(", "));
        if !polygon {
            return Ok(Output::Select { sql: lines, crs: nr.crs.clone(), view: false });
        }
        let utm = ctx.utm(nr)?;
        let (from, to) = (lit(&crate::crs::resolve(crs)), lit(&crate::crs::resolve(&utm)));
        Ok(Output::Select {
            sql: format!(
                "SELECT facility, any_value(cutoff) AS cutoff, any_value(cost_unit) AS cost_unit, {buf:?} AS buffer_m, ST_Transform(ST_Buffer(ST_Transform(ST_Union_Agg(geom), {from}, {to}, true), {buf:?}), {to}, {from}, true) AS geom FROM ({lines}) GROUP BY facility"
            ),
            crs: nr.crs.clone(),
            view: false,
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------

pub struct ClosestFacility;
impl Tool for ClosestFacility {
    fn spec(&self) -> ToolSpec {
        let mut params = vec![
            id_param("incident_id", "Incident name field (empty: row number)", "incidents"),
            id_param("facility_id", "Facility name field (empty: row number)", "facilities"),
            Param::new("count", "Facilities to find per incident", ParamKind::Number).default(1),
            Param::new("cutoff", "Cutoff (0 = none; metres, or minutes for travel time)", ParamKind::Number).default(0),
            Param::new("direction", "Travel", ParamKind::Enum).default("to").options(&[("to", "From the incident to the facility"), ("from", "From the facility to the incident")]),
        ];
        params.extend(net_params());
        ToolSpec { id: "closest_facility", name: "Closest Facility", category: "Analysis › Network", role: Role::Tool,
            description: concat!("For each incident, the nearest facilities by travel cost over the network (Dijkstra's algorithm), as one route line per incident and facility with its rank, cost, length in metres and the snap distances. Travel runs from the incident to the facility or the other way (it matters with one-way streets). An incident that cannot reach any facility is kept as a row with an empty line and a status. Cost is length in metres or travel time in minutes. ", net_help!()),
            inputs: vec![net_port(), stop_port("incidents", "Incidents (points)"), stop_port("facilities", "Facilities (points)")], output: Some(PortKind::Vector), params }
    }
    fn output_fields(&self) -> OutFields {
        OutFields::from(&[]).add(NewField::new("incident", "VARCHAR")).add(NewField::new("facility", "VARCHAR")).add(NewField::new("rank", "INTEGER")).add(NewField::new("cost", "DOUBLE")).add(NewField::new("cost_unit", "VARCHAR")).add(NewField::new("length_m", "DOUBLE")).add(NewField::new("incident_snap_m", "DOUBLE")).add(NewField::new("facility_snap_m", "DOUBLE")).add(NewField::new("status", "VARCHAR"))
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let (nr, ir, fr) = (&inputs["net"], &inputs["incidents"], &inputs["facilities"]);
        let c = cfg(p)?;
        let count = p.f64("count").unwrap_or(1.0);
        if !(count.is_finite() && count >= 1.0 && count.fract() == 0.0) {
            bail!("'Facilities to find per incident' must be a whole number, 1 or more");
        }
        let cutoff = p.f64("cutoff").unwrap_or(0.0);
        if !(cutoff.is_finite() && cutoff >= 0.0) {
            bail!("'Cutoff' must be 0 (none) or a number greater than 0");
        }
        let cutoff = if cutoff == 0.0 { f64::INFINITY } else { cutoff };
        let from_fac = p.str("direction").as_deref() == Some("from");
        let net = read_net(ctx, nr, &c)?;
        let inc = read_stops(ctx, ir, p.str("incident_id").as_deref(), "the incidents")?;
        let fac = read_stops(ctx, fr, p.str("facility_id").as_deref(), "the facilities")?;
        let (si, sf) = (snap_all(ctx, &net, &inc, c.max_snap, "incidents"), snap_all(ctx, &net, &fac, c.max_snap, "facilities"));
        let mut snaps = si.clone();
        snaps.extend(sf.iter().cloned());
        let fwd = nw::graph_with_stops(&net, &snaps);
        let g = if from_fac { nw::reversed(&fwd) } else { fwd };
        let back = geomkit::tx("EPSG:4326", need_crs(nr)?)?;
        let n0 = net.nodes.len();
        let targets: Vec<usize> = (0..fac.len()).filter(|k| sf[*k].is_some()).map(|k| n0 + inc.len() + k).collect();
        let task = crate::progress::Task::start("Closest facility").cancellable();
        let mut rows = vec![];
        let mut none = 0usize;
        for (k, i) in inc.iter().enumerate() {
            task.check()?;
            task.set(k as f64 / inc.len() as f64, None::<String>);
            let isnap = si[k].as_ref().map(|s| s.dist_m);
            let mut found: Vec<(f64, usize)> = vec![];
            let mut s = None;
            if si[k].is_some() && !targets.is_empty() {
                let r = nw::dijkstra(&g, n0 + k, &targets, count as usize, cutoff);
                found = targets.iter().filter(|t| r.dist[**t].is_finite()).map(|t| (r.dist[*t], *t)).collect();
                found.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.cmp(&b.1)));
                found.truncate(count as usize);
                s = Some(r);
            }
            if found.is_empty() {
                none += 1;
                let why = if si[k].is_none() { "incident not on the network" } else { "no facility reachable" };
                rows.push(format!("({}, NULL, NULL, NULL, '{}', NULL, {}, NULL, '{why}', CAST(NULL AS GEOMETRY))", lit(&i.label), c.unit(), sqlnum(isnap.unwrap_or(f64::NAN))));
                continue;
            }
            let s = s.unwrap();
            for (rank, (cost, node)) in found.iter().enumerate() {
                let fk = node - n0 - inc.len();
                let (pts, len) = nw::path_geometry(&net, &g, &s, *node);
                // On the reversed graph the path runs from the incident back to the facility's travel start: turn it to the travel direction.
                let pts = if from_fac { pts.into_iter().rev().collect() } else { pts };
                rows.push(format!("({}, {}, {}, {}, '{}', {}, {}, {}, 'ok', {})", lit(&i.label), lit(&fac[fk].label), rank + 1, sqlnum(*cost), c.unit(), sqlnum(len), sqlnum(isnap.unwrap_or(f64::NAN)), sqlnum(sf[fk].as_ref().map(|s| s.dist_m).unwrap_or(f64::NAN)), geom_sql(line_wkt(&back, &pts))));
            }
        }
        if none > 0 {
            ctx.add_note(format!("Closest Facility: {none} of {} incidents have no route to a facility (off the network, disconnected, or beyond the cutoff); they are in the result with an empty line and a status.", inc.len()));
        }
        Ok(Output::Select {
            sql: format!("SELECT * FROM (VALUES {}) t(incident, facility, rank, cost, cost_unit, length_m, incident_snap_m, facility_snap_m, status, geom)", rows.join(", ")),
            crs: nr.crs.clone(),
            view: false,
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------

pub struct OdMatrix;
impl Tool for OdMatrix {
    fn spec(&self) -> ToolSpec {
        let mut params = vec![
            id_param("origin_id", "Origin name field (empty: row number)", "origins"),
            id_param("dest_id", "Destination name field (empty: row number)", "destinations"),
            Param::new("cutoff", "Cutoff (0 = none; metres, or minutes for travel time)", ParamKind::Number).default(0),
        ];
        params.extend(net_params());
        ToolSpec { id: "od_matrix", name: "OD Cost Matrix", category: "Analysis › Network", role: Role::Tool,
            description: concat!("The least cost from every origin to every destination over the network (Dijkstra's algorithm from each origin), as a table with one row per pair: cost, the length of the cheapest route in metres, the snap distances and a status (ok, unreachable, or not on the network). Pairs beyond the cutoff are unreachable. Up to 250 000 pairs. Cost is length in metres or travel time in minutes. ", net_help!()),
            inputs: vec![net_port(), stop_port("origins", "Origins (points)"), stop_port("destinations", "Destinations (points)")], output: Some(PortKind::Table), params }
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let (nr, or, dr) = (&inputs["net"], &inputs["origins"], &inputs["destinations"]);
        let c = cfg(p)?;
        let cutoff = p.f64("cutoff").unwrap_or(0.0);
        if !(cutoff.is_finite() && cutoff >= 0.0) {
            bail!("'Cutoff' must be 0 (none) or a number greater than 0");
        }
        let cutoff = if cutoff == 0.0 { f64::INFINITY } else { cutoff };
        let net = read_net(ctx, nr, &c)?;
        let org = read_stops(ctx, or, p.str("origin_id").as_deref(), "the origins")?;
        let dst = read_stops(ctx, dr, p.str("dest_id").as_deref(), "the destinations")?;
        if org.len() * dst.len() > 250_000 {
            bail!("{} origins × {} destinations is {} pairs; the limit is 250 000. Use fewer stops.", org.len(), dst.len(), org.len() * dst.len());
        }
        let (so, sd) = (snap_all(ctx, &net, &org, c.max_snap, "origins"), snap_all(ctx, &net, &dst, c.max_snap, "destinations"));
        let mut snaps = so.clone();
        snaps.extend(sd.iter().cloned());
        let g = nw::graph_with_stops(&net, &snaps);
        let n0 = net.nodes.len();
        let targets: Vec<usize> = (0..dst.len()).filter(|k| sd[*k].is_some()).map(|k| n0 + org.len() + k).collect();
        let task = crate::progress::Task::start("OD cost matrix").cancellable();
        let mut rows = vec![];
        let (mut unreachable, mut off) = (0usize, 0usize);
        for (k, o) in org.iter().enumerate() {
            task.check()?;
            task.set(k as f64 / org.len() as f64, None::<String>);
            let s = so[k].as_ref().map(|_| nw::dijkstra(&g, n0 + k, &targets, usize::MAX, cutoff));
            for (j, d) in dst.iter().enumerate() {
                let (osn, dsn) = (so[k].as_ref().map(|x| x.dist_m), sd[j].as_ref().map(|x| x.dist_m));
                let node = n0 + org.len() + j;
                let (status, cost, len) = match (&s, &sd[j]) {
                    (Some(r), Some(_)) if r.dist[node].is_finite() => {
                        let len: f64 = nw::path_arcs(&g, r, node).iter().map(|a| (a.f1 - a.f0).abs() * net.edges[a.edge].len).sum();
                        ("ok", r.dist[node], len)
                    }
                    (Some(_), Some(_)) => {
                        unreachable += 1;
                        ("unreachable", f64::NAN, f64::NAN)
                    }
                    _ => {
                        off += 1;
                        ("not on the network", f64::NAN, f64::NAN)
                    }
                };
                rows.push(format!("({}, {}, {}, '{}', {}, {}, {}, '{status}')", lit(&o.label), lit(&d.label), sqlnum(cost), c.unit(), sqlnum(len), sqlnum(osn.unwrap_or(f64::NAN)), sqlnum(dsn.unwrap_or(f64::NAN))));
            }
        }
        if unreachable + off > 0 {
            ctx.add_note(format!("OD Cost Matrix: {unreachable} pair(s) are unreachable and {off} involve a stop that is not on the network; they are in the table with a status."));
        }
        Ok(Output::Select {
            sql: format!("SELECT * FROM (VALUES {}) t(origin, destination, cost, cost_unit, length_m, origin_snap_m, destination_snap_m, status)", rows.join(", ")),
            crs: None,
            view: false,
        })
    }
}

pub fn keywords(id: &str) -> &'static [&'static str] {
    match id {
        "shortest_route" => &["directions", "routing", "driving route", "path between points", "fastest route", "shortest path"],
        "service_area" => &["drive time", "isochrone", "catchment by road", "walk shed", "reachable area", "travel time area"],
        "closest_facility" => &["nearest hospital", "nearest station", "nearest by road", "emergency response", "which is closest"],
        "od_matrix" => &["origin destination", "travel time table", "distance matrix", "cost matrix", "many to many"],
        _ => &[],
    }
}

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(ShortestRoute), Box::new(ServiceArea), Box::new(ClosestFacility), Box::new(OdMatrix)]
}
