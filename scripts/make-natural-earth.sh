#!/usr/bin/env bash
# Rebuilds crates/engine/assets/natural-earth/natural_earth_110m.tif (the bundled Natural Earth basemap, public domain)
# from the 1:110m Natural Earth land, lakes and admin-0 country boundaries. Needs GDAL command-line tools.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
W="$(mktemp -d)"; trap 'rm -rf "$W"' EXIT
cd "$W"
for f in physical/ne_110m_land physical/ne_110m_lakes cultural/ne_110m_admin_0_countries; do
  curl -fsSLO "https://naciscdn.org/naturalearth/110m/$f.zip"; unzip -oq "$(basename "$f").zip" -d x
done
ogr2ogr -f GPKG b.gpkg x/ne_110m_admin_0_countries.shp -dialect SQLite -sql "SELECT ST_Boundary(geometry) AS geometry FROM ne_110m_admin_0_countries" -nln b
OUT="$ROOT/crates/engine/assets/natural-earth/natural_earth_110m.tif"
rm -f "$OUT"
gdal_create -q -of GTiff -outsize 4096 2048 -bands 3 -ot Byte -a_srs EPSG:4326 -a_ullr -180 90 180 -90 -burn 170 -burn 211 -burn 232 -co COMPRESS=DEFLATE -co PREDICTOR=2 "$OUT"
B="-b 1 -b 2 -b 3"
gdal_rasterize -q $B -burn 238 -burn 235 -burn 224 x/ne_110m_land.shp "$OUT"
gdal_rasterize -q $B -burn 170 -burn 211 -burn 232 x/ne_110m_lakes.shp "$OUT"
gdal_rasterize -q $B -burn 160 -burn 150 -burn 140 -l b b.gpkg "$OUT"
ls -l "$OUT"
