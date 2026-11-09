//! DBSCAN clustering (Wave 1, package T6): density-based clusters of points (Ester, Kriegel, Sander and Xu 1996, "A density-based algorithm for
//! discovering clusters in large spatial databases with noise", KDD-96). A point with at least `min_points` points (itself included) within `eps`
//! metres is a core point; clusters grow from core points through points within `eps`; points reached by no core point are noise.
//! Written directly from the paper (about forty lines) instead of through a library so that `eps` is in metres and the distances are geodesic
//! on a geographic coordinate system (`geomkit::Space`): the same eps means the same ground distance at the equator and at 60 degrees north.
//! The result is deterministic: clusters are numbered in the order of the lowest row number of their first core point.

use super::*;
use crate::geomkit::{self, Space};

pub struct Dbscan;
impl Tool for Dbscan {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "dbscan", name: "Density-based Clusters (DBSCAN)", category: "Analysis › Spatial Statistics", role: Role::Tool,
            description: "Find clusters of points without saying how many there are (DBSCAN, Ester et al. 1996). A point with at least the minimum number of points (itself included) within the search distance is a core point; clusters grow outward from core points; points no cluster reaches are noise. The search distance is in metres and is geodesic on a geographic coordinate system, so it means the same ground distance everywhere; in a projected coordinate system it is the plane distance in that projection converted to metres. Adds cluster (1, 2, 3 … ; 0 = noise) and label ('noise' or 'cluster N'); the number of clusters and of noise points is reported. Lines and polygons are clustered by their centroid; features without a position get an empty cluster and are counted.",
            inputs: vec![Port { id: "in", label: "Points", kind: PortKind::Vector }], output: Some(PortKind::Vector),
            params: vec![
                Param::new("eps", "Search distance (metres)", ParamKind::Number).default(500),
                Param::new("min_points", "Minimum points in a neighbourhood", ParamKind::Number).default(4),
            ] }
    }
    fn output_fields(&self) -> OutFields {
        OutFields::default().add(NewField::new("cluster", "INTEGER")).add(NewField::new("label", "VARCHAR"))
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let crs = need_crs(i)?;
        if !i.has_geom {
            bail!("Clustering needs a layer with geometry");
        }
        let eps = p.f64("eps").unwrap_or(f64::NAN);
        let minp = p.f64("min_points").unwrap_or(f64::NAN);
        if !(eps.is_finite() && eps > 0.0) {
            bail!("'Search distance' must be a number of metres greater than 0");
        }
        if !(minp.is_finite() && minp >= 1.0 && minp.fract() == 0.0) {
            bail!("'Minimum points' must be a whole number, 1 or more");
        }
        let m = geomkit::metric(crs)?;
        let mut stmt = ctx.conn.prepare(&format!(
            "SELECT rn, ST_X(c), ST_Y(c) FROM (SELECT rn, ST_Centroid(geom) AS c FROM (SELECT row_number() OVER () AS rn, geom FROM {}) WHERE geom IS NOT NULL AND NOT ST_IsEmpty(geom)) WHERE c IS NOT NULL",
            ident(&i.table)
        ))?;
        let rows: Vec<(i64, f64, f64)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?;
        let total: i64 = ctx.conn.query_row(&format!("SELECT count(*) FROM {}", ident(&i.table)), [], |r| r.get(0))?;
        if rows.is_empty() {
            bail!("The layer has no features with a position");
        }
        let space = Space::new(m, rows.iter().map(|r| (r.1, r.2)).collect());
        let grid = space.grid(eps);
        let n = rows.len();
        const UNSEEN: i64 = -2;
        const NOISE: i64 = 0;
        let mut label = vec![UNSEEN; n];
        let mut next = 0i64;
        let task = crate::progress::Task::start("Clustering (DBSCAN)").cancellable();
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by_key(|k| rows[*k].0);
        for (done, &k) in order.iter().enumerate() {
            if done % 512 == 0 {
                task.check()?;
                task.set(done as f64 / n as f64, None::<String>);
            }
            if label[k] != UNSEEN {
                continue;
            }
            let nb = space.within(&grid, k, eps);
            if nb.len() + 1 < minp as usize {
                label[k] = NOISE;
                continue;
            }
            next += 1;
            label[k] = next;
            let mut queue: Vec<usize> = nb.into_iter().map(|c| c.0).collect();
            let mut qi = 0;
            while qi < queue.len() {
                let q = queue[qi];
                qi += 1;
                if label[q] == NOISE {
                    label[q] = next; // a border point reached from a core point
                }
                if label[q] != UNSEEN {
                    continue;
                }
                label[q] = next;
                let qn = space.within(&grid, q, eps);
                if qn.len() + 1 >= minp as usize {
                    queue.extend(qn.into_iter().map(|c| c.0));
                }
            }
        }
        let noise = label.iter().filter(|l| **l == NOISE).count();
        ctx.add_note(format!("DBSCAN: {next} cluster(s), {noise} noise point(s) of {n} (search distance {eps} m, at least {} points).", minp as usize));
        if total > n as i64 {
            ctx.add_note(format!("DBSCAN: {} feature(s) without a position have an empty cluster.", total - n as i64));
        }
        let vals: Vec<String> = rows.iter().zip(&label).map(|(r, l)| format!("({}, {l}, '{}')", r.0, if *l == NOISE { "noise".to_string() } else { format!("cluster {l}") })).collect();
        Ok(Output::Select {
            sql: format!("SELECT t.* EXCLUDE (rn), c.cluster, c.label FROM (SELECT row_number() OVER () AS rn, * FROM {}) t LEFT JOIN (SELECT * FROM (VALUES {}) v(rid, cluster, label)) c ON t.rn = c.rid", ident(&i.table), vals.join(", ")),
            crs: i.crs.clone(),
            view: false,
        })
    }
}

pub fn keywords(id: &str) -> &'static [&'static str] {
    match id {
        "dbscan" => &["clusters", "group nearby points", "outliers", "noise", "hotspot groups", "density clustering"],
        _ => &[],
    }
}

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(Dbscan)]
}
