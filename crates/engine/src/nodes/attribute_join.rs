use super::*;

pub struct AttributeJoin;

impl Tool for AttributeJoin {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "attribute_join",
            name: "Join Field",
            category: "Tables",
            role: Role::Tool,
            description: "Join a table to features on matching key fields (e.g. GEOID).",
            inputs: vec![
                Port { id: "target", label: "Input", kind: PortKind::Table },
                Port { id: "join", label: "Join table", kind: PortKind::Table },
            ],
            output: Some(PortKind::Table),
            params: vec![
                Param::new("target_field", "Input key field", ParamKind::Field).field_of("target"),
                Param::new("join_field", "Join key field", ParamKind::Field).field_of("join"),
                Param::new("keep_all", "Keep all input rows", ParamKind::Bool).default(true),
            ],
        }
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let (a, b) = (&inputs["target"], &inputs["join"]);
        let tf = p.req_str("target_field", "Input key field")?;
        let jf = p.req_str("join_field", "Join key field")?;
        let a_cols = ctx.columns(a)?;
        let b_only: Vec<String> = ctx.columns(b)?.into_iter().filter(|c| c != &jf).collect();
        let b_cols = join_columns(&a_cols, &b_only, "b");
        let b_sel = if b_cols.is_empty() { String::new() } else { format!(", {b_cols}") };
        let join = if p.bool("keep_all") { "LEFT JOIN" } else { "JOIN" };
        // Compare as text so "01" (string) and 1 (number) keys behave predictably.
        let sql = format!(
            "SELECT a.*{b_sel} FROM {at} a {join} {bt} b ON CAST(a.{tf} AS VARCHAR) = CAST(b.{jf} AS VARCHAR)",
            at = ident(&a.table),
            bt = ident(&b.table),
            tf = ident(&tf),
            jf = ident(&jf)
        );
        Ok(Output::Select { sql, crs: a.crs.clone(), view: false })
    }
}
