//! USNG / MGRS tools: add a grid reference field, turn a table of references into points, and draw the grid itself.
//! The grid and its lettering live in `usng.rs` (one definition; the Export Table dialog and the layout builder use it too).

use super::{NewField, OutFields};
use super::*;
use crate::layout::geo::{Body, FrameCrs};
use crate::usng::{self, Datum, Style};
use std::collections::HashMap;

const STYLES: &[(&str, &str)] = &[("usng", "USNG (15T VG 97470 47556)"), ("mgrs", "MGRS (15TVG9747047556)")];
const DATUMS: &[(&str, &str)] = &[("auto", "NAD 83 in North America, WGS 84 elsewhere"), ("nad83", "NAD 83"), ("wgs84", "WGS 84")];
const PRECISIONS: &[(&str, &str)] = &[
    ("5", "1 m (5 digits)"),
    ("4", "10 m (4 digits)"),
    ("3", "100 m (3 digits)"),
    ("2", "1 km (2 digits)"),
    ("1", "10 km (1 digit)"),
    ("0", "100 km square only"),
];

fn precision(p: &Params) -> u8 {
    p.str("precision").and_then(|s| s.parse().ok()).filter(|n| *n <= 5).unwrap_or(5)
}

/// Add a USNG or MGRS reference for each feature (a point on a line or polygon) as a text field.
pub struct AddUsng;

impl Tool for AddUsng {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "add_usng",
            name: "Add USNG / MGRS Field",
            category: "Coordinate Systems",
            role: Role::Tool,
            description: "Add the United States National Grid or Military Grid Reference System reference of each feature (a point on lines and polygons) as a text field. The UTM zone is chosen for each feature.",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Vector }],
            output: Some(PortKind::Vector),
            params: vec![
                Param::new("field", "Field name", ParamKind::Text).default("USNG"),
                Param::new("style", "Written as", ParamKind::Enum).default("usng").options(STYLES).help("USNG and MGRS are the same grid; USNG puts spaces in"),
                Param::new("precision", "Precision", ParamKind::Enum).default("5").options(PRECISIONS),
                Param::new("datum", "Datum", ParamKind::Enum).default("auto").options(DATUMS).help("USNG is defined on NAD 83 (the same as WGS 84 to about a metre in the United States)"),
            ],
        }
    }

    fn output_fields(&self) -> OutFields {
        OutFields::from(&["in"]).add(NewField::new("USNG", "VARCHAR").named_by("field"))
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let crs = need_crs(i)?.to_string();
        let field = p.req_str("field", "Field name")?;
        let style = Style::parse(&p.str("style").unwrap_or_default())?;
        let datum = Datum::parse(&p.str("datum").unwrap_or_default())?;
        let pt = if ctx.dimension(i)? == 0 { "t.geom" } else { "ST_PointOnSurface(t.geom)" };
        let (sql, skipped) = usng::add_field_sql(Some(ctx.conn), &i.table, &crs, pt, &field, precision(p), style, datum, None)?;
        if skipped > 0 {
            ctx.add_note(format!("{skipped} feature{} {} no {} reference: no location, or outside 80°S–84°N where the grid is not defined here.", if skipped == 1 { "" } else { "s" }, if skipped == 1 { "has" } else { "have" }, style.name()));
        }
        Ok(Output::Select { sql: format!("SELECT * EXCLUDE (geom), geom FROM ({sql})"), crs: i.crs.clone(), view: false })
    }
}

/// Points from a table of USNG or MGRS references.
pub struct UsngToPoints;

impl Tool for UsngToPoints {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "usng_to_points",
            name: "USNG / MGRS to Points",
            category: "Coordinate Systems",
            role: Role::Tool,
            description: "Make points from a field of USNG or MGRS references (any spacing or case; fewer digits name a square). Rows that are not valid references are left out and counted.",
            inputs: vec![Port { id: "in", label: "Table with references", kind: PortKind::Table }],
            output: Some(PortKind::Vector),
            params: vec![
                Param::new("field", "Reference field", ParamKind::Field).field_of("in"),
                Param::new("where", "Place the point at", ParamKind::Enum).default("centre").options(&[("centre", "The centre of the named square"), ("corner", "Its lower-left corner")]),
                Param::new("datum", "Datum", ParamKind::Enum).default("auto").options(DATUMS),
            ],
        }
    }

    fn output_fields(&self) -> OutFields {
        OutFields::from(&["in"])
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let field = p.req_str("field", "Reference field")?;
        let datum = Datum::parse(&p.str("datum").unwrap_or_default())?;
        let centre = p.str("where").as_deref() != Some("corner");
        let t = ident(&i.table);
        let values: Vec<String> = {
            let mut st = ctx.conn.prepare(&format!("SELECT DISTINCT CAST({f} AS VARCHAR) FROM {t} WHERE {f} IS NOT NULL", f = ident(&field)))?;
            let rows = st.query_map([], |r| r.get::<_, String>(0))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        if values.len() > 300_000 {
            bail!("{} different references is more than one step handles (300,000); split the table first", values.len());
        }
        let body = Body::get("earth")?;
        let crs_cache: std::cell::RefCell<HashMap<String, FrameCrs>> = std::cell::RefCell::new(HashMap::new());
        let (mut rows, mut bad, mut first_error): (Vec<String>, usize, Option<String>) = (Vec::new(), 0, None);
        let mut any_wgs = false;
        for v in &values {
            let one = (|| -> Result<(f64, f64)> {
                let r = usng::parse(v)?;
                let north = r.band >= 'N';
                let code = usng::utm_crs(r.zone, north, datum);
                if !code.starts_with("EPSG:269") {
                    any_wgs = true;
                }
                let to_ll = |e: f64, n: f64, _north: bool| -> Result<(f64, f64)> {
                    let mut cache = crs_cache.borrow_mut();
                    if !cache.contains_key(&code) {
                        cache.insert(code.clone(), FrameCrs::new(&code, &body)?);
                    }
                    cache[&code].to_lonlat(e, n)
                };
                let (e, n, _) = usng::to_utm(&r, &to_ll)?;
                let half = if centre { r.cell_m() / 2.0 } else { 0.0 };
                to_ll(e + half, n + half, north)
            })();
            match one {
                Ok((lon, lat)) => rows.push(format!("({}, {lon}, {lat})", lit(v))),
                Err(e) => {
                    bad += 1;
                    first_error.get_or_insert(e.to_string());
                }
            }
        }
        if rows.is_empty() {
            bail!("None of the {} values in '{field}' is a valid USNG/MGRS reference. {}", values.len(), first_error.unwrap_or_default());
        }
        let out_crs = if any_wgs || datum == Datum::Wgs84 { "EPSG:4326" } else { "EPSG:4269" };
        if bad > 0 {
            ctx.add_note(format!("{bad} different value{} in '{field}' {} not a valid USNG/MGRS reference and those rows were left out (first: {}).", if bad == 1 { "" } else { "s" }, if bad == 1 { "is" } else { "are" }, first_error.unwrap_or_default()));
        }
        // Table inputs may already carry a geometry column (a vector layer): the new points replace it.
        let has_geom: bool = ctx.conn.query_row(&format!("SELECT count(*) > 0 FROM information_schema.columns WHERE table_name = {} AND column_name = 'geom'", lit(&i.table)), [], |r| r.get(0)).unwrap_or(false);
        let star = if has_geom { "t.* EXCLUDE (geom)" } else { "t.*" };
        Ok(Output::Select {
            sql: format!("SELECT {star}, ST_Point(m.lon, m.lat) AS geom FROM {t} t JOIN (VALUES {}) m(k, lon, lat) ON CAST(t.{} AS VARCHAR) = m.k", rows.join(", "), ident(&field)),
            crs: Some(out_crs.into()),
            view: false,
        })
    }
}

/// The USNG / MGRS grid as squares over the input's extent.
pub struct UsngGrid;

impl Tool for UsngGrid {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "usng_grid",
            name: "USNG / MGRS Grid",
            category: "Coordinate Systems",
            role: Role::Tool,
            description: "Squares of the USNG/MGRS grid (100 km, 10 km, 1 km or 100 m) over the input's extent, in the UTM zone at its centre, each labelled with its reference.",
            inputs: vec![Port { id: "in", label: "Extent from", kind: PortKind::Vector }],
            output: Some(PortKind::Vector),
            params: vec![
                Param::new("interval", "Square size", ParamKind::Enum).default("1000").options(&[("100000", "100 km"), ("10000", "10 km"), ("1000", "1 km"), ("100", "100 m")]),
                Param::new("style", "Written as", ParamKind::Enum).default("usng").options(STYLES),
                Param::new("datum", "Datum", ParamKind::Enum).default("auto").options(DATUMS),
                Param::new("clip", "Only squares touching features", ParamKind::Bool).default(false),
            ],
        }
    }

    fn output_fields(&self) -> OutFields {
        OutFields::from(&[]).add(NewField::new("USNG", "VARCHAR"))
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let crs = need_crs(i)?.to_string();
        let style = Style::parse(&p.str("style").unwrap_or_default())?;
        let datum = Datum::parse(&p.str("datum").unwrap_or_default())?;
        let size: f64 = p.f64("interval").unwrap_or(1000.0);
        let digits = match size as i64 {
            100_000 => 0,
            10_000 => 1,
            1_000 => 2,
            100 => 3,
            _ => bail!("Square size must be 100 km, 10 km, 1 km or 100 m"),
        };
        let (lon, lat) = ctx.center_lonlat(i)?;
        if !(-80.0..=84.0).contains(&lat) {
            bail!("The USNG/MGRS grid covers 80°S to 84°N; this layer is centred at {lat:.1}°");
        }
        let zone = usng::zone_of(lon, lat);
        let north = lat >= 0.0;
        let utm = usng::utm_crs(zone, north, datum);
        let (c, u) = (lit(&crate::crs::resolve(&crs)), lit(&utm));
        let (x0, y0, x1, y1): (f64, f64, f64, f64) = ctx.conn.query_row(
            &format!("SELECT min(ST_XMin(g)), min(ST_YMin(g)), max(ST_XMax(g)), max(ST_YMax(g)) FROM (SELECT ST_Transform(geom, {c}, {u}, true) AS g FROM {})", ident(&i.table)),
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
        let (ix0, ix1, iy0, iy1) = ((x0 / size).floor() as i64, (x1 / size).floor() as i64, (y0 / size).floor() as i64, (y1 / size).floor() as i64);
        let n = (ix1 - ix0 + 1) * (iy1 - iy0 + 1);
        if n > 200_000 {
            bail!("That would make {n} squares. Use a larger square size or a smaller area.");
        }
        ctx.add_note(format!("The grid is drawn in {utm} (UTM zone {zone}{}); squares beyond this zone's edge keep this zone's lines.", if north { "N" } else { "S" }));
        let cells = format!(
            "SELECT ST_MakeEnvelope((ix) * {size}, (iy) * {size}, (ix + 1) * {size}, (iy + 1) * {size}) AS geom FROM range({ix0}, {}) a(ix), range({iy0}, {}) b(iy)",
            ix1 + 1,
            iy1 + 1
        );
        let (labelled, _) = usng::add_field_sql(None, "\"{out}_cells\"", &utm, "ST_Centroid(t.geom)", "USNG", digits, style, datum, Some((zone, north)))?;
        let touch = if p.bool("clip") {
            format!(" WHERE EXISTS (SELECT 1 FROM {} f WHERE ST_Intersects(l.geom, ST_Transform(f.geom, {c}, {u}, true)))", ident(&i.table))
        } else {
            String::new()
        };
        Ok(Output::Script {
            statements: vec![
                format!("CREATE OR REPLACE TABLE \"{{out}}_cells\" AS {cells}"),
                format!("CREATE OR REPLACE TABLE \"{{out}}\" AS SELECT USNG, geom FROM ({labelled}) l{touch}"),
                "DROP TABLE \"{out}_cells\"".to_string(),
            ],
            crs: Some(utm),
        })
    }
}
