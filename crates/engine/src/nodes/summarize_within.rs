use super::{NewField, OutFields};
use super::*;

pub struct SummarizeWithin;

impl Tool for SummarizeWithin {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "summarize_within",
            name: "Summarize Within",
            category: "Analysis › Summaries",
            role: Role::Tool,
            description: "For each polygon, count the features inside it and optionally sum/average one of their fields.",
            inputs: vec![
                Port { id: "polys", label: "Polygons", kind: PortKind::Vector },
                Port { id: "items", label: "Features to summarize", kind: PortKind::Vector },
            ],
            output: Some(PortKind::Vector),
            params: vec![
                Param::new("field", "Summary field", ParamKind::Field).field_of("items").optional(),
                Param::new("stat", "Statistic", ParamKind::Enum).default("sum").options(&[
                    ("sum", "Sum"),
                    ("avg", "Mean"),
                    ("min", "Minimum"),
                    ("max", "Maximum"),
                ]),
            ],
        }
    }

    fn output_fields(&self) -> OutFields {
        OutFields::from(&["polys"]).add(NewField::new("COUNT", "BIGINT"))
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let (a, b) = (&inputs["polys"], &inputs["items"]);
        let crs = need_crs(a)?.to_string();
        let bg = ctx.geom_in(b, "b", &crs)?;
        let stat = match p.str("stat").as_deref().unwrap_or("sum") {
            s @ ("sum" | "avg" | "min" | "max") => s.to_string(),
            s => bail!("Unknown statistic {s}"),
        };
        let (extra_sel, extra_out) = match p.str("field") {
            Some(f) => {
                let name = format!("{}_{}", stat.to_uppercase(), f);
                (
                    format!(", {stat}(TRY_CAST(b2.{} AS DOUBLE)) AS {}", ident(&f), ident(&name)),
                    format!(", s.{}", ident(&name)),
                )
            }
            None => (String::new(), String::new()),
        };
        // Points count where they fall; lines/polygons count where their center falls.
        let sql = format!(
            "WITH a AS (SELECT row_number() OVER () AS __rid, * FROM {at}),
             b2 AS (SELECT * REPLACE ({bg} AS geom) FROM {bt} b),
             s AS (SELECT a.__rid, count(*) AS COUNT{extra_sel}
                   FROM a JOIN b2 ON ST_Intersects(a.geom, ST_PointOnSurface(b2.geom)) GROUP BY a.__rid)
             SELECT a.* EXCLUDE (__rid, geom), coalesce(s.COUNT, 0) AS COUNT{extra_out}, a.geom
             FROM a LEFT JOIN s ON a.__rid = s.__rid",
            at = ident(&a.table),
            bt = ident(&b.table)
        );
        Ok(Output::Select { sql, crs: Some(crs), view: false })
    }
}
