//! Wave 1 Lane B tools: one place that lists the tool families added in this wave and their search words, so the shared `mod.rs` and
//! `keywords.rs` need one line each. Add a family here when it is added.

use super::*;

pub fn tools() -> Vec<Box<dyn Tool>> {
    let mut v = whitebox_tools::tools();
    v.extend(gdal_tools::tools());
    v.extend(centrography::tools());
    v.extend(point_patterns::tools());
    v.extend(kernel_density::tools());
    v.extend(tabulate_area::tools());
    v.extend(cut_fill::tools());
    v.extend(profile_line::tools());
    v.extend(network_tools::tools());
    v.extend(hex_bin::tools());
    v.extend(dbscan::tools());
    v.extend(tin_interpolation::tools());
    v.extend(classify_supervised::tools());
    v
}

pub fn keywords(id: &str) -> &'static [&'static str] {
    for k in [
        whitebox_tools::keywords(id),
        gdal_tools::keywords(id),
        centrography::keywords(id),
        point_patterns::keywords(id),
        kernel_density::keywords(id),
        tabulate_area::keywords(id),
        cut_fill::keywords(id),
        profile_line::keywords(id),
        network_tools::keywords(id),
        hex_bin::keywords(id),
        dbscan::keywords(id),
        tin_interpolation::keywords(id),
        classify_supervised::keywords(id),
    ] {
        if !k.is_empty() {
            return k;
        }
    }
    &[]
}
