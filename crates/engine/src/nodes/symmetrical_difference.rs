use super::OutFields;
use super::union::{minus_sql, overlay_ctes};
use super::*;

pub struct SymmetricalDifference;

impl Tool for SymmetricalDifference {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "symmetrical_difference",
            name: "Symmetrical Difference",
            category: "Vector › Overlay",
            role: Role::Tool,
            description: "The parts of either layer that the other does not cover (the overlap is dropped). QGIS: Symmetrical difference, native:symmetricaldifference.",
            inputs: vec![
                Port { id: "a", label: "Input", kind: PortKind::Vector },
                Port { id: "b", label: "Overlay features", kind: PortKind::Vector },
            ],
            output: Some(PortKind::Vector),
            params: vec![],
        }
    }

    fn output_fields(&self) -> OutFields {
        OutFields::from(&["a", "b"])
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, _p: &Params) -> Result<Output> {
        let (ctes, crs, ta, tb) = overlay_ctes(ctx, inputs)?;
        let sql = format!(
            "WITH {ctes},
             a_only AS ({a_only}),
             b_only AS ({b_only})
             SELECT * FROM a_only UNION ALL BY NAME SELECT * FROM b_only",
            a_only = minus_sql("a", "b2", ta),
            b_only = minus_sql("b2", "a", tb),
        );
        Ok(Output::Select { sql, crs: Some(crs), view: false })
    }
}
