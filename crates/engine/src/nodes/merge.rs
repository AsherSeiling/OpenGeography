use super::*;

pub struct Merge;

impl Tool for Merge {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "merge",
            name: "Merge",
            category: "General",
            role: Role::Tool,
            description: "Combine the rows of two layers into one. Fields are matched by name; missing values are left empty.",
            inputs: vec![
                Port { id: "a", label: "Input", kind: PortKind::Table },
                Port { id: "b", label: "Merge with", kind: PortKind::Table },
            ],
            output: Some(PortKind::Table),
            params: vec![Param::new("source_field", "Add source field", ParamKind::Bool).default(true)
                .help("Adds MERGE_SRC = 1 or 2 so you can tell rows apart")],
        }
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let (a, b) = (&inputs["a"], &inputs["b"]);
        let src = |n: u8| if p.bool("source_field") { format!(", {n} AS MERGE_SRC") } else { String::new() };
        let (sel_b, crs) = match (a.has_geom, b.has_geom) {
            (true, true) => {
                let crs = need_crs(a)?.to_string();
                (format!("SELECT * REPLACE ({} AS geom){} FROM {} b", ctx.geom_in(b, "b", &crs)?, src(2), ident(&b.table)), Some(crs))
            }
            _ => (format!("SELECT *{} FROM {} b", src(2), ident(&b.table)), a.crs.clone()),
        };
        let sql = format!("SELECT *{} FROM {} UNION ALL BY NAME {sel_b}", src(1), ident(&a.table));
        Ok(Output::Select { sql, crs, view: false })
    }
}
