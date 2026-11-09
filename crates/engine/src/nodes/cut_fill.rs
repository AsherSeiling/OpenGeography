//! Cut and Fill (Wave 1, package T4): volume of material removed and added between two elevation surfaces of the same grid.
//!
//! Volume = sum over cells of (after - before) * cell area, taken separately where the difference is negative (cut: the surface went down) and
//! positive (fill: it went up) (the definition of the ArcGIS Cut Fill tool and of any grid-based earthwork volume, e.g. Easa 1988). The cell
//! area is the true area of the cell: planar width times height times the squared linear unit in a projected CRS, and the authalic area of the
//! longitude/latitude cell on the ellipsoid in a geographic CRS (Snyder 1987). Heights are converted to metres with the stated height unit, so
//! volumes are in cubic metres. Cells that have no data in either surface are left out and counted.

use super::*;
use crate::geomkit;
use std::path::Path;

pub struct CutFill;
impl Tool for CutFill {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "cut_fill", name: "Cut and Fill (Volume Between Surfaces)", category: "Raster › Terrain and Contours", role: Role::Tool,
            description: "How much material was removed (cut) and added (fill) between a before surface and an after surface: the sum of the height difference (after minus before) times the cell area, separately where the surface went down and where it went up, plus the net volume and the areas. Both elevation rasters must be on the same grid and in the same coordinate system; if not, use Align Raster first. Cell areas are true areas (planar in a projected coordinate system, on the ellipsoid in a geographic one). Say what unit the heights are in; volumes are in cubic metres. Cells with no data in either surface are left out and counted. One-row table.",
            inputs: vec![Port { id: "before", label: "Before surface (elevation raster)", kind: PortKind::Raster }, Port { id: "after", label: "After surface (elevation raster)", kind: PortKind::Raster }], output: Some(PortKind::Table),
            params: vec![Param::new("z_unit", "Height unit of both rasters", ParamKind::Enum).default("m").options(&[("m", "Metres"), ("ft", "Feet (international)"), ("usft", "US survey feet")])] }
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let (b, a) = (&inputs["before"], &inputs["after"]);
        let (bp, ap) = (Path::new(need_raster(b)?), Path::new(need_raster(a)?));
        let crs = need_raster_crs(b)?;
        need_raster_crs(a)?;
        let (gb, ga) = (crate::raster_ops::grid_of(bp)?, crate::raster_ops::grid_of(ap)?);
        crate::raster_ops::same_grid(&gb, &ga, "Cut and Fill")?;
        if gb.gt[2] != 0.0 || gb.gt[4] != 0.0 {
            bail!("Cut and Fill needs north-up rasters. Use Warp to make them north-up first.");
        }
        if let Some(w) = crate::vertical::check("Cut and Fill", &[bp, ap])? {
            ctx.add_note(w);
        }
        let zf = to_meters(1.0, p.str("z_unit").as_deref().unwrap_or("m"))?;
        let m = geomkit::metric(crs)?;
        let (dsb, dsa) = (gdal::Dataset::open(bp)?, gdal::Dataset::open(ap)?);
        let (bb, ba) = (dsb.rasterband(1)?, dsa.rasterband(1)?);
        let (ndb, nda) = (bb.no_data_value(), ba.no_data_value());
        let (cols, rows) = (gb.cols, gb.rows);
        let dy = gb.gt[5];
        let task = crate::progress::Task::start("Cut and fill").cancellable();
        let (mut cut_v, mut fill_v, mut cut_a, mut fill_a, mut same_a) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
        let (mut used, mut nodata) = (0u64, 0u64);
        for r in 0..rows {
            if r % 64 == 0 {
                task.check()?;
                task.set(r as f64 / rows as f64, None::<String>);
            }
            let area = if m.geographic { m.band_area(gb.gt[3] + r as f64 * dy, gb.gt[3] + (r + 1) as f64 * dy, gb.gt[1]) } else { (gb.gt[1] * dy).abs() * m.unit_m * m.unit_m };
            let rb = bb.read_as::<f64>((0, r as isize), (cols, 1), (cols, 1), None)?;
            let ra = ba.read_as::<f64>((0, r as isize), (cols, 1), (cols, 1), None)?;
            for (x, y) in rb.data().iter().zip(ra.data()) {
                if !x.is_finite() || !y.is_finite() || ndb == Some(*x) || nda == Some(*y) {
                    nodata += 1;
                    continue;
                }
                used += 1;
                let d = (y - x) * zf;
                if d < 0.0 {
                    cut_v += -d * area;
                    cut_a += area;
                } else if d > 0.0 {
                    fill_v += d * area;
                    fill_a += area;
                } else {
                    same_a += area;
                }
            }
        }
        if used == 0 {
            bail!("The two surfaces have no cell with data in both, so there is nothing to compare");
        }
        if nodata > 0 {
            ctx.add_note(format!("Cut and Fill: {nodata} of {} cells have no data in one or both surfaces and were left out.", used + nodata));
        }
        if !m.geographic {
            if let Some(n) = crate::crs_warnings::planar_note("Cut and Fill", crs, [gb.gt[0], gb.gt[3] + rows as f64 * dy, gb.gt[0] + cols as f64 * gb.gt[1], gb.gt[3]]) {
                ctx.add_note(n);
            }
        }
        use super::centrography::sqlnum;
        Ok(Output::Select {
            sql: format!(
                "SELECT {} AS cut_volume_m3, {} AS fill_volume_m3, {} AS net_volume_m3, {} AS cut_area_m2, {} AS fill_area_m2, {} AS unchanged_area_m2, {used} AS cells_compared, {nodata} AS cells_no_data, '{}' AS height_unit, 'after minus before; cut = surface lowered, fill = surface raised' AS difference",
                sqlnum(cut_v), sqlnum(fill_v), sqlnum(fill_v - cut_v), sqlnum(cut_a), sqlnum(fill_a), sqlnum(same_a), p.str("z_unit").as_deref().unwrap_or("m")
            ),
            crs: None,
            view: false,
        })
    }
}

pub fn keywords(id: &str) -> &'static [&'static str] {
    match id {
        "cut_fill" => &["earthwork", "excavation volume", "volume change", "dem difference", "stockpile", "grading", "removed and added"],
        _ => &[],
    }
}

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(CutFill)]
}
