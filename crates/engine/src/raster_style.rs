//! Raster symbology (`LayerStyle.raster`): single-band gray and pseudocolour
//! renderers with stretches, a colour-ramp library, transparency and image
//! adjustments. `raster::render_extent_styled` warps the data and calls
//! [`colorize`]; the map tiles, the projected map view and layouts share it.
//!
//! The renderer names and settings follow the common cartographic ones
//! (QGIS "Singleband gray" / "Singleband pseudocolor", ArcGIS Pro "Stretch"
//! and "Classify"); the code is our own. Ramp colours: the viridis family
//! (matplotlib, CC0) and ColorBrewer 2.0 (Cynthia Brewer, Apache-2.0).

use crate::raster::RasterInfo;
use anyhow::{anyhow, bail, Result};
use base64::Engine as _;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Renderer {
    /// Elevation ramp + hillshade for one-band rasters, RGB for imagery (today's look).
    #[default]
    Default,
    Gray,
    Pseudocolor,
    /// Unique values: each listed cell value gets its own colour; other values are transparent (QGIS "Paletted/Unique values", ArcGIS "Unique Values"). Drawn with nearest-neighbour resampling.
    Paletted,
    /// Three bands as red, green and blue, each stretched on its own (QGIS "Multiband color", ArcGIS "RGB Composite").
    Multiband,
}

/// One class of a paletted raster.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PalEntry {
    pub value: f64,
    pub color: String,
    #[serde(default)]
    pub label: String,
    /// Hidden classes are transparent.
    #[serde(default)]
    pub hidden: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum StretchMode {
    #[default]
    MinMax,
    /// Clip `pct_low`..`pct_high` percent (e.g. 2 and 98).
    Percentile,
    /// mean ± `sigma` standard deviations.
    StdDev,
    Manual,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ColorMode {
    /// Colours blend between stops.
    #[default]
    Linear,
    /// The range is cut into one equal class per stop.
    Discrete,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Stretch {
    pub mode: StretchMode,
    pub pct_low: f64,
    pub pct_high: f64,
    pub sigma: f64,
    pub min: f64,
    pub max: f64,
}

impl Default for Stretch {
    fn default() -> Self {
        Stretch { mode: StretchMode::MinMax, pct_low: 2.0, pct_high: 98.0, sigma: 2.0, min: 0.0, max: 1.0 }
    }
}

/// One colour stop of a custom ramp: `pos` 0..1 along the stretched range.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Stop {
    pub pos: f32,
    pub color: String,
    #[serde(default = "one")]
    pub alpha: f32,
}

fn one() -> f32 {
    1.0
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct RasterStyle {
    pub renderer: Renderer,
    /// 1-based band drawn by the gray and pseudocolour renderers.
    pub band: usize,
    pub stretch: Stretch,
    /// A name from [`ramps`], or "custom" to use `stops`.
    pub ramp: String,
    pub stops: Vec<Stop>,
    pub invert: bool,
    pub mode: ColorMode,
    /// Class upper bounds (ascending, n-1 for n classes). With `mode: "discrete"` and breaks, the ramp is sampled once per class (classified rendering); without, one class per ramp stop.
    pub breaks: Vec<f64>,
    /// Multiband: the band drawn as red, green, blue (1-based).
    pub rgb_bands: [usize; 3],
    /// Multiband: a stretch per channel; empty = `stretch` applied to every channel with that band's own statistics.
    pub channel_stretch: Vec<Stretch>,
    /// Classes of the paletted renderer.
    pub entries: Vec<PalEntry>,
    pub opacity: f32,
    /// Cells equal to the raster's NoData value are transparent.
    pub nodata_transparent: bool,
    /// Cell values inside any `[lo, hi]` here are transparent.
    pub transparent_ranges: Vec<[f64; 2]>,
    /// -1..1 added to each channel.
    pub brightness: f32,
    /// -1..1; 0 = unchanged.
    pub contrast: f32,
    /// > 0; 1 = unchanged, above 1 brightens mid-tones.
    pub gamma: f32,
    /// -1..1; -1 = grayscale.
    pub saturation: f32,
}

impl Default for RasterStyle {
    fn default() -> Self {
        RasterStyle {
            renderer: Renderer::Default,
            band: 1,
            stretch: Stretch::default(),
            ramp: "viridis".into(),
            stops: Vec::new(),
            invert: false,
            mode: ColorMode::Linear,
            breaks: Vec::new(),
            entries: Vec::new(),
            rgb_bands: [1, 2, 3],
            channel_stretch: Vec::new(),
            opacity: 1.0,
            nodata_transparent: true,
            transparent_ranges: Vec::new(),
            brightness: 0.0,
            contrast: 0.0,
            gamma: 1.0,
            saturation: 0.0,
        }
    }
}

impl RasterStyle {
    /// Decode the token carried in a tile URL (`base64url(JSON)`); empty → default.
    pub fn from_token(token: &str) -> Result<RasterStyle> {
        if token.is_empty() {
            return Ok(RasterStyle::default());
        }
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(token).map_err(|e| anyhow!("The raster style in the tile request is not valid: {e}"))?;
        serde_json::from_slice(&bytes).map_err(|e| anyhow!("The raster style is not valid: {e}"))
    }

    pub fn to_token(&self) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(serde_json::to_vec(self).unwrap_or_default())
    }

    pub fn validate(&self, info: &RasterInfo) -> Result<()> {
        if self.band == 0 || self.band > info.bands {
            bail!("Band {} doesn't exist; this raster has {} band(s)", self.band, info.bands);
        }
        if self.gamma <= 0.0 {
            bail!("Gamma must be above 0");
        }
        if self.stretch.mode == StretchMode::Manual && self.stretch.min >= self.stretch.max {
            bail!("Stretch minimum must be below the maximum");
        }
        if self.breaks.iter().any(|b| !b.is_finite()) || self.breaks.windows(2).any(|w| w[0] >= w[1]) {
            bail!("Class breaks must be numbers in increasing order");
        }
        if self.renderer == Renderer::Multiband {
            if let Some(b) = self.rgb_bands.iter().find(|b| **b == 0 || **b > info.bands) {
                bail!("Band {b} doesn't exist; this raster has {} band(s)", info.bands);
            }
            if !self.channel_stretch.is_empty() && self.channel_stretch.len() != 3 {
                bail!("Give a stretch for each of the three colour channels");
            }
            return Ok(());
        }
        if self.renderer == Renderer::Paletted {
            if self.entries.is_empty() {
                bail!("Add at least one class to the unique-values symbology");
            }
            if let Some(e) = self.entries.iter().find(|e| parse_hex(&e.color).is_none()) {
                bail!("'{}' is not a colour like #aabbcc", e.color);
            }
            return Ok(());
        }
        if self.ramp == "custom" && self.stops.len() < 2 {
            bail!("A custom colour ramp needs at least two stops");
        }
        if self.ramp != "custom" && ramp_colors(&self.ramp).is_none() {
            bail!("Unknown colour ramp '{}'", self.ramp);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- ramps

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RampInfo {
    pub name: &'static str,
    /// "Scientific" | "Terrain" | "Sequential" | "Diverging" | "Qualitative" | "Gray".
    pub group: &'static str,
    pub colors: Vec<String>,
    /// Qualitative ramps are meant for classes, not smooth blending.
    pub discrete: bool,
    /// Distinguishable with the common colour-vision deficiencies (viridis family, single-hue sequentials, and the blue–orange/brown–teal diverging sets).
    pub cb_safe: bool,
}

/// Ramps that stay readable for red–green colour-blind viewers.
const CB_SAFE: &[&str] = &["viridis", "magma", "inferno", "plasma", "cividis", "gray", "bathymetry", "Blues", "Greens", "Oranges", "Purples", "YlOrRd", "YlGnBu", "RdBu", "BrBG", "PuOr"];

const RAMPS: &[(&str, &str, bool, &str)] = &[
    ("viridis", "Scientific", false, "440154 482878 3e4989 31688e 26828e 1f9e89 35b779 6ece58 b5de2b fde725"),
    ("magma", "Scientific", false, "000004 180f3e 451077 721f81 9f2f7f cd4071 f1605d fd9567 fec98d fcfdbf"),
    ("inferno", "Scientific", false, "000004 1b0c42 4b0c6b 781c6d a52c60 cf4446 ed6925 fb9b06 f7d13d fcffa4"),
    ("plasma", "Scientific", false, "0d0887 4b03a1 7d03a8 a82296 cb4679 e56b5d f89441 fdc328 f0f921"),
    ("cividis", "Scientific", false, "00204d 00336f 39486b 575c6e 707173 8a8779 a69d75 c4b56c e4cf5b ffea46"),
    ("hypsometric", "Terrain", false, "46784f 76a662 ccce8a d6b278 aa8264 f5f5f5"),
    ("terrain", "Terrain", false, "333399 0099ff 00cc66 99dd66 ffff99 cc9933 996633 ffffff"),
    ("bathymetry", "Terrain", false, "08306b 2171b5 6baed6 c6dbef f7fbff"),
    ("gray", "Gray", false, "000000 ffffff"),
    ("Blues", "Sequential", false, "f7fbff deebf7 c6dbef 9ecae1 6baed6 4292c6 2171b5 08519c 08306b"),
    ("Greens", "Sequential", false, "f7fcf5 e5f5e0 c7e9c0 a1d99b 74c476 41ab5d 238b45 006d2c 00441b"),
    ("Oranges", "Sequential", false, "fff5eb fee6ce fdd0a2 fdae6b fd8d3c f16913 d94801 a63603 7f2704"),
    ("Reds", "Sequential", false, "fff5f0 fee0d2 fcbba1 fc9272 fb6a4a ef3b2c cb181d a50f15 67000d"),
    ("Purples", "Sequential", false, "fcfbfd efedf5 dadaeb bcbddc 9e9ac8 807dba 6a51a3 54278f 3f007d"),
    ("YlOrRd", "Sequential", false, "ffffcc ffeda0 fed976 feb24c fd8d3c fc4e2a e31a1c bd0026 800026"),
    ("YlGnBu", "Sequential", false, "ffffd9 edf8b1 c7e9b4 7fcdbb 41b6c4 1d91c0 225ea8 253494 081d58"),
    ("RdBu", "Diverging", false, "67001f b2182b d6604d f4a582 fddbc7 f7f7f7 d1e5f0 92c5de 4393c3 2166ac 053061"),
    ("RdYlBu", "Diverging", false, "a50026 d73027 f46d43 fdae61 fee090 ffffbf e0f3f8 abd9e9 74add1 4575b4 313695"),
    ("RdYlGn", "Diverging", false, "a50026 d73027 f46d43 fdae61 fee08b ffffbf d9ef8b a6d96a 66bd63 1a9850 006837"),
    ("Spectral", "Diverging", false, "9e0142 d53e4f f46d43 fdae61 fee08b ffffbf e6f598 abdda4 66c2a5 3288bd 5e4fa2"),
    ("BrBG", "Diverging", false, "543005 8c510a bf812d dfc27d f6e8c3 f5f5f5 c7eae5 80cdc1 35978f 01665e 003c30"),
    ("PuOr", "Diverging", false, "7f3b08 b35806 e08214 fdb863 fee0b6 f7f7f7 d8daeb b2abd2 8073ac 542788 2d004b"),
    ("Set1", "Qualitative", true, "e41a1c 377eb8 4daf4a 984ea3 ff7f00 ffff33 a65628 f781bf 999999"),
    ("Set2", "Qualitative", true, "66c2a5 fc8d62 8da0cb e78ac3 a6d854 ffd92f e5c494 b3b3b3"),
    ("Dark2", "Qualitative", true, "1b9e77 d95f02 7570b3 e7298a 66a61e e6ab02 a6761d 666666"),
    ("Paired", "Qualitative", true, "a6cee3 1f78b4 b2df8a 33a02c fb9a99 e31a1c fdbf6f ff7f00 cab2d6 6a3d9a ffff99 b15928"),
];

/// The ramp library (the UI reads it through the `raster_ramps` command; no parallel list there).
pub fn ramps() -> Vec<RampInfo> {
    RAMPS
        .iter()
        .map(|(name, group, discrete, hex)| RampInfo { name, group, colors: hex.split(' ').map(|h| format!("#{h}")).collect(), discrete: *discrete, cb_safe: CB_SAFE.contains(name) })
        .collect()
}

fn ramp_colors(name: &str) -> Option<Vec<[f32; 3]>> {
    RAMPS.iter().find(|r| r.0 == name).map(|r| r.3.split(' ').filter_map(|h| parse_hex(h)).collect())
}

pub fn parse_hex(s: &str) -> Option<[f32; 3]> {
    let s = s.trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some([((v >> 16) & 255) as f32, ((v >> 8) & 255) as f32, (v & 255) as f32])
}

/// Stops (position, rgb 0–255, alpha) after resolving the named ramp and `invert`.
pub fn resolved_stops(style: &RasterStyle) -> Result<Vec<(f32, [f32; 3], f32)>> {
    let mut stops: Vec<(f32, [f32; 3], f32)> = if style.ramp == "custom" {
        let mut v = Vec::new();
        for s in &style.stops {
            let c = parse_hex(&s.color).ok_or_else(|| anyhow!("'{}' is not a colour like #aabbcc", s.color))?;
            v.push((s.pos.clamp(0.0, 1.0), c, s.alpha.clamp(0.0, 1.0)));
        }
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        v
    } else {
        let cols = ramp_colors(&style.ramp).ok_or_else(|| anyhow!("Unknown colour ramp '{}'", style.ramp))?;
        let n = (cols.len() - 1).max(1) as f32;
        cols.iter().enumerate().map(|(i, c)| (i as f32 / n, *c, 1.0)).collect()
    };
    if stops.len() < 2 {
        bail!("A colour ramp needs at least two stops");
    }
    if style.invert {
        stops = stops.into_iter().rev().map(|(p, c, a)| (1.0 - p, c, a)).collect();
    }
    Ok(stops)
}

const LUT: usize = 256;

/// 256-entry RGBA lookup for t in 0..1.
fn build_lut(style: &RasterStyle) -> Result<Vec<[f32; 4]>> {
    let stops = resolved_stops(style)?;
    let at = |t: f32| -> [f32; 4] {
        if t <= stops[0].0 {
            let s = &stops[0];
            return [s.1[0], s.1[1], s.1[2], s.2];
        }
        for w in stops.windows(2) {
            if t <= w[1].0 {
                let span = (w[1].0 - w[0].0).max(1e-9);
                let f = (t - w[0].0) / span;
                let (a, b) = (&w[0], &w[1]);
                return [a.1[0] + (b.1[0] - a.1[0]) * f, a.1[1] + (b.1[1] - a.1[1]) * f, a.1[2] + (b.1[2] - a.1[2]) * f, a.2 + (b.2 - a.2) * f];
            }
        }
        let s = &stops[stops.len() - 1];
        [s.1[0], s.1[1], s.1[2], s.2]
    };
    let n = stops.len();
    Ok((0..LUT)
        .map(|i| {
            let t = i as f32 / (LUT - 1) as f32;
            match style.mode {
                ColorMode::Linear => at(t),
                ColorMode::Discrete => {
                    let k = ((t * n as f32).floor() as usize).min(n - 1);
                    let s = &stops[k];
                    [s.1[0], s.1[1], s.1[2], s.2]
                }
            }
        })
        .collect())
}

// ---------------------------------------------------------------- statistics

/// Statistics of one band from a ≤ 1024×1024 sample (exact for small rasters).
#[derive(Debug)]
pub struct BandStats {
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub std: f64,
    /// Sorted valid sample values.
    sorted: Vec<f32>,
}

impl BandStats {
    pub fn percentile(&self, pct: f64) -> f64 {
        if self.sorted.is_empty() {
            return 0.0;
        }
        let r = (pct.clamp(0.0, 100.0) / 100.0) * (self.sorted.len() - 1) as f64;
        let (i, f) = (r.floor() as usize, r.fract());
        let a = self.sorted[i] as f64;
        let b = self.sorted[(i + 1).min(self.sorted.len() - 1)] as f64;
        a + (b - a) * f
    }

    /// Histogram of the sample: `bins` counts between min and max.
    pub fn histogram(&self, bins: usize) -> Vec<u64> {
        let mut h = vec![0u64; bins.max(1)];
        let span = (self.max - self.min).max(1e-12);
        for v in &self.sorted {
            let i = (((*v as f64 - self.min) / span) * h.len() as f64) as usize;
            let n = h.len();
            h[i.min(n - 1)] += 1;
        }
        h
    }
}

type StatsCache = Mutex<HashMap<(String, usize), Arc<BandStats>>>;

fn cache() -> &'static StatsCache {
    static C: OnceLock<StatsCache> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn band_stats(info: &RasterInfo, band: usize) -> Result<Arc<BandStats>> {
    let key = (info.path.clone(), band);
    if let Some(s) = cache().lock().get(&key) {
        return Ok(s.clone());
    }
    let ds = gdal::Dataset::open(&info.path).map_err(|e| anyhow!("GDAL could not open {}: {e}", info.path))?;
    let b = ds.rasterband(band).map_err(|_| anyhow!("Band {band} doesn't exist"))?;
    let (w, h) = (info.width, info.height);
    let scale = (1024.0 / w.max(h) as f64).min(1.0);
    let (sw, sh) = (((w as f64 * scale) as usize).max(1), ((h as f64 * scale) as usize).max(1));
    let data = b.read_as::<f32>((0, 0), (w, h), (sw, sh), None).map_err(|e| anyhow!("Could not read band {band}: {e}"))?;
    let nd = b.no_data_value().map(|v| v as f32);
    let mut v: Vec<f32> = data.data().iter().copied().filter(|x| x.is_finite() && nd.map_or(true, |n| (*x - n).abs() > 1e-6)).collect();
    v.sort_by(|a, b| a.total_cmp(b));
    if v.is_empty() {
        bail!("Band {band} has no valid cells to draw");
    }
    let n = v.len() as f64;
    let mean = v.iter().map(|x| *x as f64).sum::<f64>() / n;
    let std = (v.iter().map(|x| (*x as f64 - mean).powi(2)).sum::<f64>() / n).sqrt();
    let s = Arc::new(BandStats { min: v[0] as f64, max: v[v.len() - 1] as f64, mean, std, sorted: v });
    cache().lock().insert(key, s.clone());
    Ok(s)
}

/// The data range the ramp is stretched over.
pub fn stretch_range(info: &RasterInfo, style: &RasterStyle) -> Result<(f64, f64)> {
    range_of(info, style.band, &style.stretch)
}

/// Range of one multiband colour channel (0 = red, 1 = green, 2 = blue).
pub fn channel_range(info: &RasterInfo, style: &RasterStyle, ch: usize) -> Result<(f64, f64)> {
    range_of(info, style.rgb_bands[ch], style.channel_stretch.get(ch).unwrap_or(&style.stretch))
}

fn range_of(info: &RasterInfo, band: usize, st: &Stretch) -> Result<(f64, f64)> {
    let (lo, hi) = match st.mode {
        StretchMode::Manual => (st.min, st.max),
        StretchMode::MinMax => {
            let s = band_stats(info, band)?;
            (s.min, s.max)
        }
        StretchMode::Percentile => {
            let s = band_stats(info, band)?;
            (s.percentile(st.pct_low), s.percentile(st.pct_high))
        }
        StretchMode::StdDev => {
            let s = band_stats(info, band)?;
            ((s.mean - st.sigma * s.std).max(s.min), (s.mean + st.sigma * s.std).min(s.max))
        }
    };
    Ok(if hi > lo { (lo, hi) } else { (lo, lo + 1e-6) })
}

// ---------------------------------------------------------------- classification

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ClassMethod {
    Equal,
    Quantile,
    /// Fisher–Jenks natural breaks (Jenks 1967; Fisher 1958), on a ≤ 1000-value quantile sample.
    Jenks,
    /// Classes one standard deviation wide, centred on the mean.
    StdDev,
}

/// Class upper bounds (n-1 interior breaks; fewer when values repeat) for `n` classes.
pub fn classify(stats: &BandStats, method: ClassMethod, n: usize) -> Result<Vec<f64>> {
    if !(2..=64).contains(&n) {
        bail!("Choose between 2 and 64 classes");
    }
    let mut b: Vec<f64> = match method {
        ClassMethod::Equal => (1..n).map(|i| stats.min + (stats.max - stats.min) * i as f64 / n as f64).collect(),
        ClassMethod::Quantile => (1..n).map(|i| stats.percentile(100.0 * i as f64 / n as f64)).collect(),
        ClassMethod::StdDev => {
            let half = n as f64 / 2.0;
            (1..n).map(|i| stats.mean + (i as f64 - half) * stats.std).filter(|v| *v > stats.min && *v < stats.max).collect()
        }
        ClassMethod::Jenks => jenks(&stats.sorted, n),
    };
    b.dedup_by(|a, c| (*a - *c).abs() < 1e-12);
    Ok(b)
}

fn jenks(sorted: &[f32], n: usize) -> Vec<f64> {
    let m = sorted.len().min(1000);
    if m < n + 1 {
        return Vec::new();
    }
    let data: Vec<f64> = (0..m).map(|i| sorted[i * (sorted.len() - 1) / (m - 1)] as f64).collect();
    // Prefix sums for O(1) within-class variance.
    let (mut s1, mut s2) = (vec![0.0; m + 1], vec![0.0; m + 1]);
    for i in 0..m {
        s1[i + 1] = s1[i] + data[i];
        s2[i + 1] = s2[i] + data[i] * data[i];
    }
    let cost = |a: usize, b: usize| -> f64 {
        // sum of squared deviations of data[a..b]
        let (c, s, q) = ((b - a) as f64, s1[b] - s1[a], s2[b] - s2[a]);
        q - s * s / c
    };
    let mut dp = vec![vec![f64::INFINITY; m + 1]; n + 1];
    let mut cut = vec![vec![0usize; m + 1]; n + 1];
    dp[0][0] = 0.0;
    for k in 1..=n {
        for j in k..=m {
            for i in (k - 1)..j {
                let c = dp[k - 1][i] + cost(i, j);
                if c < dp[k][j] {
                    dp[k][j] = c;
                    cut[k][j] = i;
                }
            }
        }
    }
    let mut breaks = Vec::new();
    let mut j = m;
    for k in (2..=n).rev() {
        let i = cut[k][j];
        // The break sits between the classes: the last value of the lower class.
        breaks.push(data[i - 1]);
        j = i;
    }
    breaks.reverse();
    breaks
}

// ---------------------------------------------------------------- colouring

fn adjust(c: [f32; 3], s: &RasterStyle) -> [f32; 3] {
    if s.brightness == 0.0 && s.contrast == 0.0 && s.gamma == 1.0 && s.saturation == 0.0 {
        return c;
    }
    let inv_g = 1.0 / s.gamma;
    let mut o = [0f32; 3];
    for i in 0..3 {
        let v = (c[i] / 255.0).clamp(0.0, 1.0).powf(inv_g);
        o[i] = (v - 0.5) * (1.0 + s.contrast) + 0.5 + s.brightness;
    }
    if s.saturation != 0.0 {
        let l = 0.2126 * o[0] + 0.7152 * o[1] + 0.0722 * o[2];
        for v in &mut o {
            *v = l + (*v - l) * (1.0 + s.saturation);
        }
    }
    [o[0] * 255.0, o[1] * 255.0, o[2] * 255.0]
}

/// RGBA pixels for one band's `values` (`alpha` from the warp: 0 outside the raster).
pub fn colorize(values: &[f32], alpha: &[f32], info: &RasterInfo, style: &RasterStyle, range: (f64, f64)) -> Result<Vec<u8>> {
    if style.renderer == Renderer::Paletted {
        return colorize_paletted(values, alpha, info, style);
    }
    let gray = style.renderer == Renderer::Gray;
    let classed = style.mode == ColorMode::Discrete && !style.breaks.is_empty();
    let lut = if gray { Vec::new() } else if classed { build_lut(&RasterStyle { mode: ColorMode::Linear, ..style.clone() })? } else { build_lut(style)? };
    let span = (range.1 - range.0).max(1e-12);
    let nodata = info.nodata.map(|n| n as f32);
    let mut out = vec![0u8; values.len() * 4];
    for (i, &v) in values.iter().enumerate() {
        if alpha[i] < 1.0 || !v.is_finite() {
            continue;
        }
        if style.nodata_transparent && nodata.map_or(false, |n| (v - n).abs() < 1e-6) {
            continue;
        }
        if style.transparent_ranges.iter().any(|r| (v as f64) >= r[0] && (v as f64) <= r[1]) {
            continue;
        }
        let mut t = (((v as f64 - range.0) / span).clamp(0.0, 1.0)) as f32;
        if classed {
            let k = style.breaks.partition_point(|b| *b < v as f64);
            t = k as f32 / (style.breaks.len().max(1)) as f32;
        }
        let (rgb, a) = if gray {
            let g = if style.invert { 1.0 - t } else { t } * 255.0;
            ([g, g, g], 1.0)
        } else {
            let e = lut[(t * (LUT - 1) as f32).round() as usize];
            ([e[0], e[1], e[2]], e[3])
        };
        let rgb = adjust(rgb, style);
        for c in 0..3 {
            out[i * 4 + c] = rgb[c].clamp(0.0, 255.0).round() as u8;
        }
        out[i * 4 + 3] = (a * style.opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    Ok(out)
}

/// RGBA pixels for a three-band composite; each channel is stretched over its own range.
pub fn colorize_rgb(ch: [&[f32]; 3], alpha: &[f32], info: &RasterInfo, style: &RasterStyle, ranges: [(f64, f64); 3]) -> Result<Vec<u8>> {
    let nodata = info.nodata.map(|n| n as f32);
    let mut out = vec![0u8; alpha.len() * 4];
    for i in 0..alpha.len() {
        let v = [ch[0][i], ch[1][i], ch[2][i]];
        if alpha[i] < 1.0 || v.iter().any(|x| !x.is_finite()) {
            continue;
        }
        if style.nodata_transparent && nodata.map_or(false, |n| v.iter().all(|x| (*x - n).abs() < 1e-6)) {
            continue;
        }
        let mut c = [0f32; 3];
        for k in 0..3 {
            let span = (ranges[k].1 - ranges[k].0).max(1e-12);
            c[k] = (((v[k] as f64 - ranges[k].0) / span).clamp(0.0, 1.0) * 255.0) as f32;
        }
        let c = adjust(c, style);
        for k in 0..3 {
            out[i * 4 + k] = c[k].clamp(0.0, 255.0).round() as u8;
        }
        out[i * 4 + 3] = (style.opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    Ok(out)
}

fn colorize_paletted(values: &[f32], alpha: &[f32], info: &RasterInfo, style: &RasterStyle) -> Result<Vec<u8>> {
    let mut table: HashMap<i64, [f32; 3]> = HashMap::new();
    for e in style.entries.iter().filter(|e| !e.hidden) {
        let c = parse_hex(&e.color).ok_or_else(|| anyhow!("'{}' is not a colour like #aabbcc", e.color))?;
        table.insert(e.value.round() as i64, c);
    }
    let nodata = info.nodata.map(|n| n as f32);
    let mut out = vec![0u8; values.len() * 4];
    for (i, &v) in values.iter().enumerate() {
        if alpha[i] < 1.0 || !v.is_finite() || (style.nodata_transparent && nodata.map_or(false, |n| (v - n).abs() < 1e-6)) {
            continue;
        }
        let Some(c) = table.get(&(v.round() as i64)) else { continue };
        let c = adjust(*c, style);
        for k in 0..3 {
            out[i * 4 + k] = c[k].clamp(0.0, 255.0).round() as u8;
        }
        out[i * 4 + 3] = (style.opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    Ok(out)
}

/// Distinct cell values of a band with counts, and the colour-table colour GDAL stores for each (if any).
/// Reads the whole band up to 16 million cells, otherwise a ≤ 1024×1024 sample (`sampled` = true; counts are then relative).
#[derive(Serialize, Debug)]
pub struct UniqueValue {
    pub value: f64,
    pub count: u64,
    pub color: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct UniqueValues {
    pub values: Vec<UniqueValue>,
    pub sampled: bool,
    /// More distinct values than `max`: the list was cut (an integer land-cover raster has few, a float DEM thousands).
    pub truncated: bool,
}

pub fn unique_values(info: &RasterInfo, band: usize, max: usize) -> Result<UniqueValues> {
    let ds = gdal::Dataset::open(&info.path).map_err(|e| anyhow!("GDAL could not open {}: {e}", info.path))?;
    let b = ds.rasterband(band).map_err(|_| anyhow!("Band {band} doesn't exist"))?;
    let (w, h) = (info.width, info.height);
    let sampled = w * h > 16_000_000;
    let (sw, sh) = if sampled {
        let k = 1024.0 / w.max(h) as f64;
        (((w as f64 * k) as usize).max(1), ((h as f64 * k) as usize).max(1))
    } else {
        (w, h)
    };
    let data = b.read_as::<f32>((0, 0), (w, h), (sw, sh), None).map_err(|e| anyhow!("Could not read band {band}: {e}"))?;
    let nd = b.no_data_value().map(|v| v as f32);
    let mut counts: std::collections::BTreeMap<i64, u64> = std::collections::BTreeMap::new();
    for &v in data.data() {
        if !v.is_finite() || nd.map_or(false, |n| (v - n).abs() < 1e-6) {
            continue;
        }
        *counts.entry(v.round() as i64).or_default() += 1;
    }
    let truncated = counts.len() > max;
    let ct = b.color_table();
    let values = counts
        .into_iter()
        .take(max)
        .map(|(v, count)| {
            let color = ct.as_ref().and_then(|t| if v >= 0 { t.entry_as_rgb(v as usize) } else { None }).map(|c| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b));
            UniqueValue { value: v as f64, count, color }
        })
        .collect();
    Ok(UniqueValues { values, sampled, truncated })
}

/// Legend colour bar: `n` colours across the ramp (hex), with the stretched range, for layouts and the UI.
pub fn legend_bar(info: &RasterInfo, style: &RasterStyle, n: usize) -> Result<(Vec<String>, f64, f64)> {
    let range = stretch_range(info, style)?;
    let mut s2 = style.clone();
    s2.brightness = style.brightness;
    let lut = build_lut(&s2)?;
    let cols = (0..n.max(2))
        .map(|i| {
            let t = i as f32 / (n.max(2) - 1) as f32;
            let c = if style.renderer == Renderer::Gray {
                let g = if style.invert { 1.0 - t } else { t } * 255.0;
                [g, g, g]
            } else {
                let e = lut[(t * (LUT - 1) as f32).round() as usize];
                [e[0], e[1], e[2]]
            };
            let c = adjust(c, style);
            format!("#{:02x}{:02x}{:02x}", c[0].clamp(0.0, 255.0) as u8, c[1].clamp(0.0, 255.0) as u8, c[2].clamp(0.0, 255.0) as u8)
        })
        .collect();
    Ok((cols, range.0, range.1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> RasterInfo {
        RasterInfo { path: String::new(), width: 2, height: 2, bands: 1, data_type: "Float32".into(), nodata: Some(-9999.0), min: 0.0, max: 10.0, kind: "elevation".into(), pixel_size: 1.0, empty: false }
    }

    fn pseudo(ramp: &str) -> RasterStyle {
        RasterStyle { renderer: Renderer::Pseudocolor, ramp: ramp.into(), ..Default::default() }
    }

    #[test]
    fn ramps_parse_and_are_unique() {
        let all = ramps();
        assert!(all.len() >= 25);
        for r in &all {
            assert!(r.colors.len() >= 2, "{}", r.name);
            assert!(r.colors.iter().all(|c| parse_hex(c).is_some()), "{}", r.name);
        }
        let mut names: Vec<_> = all.iter().map(|r| r.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), all.len());
        for g in ["Scientific", "Terrain", "Sequential", "Diverging", "Qualitative"] {
            assert!(all.iter().any(|r| r.group == g), "{g}");
        }
    }

    #[test]
    fn linear_ramp_ends_and_middle() {
        let s = pseudo("gray");
        let px = colorize(&[0.0, 5.0, 10.0], &[255.0; 3], &info(), &s, (0.0, 10.0)).unwrap();
        assert_eq!(&px[0..4], &[0, 0, 0, 255]);
        assert!((px[4] as i32 - 128).abs() <= 1);
        assert_eq!(&px[8..12], &[255, 255, 255, 255]);
    }

    #[test]
    fn invert_swaps_ends() {
        let mut s = pseudo("gray");
        s.invert = true;
        let px = colorize(&[0.0, 10.0], &[255.0; 2], &info(), &s, (0.0, 10.0)).unwrap();
        assert_eq!(&px[0..3], &[255, 255, 255]);
        assert_eq!(&px[4..7], &[0, 0, 0]);
    }

    #[test]
    fn discrete_makes_flat_classes() {
        let mut s = pseudo("Set1");
        s.mode = ColorMode::Discrete;
        let n = ramp_colors("Set1").unwrap().len() as f32;
        let a = colorize(&[0.01], &[255.0], &info(), &s, (0.0, 10.0)).unwrap();
        let b = colorize(&[(0.9 / n) * 10.0], &[255.0], &info(), &s, (0.0, 10.0)).unwrap();
        assert_eq!(&a[0..3], &b[0..3]);
        let c = colorize(&[(1.1 / n) * 10.0], &[255.0], &info(), &s, (0.0, 10.0)).unwrap();
        assert_ne!(&a[0..3], &c[0..3]);
    }

    #[test]
    fn transparency_rules() {
        let mut s = pseudo("viridis");
        s.transparent_ranges = vec![[4.0, 6.0]];
        s.opacity = 0.5;
        let px = colorize(&[-9999.0, 5.0, 8.0, 8.0], &[255.0, 255.0, 255.0, 0.0], &info(), &s, (0.0, 10.0)).unwrap();
        assert_eq!(px[3], 0, "nodata");
        assert_eq!(px[7], 0, "range");
        assert_eq!(px[11], 128, "opacity");
        assert_eq!(px[15], 0, "outside raster");
        s.nodata_transparent = false;
        let px = colorize(&[-9999.0], &[255.0], &info(), &s, (0.0, 10.0)).unwrap();
        assert_eq!(px[3], 128);
    }

    #[test]
    fn adjustments() {
        let mut s = pseudo("gray");
        s.brightness = 0.2;
        let a = colorize(&[0.0], &[255.0], &info(), &s, (0.0, 10.0)).unwrap();
        assert_eq!(a[0], 51);
        let mut s = pseudo("gray");
        s.saturation = -1.0;
        let mut s2 = pseudo("viridis");
        s2.saturation = -1.0;
        let a = colorize(&[3.0], &[255.0], &info(), &s2, (0.0, 10.0)).unwrap();
        assert!((a[0] as i32 - a[1] as i32).abs() <= 1 && (a[1] as i32 - a[2] as i32).abs() <= 1);
        s.gamma = 2.0;
        let g = colorize(&[2.5], &[255.0], &info(), &s, (0.0, 10.0)).unwrap();
        assert!(g[0] > 64, "gamma > 1 lightens mid-tones: {}", g[0]);
    }

    #[test]
    fn custom_stops_with_alpha_and_validation() {
        let mut s = pseudo("custom");
        s.stops = vec![Stop { pos: 0.0, color: "#ff0000".into(), alpha: 0.0 }, Stop { pos: 1.0, color: "#0000ff".into(), alpha: 1.0 }];
        let px = colorize(&[0.0, 10.0], &[255.0; 2], &info(), &s, (0.0, 10.0)).unwrap();
        assert_eq!(&px[0..4], &[255, 0, 0, 0]);
        assert_eq!(&px[4..8], &[0, 0, 255, 255]);
        s.stops.truncate(1);
        assert!(s.validate(&info()).unwrap_err().to_string().contains("two stops"));
        assert!(pseudo("nope").validate(&info()).unwrap_err().to_string().contains("Unknown colour ramp"));
        let mut s = pseudo("viridis");
        s.band = 3;
        assert!(s.validate(&info()).unwrap_err().to_string().contains("Band 3"));
    }

    fn stats_of(v: Vec<f32>) -> BandStats {
        let n = v.len() as f64;
        let mean = v.iter().map(|x| *x as f64).sum::<f64>() / n;
        let std = (v.iter().map(|x| (*x as f64 - mean).powi(2)).sum::<f64>() / n).sqrt();
        BandStats { min: v[0] as f64, max: v[v.len() - 1] as f64, mean, std, sorted: v }
    }

    #[test]
    fn classification_methods() {
        let uni = stats_of((0..=100).map(|i| i as f32).collect());
        assert_eq!(classify(&uni, ClassMethod::Equal, 4).unwrap(), vec![25.0, 50.0, 75.0]);
        assert_eq!(classify(&uni, ClassMethod::Quantile, 2).unwrap(), vec![50.0]);
        // Two tight clusters: Jenks cuts between them, equal interval does not have to.
        let mut v: Vec<f32> = (0..50).map(|i| i as f32 * 0.1).collect();
        v.extend((0..50).map(|i| 100.0 + i as f32 * 0.1));
        let two = stats_of(v);
        let j = classify(&two, ClassMethod::Jenks, 2).unwrap();
        assert_eq!(j.len(), 1);
        assert!(j[0] >= 4.9 && j[0] < 100.0, "{j:?}");
        let sd = classify(&uni, ClassMethod::StdDev, 4).unwrap();
        assert!(sd.windows(2).all(|w| w[0] < w[1]) && sd.iter().all(|b| *b > 0.0 && *b < 100.0));
        assert!(classify(&uni, ClassMethod::Equal, 1).is_err());
    }

    #[test]
    fn classed_rendering_uses_one_colour_per_class() {
        let mut s = pseudo("gray");
        s.mode = ColorMode::Discrete;
        s.breaks = vec![3.0, 7.0];
        let px = colorize(&[1.0, 2.9, 5.0, 9.0], &[255.0; 4], &info(), &s, (0.0, 10.0)).unwrap();
        assert_eq!(px[0], 0);
        assert_eq!(px[4], 0);
        assert!((px[8] as i32 - 128).abs() <= 1);
        assert_eq!(px[12], 255);
        s.breaks = vec![7.0, 3.0];
        assert!(s.validate(&info()).is_err());
    }

    #[test]
    fn colour_blind_safe_flags() {
        let r = ramps();
        assert!(r.iter().find(|x| x.name == "viridis").unwrap().cb_safe);
        assert!(!r.iter().find(|x| x.name == "RdYlGn").unwrap().cb_safe);
        assert!(CB_SAFE.iter().all(|n| r.iter().any(|x| x.name == *n)));
    }

    #[test]
    fn paletted_colours_listed_values_only() {
        let s = RasterStyle {
            renderer: Renderer::Paletted,
            entries: vec![
                PalEntry { value: 1.0, color: "#ff0000".into(), label: "A".into(), hidden: false },
                PalEntry { value: 2.0, color: "#00ff00".into(), label: "B".into(), hidden: true },
                PalEntry { value: 3.0, color: "#0000ff".into(), label: String::new(), hidden: false },
            ],
            ..Default::default()
        };
        s.validate(&info()).unwrap();
        let px = colorize(&[1.0, 2.0, 3.0, 4.0, -9999.0], &[255.0; 5], &info(), &s, (0.0, 1.0)).unwrap();
        assert_eq!(&px[0..4], &[255, 0, 0, 255]);
        assert_eq!(px[7], 0, "hidden class");
        assert_eq!(&px[8..12], &[0, 0, 255, 255]);
        assert_eq!(px[15], 0, "value without a class");
        assert_eq!(px[19], 0, "NoData");
        let empty = RasterStyle { renderer: Renderer::Paletted, ..Default::default() };
        assert!(empty.validate(&info()).unwrap_err().to_string().contains("at least one class"));
    }

    #[test]
    fn multiband_stretches_each_channel_and_validates_bands() {
        let mut i3 = info();
        i3.bands = 3;
        let s = RasterStyle { renderer: Renderer::Multiband, rgb_bands: [3, 2, 1], ..Default::default() };
        s.validate(&i3).unwrap();
        let (r, g, b) = ([0.0f32, 10.0], [10.0f32, 0.0], [5.0f32, 5.0]);
        let px = colorize_rgb([&r, &g, &b], &[255.0; 2], &i3, &s, [(0.0, 10.0); 3]).unwrap();
        assert_eq!(&px[0..4], &[0, 255, 128, 255]);
        assert_eq!(&px[4..8], &[255, 0, 128, 255]);
        // Different range per channel.
        let px = colorize_rgb([&r, &g, &b], &[255.0; 2], &i3, &s, [(0.0, 20.0), (0.0, 10.0), (0.0, 10.0)]).unwrap();
        assert_eq!(px[4], 128);
        let bad = RasterStyle { renderer: Renderer::Multiband, rgb_bands: [1, 2, 4], ..Default::default() };
        assert!(bad.validate(&i3).unwrap_err().to_string().contains("Band 4"));
        let bad = RasterStyle { renderer: Renderer::Multiband, channel_stretch: vec![Stretch::default()], ..Default::default() };
        assert!(bad.validate(&i3).is_err());
    }

    #[test]
    fn token_round_trip() {
        let s = RasterStyle { renderer: Renderer::Gray, invert: true, ..Default::default() };
        assert_eq!(RasterStyle::from_token(&s.to_token()).unwrap(), s);
        assert_eq!(RasterStyle::from_token("").unwrap(), RasterStyle::default());
        assert!(RasterStyle::from_token("!!!").is_err());
        // Old/partial JSON keeps defaults for the missing fields.
        let partial = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"renderer":"pseudocolor"}"#);
        let p = RasterStyle::from_token(&partial).unwrap();
        assert_eq!((p.renderer, p.gamma, p.opacity), (Renderer::Pseudocolor, 1.0, 1.0));
    }
}
