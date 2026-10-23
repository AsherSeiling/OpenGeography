use super::{NewField, OutFields};
use super::*;

pub struct Dissolve;

impl Tool for Dissolve {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "dissolve",
            name: "Dissolve",
            category: "Generalize",
            role: Role::Tool,
            description: "Merge features, optionally grouped by a field. Adds a COUNT field.",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Vector }],
            output: Some(PortKind::Vector),
            params: vec![Param::new("field", "Dissolve field", ParamKind::Field).field_of("in").optional()],
        }
    }

    fn output_fields(&self) -> OutFields {
        OutFields { keep_param: Some("field"), ..OutFields::from(&["in"]) }.add(NewField::new("COUNT", "BIGINT"))
    }

    fn build(&self, _ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let t = ident(&i.table);
        let sql = match p.str("field") {
            Some(f) => format!("SELECT {f}, count(*) AS COUNT, ST_Union_Agg(geom) AS geom FROM {t} GROUP BY {f}", f = ident(&f)),
            None => format!("SELECT count(*) AS COUNT, ST_Union_Agg(geom) AS geom FROM {t}"),
        };
        Ok(Output::Select { sql, crs: i.crs.clone(), view: false })
    }
}
