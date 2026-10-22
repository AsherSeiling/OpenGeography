//! Geodesic measurements used by Calculate Geometry (tool and table command).

use crate::db::lit;
use crate::nodes::to_meters;
use anyhow::{bail, Result};

pub const AREA_UNITS: &[(&str, &str)] = &[
    ("sqm", "Square meters"),
    ("sqkm", "Square kilometers"),
    ("ha", "Hectares"),
    ("acres", "Acres"),
    ("sqmi", "Square miles"),
    ("sqft", "Square feet"),
];

pub const PROPERTIES: &[(&str, &str)] = &[
    ("area", "Area (geodesic)"),
    ("length", "Length (geodesic)"),
    ("perimeter", "Perimeter (geodesic)"),
    ("x", "Centroid longitude"),
    ("y", "Centroid latitude"),
];

/// SQL for a geometry property of the `geom` column in `crs`. Areas and
/// lengths are measured on the WGS 84 ellipsoid, so they are correct in any
/// coordinate system. (With geometry_always_xy set in db.rs, the spheroid
/// functions take lon/lat order.)
pub fn geometry_expr(crs: &str, property: &str, area_units: &str, length_units: &str) -> Result<String> {
    let ll = format!("ST_Transform(geom, {}, 'EPSG:4326', true)", lit(crs));
    Ok(match property {
        "area" => {
            let div = match area_units {
                "sqm" => 1.0,
                "sqkm" => 1e6,
                "ha" => 1e4,
                "acres" => 4046.8564224,
                "sqmi" => 2_589_988.110336,
                "sqft" => 0.09290304,
                u => bail!("Unknown area unit '{u}'"),
            };
            format!("ST_Area_Spheroid({ll}) / {div}")
        }
        "length" | "perimeter" => {
            let div = to_meters(1.0, length_units)?;
            let func = if property == "length" { "ST_Length_Spheroid" } else { "ST_Perimeter_Spheroid" };
            format!("{func}({ll}) / {div}")
        }
        "x" => format!("ST_X(ST_Centroid({ll}))"),
        "y" => format!("ST_Y(ST_Centroid({ll}))"),
        other => bail!("Unknown property '{other}'"),
    })
}
