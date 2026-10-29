use super::batching::{self, BIG_BUFFER};
use super::OutFields;
use super::*;
use crate::crs::utm_epsg;

pub struct Buffer;

impl Tool for Buffer {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "buffer",
            name: "Buffer",
            category: "Vector › Proximity",
            role: Role::Tool,
            description: "Polygons around features at a distance. Geodesic: each feature is buffered in its own azimuthal equidistant projection, so the distance is true on the ellipsoid anywhere (like ArcGIS's geodesic buffer).",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Vector }],
            output: Some(PortKind::Vector),
            params: vec![
                Param::new("distance", "Distance", ParamKind::Number).default(100),
                Param::new("distance_field", "Or distance from field", ParamKind::Field).field_of("in").optional()
                    .help("Each feature buffered by its own value (in the units below), like QGIS's data-defined distance"),
                Param::new("units", "Units", ParamKind::Enum).default("m").options(LINEAR_UNITS),
                Param::new("method", "Method", ParamKind::Enum).default("geodesic").options(&[
                    ("geodesic", "Geodesic (true distance anywhere)"),
                    ("planar", "Planar (local UTM zone, faster)"),
                ]),
                Param::new("dissolve", "Dissolve all output", ParamKind::Bool).default(false),
            ],
        }
    }

    fn output_fields(&self) -> OutFields {
        OutFields { none_if: Some("dissolve"), ..OutFields::default() }
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let crs = need_crs(i)?.to_string();
        let units = p.str("units").unwrap_or_else(|| "m".into());
        let per = to_meters(1.0, &units)?;
        let d = match p.str("distance_field") {
            Some(f) => format!("(coalesce(TRY_CAST({} AS DOUBLE), 0) * {per})", ident(&f)),
            None => format!("{}", to_meters(p.f64("distance").ok_or_else(|| anyhow!("'Distance' is required"))?, &units)?),
        };
        let c = lit(&crate::crs::resolve(&crs));
        let buffered = if p.str("method").as_deref() == Some("planar") {
            // One UTM zone for the whole layer: fast, accurate for local data.
            let (lon, lat) = ctx.center_lonlat(i)?;
            let u = lit(&utm_epsg(lon, lat));
            format!("ST_Transform(ST_Buffer(ST_Transform(geom, {c}, {u}, true), {d}, 16), {u}, {c}, true)")
        } else {
            // Geodesic: per-feature azimuthal equidistant projection centered on the feature.
            let a = aeqd_for("geom", &crs);
            format!("ST_Transform(ST_Buffer(ST_Transform(geom, {c}, {a}, true), {d}, 32), {a}, {c}, true)")
        };
        // Big layers: see nodes/batching.rs (B-T4). Geodesic features are
        // processed in order of their projection centre; Dissolve unions tile by tile.
        let geodesic = p.str("method").as_deref() != Some("planar");
        let dissolve = p.bool("dissolve");
        let rows = batching::row_count(ctx.conn, &i.table)?;
        if rows > BIG_BUFFER && (geodesic || dissolve) {
            let src = "\"{out}_src\"";
            let mut statements = Vec::new();
            let from = if geodesic {
                let a = aeqd_for("geom", &crs);
                statements.push(format!("CREATE OR REPLACE TABLE {src} AS SELECT row_number() OVER () AS __orig, * FROM {} ORDER BY {a}", ident(&i.table)));
                src.to_string()
            } else {
                ident(&i.table)
            };
            if dissolve {
                let tiles = batching::tiles(ctx.conn, &i.table, rows)?;
                statements.push(format!("CREATE OR REPLACE TABLE \"{{out}}_t\" AS SELECT {} AS __tx, {} AS __ty, {buffered} AS geom FROM {from}", tiles.x, tiles.y));
                statements.extend(batching::union_tiles(&tiles, None, false));
            } else {
                // Buffer in the sorted order, then restore the layer's own row order.
                statements.push(format!("CREATE OR REPLACE TABLE \"{{out}}_b\" AS SELECT * REPLACE ({buffered} AS geom) FROM {from}"));
                statements.push("CREATE OR REPLACE TABLE \"{out}\" AS SELECT * EXCLUDE (__orig) FROM \"{out}_b\" ORDER BY __orig".into());
                statements.push("DROP TABLE \"{out}_b\"".into());
            }
            if geodesic {
                statements.push(format!("DROP TABLE {src}"));
            }
            return Ok(Output::Script { statements, crs: Some(crs) });
        }
        let sql = if dissolve {
            format!("SELECT ST_Union_Agg({buffered}) AS geom FROM {}", ident(&i.table))
        } else {
            format!("SELECT * REPLACE ({buffered} AS geom) FROM {}", ident(&i.table))
        };
        Ok(Output::Select { sql, crs: Some(crs), view: false })
    }
}
