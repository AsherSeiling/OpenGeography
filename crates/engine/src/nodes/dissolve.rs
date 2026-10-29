use super::{NewField, OutFields};
use super::batching;
use super::*;

/// Above this many features, Dissolve works tile by tile.
const BIG_DISSOLVE: i64 = 20_000;

pub struct Dissolve;

impl Tool for Dissolve {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "dissolve",
            name: "Dissolve",
            category: "Vector › Generalize",
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

    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let t = ident(&i.table);
        // Big layers: partial unions per spatial tile, then a union of those, so
        // Cancel can stop between the pieces (B-T4).
        let rows = batching::row_count(ctx.conn, &i.table)?;
        if rows > BIG_DISSOLVE {
            let tiles = batching::tiles(ctx.conn, &i.table, rows)?;
            let field = p.str("field");
            let lead = field.as_ref().map(|f| format!("{}, ", ident(f))).unwrap_or_default();
            let mut statements = vec![format!("CREATE OR REPLACE TABLE \"{{out}}_t\" AS SELECT {lead}{} AS __tx, {} AS __ty, geom FROM {t}", tiles.x, tiles.y)];
            statements.extend(batching::union_tiles(&tiles, field.as_deref(), true));
            return Ok(Output::Script { statements, crs: i.crs.clone() });
        }
        let sql = match p.str("field") {
            Some(f) => format!("SELECT {f}, count(*) AS COUNT, ST_Union_Agg(geom) AS geom FROM {t} GROUP BY {f}", f = ident(&f)),
            None => format!("SELECT count(*) AS COUNT, ST_Union_Agg(geom) AS geom FROM {t}"),
        };
        Ok(Output::Select { sql, crs: i.crs.clone(), view: false })
    }
}
