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
                Param::new("language", "Language", ParamKind::Enum).default("sql").options(&[("sql", "SQL"), ("python", "Python")]),
                Param::new("expression", "Expression", ParamKind::Expression)
                    .field_of("in")
                    .help("SQL expression, e.g. POP / AREA_SQMI  or  upper(NAME). In Python: def calculate(row): return row[\"POP\"] / row[\"AREA_SQMI\"]"),
                Param::new("field_type", "Python result type", ParamKind::Enum)
                    .default("auto")
                    .options(&[("auto", "Automatic"), ("integer", "Integer"), ("double", "Decimal"), ("text", "Text"), ("boolean", "Boolean"), ("date", "Date"), ("timestamp", "Date and time")])
                    .optional(),
                Param::new("on_failed_rows", "If a Python row fails", ParamKind::Enum)
                    .default("stop")
                    .options(&[("stop", "Stop and change nothing"), ("skip", "Leave those rows empty and report them")])
                    .optional(),
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
        if p.str("language").as_deref() == Some("python") {
            return python_field(ctx, i, &f, &p.raw_str("expression").unwrap_or(e), exists, p);
        }
        let sql = if exists {
            format!("SELECT * REPLACE (({e}) AS {f}) FROM {}", ident(&i.table), f = ident(&f))
        } else {
            format!("SELECT *, ({e}) AS {f} FROM {}", ident(&i.table), f = ident(&f))
        };
        Ok(Output::Select { sql, crs: i.crs.clone(), view: false })
    }
}

/// The Python form: `def calculate(row)` runs over the attributes of every row; its return value goes into the field.
fn python_field(ctx: &Ctx, i: &Rel, f: &str, script: &str, exists: bool, p: &Params) -> Result<Output> {
    use crate::pyscript::{self, Mode, OutType};
    let out_type = OutType::parse(p.str("field_type").as_deref())?;
    let skip = p.str("on_failed_rows").as_deref() == Some("skip");
    let ev = pyscript::evaluate(ctx.conn, &i.table, script, Mode::Field, out_type, skip)?;
    if ev.failures.count > 0 {
        ctx.add_note(format!("{}. Those rows were left empty.", ev.failures.report(ev.total)));
    }
    let value = ev.out_type.value_sql("o");
    let target = ident(f);
    let (snap, stage) = (ident(&ev.snapshot), ident(&ev.staging));
    let body = if exists {
        format!("SELECT s.* REPLACE (({value}) AS {target}) FROM {snap} s LEFT JOIN {stage} o USING (__rn) ORDER BY s.__rn")
    } else {
        format!("SELECT s.*, ({value}) AS {target} FROM {snap} s LEFT JOIN {stage} o USING (__rn) ORDER BY s.__rn")
    };
    Ok(Output::Script {
        statements: vec![format!("CREATE OR REPLACE TABLE \"{{out}}\" AS SELECT * EXCLUDE (__rn) FROM ({body})"), format!("DROP TABLE {snap}"), format!("DROP TABLE {stage}")],
        crs: i.crs.clone(),
    })
}
