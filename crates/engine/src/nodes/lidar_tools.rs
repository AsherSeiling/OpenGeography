//! LiDAR nodes (package T8): LAS/LAZ point clouds to rasters, ground classification and height normalisation, run by the bundled WhiteboxTools
//! program (MIT, John Lindsay; its "LiDAR Tools" toolbox) through `whitebox.rs`, with the file read by `pointcloud.rs` (the `las` crate).
//!
//! The point cloud is a file, not a layer: the node takes the file's path (a "LAS or LAZ file" setting with a file chooser) and never modifies it.
//! The coordinate system comes from the file (WKT record or GeoTIFF keys). A file without one is refused unless the user states it in the
//! "Coordinate system" setting; if both exist and differ, the node stops instead of choosing. Raster results get that CRS written in, and the
//! vertical datum label when the file names one that GIS Studio tracks (NAVD88 for 3DEP).
//!
//! Methods: LidarTINGridding (Delaunay TIN of the points, linear interpolation in each triangle), LidarIdwInterpolation (inverse-distance
//! weighting, Shepard 1968), LidarGroundPointFilter (slope-based ground filter after Vosselman 2000 as implemented by Lindsay, Whitebox),
//! HeightAboveGround (normalises each point to the nearest ground-classified point), LidarDigitalSurfaceModel (top surface, Lindsay),
//! LidarPointDensity, LidarBlockMaximum. 3DEP lidar (USGS National Map) is the target data.

use super::export::refuse_source_file;
use super::*;
use crate::pointcloud::{self, PointCloudInfo};
use crate::raster;
use std::path::{Path, PathBuf};

/// Largest raster these tools will ask the program to build (cells); beyond it the user is told to use a coarser resolution or a smaller file.
const MAX_CELLS: f64 = 400_000_000.0;

const NON_GROUND: &str = "0,1,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18";

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Dem,
    Dsm,
    Density,
    Canopy,
    ClassifyGround,
    Normalize,
}

struct Lidar(Kind);

pub fn tools() -> Vec<Box<dyn Tool>> {
    [Kind::Dem, Kind::Dsm, Kind::Density, Kind::Canopy, Kind::ClassifyGround, Kind::Normalize].into_iter().map(|k| Box::new(Lidar(k)) as Box<dyn Tool>).collect()
}

pub fn keywords(id: &str) -> &'static [&'static str] {
    match id {
        "lidar_dem" => &["las to dem", "laz", "point cloud", "bare earth", "dtm", "3dep lidar", "tin gridding", "idw", "terrain from points"],
        "lidar_dsm" => &["las to dsm", "laz", "point cloud", "surface model", "top surface", "canopy top", "first return"],
        "lidar_point_density" => &["las", "laz", "point cloud", "returns per square metre", "pulse density", "coverage"],
        "lidar_canopy_height" => &["chm", "canopy height model", "vegetation height", "normalised height", "height above ground", "las", "laz", "forest"],
        "lidar_classify_ground" => &["ground filter", "classify ground", "bare earth points", "las", "laz", "point cloud"],
        "lidar_normalize_heights" => &["height above ground", "normalise", "normalize", "las", "laz", "point cloud"],
        _ => &[],
    }
}

fn spec_of(k: Kind) -> ToolSpec {
    let file = Param::new("file", "LAS or LAZ file", ParamKind::OpenPath).help("A point cloud file. It is read, never changed.");
    let crs = Param::new("crs", "Coordinate system (only if the file has none)", ParamKind::Crs).optional().help("Most files name their coordinate system; if this file does not, say which it is. It is not guessed.");
    let res = |d: f64| Param::new("resolution", "Cell size (file's horizontal unit)", ParamKind::Number).default(d).help("In the unit of the file's coordinate system (metres for UTM). Choose about 2 to 4 times the average point spacing.");
    let ground = Param::new("ground", "Ground points", ParamKind::Enum).default("classified").options(&[
        ("classified", "Use the points classified as ground (class 2)"),
        ("filter", "Find ground first with the slope-based filter"),
    ]);
    let filter_params = || {
        vec![
            Param::new("filter_radius", "Ground filter: search radius", ParamKind::Number).default(2.0).help("Only used when ground is found with the filter. In the file's horizontal unit."),
            Param::new("filter_slope", "Ground filter: maximum slope between points (degrees)", ParamKind::Number).default(45.0),
            Param::new("filter_height", "Ground filter: height difference for off-ground (file's vertical unit)", ParamKind::Number).default(1.0),
        ]
    };
    match k {
        Kind::Dem => {
            let mut params = vec![
                file,
                crs,
                res(1.0),
                Param::new("method", "Interpolation", ParamKind::Enum).default("tin").options(&[("tin", "Triangulated network (TIN)"), ("idw", "Inverse distance weighting (IDW)")]),
                ground,
                Param::new("radius", "IDW search radius", ParamKind::Number).default(3.0).help("Only for IDW. In the file's horizontal unit; a cell with no point inside stays empty."),
                Param::new("weight", "IDW weight (power)", ParamKind::Number).default(2.0),
                Param::new("max_edge", "TIN: longest triangle edge to fill (0 = no limit)", ParamKind::Number).default(0.0).help("Triangles with a longer edge are left empty, so water and gaps are not bridged."),
            ];
            params.extend(filter_params());
            ToolSpec { id: "lidar_dem", name: "LAS to Bare-earth DEM", category: "Raster › LiDAR", role: Role::Data,
                description: "Make a bare-earth elevation raster from a LAS or LAZ point cloud (for example USGS 3DEP lidar). Only ground points are used: those the file already classifies as ground (class 2), or ground found with the slope-based filter of Vosselman (2000) as implemented in WhiteboxTools (LidarGroundPointFilter). Cells are interpolated by a Delaunay triangulation of the ground points (LidarTINGridding) or by inverse distance weighting (Shepard 1968; LidarIdwInterpolation), in the file's own coordinate system, which is written into the result with the vertical datum when the file names one. A file without a coordinate system is refused unless you state it. Heights keep the file's vertical unit. Runs WhiteboxTools as a separate process.",
                inputs: vec![], output: Some(PortKind::Raster), params }
        }
        Kind::Dsm => ToolSpec { id: "lidar_dsm", name: "LAS to Surface Model (DSM)", category: "Raster › LiDAR", role: Role::Data,
            description: "Make a top-surface elevation raster (tree tops and roofs included) from a LAS or LAZ point cloud: the highest point in each cell's search area, with gaps filled by triangulation (WhiteboxTools: LidarDigitalSurfaceModel, Lindsay). Uses every point, so remove noise first for clean results. The file's coordinate system and vertical datum are written into the result; a file without a coordinate system is refused unless you state it.",
            inputs: vec![], output: Some(PortKind::Raster),
            params: vec![file, crs, res(1.0), Param::new("radius", "Search radius (file's horizontal unit)", ParamKind::Number).default(0.5), Param::new("max_edge", "Longest triangle edge to fill (0 = no limit)", ParamKind::Number).default(0.0)] },
        Kind::Density => ToolSpec { id: "lidar_point_density", name: "LAS Point Density", category: "Raster › LiDAR", role: Role::Data,
            description: "Points per square unit around each cell of a LAS or LAZ point cloud, counted within a search radius (WhiteboxTools: LidarPointDensity). Useful for checking coverage and choosing a cell size before making a DEM. The result is in points per square unit of the file's coordinate system (per square metre for UTM). A file without a coordinate system is refused unless you state it.",
            inputs: vec![], output: Some(PortKind::Raster),
            params: vec![file, crs, res(2.0), Param::new("radius", "Search radius (file's horizontal unit)", ParamKind::Number).default(2.0), Param::new("returns", "Returns counted", ParamKind::Enum).default("all").options(&[("all", "All returns"), ("last", "Last returns"), ("first", "First returns")])] },
        Kind::Canopy => {
            let mut params = vec![file, crs, res(1.0), ground];
            params.extend(filter_params());
            ToolSpec { id: "lidar_canopy_height", name: "LAS to Canopy Height (Normalised)", category: "Raster › LiDAR", role: Role::Data,
                description: "Height of the vegetation and structures above the ground, from a LAS or LAZ point cloud: every point is made relative to the nearest ground point (WhiteboxTools: HeightAboveGround), then the highest normalised point in each cell is kept (LidarBlockMaximum). Ground is the file's class 2, or found with the slope-based filter. Cells with no point are empty. The result is a height above ground in the file's vertical unit, not an elevation, so it carries no vertical datum. A file without a coordinate system is refused unless you state it.",
                inputs: vec![], output: Some(PortKind::Raster), params }
        }
        Kind::ClassifyGround => ToolSpec { id: "lidar_classify_ground", name: "Classify Ground Points (write LAS)", category: "Raster › LiDAR", role: Role::Output,
            description: "Mark each point of a LAS or LAZ file as ground (class 2) or not ground (class 1) with the slope-based filter of Vosselman (2000) as implemented in WhiteboxTools (LidarGroundPointFilter), and write a new LAS file. The input file is never changed; the output must be a different file. Settings are in the file's horizontal and vertical units.",
            inputs: vec![], output: None,
            params: vec![file, Param::new("path", "Output LAS file", ParamKind::SavePath), Param::new("filter_radius", "Search radius", ParamKind::Number).default(2.0), Param::new("filter_slope", "Maximum slope between points (degrees)", ParamKind::Number).default(45.0), Param::new("filter_height", "Height difference for off-ground", ParamKind::Number).default(1.0), Param::new("min_neighbours", "Minimum neighbouring points", ParamKind::Number).default(0.0)] },
        Kind::Normalize => ToolSpec { id: "lidar_normalize_heights", name: "Normalise Point Heights (write LAS)", category: "Raster › LiDAR", role: Role::Output,
            description: "Replace each point's elevation by its height above the nearest ground-classified point (WhiteboxTools: HeightAboveGround) and write a new LAS file. The input must already have ground points (class 2); use Classify Ground Points first if it does not. The input file is never changed; the output must be a different file.",
            inputs: vec![], output: None, params: vec![file, Param::new("path", "Output LAS file", ParamKind::SavePath)] },
    }
}

impl Tool for Lidar {
    fn spec(&self) -> ToolSpec {
        spec_of(self.0)
    }
    fn result_look(&self) -> ResultLook {
        ResultLook::Derived
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let _ = inputs;
        let k = self.0;
        let spec = spec_of(k);
        let file = PathBuf::from(p.req_str("file", "LAS or LAZ file")?);
        if !pointcloud::is_pointcloud_path(&file) {
            bail!("'{}' must be a .las or .laz file", file.display());
        }
        if !crate::whitebox::available() {
            bail!("{} needs WhiteboxTools, which is not installed with this copy of GIS Studio. Reinstall the app, or set GIS_STUDIO_WHITEBOX to the whitebox_tools program.", spec.name);
        }
        let num = |id: &str, label: &str, min: f64, allow_min: bool| -> Result<f64> {
            // A setting left alone has the default the spec shows.
            let v = p.f64(id).or_else(|| spec.params.iter().find(|q| q.id == id).and_then(|q| q.default.as_f64())).ok_or_else(|| anyhow!("'{label}' must be a number"))?;
            if !v.is_finite() || v < min || (!allow_min && v == min) {
                bail!("'{label}' must be {} {min}", if allow_min { "at least" } else { "more than" });
            }
            Ok(v)
        };
        // Header first (cheap); class counts only when ground points matter.
        let needs_ground = matches!(k, Kind::Dem | Kind::Canopy | Kind::Normalize) && p.str("ground").as_deref() != Some("filter");
        let info = pointcloud::inspect(&file, needs_ground)?;
        if info.point_count == 0 {
            bail!("{} has no points", info.name);
        }
        if needs_ground {
            let ground = info.classes.iter().flatten().find(|c| c.code == 2).map(|c| c.count).unwrap_or(0);
            if ground == 0 {
                bail!("{} has no points classified as ground (class 2); it has: {}. Choose 'Find ground first with the slope-based filter', or run Classify Ground Points and use its output.", info.name, pointcloud::class_summary(&info));
            }
            ctx.add_note(format!("{}: {ground} of {} points are classified as ground and were used.", spec.name, info.point_count));
        }
        let work = crate::saga::Work::new()?;

        match k {
            Kind::ClassifyGround | Kind::Normalize => {
                let out = p.req_str("path", "Output LAS file")?;
                refuse_source_file(ctx, &out)?;
                let (a, b) = (std::fs::canonicalize(&file).unwrap_or(file.clone()), std::fs::canonicalize(&out).unwrap_or(PathBuf::from(&out)));
                if a == b {
                    bail!("The output must be a different file from the input; the original point cloud is never overwritten.");
                }
                if !out.to_ascii_lowercase().ends_with(".las") {
                    bail!("The output file must end in .las (WhiteboxTools writes LAS).");
                }
                let src = las_input(&file, &work)?;
                if k == Kind::ClassifyGround {
                    let args = vec![
                        ("input", src.display().to_string()),
                        ("output", out.clone()),
                        ("radius", num("filter_radius", "Search radius", 0.0, false)?.to_string()),
                        ("min_neighbours", format!("{}", num("min_neighbours", "Minimum neighbouring points", 0.0, true)? as i64)),
                        ("slope_threshold", num("filter_slope", "Maximum slope between points (degrees)", 0.0, false)?.to_string()),
                        ("height_threshold", num("filter_height", "Height difference for off-ground", 0.0, true)?.to_string()),
                        ("classify", "true".into()),
                    ];
                    crate::whitebox::run("LidarGroundPointFilter", &args)?;
                } else {
                    crate::whitebox::run("HeightAboveGround", &[("input", src.display().to_string()), ("output", out.clone())])?;
                }
                if !Path::new(&out).is_file() {
                    bail!("WhiteboxTools did not write {out}");
                }
                let mut msg = format!("Wrote {out} from {} ({} points).", info.name, info.point_count);
                if info.crs.is_none() {
                    msg.push_str(" The input file has no coordinate system, so none was recorded.");
                }
                return Ok(Output::Done(msg));
            }
            _ => {}
        }

        let crs = resolve_crs(&info, p.str("crs"))?;
        let res = num("resolution", "Cell size", 0.0, false)?;
        let cells = ((info.bounds[3] - info.bounds[0]) / res) * ((info.bounds[4] - info.bounds[1]) / res);
        if cells > MAX_CELLS {
            bail!("A cell size of {res} would make a raster of about {:.0} million cells for {}. Use a coarser cell size.", cells / 1e6, info.name);
        }
        let mut src = las_input(&file, &work)?;
        // Ground found by the filter goes through a temporary LAS (the file itself is untouched).
        if p.str("ground").as_deref() == Some("filter") && matches!(k, Kind::Dem | Kind::Canopy) {
            let g = work.path("ground.las");
            let args = vec![
                ("input", src.display().to_string()),
                ("output", g.display().to_string()),
                ("radius", num("filter_radius", "Ground filter: search radius", 0.0, false)?.to_string()),
                ("min_neighbours", "0".into()),
                ("slope_threshold", num("filter_slope", "Ground filter: maximum slope between points (degrees)", 0.0, false)?.to_string()),
                ("height_threshold", num("filter_height", "Ground filter: height difference for off-ground", 0.0, true)?.to_string()),
                ("classify", "true".into()),
            ];
            crate::whitebox::run("LidarGroundPointFilter", &args)?;
            src = g;
        }
        let out = raster::new_store_path(spec.id, "tif");
        let run = |tool: &str, args: Vec<(&str, String)>| -> Result<()> {
            if let Err(e) = crate::whitebox::run(tool, &args) {
                let _ = std::fs::remove_file(&out);
                return Err(e);
            }
            Ok(())
        };
        let o = out.display().to_string();
        let mut keeps_elevation = true;
        match k {
            Kind::Dem => {
                let mut a: Vec<(&str, String)> = vec![("input", src.display().to_string()), ("output", o.clone()), ("resolution", res.to_string()), ("parameter", "elevation".into()), ("returns", "all".into()), ("exclude_cls", NON_GROUND.into())];
                if p.str("method").as_deref() == Some("idw") {
                    a.push(("radius", num("radius", "IDW search radius", 0.0, false)?.to_string()));
                    a.push(("weight", num("weight", "IDW weight (power)", 0.0, true)?.to_string()));
                    run("LidarIdwInterpolation", a)?;
                } else {
                    let me = num("max_edge", "TIN: longest triangle edge to fill", 0.0, true)?;
                    if me > 0.0 {
                        a.push(("max_triangle_edge_length", me.to_string()));
                    }
                    run("LidarTINGridding", a)?;
                }
            }
            Kind::Dsm => {
                let mut a: Vec<(&str, String)> = vec![("input", src.display().to_string()), ("output", o.clone()), ("resolution", res.to_string()), ("radius", num("radius", "Search radius", 0.0, false)?.to_string())];
                let me = num("max_edge", "Longest triangle edge to fill", 0.0, true)?;
                if me > 0.0 {
                    a.push(("max_triangle_edge_length", me.to_string()));
                }
                run("LidarDigitalSurfaceModel", a)?;
            }
            Kind::Density => {
                let returns = p.str("returns").unwrap_or_else(|| "all".into());
                if !matches!(returns.as_str(), "all" | "last" | "first") {
                    bail!("'Returns counted' must be all, last or first");
                }
                keeps_elevation = false;
                run("LidarPointDensity", vec![("input", src.display().to_string()), ("output", o.clone()), ("resolution", res.to_string()), ("radius", num("radius", "Search radius", 0.0, false)?.to_string()), ("returns", returns)])?;
            }
            Kind::Canopy => {
                keeps_elevation = false;
                let norm = work.path("normalised.las");
                crate::whitebox::run("HeightAboveGround", &[("input", src.display().to_string()), ("output", norm.display().to_string())])?;
                run("LidarBlockMaximum", vec![("input", norm.display().to_string()), ("output", o.clone()), ("resolution", res.to_string())])?;
            }
            _ => unreachable!(),
        }
        if !out.is_file() {
            bail!("WhiteboxTools did not produce a raster from {}. Check the file has points inside its bounds and the cell size suits them.", info.name);
        }
        stamp_crs(&out, &crs)?;
        raster::build_overviews(&out)?;
        if keeps_elevation {
            if let Some(v) = &info.vertical_datum {
                let _ = crate::vertical::set(&out, v);
            }
        }
        for n in &info.notes {
            if !n.contains("no coordinate system") {
                ctx.add_note(format!("{}: {n}", info.name));
            }
        }
        ctx.add_note(format!("{}: coordinate system {} from {} was written into the result.", spec.name, info.crs_name.clone().unwrap_or_else(|| crs.clone()), info.crs_source.clone().unwrap_or_else(|| "the Coordinate system setting".into())));
        if let Some((n, bad)) = count_empty(&out) {
            if bad > 0 {
                ctx.add_note(format!("{}: {bad} of {n} cells have no data (no point reached them).", spec.name));
            }
        }
        Ok(Output::Raster { path: out.display().to_string(), crs: Some(crs) })
    }
}

/// The CRS to use: the file's own, or the one the user stated when the file has none. Both and different: stop.
fn resolve_crs(info: &PointCloudInfo, user: Option<String>) -> Result<String> {
    match (&info.crs, user) {
        (Some(f), Some(u)) => {
            if crate::crs::same(f, &u) {
                Ok(f.clone())
            } else {
                bail!("{} says its coordinate system is {} but you chose {}. They differ, so nothing was run. Clear the 'Coordinate system' setting to use the file's own.", info.name, info.crs_name.clone().unwrap_or_else(|| f.clone()), u)
            }
        }
        (Some(f), None) => Ok(f.clone()),
        (None, Some(u)) => {
            crate::crs::srs(&crate::crs::resolve(&u))?;
            Ok(u)
        }
        (None, None) => bail!("{} has no coordinate system (no WKT record and no GeoTIFF keys), and none is guessed. Choose its coordinate system in the 'Coordinate system' setting.", info.name),
    }
}

/// The program reads LAS; LAZ is unpacked first into the work folder. Returns the path to hand over.
fn las_input(file: &Path, work: &crate::saga::Work) -> Result<PathBuf> {
    let is_laz = file.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("laz"));
    if !is_laz {
        return Ok(file.to_path_buf());
    }
    let out = work.path("input.las");
    let mut reader = las::Reader::from_path(file).map_err(|e| anyhow!("Could not read {}: {e}", file.display()))?;
    let header = las::Builder::from(reader.header().clone()).into_header().map_err(|e| anyhow!("{e}"))?;
    let mut w = las::Writer::from_path(&out, header).map_err(|e| anyhow!("Could not write the unpacked copy: {e}"))?;
    let total = reader.header().number_of_points().max(1);
    let task = crate::progress::Task::start(format!("Unpacking {}", file.file_name().and_then(|s| s.to_str()).unwrap_or("LAZ"))).cancellable();
    let mut pd = las::PointDataBuilder::new().for_header(reader.header()).build();
    let mut done = 0u64;
    loop {
        let n = reader.fill_points(1_000_000, &mut pd).map_err(|e| anyhow!("{e}"))?;
        if n == 0 {
            break;
        }
        w.write_points(&pd).map_err(|e| anyhow!("{e}"))?;
        done += n;
        task.set(done as f64 / total as f64, Some(format!("{done} of {total} points")));
        task.check()?;
    }
    w.close().map_err(|e| anyhow!("{e}"))?;
    Ok(out)
}

/// Write the CRS into the result raster (WhiteboxTools does not take it from the LAS file).
fn stamp_crs(out: &Path, crs: &str) -> Result<()> {
    let mut ds = gdal::Dataset::open_ex(out, gdal::DatasetOptions { open_flags: gdal::GdalOpenFlags::GDAL_OF_UPDATE | gdal::GdalOpenFlags::GDAL_OF_RASTER, ..Default::default() })
        .map_err(|_| anyhow!("WhiteboxTools did not produce a raster."))?;
    let wkt = crate::crs::srs(&crate::crs::resolve(crs))?.to_wkt()?;
    ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_wkt(&wkt)?)?;
    Ok(())
}

fn count_empty(path: &Path) -> Option<(u64, u64)> {
    let ds = gdal::Dataset::open(path).ok()?;
    let band = ds.rasterband(1).ok()?;
    let nd = band.no_data_value();
    let (cols, rows) = ds.raster_size();
    let step = (4_000_000 / cols.max(1)).clamp(1, rows.max(1));
    let (mut n, mut bad, mut r) = (0u64, 0u64, 0);
    while r < rows {
        let h = step.min(rows - r);
        let buf = band.read_as::<f64>((0, r as isize), (cols, h), (cols, h), None).ok()?;
        for v in buf.data() {
            n += 1;
            if !v.is_finite() || nd.is_some_and(|d| *v == d) {
                bad += 1;
            }
        }
        r += h;
    }
    Some((n, bad))
}
