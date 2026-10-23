//! Vector tools that mirror QGIS Processing algorithms. Each is an original
//! implementation on DuckDB Spatial (QGIS's source is not used); the QGIS
//! algorithm id and documentation it matches are cited per tool, so names,
//! parameters and results line up for people coming from QGIS.
//! QGIS documentation: https://docs.qgis.org/latest/en/docs/user_manual/processing_algs/qgis/

use super::{NewField, OutFields};
use super::*;

macro_rules! tool {
    ($name:ident) => {
        pub struct $name;
    };
}

fn vin() -> Vec<Port> {
    vec![Port { id: "in", label: "Input", kind: PortKind::Vector }]
}

fn sel(sql: String, i: &Rel) -> Result<Output> {
    Ok(Output::Select { sql, crs: i.crs.clone(), view: false })
}

// ---------------------------------------------------------------------------
tool!(PolygonsToLines);
/// QGIS: Polygons to lines (native:polygonstolines) — vectorgeometry.html#polygons-to-lines
impl Tool for PolygonsToLines {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "polygons_to_lines", name: "Polygons to Lines", category: "Geometry", role: Role::Tool,
            description: "Outlines of polygons as lines (QGIS: Polygons to lines).", inputs: vin(), output: Some(PortKind::Vector), params: vec![] }
    }
    fn build(&self, _c: &Ctx, inputs: &Inputs, _p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        sel(format!("SELECT * REPLACE (ST_Boundary(geom) AS geom) FROM {} WHERE ST_Dimension(geom) = 2", ident(&i.table)), i)
    }
}

tool!(LinesToPolygons);
/// QGIS: Lines to polygons (native:linestopolygons) — vectorgeometry.html#lines-to-polygons
impl Tool for LinesToPolygons {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "lines_to_polygons", name: "Lines to Polygons", category: "Geometry", role: Role::Tool,
            description: "Polygons from closed lines (rings); open lines are skipped (QGIS: Lines to polygons).", inputs: vin(), output: Some(PortKind::Vector), params: vec![] }
    }
    fn build(&self, _c: &Ctx, inputs: &Inputs, _p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        sel(format!("SELECT * FROM (SELECT * REPLACE (ST_CollectionExtract(ST_Polygonize([geom]), 3) AS geom) FROM {}) WHERE NOT ST_IsEmpty(geom)", ident(&i.table)), i)
    }
}

tool!(ExtractVertices);
/// QGIS: Extract vertices (native:extractvertices) — vectorgeometry.html#extract-vertices
impl Tool for ExtractVertices {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "extract_vertices", name: "Extract Vertices", category: "Geometry", role: Role::Tool,
            description: "One point per vertex, with VERTEX_INDEX (QGIS: Extract vertices).", inputs: vin(), output: Some(PortKind::Vector), params: vec![] }
    }
    fn output_fields(&self) -> OutFields {
        OutFields::from(&["in"]).add(NewField::new("VERTEX_INDEX", "BIGINT"))
    }

    fn build(&self, _c: &Ctx, inputs: &Inputs, _p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        sel(format!(
            "SELECT * EXCLUDE (geom, __v), __v.path[1] - 1 AS VERTEX_INDEX, __v.geom AS geom
             FROM (SELECT *, UNNEST(ST_Dump(ST_Points(geom))) AS __v FROM {})",
            ident(&i.table)
        ), i)
    }
}

tool!(DeleteDuplicates);
/// QGIS: Delete duplicate geometries (native:deleteduplicategeometries) — vectorgeneral.html#delete-duplicate-geometries
impl Tool for DeleteDuplicates {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "delete_duplicates", name: "Delete Duplicate Geometries", category: "General", role: Role::Tool,
            description: "Keep one feature per identical shape (QGIS: Delete duplicate geometries).", inputs: vin(), output: Some(PortKind::Vector), params: vec![] }
    }
    fn build(&self, _c: &Ctx, inputs: &Inputs, _p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        sel(format!("SELECT * FROM {} QUALIFY row_number() OVER (PARTITION BY ST_AsWKB(ST_Normalize(geom))) = 1", ident(&i.table)), i)
    }
}

tool!(RemoveNull);
/// QGIS: Remove null geometries (native:removenullgeometries) — vectorselection.html#remove-null-geometries
impl Tool for RemoveNull {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "remove_null", name: "Remove Null Geometries", category: "General", role: Role::Tool,
            description: "Drop features with no shape or an empty shape (QGIS: Remove null geometries).", inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Table }], output: Some(PortKind::Vector), params: vec![] }
    }
    fn build(&self, _c: &Ctx, inputs: &Inputs, _p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        sel(format!("SELECT * FROM {} WHERE geom IS NOT NULL AND NOT ST_IsEmpty(geom)", ident(&i.table)), i)
    }
}

tool!(LineIntersections);
/// QGIS: Line intersections (native:lineintersections) — vectoroverlay.html#line-intersections
impl Tool for LineIntersections {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "line_intersections", name: "Line Intersections", category: "Overlay", role: Role::Tool,
            description: "Points where lines from two layers cross, with attributes from both (QGIS: Line intersections).",
            inputs: vec![Port { id: "a", label: "Input lines", kind: PortKind::Vector }, Port { id: "b", label: "Intersect lines", kind: PortKind::Vector }],
            output: Some(PortKind::Vector), params: vec![] }
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, _p: &Params) -> Result<Output> {
        let (a, b) = (&inputs["a"], &inputs["b"]);
        let crs = need_crs(a)?.to_string();
        let bg = ctx.geom_in(b, "b", &crs)?;
        let a_cols = ctx.columns(a)?;
        let b_cols = join_columns(&a_cols, &ctx.columns(b)?, "b2");
        let b_sel = if b_cols.is_empty() { String::new() } else { format!(", {b_cols}") };
        Ok(Output::Select {
            sql: format!(
                "WITH b2 AS (SELECT * REPLACE ({bg} AS geom) FROM {bt} b),
                 x AS (SELECT a.* EXCLUDE (geom){b_sel}, ST_CollectionExtract(ST_Intersection(a.geom, b2.geom), 1) AS g
                       FROM {at} a JOIN b2 ON ST_Intersects(a.geom, b2.geom))
                 SELECT * EXCLUDE (g, __p), __p.geom AS geom FROM (SELECT *, UNNEST(ST_Dump(g)) AS __p FROM x WHERE NOT ST_IsEmpty(g))",
                at = ident(&a.table), bt = ident(&b.table)
            ),
            crs: Some(crs), view: false,
        })
    }
}

tool!(RandomPointsInPolygons);
/// QGIS: Random points in polygons (native:randompointsinpolygons) — vectorcreation.html#random-points-in-polygons
/// Rejection sampling, as QGIS does: random candidates in each polygon's
/// bounding box, keeping the first N that fall inside.
impl Tool for RandomPointsInPolygons {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "random_points", name: "Random Points in Polygons", category: "Sampling", role: Role::Tool,
            description: "A number of random points inside each polygon, keeping its attributes (QGIS: Random points in polygons).", inputs: vin(), output: Some(PortKind::Vector),
            params: vec![Param::new("count", "Points per polygon", ParamKind::Number).default(10)] }
    }
    fn build(&self, _c: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let n = p.f64("count").unwrap_or(10.0).clamp(1.0, 100_000.0) as i64;
        sel(format!(
            "WITH p AS (SELECT row_number() OVER () AS __rid, *, ST_XMin(geom) AS __x0, ST_YMin(geom) AS __y0,
                               ST_XMax(geom) - ST_XMin(geom) AS __w, ST_YMax(geom) - ST_YMin(geom) AS __h,
                               least(200000, ceil({n} * 3 * (ST_XMax(geom) - ST_XMin(geom)) * (ST_YMax(geom) - ST_YMin(geom)) / greatest(ST_Area(geom), 1e-18)))::BIGINT AS __k
                        FROM {t} WHERE ST_Dimension(geom) = 2 AND ST_Area(geom) > 0),
             c AS (SELECT p.*, ST_Point(__x0 + random() * __w, __y0 + random() * __h) AS __pt FROM p, LATERAL range(__k))
             SELECT * EXCLUDE (geom, __rid, __x0, __y0, __w, __h, __k, __pt), __pt AS geom FROM c
             WHERE ST_Within(__pt, geom)
             QUALIFY row_number() OVER (PARTITION BY __rid) <= {n}",
            t = ident(&i.table)
        ), i)
    }
}

tool!(Voronoi);
/// QGIS: Voronoi polygons (native:voronoipolygons) — vectorgeometry.html#voronoi-polygons
impl Tool for Voronoi {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "voronoi", name: "Voronoi Polygons", category: "Geometry", role: Role::Tool,
            description: "The area closest to each point, carrying that point's attributes (QGIS: Voronoi polygons).", inputs: vec![Port { id: "in", label: "Points", kind: PortKind::Vector }], output: Some(PortKind::Vector),
            params: vec![Param::new("buffer", "Buffer region (% of extent)", ParamKind::Number).default(10)] }
    }
    fn build(&self, _c: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let pct = p.f64("buffer").unwrap_or(10.0) / 100.0;
        let t = ident(&i.table);
        sel(format!(
            "WITH pts AS (SELECT * FROM {t} WHERE ST_Dimension(geom) = 0),
             ext AS (SELECT ST_Extent_Agg(geom) AS e FROM pts),
             env AS (SELECT ST_Buffer(e::GEOMETRY, greatest(ST_XMax(e) - ST_XMin(e), ST_YMax(e) - ST_YMin(e)) * {pct}) AS g FROM ext),
             cells AS (SELECT UNNEST(ST_Dump(ST_VoronoiDiagram(ST_Collect(list(geom))))) AS c FROM pts)
             SELECT pts.* REPLACE (ST_Intersection(cells.c.geom, env.g) AS geom)
             FROM cells, env, pts WHERE ST_Intersects(cells.c.geom, pts.geom)"
        ), i)
    }
}

tool!(ConcaveHull);
/// QGIS: Concave hull (native:concavehull) — vectorgeometry.html#concave-hull
impl Tool for ConcaveHull {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "concave_hull", name: "Concave Hull", category: "Geometry", role: Role::Tool,
            description: "A tighter-fitting outline around all features than a convex hull (QGIS: Concave hull).", inputs: vin(), output: Some(PortKind::Vector),
            params: vec![
                Param::new("ratio", "Threshold (0–1)", ParamKind::Number).default(0.3).help("0 = tightest, 1 = convex hull"),
                Param::new("holes", "Allow holes", ParamKind::Bool).default(false),
                Param::new("field", "Group by field", ParamKind::Field).field_of("in").optional(),
            ] }
    }
    fn build(&self, _c: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let r = p.f64("ratio").unwrap_or(0.3).clamp(0.0, 1.0);
        let h = p.bool("holes");
        let t = ident(&i.table);
        let hull = format!("ST_ConcaveHull(ST_Collect(list(geom)), {r}, {h})");
        sel(match p.str("field") {
            Some(f) => format!("SELECT {f}, count(*) AS COUNT, {hull} AS geom FROM {t} GROUP BY {f}", f = ident(&f)),
            None => format!("SELECT count(*) AS COUNT, {hull} AS geom FROM {t}"),
        }, i)
    }
}

tool!(JoinByNearest);
/// QGIS: Join attributes by nearest (native:joinbynearest) — vectorgeneral.html#join-attributes-by-nearest
impl Tool for JoinByNearest {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "join_nearest", name: "Join Attributes by Nearest", category: "Proximity", role: Role::Tool,
            description: "Attach the attributes of the closest feature from another layer, plus the geodesic distance (QGIS: Join attributes by nearest).",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Vector }, Port { id: "near", label: "Join features", kind: PortKind::Vector }],
            output: Some(PortKind::Vector),
            params: vec![
                Param::new("radius", "Maximum distance", ParamKind::Number).default(10),
                Param::new("units", "Units", ParamKind::Enum).default("km").options(LINEAR_UNITS),
                Param::new("keep_all", "Keep features with no match", ParamKind::Bool).default(true),
            ] }
    }
    fn output_fields(&self) -> OutFields {
        OutFields::from(&["in", "near"]).add(NewField::new("DISTANCE", "DOUBLE"))
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let (a, b) = (&inputs["in"], &inputs["near"]);
        let crs = need_crs(a)?.to_string();
        let utm = ctx.utm(a)?;
        let units = p.str("units").unwrap_or_else(|| "km".into());
        let r = to_meters(p.f64("radius").unwrap_or(10.0), &units)?;
        let per = to_meters(1.0, &units)?;
        let am = format!("ST_Transform(a.geom, {}, {}, true)", lit(&crs::resolve(&crs)), lit(&utm));
        let bm = format!("ST_Transform({}, {}, {}, true)", ctx.geom_in(b, "b", &crs)?, lit(&crs::resolve(&crs)), lit(&utm));
        let a_cols = ctx.columns(a)?;
        let b_cols = join_columns(&a_cols, &ctx.columns(b)?, "bb");
        let b_sel = if b_cols.is_empty() { String::new() } else { format!(", {b_cols}") };
        let join = if p.bool("keep_all") { "LEFT JOIN" } else { "JOIN" };
        Ok(Output::Select {
            sql: format!(
                "WITH a AS (SELECT row_number() OVER () AS __rid, *, {am} AS __m, {aeqd} AS __aeqd FROM {at} a),
                 bb AS (SELECT row_number() OVER () AS __nid, *, {bm} AS __m, {bg} AS __g FROM {bt} b),
                 cand AS (SELECT a.__rid, bb.__nid, ST_Distance(ST_Transform(a.geom, {c}, a.__aeqd, true), ST_Transform(bb.__g, {c}, a.__aeqd, true)) AS gd
                          FROM a JOIN bb ON ST_DWithin(a.__m, bb.__m, {r} * 1.05)),
                 n AS (SELECT __rid, arg_min(__nid, gd) AS nid, min(gd) AS d FROM cand WHERE gd <= {r} GROUP BY __rid)
                 SELECT a.* EXCLUDE (__rid, __m, __aeqd, geom){b_sel}, n.d / {per} AS DISTANCE, a.geom
                 FROM a {join} n ON a.__rid = n.__rid LEFT JOIN bb ON bb.__nid = n.nid",
                at = ident(&a.table), bt = ident(&b.table),
                c = lit(&crs::resolve(&crs)), aeqd = aeqd_for("a.geom", &crs), bg = ctx.geom_in(b, "b", &crs)?
            ),
            crs: Some(crs), view: false,
        })
    }
}

tool!(PointsToPath);
/// QGIS: Points to path (native:pointstopath) — vectorcreation.html#points-to-path
impl Tool for PointsToPath {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "points_to_path", name: "Points to Path", category: "Geometry", role: Role::Tool,
            description: "Connect points into lines in the order of a field, optionally one line per group (QGIS: Points to path).",
            inputs: vec![Port { id: "in", label: "Points", kind: PortKind::Vector }], output: Some(PortKind::Vector),
            params: vec![
                Param::new("order", "Order by field", ParamKind::Field).field_of("in"),
                Param::new("group", "Group by field (one line each)", ParamKind::Field).field_of("in").optional(),
            ] }
    }
    fn build(&self, _c: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let o = ident(&p.req_str("order", "Order by field")?);
        let t = ident(&i.table);
        let line = format!("ST_MakeLine(list(geom ORDER BY {o}))");
        sel(match p.str("group") {
            Some(g) => format!("SELECT {g}, count(*) AS NUM_POINTS, {line} AS geom FROM {t} WHERE ST_Dimension(geom) = 0 GROUP BY {g} HAVING count(*) > 1", g = ident(&g)),
            None => format!("SELECT count(*) AS NUM_POINTS, {line} AS geom FROM {t} WHERE ST_Dimension(geom) = 0"),
        }, i)
    }
}

tool!(PointsAlongLines);
/// QGIS: Points along geometry (native:pointsalonglines) — vectorgeometry.html#points-along-geometry
impl Tool for PointsAlongLines {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "points_along_lines", name: "Points Along Lines", category: "Geometry", role: Role::Tool,
            description: "Points every set distance along lines or polygon outlines (QGIS: Points along geometry).", inputs: vec![Port { id: "in", label: "Lines or polygons", kind: PortKind::Vector }], output: Some(PortKind::Vector),
            params: vec![
                Param::new("distance", "Distance", ParamKind::Number).default(100),
                Param::new("units", "Units", ParamKind::Enum).default("m").options(LINEAR_UNITS),
            ] }
    }
    fn output_fields(&self) -> OutFields {
        OutFields::from(&["in"]).add(NewField::new("DISTANCE", "DOUBLE"))
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let crs = crs::resolve(need_crs(i)?);
        let utm = ctx.utm(i)?;
        let d = to_meters(p.f64("distance").unwrap_or(100.0), &p.str("units").unwrap_or_else(|| "m".into()))?;
        if d <= 0.0 {
            bail!("Distance must be greater than zero");
        }
        let (c, u) = (lit(&crs), lit(&utm));
        Ok(Output::Select {
            sql: format!(
                "WITH parts AS (SELECT * EXCLUDE (__d), __d.geom AS __m FROM (
                    SELECT *, UNNEST(ST_Dump(ST_Transform(CASE WHEN ST_Dimension(geom) = 2 THEN ST_Boundary(geom) ELSE geom END, {c}, {u}, true))) AS __d
                    FROM {t} WHERE ST_Dimension(geom) >= 1)),
                 p AS (SELECT * EXCLUDE (__m), UNNEST(ST_Dump(ST_LineInterpolatePoints(__m, least(1.0, {d} / greatest(ST_Length(__m), 1e-9)), true))) AS __p
                       FROM parts WHERE ST_Length(__m) > 0)
                 SELECT * EXCLUDE (geom, __p), (__p.path[1] - 1) * {d} AS DISTANCE, ST_Transform(__p.geom, {u}, {c}, true) AS geom FROM p",
                t = ident(&i.table)
            ),
            crs: Some(crs), view: false,
        })
    }
}
