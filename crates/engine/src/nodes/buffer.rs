use super::OutFields;
use super::*;
use crate::crs::utm_epsg;

pub struct Buffer;

impl Tool for Buffer {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "buffer",
            name: "Buffer",
            category: "Proximity",
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
        let sql = if p.bool("dissolve") {
            format!("SELECT ST_Union_Agg({buffered}) AS geom FROM {}", ident(&i.table))
        } else {
            format!("SELECT * REPLACE ({buffered} AS geom) FROM {}", ident(&i.table))
        };
        Ok(Output::Select { sql, crs: Some(crs), view: false })
    }
}
