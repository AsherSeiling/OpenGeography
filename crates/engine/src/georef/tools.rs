//! Georeferencing as Model tools (GR4): one `ToolSpec` each for the raster warp and the vector adjustment. The control point set is
//! chosen by its name in this project; a missing name, or a set that can't be used, stops the node with a readable sentence before
//! any work starts. The work is the same code the Georeference and Adjust Shapes windows use (`warp::run`, `adjust::move_shapes`).

use super::adjust::{self, AdjustMethod};
use super::warp::{self, Dest, Method};
use crate::db::ident;
use crate::nodes::{need_crs, need_raster, Ctx, Inputs, Output, Param, ParamKind, Params, Port, PortKind, ResultLook, Role, Tool, ToolSpec};
use crate::progress::Task;
use anyhow::{bail, Result};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

const SET_HELP: &str = "The name of a control point set saved in this project (made in Map > Georeference). Names are matched ignoring case.";

fn set_param() -> Param {
    Param::new("set", "Control point set", ParamKind::Text).help(SET_HELP)
}

/// Raster warp: an image in pixel space becomes a georeferenced GeoTIFF.
pub struct GeorefImage;

fn raster_method(id: &str) -> Result<Method> {
    Ok(match id {
        "" | "affine" => Method::Polynomial { order: 1 },
        "poly2" => Method::Polynomial { order: 2 },
        "poly3" => Method::Polynomial { order: 3 },
        "spline" => Method::Tps,
        other => bail!("'{other}' is not a georeferencing method."),
    })
}

impl Tool for GeorefImage {
    fn result_look(&self) -> ResultLook {
        ResultLook::Source
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "georef_image",
            name: "Georeference Image (Control Points)",
            category: "Raster › Management",
            role: Role::Tool,
            description: "Place an image on the Earth with a saved set of control points: the image is stretched or bent to fit them and written as a GeoTIFF in the set's coordinate system.",
            inputs: vec![Port { id: "in", label: "Image", kind: PortKind::Raster }],
            output: Some(PortKind::Raster),
            params: vec![
                set_param(),
                Param::new("method", "Method", ParamKind::Enum).default("affine").options(&[
                    ("affine", "Affine (3+ points)"),
                    ("poly2", "2nd-order polynomial (6+ points)"),
                    ("poly3", "3rd-order polynomial (10+ points)"),
                    ("spline", "Thin plate spline (3+ points, passes through every point)"),
                ]),
                Param::new("resampling", "Resampling", ParamKind::Enum).default("nearest").options(&[
                    ("nearest", "Nearest (keeps the original colours)"),
                    ("bilinear", "Bilinear"),
                    ("cubic", "Cubic"),
                    ("cubicspline", "Cubic spline"),
                    ("lanczos", "Lanczos"),
                ]),
            ],
        }
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let src = need_raster(i)?;
        let name = p.req_str("set", "Control point set")?;
        let set = super::set_named(ctx.conn, &name)?;
        let method = raster_method(&p.str("method").unwrap_or_default())?;
        let resampling = p.str("resampling").unwrap_or_default();
        // A set that can't be used (too few points, collinear, no CRS) fails here, before a task or a file exists.
        warp::errors(&set, method)?;
        let stem = Path::new(src).file_stem().and_then(|s| s.to_str()).unwrap_or("image").to_string();
        let task = Task::start(format!("Georeferencing {stem}")).cancellable();
        let dest = crate::raster::new_store_path(&format!("{stem}_georef"), "tif");
        let wctx = warp::Ctx { stop: Arc::new(AtomicBool::new(false)), task: Some(task.handle()) };
        let (ds, out) = warp::run(Path::new(src), &set, method, &resampling, Dest::GeoTiff(&dest), None, &wctx).map_err(|e| {
            let _ = std::fs::remove_file(&dest);
            e
        })?;
        drop(ds);
        task.detail("Building overviews");
        if let Err(e) = crate::raster::build_overviews(&dest) {
            let _ = std::fs::remove_file(&dest);
            return Err(e);
        }
        let r = &out.report;
        ctx.add_note(format!(
            "Georeferenced with {} fitted to {} points: RMS {:.3} {} ({:.2} m).{}",
            r.method_label,
            r.enabled_count,
            r.rms,
            r.unit,
            r.rms_m,
            r.warnings.iter().map(|w| format!(" {w}")).collect::<String>()
        ));
        Ok(Output::Raster { path: dest.display().to_string(), crs: Some(set.target_crs.clone()) })
    }
}

/// Vector adjustment: shapes are moved with the set's points, in the layer's own coordinate system.
pub struct AdjustShapes;

fn adjust_method(id: &str) -> Result<AdjustMethod> {
    Ok(match id {
        "" | "affine" => AdjustMethod::Affine,
        "similarity" => AdjustMethod::Similarity,
        "rubber_sheet" => AdjustMethod::RubberSheet,
        other => bail!("'{other}' is not an adjustment method."),
    })
}

static SCRATCH: AtomicU64 = AtomicU64::new(0);

impl Tool for AdjustShapes {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "adjust_shapes",
            name: "Adjust Shapes (Control Points)",
            category: "Coordinate Systems",
            role: Role::Tool,
            description: "Move a layer's shapes with a saved set of control points (From = where a place is now, To = where it should be): affine, similarity or rubber sheet. The result stays in the layer's coordinate system; attributes are unchanged; shapes that cannot be moved are left out and listed.",
            inputs: vec![Port { id: "in", label: "Features", kind: PortKind::Vector }],
            output: Some(PortKind::Vector),
            params: vec![
                set_param(),
                Param::new("method", "Method", ParamKind::Enum).default("affine").options(&[
                    ("affine", "Affine (3+ points)"),
                    ("similarity", "Similarity: scale, rotation, shift (2+ points)"),
                    ("rubber_sheet", "Rubber sheet: thin plate spline (3+ points)"),
                ]),
            ],
        }
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let crs = need_crs(i)?.to_string();
        let name = p.req_str("set", "Control point set")?;
        let set = super::set_named(ctx.conn, &name)?;
        let method = adjust_method(&p.str("method").unwrap_or_default())?;
        // Validates the set against this layer and fits; a problem is a sentence, before any copy is made.
        let prep = adjust::prepare(ctx.conn, &crs, &set, method)?;
        let n = SCRATCH.fetch_add(1, Ordering::Relaxed);
        let (src, scratch) = (format!("__adjn_{n}_src"), format!("__adjn_{n}"));
        let drop_both = |c: &duckdb::Connection| {
            let _ = c.execute_batch(&format!("DROP TABLE IF EXISTS {}; DROP TABLE IF EXISTS {}", ident(&src), ident(&scratch)));
        };
        // The input has no stable row id of its own, so number the rows once and work from that copy.
        ctx.conn.execute_batch(&format!(
            "CREATE OR REPLACE TEMP TABLE {} AS SELECT *, row_number() OVER () AS __adj_rn FROM {}; CREATE OR REPLACE TEMP TABLE {} (fid BIGINT, wkb BLOB, bad VARCHAR)",
            ident(&src),
            ident(&i.table),
            ident(&scratch)
        ))?;
        let task = Task::start("Adjusting shapes").cancellable();
        let (dropped, total) = match adjust::move_shapes(ctx.conn, &task, &prep.fitted, &crs, &scratch, &ident(&src), "__adj_rn", "") {
            Ok(v) => v,
            Err(e) => {
                drop_both(ctx.conn);
                return Err(e);
            }
        };
        for n in &prep.notes {
            ctx.add_note(n.clone());
        }
        ctx.add_note(format!("Adjusted {} feature{} with a {} fitted to {} points (RMS {:.3} {}).", total - dropped.len() as i64, if total - dropped.len() as i64 == 1 { "" } else { "s" }, prep.report.method_label, prep.report.enabled_count, prep.report.rms, prep.report.unit));
        for w in &prep.report.warnings {
            ctx.add_note(w.clone());
        }
        if !dropped.is_empty() {
            ctx.add_note(format!(
                "{} feature{} left out of the result (row numbers in the input): {}.",
                dropped.len(),
                if dropped.len() == 1 { " was" } else { "s were" },
                dropped.iter().take(20).map(|d| format!("#{} ({})", d.fid, d.reason)).collect::<Vec<_>>().join("; ")
            ));
        }
        // The two scratch tables are dropped by the same script that builds the result, so a cancelled step cleans up with it.
        let (s, a) = (ident(&src), ident(&scratch));
        Ok(Output::Script {
            statements: vec![
                format!("CREATE TABLE \"{{out}}\" AS SELECT s.* EXCLUDE (__adj_rn) REPLACE (CASE WHEN a.wkb IS NULL THEN s.geom ELSE ST_GeomFromWKB(a.wkb) END AS geom) FROM {s} s LEFT JOIN {a} a ON a.fid = s.__adj_rn WHERE s.geom IS NULL OR (a.fid IS NOT NULL AND a.bad IS NULL) ORDER BY s.__adj_rn"),
                format!("DROP TABLE IF EXISTS {a}"),
                format!("DROP TABLE IF EXISTS {s}"),
            ],
            crs: Some(crs),
        })
    }
}
