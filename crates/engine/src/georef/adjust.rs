//! Vector spatial adjustment (GR3): move a layer's shapes with control points. Affine and rubber sheet use GDAL's own GCP polynomial
//! (order 1) and thin plate spline transformers; similarity (Helmert: scale, rotation, shift) is the closed-form least-squares
//! solution, since GDAL has no such transformer.
//!
//! In a control point used for adjustment, `pixel` and `line` hold the *from* position (x, y in the layer's CRS: x east or longitude,
//! y north or latitude; `line` is not flipped here) and `x`, `y` hold the *to* position in the set's target CRS. The to positions are
//! moved into the layer's CRS through an explicit PROJ transform (or used as they are when the CRS is the same), and the adjusted layer
//! is in the layer's CRS, which the result states. Both coordinate sets are centred on their means before fitting, so large projected
//! coordinates don't cost precision. Only x and y change; Z and M are carried through unchanged.

use super::warp::{collinear, rms, spline_warning, units_of, PointError, RMS_FORMULA};
use super::{ControlPoint, ControlSet};
use crate::db::ident;
use crate::edit::{parse_wkb, write_wkb};
use crate::layers::{self, ImportSource, LayerInfo};
use crate::progress::Task;
use crate::{sources, Engine};
use anyhow::{anyhow, bail, Result};
use duckdb::Connection;
use serde::{Deserialize, Serialize};
use std::ffi::{c_int, c_void, CString};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum AdjustMethod {
    /// Least-squares affine (6 parameters): GDAL's order-1 GCP polynomial.
    Affine,
    /// Scale, rotation and shift only (4 parameters), keeping shapes' angles. Needs 2 points.
    Similarity,
    /// Thin plate spline through every point (GDAL's TPS transformer): shapes bend, control points land exactly.
    RubberSheet,
}

impl AdjustMethod {
    pub fn min_points(self) -> usize {
        match self {
            AdjustMethod::Similarity => 2,
            _ => 3,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AdjustMethod::Affine => "affine (GDAL 1st-order polynomial)",
            AdjustMethod::Similarity => "similarity (scale, rotation, shift)",
            AdjustMethod::RubberSheet => "rubber sheet (GDAL thin plate spline)",
        }
    }
}

/// The error table for an adjustment: the same figures as the raster warp's, without the pixel columns.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AdjustReport {
    pub method: AdjustMethod,
    pub method_label: String,
    pub min_points: usize,
    pub enabled_count: usize,
    /// (dx, dy) = where the transform puts a from point minus where its to point is, in the layer's CRS units. `length_px` fields are 0.
    pub points: Vec<PointError>,
    pub rms: f64,
    pub rms_m: f64,
    pub check_rms: Option<f64>,
    pub check_rms_m: Option<f64>,
    pub unit: String,
    pub metres_kind: String,
    pub formula: String,
    pub warnings: Vec<String>,
    /// The CRS the adjusted layer is in: the layer's own.
    pub output_crs: String,
    /// Similarity only: the fitted scale factor and rotation (degrees, counter-clockwise).
    pub scale: Option<f64>,
    pub rotation_deg: Option<f64>,
}

enum Kind {
    Gdal { h: *mut c_void, tps: bool },
    Similarity { re: f64, im: f64 },
}

pub(crate) struct Fitted {
    kind: Kind,
    from_c: (f64, f64),
    to_c: (f64, f64),
}

impl Drop for Fitted {
    fn drop(&mut self) {
        if let Kind::Gdal { h, tps } = self.kind {
            unsafe {
                if tps {
                    gdal_sys::GDALDestroyTPSTransformer(h)
                } else {
                    gdal_sys::GDALDestroyGCPTransformer(h)
                }
            }
        }
    }
}

// Used by one thread (an adjustment owns its fit).
unsafe impl Send for Fitted {}

fn gdal_msg() -> String {
    unsafe { std::ffi::CStr::from_ptr(gdal_sys::CPLGetLastErrorMsg()) }.to_string_lossy().trim().to_string()
}

impl Fitted {
    fn new(method: AdjustMethod, pairs: &[((f64, f64), (f64, f64))]) -> Result<Fitted> {
        let n = pairs.len() as f64;
        let from_c = (pairs.iter().map(|p| p.0 .0).sum::<f64>() / n, pairs.iter().map(|p| p.0 .1).sum::<f64>() / n);
        let to_c = (pairs.iter().map(|p| p.1 .0).sum::<f64>() / n, pairs.iter().map(|p| p.1 .1).sum::<f64>() / n);
        let kind = match method {
            AdjustMethod::Similarity => {
                // z' = a (z - zc) + tc with a complex: a = sum(conj(p) q) / sum(|p|^2), the least-squares scale and rotation.
                let (mut num_re, mut num_im, mut den) = (0.0, 0.0, 0.0);
                for (f, t) in pairs {
                    let (px, py) = (f.0 - from_c.0, f.1 - from_c.1);
                    let (qx, qy) = (t.0 - to_c.0, t.1 - to_c.1);
                    num_re += px * qx + py * qy;
                    num_im += px * qy - py * qx;
                    den += px * px + py * py;
                }
                if den <= 0.0 {
                    bail!("The two from points are at the same place, so no scale or rotation can be found.");
                }
                let (re, im) = (num_re / den, num_im / den);
                if re.hypot(im) < 1e-9 {
                    bail!("The enabled to points are all at the same place, so every shape would collapse to one point. Check their coordinates.");
                }
                Kind::Similarity { re, im }
            }
            AdjustMethod::Affine | AdjustMethod::RubberSheet => {
                let tps = method == AdjustMethod::RubberSheet;
                let ids: Vec<CString> = (0..pairs.len()).map(|i| CString::new(format!("{i}")).unwrap()).collect();
                let info = CString::new("").unwrap();
                let gcps: Vec<gdal_sys::GDAL_GCP> = pairs
                    .iter()
                    .zip(&ids)
                    .map(|((f, t), id)| gdal_sys::GDAL_GCP {
                        pszId: id.as_ptr() as *mut _,
                        pszInfo: info.as_ptr() as *mut _,
                        dfGCPPixel: f.0 - from_c.0,
                        dfGCPLine: f.1 - from_c.1,
                        dfGCPX: t.0 - to_c.0,
                        dfGCPY: t.1 - to_c.1,
                        dfGCPZ: 0.0,
                    })
                    .collect();
                let h = unsafe {
                    if tps {
                        gdal_sys::GDALCreateTPSTransformer(gcps.len() as c_int, gcps.as_ptr(), 0)
                    } else {
                        gdal_sys::GDALCreateGCPTransformer(gcps.len() as c_int, gcps.as_ptr(), 1, 0)
                    }
                };
                if h.is_null() {
                    let m = gdal_msg();
                    bail!("GDAL could not fit a {} to these {} points{}. They may lie on a line or too close together; spread them over the layer.", method.label(), pairs.len(), if m.is_empty() { String::new() } else { format!(" ({m})") });
                }
                Kind::Gdal { h, tps }
            }
        };
        Ok(Fitted { kind, from_c, to_c })
    }

    /// Move points (x, y in the from system) to the to system. Returns false for a point the transform could not place.
    fn apply(&self, pts: &mut [(f64, f64)]) -> Vec<bool> {
        match self.kind {
            Kind::Similarity { re, im } => pts
                .iter_mut()
                .map(|p| {
                    let (px, py) = (p.0 - self.from_c.0, p.1 - self.from_c.1);
                    *p = (re * px - im * py + self.to_c.0, im * px + re * py + self.to_c.1);
                    p.0.is_finite() && p.1.is_finite()
                })
                .collect(),
            Kind::Gdal { h, tps } => {
                let mut xs: Vec<f64> = pts.iter().map(|p| p.0 - self.from_c.0).collect();
                let mut ys: Vec<f64> = pts.iter().map(|p| p.1 - self.from_c.1).collect();
                let mut ok = vec![0 as c_int; pts.len()];
                unsafe {
                    if tps {
                        gdal_sys::GDALTPSTransform(h, 0, pts.len() as c_int, xs.as_mut_ptr(), ys.as_mut_ptr(), std::ptr::null_mut(), ok.as_mut_ptr());
                    } else {
                        gdal_sys::GDALGCPTransform(h, 0, pts.len() as c_int, xs.as_mut_ptr(), ys.as_mut_ptr(), std::ptr::null_mut(), ok.as_mut_ptr());
                    }
                }
                for (i, p) in pts.iter_mut().enumerate() {
                    *p = (xs[i] + self.to_c.0, ys[i] + self.to_c.1);
                }
                ok.iter().zip(pts.iter()).map(|(&s, p)| s != 0 && p.0.is_finite() && p.1.is_finite()).collect()
            }
        }
    }
}

pub(crate) struct Prepared {
    pub(crate) fitted: Fitted,
    pub(crate) report: AdjustReport,
    pub(crate) notes: Vec<String>,
}

/// Fit the points for a layer in `layer_crs`: validate, express the to points in the layer's CRS (explicit PROJ transform), fit, and
/// compute the error table from that same fit.
pub(crate) fn prepare(conn: &Connection, layer_crs: &str, set: &ControlSet, method: AdjustMethod) -> Result<Prepared> {
    set.validate()?;
    let crs = layer_crs.trim();
    if crs.is_empty() {
        bail!("This layer has no coordinate system, so its shapes can't be adjusted. Define its projection first.");
    }
    let mut notes = Vec::new();
    let same = crate::crs::same(crs, &set.target_crs);
    if !same {
        if let Some(n) = crate::crs::datum_change_note(&set.target_crs, crs) {
            notes.push(n);
        }
        notes.push(format!("The to points are in {} and the layer is in {}: they were transformed into the layer's coordinate system with PROJ, and the adjusted layer is in {}.", super::short_crs(&set.target_crs), super::short_crs(crs), super::short_crs(crs)));
    }
    // (from, to in the layer's CRS, point)
    let mut all: Vec<((f64, f64), (f64, f64), &ControlPoint)> = Vec::with_capacity(set.points.len());
    for p in &set.points {
        let to = if same { (p.x, p.y) } else { crate::crs::transform_point(conn, p.x, p.y, &set.target_crs, crs).map_err(|e| anyhow!("Control point {} can't be moved into the layer's coordinate system: {e}", p.id))? };
        if !to.0.is_finite() || !to.1.is_finite() {
            bail!("Control point {} can't be expressed in the layer's coordinate system.", p.id);
        }
        all.push(((p.pixel, p.line), to, p));
    }
    let enabled: Vec<&((f64, f64), (f64, f64), &ControlPoint)> = all.iter().filter(|a| a.2.enabled).collect();
    let need = method.min_points();
    if enabled.len() < need {
        bail!("A {} needs at least {need} enabled control points; this set has {}. Add {} more (or turn some back on).", method.label(), enabled.len(), need - enabled.len());
    }
    let geographic = crate::crs::is_geographic(crs);
    if geographic && method == AdjustMethod::Similarity {
        // A degree of longitude is shorter than a degree of latitude away from the equator, so a rotation in degrees shears the shapes.
        bail!("A similarity adjustment keeps shapes' angles only in a coordinate system with equal units in both directions, and this layer is in degrees. Project the layer to a metre-based coordinate system first (Project), or use the affine method.");
    }
    if geographic {
        let tmp: Vec<ControlPoint> = enabled.iter().flat_map(|a| [ControlPoint { x: a.0 .0, y: a.0 .1, ..a.2.clone() }, ControlPoint { x: a.1 .0, y: a.1 .1, ..a.2.clone() }]).collect();
        super::longitude_span_ok(&tmp.iter().collect::<Vec<_>>())?;
    }
    let from_pts: Vec<(f64, f64)> = enabled.iter().map(|a| a.0).collect();
    let to_pts: Vec<(f64, f64)> = enabled.iter().map(|a| a.1).collect();
    if method != AdjustMethod::Similarity {
        if collinear(&from_pts) {
            bail!("The enabled from points all lie on one straight line (or one point may be very far from the others), so they can't fix a shift in both directions. Spread them over the layer.");
        }
        if collinear(&to_pts) {
            bail!("The enabled to points all lie on one straight line (or one point may be very far from the others). Check their coordinates and spread them over the area.");
        }
    }
    let pairs: Vec<((f64, f64), (f64, f64))> = enabled.iter().map(|a| (a.0, a.1)).collect();
    let fitted = Fitted::new(method, &pairs)?;
    let units = units_of(crs)?;
    let mut placed: Vec<(f64, f64)> = all.iter().map(|a| a.0).collect();
    let ok = fitted.apply(&mut placed);
    if ok.iter().any(|o| !o) {
        bail!("The fitted transform could not be applied to every control point. The points may be badly placed for this method.");
    }
    let mut points = Vec::with_capacity(all.len());
    let mut off_globe: Vec<u32> = Vec::new();
    for (i, a) in all.iter().enumerate() {
        let (dx, dy) = (placed[i].0 - a.1 .0, placed[i].1 - a.1 .1);
        let length = dx.hypot(dy);
        let length_m = match &units.geod {
            Some(g) => {
                let (m, off) = super::geodesic_m(g, a.1, placed[i]);
                if off {
                    off_globe.push(a.2.id);
                }
                m
            }
            None => length * units.metres_per_unit,
        };
        points.push(PointError { id: a.2.id, enabled: a.2.enabled, check: !a.2.enabled, dx, dy, length, length_m, dpixel: 0.0, dline: 0.0, length_px: 0.0 });
    }
    let on = || points.iter().filter(|e| e.enabled);
    let off = || points.iter().filter(|e| !e.enabled);
    let mut warnings = Vec::new();
    if method == AdjustMethod::RubberSheet {
        warnings.push(spline_warning());
        warnings.push("Only vertices move: a long straight edge stays straight between its two ends even where the surface bends, so long edges may not follow the adjustment between their ends.".into());
        if off().next().is_none() {
            warnings.push("No check points yet: every point is in the fit, so no honest accuracy figure exists.".into());
        }
    } else if let Some(w) = super::exact_fit_warning(enabled.len(), need) {
        warnings.push(w);
    }
    if !off_globe.is_empty() {
        let ids: Vec<String> = off_globe.iter().map(|i| i.to_string()).collect();
        warnings.push(format!("The fit puts control point{} {} off the globe (a latitude beyond 90°). Its metre figure is a lower bound, measured to the pole. Check that point, or use a different method.", if off_globe.len() == 1 { "" } else { "s" }, ids.join(", ")));
    }
    let (scale, rotation_deg) = match fitted.kind {
        Kind::Similarity { re, im } => (Some(re.hypot(im)), Some(im.atan2(re).to_degrees())),
        _ => (None, None),
    };
    let report = AdjustReport {
        method,
        method_label: method.label().into(),
        min_points: need,
        enabled_count: enabled.len(),
        rms: rms(on().map(|e| e.length)).unwrap_or(0.0),
        rms_m: rms(on().map(|e| e.length_m)).unwrap_or(0.0),
        check_rms: rms(off().map(|e| e.length)),
        check_rms_m: rms(off().map(|e| e.length_m)),
        points,
        unit: units.unit,
        metres_kind: if units.geographic { "geodesic" } else { "grid" }.into(),
        formula: RMS_FORMULA.into(),
        warnings,
        output_crs: crs.to_string(),
        scale,
        rotation_deg,
    };
    Ok(Prepared { fitted, report, notes })
}

/// What the UI sends to adjust a layer.
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AdjustRequest {
    pub layer: String,
    pub set: ControlSet,
    pub method: AdjustMethod,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Dropped {
    /// The feature's id (`__fid`).
    pub fid: i64,
    pub reason: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AdjustResult {
    pub layer: LayerInfo,
    /// Features left out of the new layer, by id, with why.
    pub dropped: Vec<Dropped>,
    /// Features moved.
    pub moved: i64,
    pub report: AdjustReport,
    pub notes: Vec<String>,
}

const BATCH: usize = 5000;

/// Move every shape of `t` (a quoted table name with a `geom` column and a unique integer column `key`) with the fitted transform into
/// the scratch table `scratch` (fid, wkb, bad), 5000 at a time, checking for Cancel between batches. Returns the features that
/// could not be adjusted, with the reason, and how many shapes were read. `def` is an extra ` AND ...` condition on `t`.
pub(crate) fn move_shapes(conn: &Connection, task: &Task, fitted: &Fitted, crs: &str, scratch: &str, t: &str, key: &str, def: &str) -> Result<(Vec<Dropped>, i64)> {
        let geographic = crate::crs::is_geographic(crs);
        let total: i64 = conn.query_row(&format!("SELECT count(*) FROM {t} WHERE geom IS NOT NULL{def}"), [], |r| r.get(0))?;
        let mut last = i64::MIN;
        let mut done = 0i64;
        loop {
            task.check()?;
            let rows: Vec<(i64, Vec<u8>)> = {
                let mut stmt = conn.prepare(&format!("SELECT {key}, ST_AsWKB(geom) FROM {t} WHERE geom IS NOT NULL AND {key} > ?{def} ORDER BY {key} LIMIT {BATCH}"))?;
                let v = stmt.query_map([last], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?)))?.collect::<Result<Vec<_>, _>>()?;
                v
            };
            if rows.is_empty() {
                break;
            }
            last = rows.last().unwrap().0;
            let mut app = conn.appender(scratch)?;
            for (fid, wkb) in &rows {
                let (out, bad): (Option<Vec<u8>>, Option<String>) = match parse_wkb(wkb) {
                    Err(_) => (None, Some("its geometry type (a collection or curve) can't be adjusted".into())),
                    Ok(mut g) => {
                        let mut xy: Vec<(f64, f64)> = g.coords().map(|c| (c.x, c.y)).collect();
                        let ok = fitted.apply(&mut xy);
                        if ok.iter().any(|o| !o) {
                            (None, Some("a vertex could not be moved by the transform".into()))
                        } else if geographic && xy.iter().any(|p| p.1.abs() > 90.0) {
                            (None, Some("a vertex moved beyond the poles (latitude over 90°)".into()))
                        } else {
                            for (c, p) in g.coords_mut().zip(xy) {
                                c.x = p.0;
                                c.y = p.1;
                            }
                            (Some(write_wkb(&g)), None)
                        }
                    }
                };
                app.append_row(duckdb::params![*fid, out, bad])?;
            }
            app.flush()?;
            done += rows.len() as i64;
            task.set(done as f64 / total.max(1) as f64 * 0.8, Some(format!("{done} of {total} features")));
        }
        // Shapes that were valid and no longer are, or that came out empty, are left out (and counted below).
        task.detail("Checking the moved shapes");
        conn.execute_batch(&format!(
            "UPDATE {s} SET bad = r.reason FROM (SELECT s.fid, CASE WHEN ST_IsEmpty(ST_GeomFromWKB(s.wkb)) THEN 'the shape came out empty' WHEN ST_IsValid(t.geom) AND NOT ST_IsValid(ST_GeomFromWKB(s.wkb)) THEN 'the shape became invalid (it crosses itself or collapsed)' END AS reason FROM {s} s JOIN {t} t ON t.{key} = s.fid WHERE s.bad IS NULL) r WHERE {s}.fid = r.fid AND r.reason IS NOT NULL",
            s = ident(scratch)
        ))?;
        task.check()?;
        let mut stmt = conn.prepare(&format!("SELECT fid, bad FROM {} WHERE bad IS NOT NULL ORDER BY fid", ident(scratch)))?;
        let dropped = stmt.query_map([], |r| Ok(Dropped { fid: r.get(0)?, reason: r.get(1)? }))?.collect::<Result<Vec<_>, _>>()?;
        Ok((dropped, total))
}

impl Engine {
    /// The error table for adjusting `layer` with `set` and `method` (nothing is changed).
    pub fn adjust_errors(&self, layer: &str, set: &ControlSet, method: AdjustMethod) -> Result<AdjustReport> {
        let info = self.layer(layer)?;
        let crs = info.crs.clone().unwrap_or_default();
        self.with_conn(|c| Ok(prepare(c, &crs, set, method)?.report))
    }

    /// Adjust the layer's shapes into a new layer (same CRS, all attributes). The source layer is not changed.
    pub fn adjust_layer(&self, req: &AdjustRequest) -> Result<AdjustResult> {
        let info = self.layer(&req.layer)?;
        if info.is_raster() || !info.has_geometry() {
            bail!("'{}' has no shapes to adjust. Choose a vector layer.", info.name);
        }
        if info.editing {
            bail!("This layer is being edited. Save or discard the shape edits first.");
        }
        let crs = info.crs.clone().unwrap_or_default();
        let name = req.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| format!("{} (adjusted)", info.name));
        let task = Task::start(format!("Adjusting {}", info.name)).cancellable();
        let out = self.with_conn(|conn| {
            let prep = prepare(conn, &crs, &req.set, req.method)?;
            let id = self.alloc_id();
            let scratch = format!("__adj_{id}");
            let t = ident(&info.table);
            let def = info.def_and();
            conn.execute_batch(&format!("CREATE OR REPLACE TEMP TABLE {} (fid BIGINT, wkb BLOB, bad VARCHAR)", ident(&scratch)))?;
            let run = move_shapes(conn, &task, &prep.fitted, &crs, &scratch, &t, "__fid", &def);
            let (dropped, total) = match run {
                Ok(v) => v,
                Err(e) => {
                    let _ = conn.execute_batch(&format!("DROP TABLE IF EXISTS {}", ident(&scratch)));
                    return Err(e);
                }
            };
            task.set(0.9, Some("Writing the new layer".to_string()));
            let hidden = "__fid, __g, __x0, __y0, __sz";
            let s = ident(&scratch);
            let src = ImportSource {
                name: name.clone(),
                select: format!(
                    "SELECT t.* EXCLUDE ({hidden}) REPLACE (CASE WHEN a.wkb IS NULL THEN t.geom ELSE ST_GeomFromWKB(a.wkb) END AS geom) FROM {t} t LEFT JOIN {s} a ON a.fid = t.__fid WHERE TRUE{def} AND (t.geom IS NULL OR (a.fid IS NOT NULL AND a.bad IS NULL)) ORDER BY t.__fid"
                ),
                crs: info.crs.clone(),
                crs_name: info.crs_name.clone(),
                warnings: vec![],
            };
            let made = layers::build_layer(conn, &id, &src);
            let _ = conn.execute_batch(&format!("DROP TABLE IF EXISTS {s}"));
            let mut layer = made?;
            layer.source = info.source.clone();
            let moved = total - dropped.len() as i64;
            let mut notes = prep.notes.clone();
            notes.push(format!("Adjusted {moved} feature{} with a {} fitted to {} points, in {}.", if moved == 1 { "" } else { "s" }, req.method.label(), prep.report.enabled_count, super::short_crs(&crs)));
            if !dropped.is_empty() {
                notes.push(format!("{} feature{} left out: {}.", dropped.len(), if dropped.len() == 1 { " was" } else { "s were" }, dropped.iter().take(20).map(|d| format!("#{} ({})", d.fid, d.reason)).collect::<Vec<_>>().join("; ")));
                layer.warnings.push(format!("{} feature{} could not be adjusted and {} left out (ids: {}).", dropped.len(), if dropped.len() == 1 { "" } else { "s" }, if dropped.len() == 1 { "was" } else { "were" }, dropped.iter().take(50).map(|d| d.fid.to_string()).collect::<Vec<_>>().join(", ")));
            }
            self.save_meta(conn, &layer)?;
            let from: Vec<&str> = std::iter::once(info.id.as_str()).chain(info.result_of.as_deref()).collect();
            sources::inherit(conn, &from, &layer.id, &layer.name)?;
            sources::record(conn, &citation(req, &prep.report, &layer.name, &layer.id, &crs, dropped.len()), &layer.id, &layer.name)?;
            Ok(AdjustResult { layer, dropped, moved, report: prep.report, notes })
        })?;
        task.set(1.0, Some("Done"));
        Ok(out)
    }
}

fn citation(req: &AdjustRequest, r: &AdjustReport, name: &str, layer_id: &str, crs: &str, dropped: usize) -> sources::Citation {
    let pts: Vec<String> = req.set.points.iter().map(|p| format!("#{} ({}, {}) -> ({}, {}){}", p.id, p.pixel, p.line, p.x, p.y, if p.enabled { "" } else { " [check point]" })).collect();
    sources::Citation {
        key: format!("adjust:{layer_id}"),
        title: format!("{name}: shapes adjusted with control points"),
        kind: "Derived data".into(),
        annotation: format!(
            "Spatial adjustment, {}, {} control points (to points in {}), output in {}. RMS {:.6} {} ({:.3} m) over {} enabled points{}{}. {} feature(s) left out. Points (from -> to): {}.",
            r.method_label,
            req.set.points.len(),
            super::short_crs(&req.set.target_crs),
            super::short_crs(crs),
            r.rms,
            r.unit,
            r.rms_m,
            r.enabled_count,
            r.check_rms.map(|c| format!("; check points RMS {c:.6} {}", r.unit)).unwrap_or_default(),
            match (r.scale, r.rotation_deg) {
                (Some(s), Some(a)) => format!("; scale {s:.8}, rotation {a:.6} deg"),
                _ => String::new(),
            },
            dropped,
            pts.join("; ")
        ),
        complete: true,
        ..Default::default()
    }
}
