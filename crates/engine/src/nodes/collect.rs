use super::{NewField, OutFields};
use super::*;

pub struct Collect;

impl Tool for Collect {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "collect",
            name: "Singlepart To Multipart",
            category: "Vector › Geometry",
            role: Role::Tool,
            description: "Combine features into multipart features, optionally one per value of a field. Unlike Dissolve, shapes are not merged. Adds a COUNT field (QGIS: Collect geometries, native:collect).",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Vector }],
            output: Some(PortKind::Vector),
            params: vec![Param::new("field", "Group field", ParamKind::Field).field_of("in").optional()],
        }
    }

    fn output_fields(&self) -> OutFields {
        OutFields { keep_param: Some("field"), ..OutFields::from(&["in"]) }.add(NewField::new("COUNT", "BIGINT"))
    }

    fn build(&self, _ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let t = ident(&i.table);
        // Flatten existing multiparts first so the result is one clean multipart.
        let parts = format!("(SELECT * EXCLUDE (geom, __part), __part.geom AS geom FROM (SELECT *, UNNEST(ST_Dump(geom)) AS __part FROM {t}))");
        let sql = match p.str("field") {
            Some(f) => format!("SELECT {f}, count(*) AS COUNT, ST_Collect(list(geom)) AS geom FROM {parts} GROUP BY {f}", f = ident(&f)),
            None => format!("SELECT count(*) AS COUNT, ST_Collect(list(geom)) AS geom FROM {parts}"),
        };
        Ok(Output::Select { sql, crs: i.crs.clone(), view: false })
    }
}
