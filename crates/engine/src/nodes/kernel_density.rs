//! Kernel Density (Wave 1, package T4): a smooth density surface from points, optionally weighted by a number field (for example population).
//!
//! Method: the quartic (biweight) kernel of Silverman (1986, "Density Estimation for Statistics and Data Analysis", eq. 4.5, two dimensions):
//! K(u) = (3 / pi) (1 - u^2)^2 for u = d / h < 1, else 0, with h the bandwidth. The density at a place is
//! f = (1 / h^2) * sum_i w_i K(d_i / h), in weight per square metre; the kernel integrates to 1, so the integral of the surface over the plane
//! is the total weight of the points. The raster is made by adding each point's kernel to the cells within h of it, evaluated at cell centres.
//! The extent is the points' bounding box grown by the bandwidth, so no kernel is cut off. Distances are planar in a projected coordinate
//! system; a geographic layer is refused (degrees are not a length), with a request to project it first.

use super::centrography::read_pts;
use super::*;
use crate::geomkit;
use crate::raster;

pub struct KernelDensity;
impl Tool for KernelDensity {
    fn result_look(&self) -> ResultLook {
        ResultLook::Derived
    }
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "kernel_density", name: "Kernel Density", category: "Raster › Interpolation", role: Role::Tool,
            description: "A smooth surface of how concentrated the points are: each point spreads its weight (1, or the value of a number field such as population) over a circle of the bandwidth radius with a quartic kernel (Silverman 1986), and the surface adds them up. The result is weight per square kilometre (or per square metre), so the sum of density times cell area equals the total weight; with no weight field that is the number of points. The bandwidth and the cell size are in metres. The raster covers the points' extent plus one bandwidth on every side. Needs a projected layer; a layer in degrees is refused (project it first). Points without a position or weight are left out and counted.",
            inputs: vec![Port { id: "in", label: "Points", kind: PortKind::Vector }], output: Some(PortKind::Raster),
            params: vec![
                Param::new("weight", "Weight field (empty: every point counts 1)", ParamKind::Field).field_of("in").optional(),
                Param::new("bandwidth", "Bandwidth / search radius (metres)", ParamKind::Number).default(1000),
                Param::new("cell", "Cell size (metres)", ParamKind::Number).default(100),
                Param::new("per", "Density per", ParamKind::Enum).default("km2").options(&[("km2", "Square kilometre"), ("m2", "Square metre")]),
            ] }
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let crs = need_crs(i)?;
        let m = geomkit::metric(crs)?;
        if m.geographic {
            bail!("Kernel Density measures distances and cell areas in metres, and this layer is in degrees. Project the points to a metre-based coordinate system first (Project).");
        }
        let (h_m, c_m) = (p.f64("bandwidth").unwrap_or(f64::NAN), p.f64("cell").unwrap_or(f64::NAN));
        if !(h_m.is_finite() && h_m > 0.0) {
            bail!("'Bandwidth' must be a number greater than 0 (metres)");
        }
        if !(c_m.is_finite() && c_m > 0.0) {
            bail!("'Cell size' must be a number greater than 0 (metres)");
        }
        let pts = read_pts(ctx, i, p.str("weight").as_deref(), "Kernel Density")?;
        let (h, c) = (h_m / m.unit_m, c_m / m.unit_m);
        if h < 2.0 * c {
            ctx.add_note(format!("Kernel Density: the bandwidth ({h_m} m) is less than two cells ({} m); the surface will look blocky and the total will be less exact. Use a smaller cell or a larger bandwidth.", 2.0 * c_m));
        }
        let bb = pts.xy.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, q| [b[0].min(q.0), b[1].min(q.1), b[2].max(q.0), b[3].max(q.1)]);
        let x0 = ((bb[0] - h) / c).floor() * c;
        let y1 = ((bb[3] + h) / c).ceil() * c;
        let cols = (((bb[2] + h) - x0) / c).ceil() as usize + 1;
        let rows = ((y1 - (bb[1] - h)) / c).ceil() as usize + 1;
        crate::raster_ops::check_cells(cols as f64, rows as f64)?;
        let mut dens = vec![0.0f64; cols * rows];
        let k0 = 3.0 / std::f64::consts::PI / (h_m * h_m); // weight per square metre
        let task = crate::progress::Task::start("Kernel density").cancellable();
        let reach = (h / c).ceil() as isize + 1;
        let n = pts.xy.len();
        for (k, ((px, py), w)) in pts.xy.iter().zip(&pts.w).enumerate() {
            if k % 256 == 0 {
                task.check()?;
                task.set(k as f64 / n as f64, None::<String>);
            }
            // Cell (col, row) has its centre at (x0 + (col + 0.5) c, y1 - (row + 0.5) c).
            let (cc, cr) = (((px - x0) / c).floor() as isize, ((y1 - py) / c).floor() as isize);
            for r in (cr - reach).max(0)..=(cr + reach).min(rows as isize - 1) {
                let cy = y1 - (r as f64 + 0.5) * c;
                for q in (cc - reach).max(0)..=(cc + reach).min(cols as isize - 1) {
                    let cx = x0 + (q as f64 + 0.5) * c;
                    let u2 = ((cx - px).powi(2) + (cy - py).powi(2)) * m.unit_m * m.unit_m / (h_m * h_m);
                    if u2 < 1.0 {
                        dens[r as usize * cols + q as usize] += w * k0 * (1.0 - u2) * (1.0 - u2);
                    }
                }
            }
        }
        let scale = if p.str("per").as_deref() == Some("m2") { 1.0 } else { 1.0e6 };
        let data: Vec<f32> = dens.iter().map(|v| (v * scale) as f32).collect();
        let wkt = crate::crs::srs(&crate::crs::resolve(crs))?.to_wkt()?;
        let grid = crate::raster_ops::Grid { cols, rows, gt: [x0, c, 0.0, y1, 0.0, -c], wkt, bands: 1 };
        let out = raster::new_store_path("kernel_density", "tif");
        {
            let ds = crate::raster_ops::create_like(&out, &grid, 1, Some(-9999.0))?;
            let mut b = ds.rasterband(1)?;
            b.write((0, 0), (cols, rows), &mut gdal::raster::Buffer::new((cols, rows), data))?;
        }
        raster::build_overviews(&out)?;
        let total: f64 = pts.w.iter().sum();
        ctx.add_note(format!(
            "Kernel Density: {n} point(s), total weight {total}; density is in weight per square {}; cell {c_m} m, bandwidth {h_m} m, {cols} × {rows} cells.",
            if scale == 1.0 { "metre" } else { "kilometre" }
        ));
        Ok(Output::Raster { path: out.display().to_string(), crs: i.crs.clone() })
    }
}

pub fn keywords(id: &str) -> &'static [&'static str] {
    match id {
        "kernel_density" => &["heat map", "heatmap", "hotspot surface", "point density", "density surface", "concentration"],
        _ => &[],
    }
}

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(KernelDensity)]
}
