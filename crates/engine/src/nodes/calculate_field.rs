use super::{NewField, OutFields};
use super::*;

pub struct CalculateField;

impl Tool for CalculateField {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "calculate_field",
            name: "Calculate Field",
            category: "Tables",
            role: Role::Tool,
            description: "Add or update a field from an expression.",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Table }],
            output: Some(PortKind::Table),
            params: vec![
                Param::new("field", "Field name", ParamKind::Text).default("NEW_FIELD"),
                Param::new("expression", "Expression", ParamKind::Expression)
                    .field_of("in")
                    .help("SQL expression, e.g. POP / AREA_SQMI  or  upper(NAME)"),
            ],
        }
    }

    fn output_fields(&self) -> OutFields {
        OutFields::from(&["in"]).add(NewField::new("", "").named_by("field"))
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let f = p.req_str("field", "Field name")?;
        let e = p.req_str("expression", "Expression")?;
        if f.eq_ignore_ascii_case("geom") {
            bail!("'geom' is reserved for the geometry");
        }
        let exists = ctx.columns(i)?.iter().any(|c| c == &f);
        let sql = if exists {
            format!("SELECT * REPLACE (({e}) AS {f}) FROM {}", ident(&i.table), f = ident(&f))
        } else {
            format!("SELECT *, ({e}) AS {f} FROM {}", ident(&i.table), f = ident(&f))
        };
        Ok(Output::Select { sql, crs: i.crs.clone(), view: false })
    }
}
