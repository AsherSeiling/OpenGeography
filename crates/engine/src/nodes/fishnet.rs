use super::{NewField, OutFields};
use super::*;

pub struct Fishnet;

impl Tool for Fishnet {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "fishnet",
            name: "Create Fishnet",
            category: "Vector › Sampling",
            role: Role::Tool,
            description: "A grid of square or hexagonal cells (real ground size) covering the input's extent (QGIS: Create grid, native:creategrid).",
            inputs: vec![Port { id: "in", label: "Extent from", kind: PortKind::Vector }],
            output: Some(PortKind::Vector),
            params: vec![
                Param::new("shape", "Cell shape", ParamKind::Enum).default("square").options(&[("square", "Square"), ("hexagon", "Hexagon")]),
                Param::new("size", "Cell size", ParamKind::Number).default(1).help("Square side, or hexagon width (flat to flat)"),
                Param::new("units", "Units", ParamKind::Enum).default("km").options(LINEAR_UNITS),
                Param::new("clip", "Only cells touching features", ParamKind::Bool).default(true),
            ],
        }
    }

    fn output_fields(&self) -> OutFields {
        OutFields::from(&[]).add(NewField::new("CELL_ID", "BIGINT")).add(NewField::new("COL", "BIGINT")).add(NewField::new("ROW", "BIGINT"))
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let crs = need_crs(i)?.to_string();
        let utm = ctx.utm(i)?;
        let size = to_meters(p.f64("size").unwrap_or(1.0), &p.str("units").unwrap_or_else(|| "km".into()))?;
        if size <= 0.0 {
            bail!("Cell size must be greater than zero");
        }
        let (c, u) = (lit(&crs), lit(&utm));
        let (x0, y0, x1, y1): (f64, f64, f64, f64) = ctx.conn.query_row(
            &format!(
                "SELECT min(ST_XMin(g)), min(ST_YMin(g)), max(ST_XMax(g)), max(ST_YMax(g)) FROM (SELECT ST_Transform(geom, {c}, {u}, true) AS g FROM {})",
                ident(&i.table)
            ),
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
        let (nx, ny) = (((x1 - x0) / size).ceil() as i64, ((y1 - y0) / size).ceil() as i64);
        if nx * ny > 2_000_000 {
            bail!("That would make {} cells. Use a larger cell size.", nx * ny);
        }
        let cells = if p.str("shape").as_deref() == Some("hexagon") {
            // Pointy-top hexagons: width = size, rows 1.5 R apart, odd rows shifted half a cell.
            let r = size / 3f64.sqrt();
            let (nx, ny) = (nx + 2, ((y1 - y0) / (1.5 * r)).ceil() as i64 + 2);
            if nx * ny > 2_000_000 {
                bail!("That would make {} cells. Use a larger cell size.", nx * ny);
            }
            format!(
                "SELECT row_number() OVER () AS CELL_ID, ix AS COL, iy AS ROW,
                        ST_Transform(ST_MakePolygon(ST_MakeLine(list_transform(range(7), k ->
                            ST_Point({x0} + ix * {size} + (iy % 2) * {size} / 2 + {r} * cos(radians(30 + 60 * k)),
                                     {y0} + iy * {r} * 1.5 + {r} * sin(radians(30 + 60 * k)))))), {u}, {c}, true) AS geom
                 FROM range({nx}) a(ix), range({ny}) b(iy)"
            )
        } else {
            format!(
                "SELECT row_number() OVER () AS CELL_ID, ix AS COL, iy AS ROW,
                        ST_Transform(ST_MakeEnvelope({x0} + ix * {size}, {y0} + iy * {size}, {x0} + (ix + 1) * {size}, {y0} + (iy + 1) * {size}), {u}, {c}, true) AS geom
                 FROM range({nx}) a(ix), range({ny}) b(iy)"
            )
        };
        let sql = if p.bool("clip") {
            format!("WITH cells AS ({cells}) SELECT * FROM cells WHERE EXISTS (SELECT 1 FROM {} f WHERE ST_Intersects(cells.geom, f.geom))", ident(&i.table))
        } else {
            cells
        };
        Ok(Output::Select { sql, crs: Some(crs), view: false })
    }
}
