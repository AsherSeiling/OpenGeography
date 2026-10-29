use super::*;

pub struct SummaryStatistics;

impl Tool for SummaryStatistics {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "summary_statistics",
            name: "Summary Statistics",
            category: "Analysis › Summaries",
            role: Role::Tool,
            description: "Count, sum, mean, min and max of a field, overall or per category. Outputs a table.",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Table }],
            output: Some(PortKind::Table),
            params: vec![
                Param::new("field", "Statistics field", ParamKind::Field).field_of("in"),
                Param::new("case", "Group by (case field)", ParamKind::Field).field_of("in").optional(),
            ],
        }
    }

    fn build(&self, _ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let f = ident(&p.req_str("field", "Statistics field")?);
        let v = format!("TRY_CAST({f} AS DOUBLE)");
        let stats = format!("count(*) AS COUNT, sum({v}) AS SUM, avg({v}) AS MEAN, min({v}) AS MIN, max({v}) AS MAX, stddev_samp({v}) AS STD");
        let sql = match p.str("case") {
            Some(c) => format!("SELECT {c}, {stats} FROM {t} GROUP BY {c} ORDER BY {c}", c = ident(&c), t = ident(&i.table)),
            None => format!("SELECT {stats} FROM {}", ident(&i.table)),
        };
        Ok(Output::Select { sql, crs: None, view: false })
    }
}
