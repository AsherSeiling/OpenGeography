//! Plain-language search words for each tool, keyed by tool id. The Shift+A menu
//! matches them besides the name, category and description (docs/UI_REDESIGN_PLAN.md §5).
//! Metadata only: it reaches the UI through `SpecOut.keywords`, not through `ToolSpec`.

pub fn keywords(id: &str) -> &'static [&'static str] {
    match id {
        "filter" => &["where", "sql", "subset", "attribute filter", "query"],
        "buffer" => &["zone", "offset", "radius", "distance around", "halo", "setback"],
        "clip" => &["cut", "trim", "crop", "cookie cutter", "extract by area"],
        "intersect" => &["overlap", "common area", "and", "overlay"],
        "dissolve" => &["combine", "aggregate", "merge polygons", "group boundaries", "remove boundaries"],
        "project" => &["reproject", "transform", "change crs", "convert coordinates", "change projection"],
        "spatial_join" => &["join by location", "attach attributes by location", "points in polygons"],
        "attribute_join" => &["join table", "vlookup", "link table", "relate", "add columns from table"],
        "calculate_field" => &["formula", "expression", "compute column", "field calculator", "new column"],
        "calculate_geometry" => &["area", "length", "perimeter", "centroid coordinates", "measure features"],
        "erase" => &["cut out", "subtract", "remove area", "difference", "punch hole"],
        "union" => &["merge", "combine", "overlay all", "add together", "or"],
        "symmetrical_difference" => &["xor", "not overlapping", "exclusive or", "non-overlap"],
        "collect" => &["group parts", "combine parts", "make multipart"],
        "delaunay" => &["tin", "triangles", "triangulate", "triangle mesh"],
        "merge" => &["append", "combine", "stack", "concatenate", "put together"],
        "select_by_location" => &["within", "touching", "near selection", "spatial filter", "inside"],
        "feature_to_point" => &["centroid", "center point", "middle point", "label point"],
        "bounding_geometry" => &["bounding box", "envelope", "convex hull", "extent polygon", "outline"],
        "simplify" => &["generalize", "reduce vertices", "smooth", "fewer points", "thin"],
        "explode" => &["split parts", "break apart", "ungroup parts", "separate parts"],
        "repair" => &["fix", "invalid", "clean", "make valid", "self-intersection"],
        "symbology" => &["style", "colour", "color", "labels", "legend", "draw"],
        "summarize_within" => &["count points", "points in polygon", "sum by area", "tally"],
        "near" => &["closest", "nearest", "distance to", "proximity", "how far"],
        "fishnet" => &["grid", "tessellation", "cells", "squares", "net"],
        "add_usng" => &["military grid", "grid reference", "mgrs column", "location code"],
        "usng_to_points" => &["military grid", "grid reference", "decode mgrs", "plot references"],
        "usng_grid" => &["military grid", "grid lines", "graticule", "map grid"],
        "summary_statistics" => &["average", "mean", "sum", "count", "pivot", "group by", "totals"],
        "polygons_to_lines" => &["outline", "boundary", "edges", "border lines"],
        "lines_to_polygons" => &["close lines", "fill", "make polygon", "build areas"],
        "extract_vertices" => &["corners", "nodes", "line to points", "polygon to points"],
        "delete_duplicates" => &["remove duplicates", "dedupe", "same geometry", "clean"],
        "remove_null" => &["empty geometry", "missing geometry", "clean", "drop blanks"],
        "line_intersections" => &["crossings", "crossing points", "where lines meet", "junctions"],
        "random_points" => &["sample", "sampling", "scatter", "survey locations"],
        "voronoi" => &["thiessen", "nearest area", "proximity polygons", "service area"],
        "concave_hull" => &["alpha shape", "outline points", "wrap", "footprint", "boundary of points"],
        "join_nearest" => &["closest", "attach nearest", "nearest neighbour", "nearest neighbor"],
        "points_to_path" => &["connect the dots", "track", "gps track", "route line", "line from points"],
        "points_along_lines" => &["interval", "stations", "mile markers", "chainage", "sample along"],
        "contours" => &["isolines", "topo lines", "elevation lines", "contour lines"],
        "hillshade" => &["relief", "shaded relief", "shadow", "terrain shading"],
        "slope" => &["steepness", "gradient", "grade", "incline"],
        "clip_raster" => &["cut", "crop", "trim", "subset raster"],
        "clip_raster_by_layer" => &["mask", "cutline", "cookie cutter", "clip by polygon", "extract by mask", "cut raster to shape"],
        "aspect" => &["facing direction", "compass direction of slope", "exposure"],
        "ruggedness" => &["tri", "roughness", "terrain roughness"],
        "warp" => &["reproject", "transform", "change crs", "georeference", "change projection"],
        "polygonize" => &["raster to polygon", "vectorize", "trace", "to vector"],
        "raster_calc" => &["map algebra", "formula", "expression", "math on raster"],
        "raster_calc2" => &["map algebra", "two rasters", "difference", "subtract rasters", "add rasters"],
        "reclassify" => &["remap", "classes", "bin", "recode", "group values"],
        "nodata" => &["missing values", "blank cells", "transparent", "mask values"],
        "select_bands" => &["extract band", "channels", "rgb", "pick bands"],
        "resample_raster" => &["change resolution", "cell size", "pixel size", "upscale", "downscale"],
        "align_raster" => &["snap", "match grid", "same cells", "register"],
        "mosaic_rasters" => &["merge", "merge rasters", "stitch", "combine tiles", "join rasters"],
        "rasterize" => &["vector to grid", "burn", "polygons to raster", "to raster"],
        "interpolate" => &["idw", "surface from points", "gridding", "natural neighbour", "tin surface"],
        "define_vertical_datum" => &["navd88", "height reference", "geoid", "elevation datum"],
        "morans_i" => &["clustering", "autocorrelation", "clustered or random", "pattern"],
        "hot_spots" => &["clusters", "hotspot", "cold spots", "gi star", "pattern"],
        "kriging" => &["geostatistics", "variogram", "surface from points", "gridding"],
        "fill_depressions" => &["sinks", "pits", "hydro conditioning", "fill holes in dem"],
        "flow_direction" => &["d8", "downhill", "drainage direction", "water path"],
        "flow_accumulation" => &["upstream area", "contributing area", "drainage area", "runoff"],
        "basins" => &["catchments", "drainage areas", "hydrologic units"],
        "extract_streams" => &["rivers", "channels", "stream network", "drainage lines"],
        "watersheds" => &["catchment", "pour points", "drainage area", "upstream of point"],
        "spectral_index" => &["ndvi", "ndwi", "nbr", "vegetation", "band ratio", "satellite"],
        "classify_kmeans" => &["land cover", "clusters", "unsupervised", "image classes", "satellite"],
        "sample_raster" => &["extract values", "read elevation at points", "point values", "values to points"],
        "zonal_statistics" => &["average in polygon", "mean by zone", "raster stats by area", "summarize raster"],
        "export" => &["save", "save as", "write file", "download", "shapefile", "geopackage", "geojson"],
        _ => match super::gdal_tools::keywords(id) {
            [] => super::whitebox_tools::keywords(id),
            k => k,
        },
    }
}

/// Every id the table covers, for the tests (kept in step with the match above).
#[cfg(test)]
pub const IDS: &[&str] = &[
    "filter",
    "buffer",
    "clip",
    "intersect",
    "dissolve",
    "project",
    "spatial_join",
    "attribute_join",
    "calculate_field",
    "calculate_geometry",
    "erase",
    "union",
    "symmetrical_difference",
    "collect",
    "delaunay",
    "merge",
    "select_by_location",
    "feature_to_point",
    "bounding_geometry",
    "simplify",
    "explode",
    "repair",
    "symbology",
    "summarize_within",
    "near",
    "fishnet",
    "add_usng",
    "usng_to_points",
    "usng_grid",
    "summary_statistics",
    "polygons_to_lines",
    "lines_to_polygons",
    "extract_vertices",
    "delete_duplicates",
    "remove_null",
    "line_intersections",
    "random_points",
    "voronoi",
    "concave_hull",
    "join_nearest",
    "points_to_path",
    "points_along_lines",
    "contours",
    "hillshade",
    "slope",
    "clip_raster",
    "clip_raster_by_layer",
    "aspect",
    "ruggedness",
    "warp",
    "polygonize",
    "raster_calc",
    "raster_calc2",
    "reclassify",
    "nodata",
    "select_bands",
    "resample_raster",
    "align_raster",
    "mosaic_rasters",
    "rasterize",
    "interpolate",
    "define_vertical_datum",
    "morans_i",
    "hot_spots",
    "kriging",
    "fill_depressions",
    "flow_direction",
    "flow_accumulation",
    "basins",
    "extract_streams",
    "watersheds",
    "spectral_index",
    "classify_kmeans",
    "sample_raster",
    "zonal_statistics",
    "export",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nodes::{registry, Role};

    #[test]
    fn every_add_menu_tool_has_keywords() {
        for t in registry() {
            let s = t.spec();
            if s.role == Role::Data || s.id == "layer_output" || s.id == "add_to_map" {
                continue;
            }
            assert!(!keywords(s.id).is_empty(), "tool {} has no keywords", s.id);
        }
    }

    #[test]
    fn no_table_id_matches_no_tool() {
        let ids: Vec<&str> = registry().iter().map(|t| t.spec().id).collect();
        for id in IDS {
            assert!(ids.contains(id), "keywords table has unknown tool id {id}");
            assert!(!keywords(id).is_empty(), "IDS lists {id} but the match has no arm");
        }
    }

    #[test]
    fn plain_words_reach_the_right_tools() {
        assert!(keywords("clip").contains(&"cut"));
        for id in ["union", "merge", "mosaic_rasters"] {
            let name = registry().iter().find(|t| t.spec().id == id).unwrap().spec().name.to_lowercase();
            assert!(keywords(id).iter().any(|w| w.contains("merge")) || name.contains("merge"), "{id} is not found by \"merge\"");
        }
    }
}
