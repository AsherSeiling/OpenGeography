use super::*;

pub struct Filter;

impl Tool for Filter {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "filter",
            name: "Definition Query",
            category: "Vector › Select and Clean",
            role: Role::Tool,
            description: "Keep only features matching a query (build it point-and-click, in Python, or SQL).",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Table }],
            output: Some(PortKind::Table),
            params: vec![
                Param::new("language", "Language", ParamKind::Enum).default("sql").options(&[("sql", "SQL"), ("python", "Python")]),
                Param::new("where", "Query", ParamKind::Query)
                    .field_of("in")
                    .help("e.g. POP100 > 1000 AND COUNTY = '109'. In Python: def calculate(row): return row[\"POP100\"] > 1000 (return True or False)"),
                Param::new("on_failed_rows", "If a Python row fails", ParamKind::Enum)
                    .default("stop")
                    .options(&[("stop", "Stop and change nothing"), ("skip", "Drop those rows and report them")])
                    .optional(),
            ],
        }
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let w = p.req_str("where", "Query")?;
        if p.str("language").as_deref() == Some("python") {
            use crate::pyscript::{self, Mode, OutType};
            let ev = pyscript::evaluate(ctx.conn, &i.table, &p.raw_str("where").unwrap_or(w), Mode::Filter, OutType::Boolean, p.str("on_failed_rows").as_deref() == Some("skip"))?;
            if ev.failures.count > 0 {
                ctx.add_note(format!("{}. Those rows were dropped.", ev.failures.report(ev.total)));
            }
            let (snap, stage) = (ident(&ev.snapshot), ident(&ev.staging));
            return Ok(Output::Script {
                statements: vec![
                    format!("CREATE OR REPLACE TABLE \"{{out}}\" AS SELECT * EXCLUDE (__rn) FROM (SELECT s.* FROM {snap} s WHERE s.__rn IN (SELECT __rn FROM {stage}) ORDER BY s.__rn)"),
                    format!("DROP TABLE {snap}"),
                    format!("DROP TABLE {stage}"),
                ],
                crs: i.crs.clone(),
            });
        }
        Ok(Output::Select { sql: format!("SELECT * FROM {} WHERE ({w})", ident(&i.table)), crs: i.crs.clone(), view: false })
    }
}
