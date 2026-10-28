use super::{NewField, OutFields};
use super::*;
use gdal::vector::Geometry;

/// Largest input (vertices are read from every feature).
const MAX_FEATURES: i64 = 200_000;

pub struct Delaunay;

impl Tool for Delaunay {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "delaunay",
            name: "Delaunay Triangulation",
            category: "Generalize",
            role: Role::Tool,
            description: "Triangles joining the vertices of all input features so no vertex lies inside another triangle's circumcircle (QGIS: Delaunay triangulation, native:delaunaytriangulation). Triangulated in the layer's coordinates, so use a projected CRS for well-shaped triangles.",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Vector }],
            output: Some(PortKind::Vector),
            params: vec![],
        }
    }

    fn output_fields(&self) -> OutFields {
        OutFields::from(&[]).add(NewField::new("TRI_ID", "BIGINT"))
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, _p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let t = ident(&i.table);
        let n: i64 = ctx.conn.query_row(&format!("SELECT count(*) FROM {t}"), [], |r| r.get(0))?;
        if n == 0 {
            bail!("The input has no features to triangulate");
        }
        if n > MAX_FEATURES {
            bail!("Delaunay triangulation supports up to {MAX_FEATURES} features (input has {n})");
        }
        let (into, back) = ctx.metric_wrap(i)?;
        let wkb: Vec<u8> = ctx.conn.query_row(
            &format!("SELECT ST_AsWKB(ST_Collect(list({})))::BLOB FROM {t} WHERE geom IS NOT NULL", into.replace("{}", "geom")),
            [],
            |r| r.get(0),
        )?;
        let tri = Geometry::from_wkb(&wkb)
            .and_then(|g| g.delaunay_triangulation(None))
            .and_then(|g| g.wkb())
            .map_err(|e| anyhow!("Delaunay triangulation failed: {e}"))?;
        let hex: String = tri.iter().map(|b| format!("{b:02X}")).collect();
        let sql = format!(
            "SELECT row_number() OVER () AS TRI_ID, {} AS geom
             FROM (SELECT UNNEST(ST_Dump(ST_GeomFromHEXWKB('{hex}'))) AS __part)",
            back.replace("{}", "__part.geom")
        );
        Ok(Output::Select { sql, crs: i.crs.clone(), view: false })
    }
}
