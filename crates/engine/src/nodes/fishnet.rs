use super::*;

pub struct Fishnet;

impl Tool for Fishnet {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "fishnet",
            name: "Create Fishnet",
            category: "Sampling",
            role: Role::Tool,
            description: "A grid of square cells (real ground size) covering the input's extent.",
            inputs: vec![Port { id: "in", label: "Extent from", kind: PortKind::Vector }],
            output: Some(PortKind::Vector),
            params: vec![
                Param::new("size", "Cell size", ParamKind::Number).default(1),
                Param::new("units", "Units", ParamKind::Enum).default("km").options(LINEAR_UNITS),
                Param::new("clip", "Only cells touching features", ParamKind::Bool).default(true),
            ],
        }
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
        let cells = format!(
            "SELECT row_number() OVER () AS CELL_ID, ix AS COL, iy AS ROW,
                    ST_Transform(ST_MakeEnvelope({x0} + ix * {size}, {y0} + iy * {size}, {x0} + (ix + 1) * {size}, {y0} + (iy + 1) * {size}), {u}, {c}, true) AS geom
             FROM range({nx}) a(ix), range({ny}) b(iy)"
        );
        let sql = if p.bool("clip") {
            format!("WITH cells AS ({cells}) SELECT * FROM cells WHERE EXISTS (SELECT 1 FROM {} f WHERE ST_Intersects(cells.geom, f.geom))", ident(&i.table))
        } else {
            cells
        };
        Ok(Output::Select { sql, crs: Some(crs), view: false })
    }
}
