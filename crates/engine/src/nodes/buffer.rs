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
            description: "Create polygons around features at a distance. Distances are true ground distances in any coordinate system.",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Vector }],
            output: Some(PortKind::Vector),
            params: vec![
                Param::new("distance", "Distance", ParamKind::Number).default(100),
                Param::new("distance_field", "Or distance from field", ParamKind::Field).field_of("in").optional()
                    .help("Each feature buffered by its own value (in the units below), like QGIS's data-defined distance"),
                Param::new("units", "Units", ParamKind::Enum).default("m").options(LINEAR_UNITS),
                Param::new("dissolve", "Dissolve all output", ParamKind::Bool).default(false),
            ],
        }
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
        // Buffer in the local UTM zone so the distance is in real meters,
        // whatever the input CRS (degrees, feet, Web Mercator...).
        let (lon, lat) = ctx.center_lonlat(i)?;
        let utm = utm_epsg(lon, lat);
        let buffered = format!(
            "ST_Transform(ST_Buffer(ST_Transform(geom, {c}, {u}, true), {d}, 16), {u}, {c}, true)",
            c = lit(&crs),
            u = lit(&utm)
        );
        let sql = if p.bool("dissolve") {
            format!("SELECT ST_Union_Agg({buffered}) AS geom FROM {}", ident(&i.table))
        } else {
            format!("SELECT * REPLACE ({buffered} AS geom) FROM {}", ident(&i.table))
        };
        Ok(Output::Select { sql, crs: Some(crs), view: false })
    }
}
