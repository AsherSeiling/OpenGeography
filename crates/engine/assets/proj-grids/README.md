# Bundled PROJ datum grids

NOAA NADCON grids for NAD27 ↔ NAD83 in the United States (converted to GeoTIFF by the PROJ-data project, https://cdn.proj.org).
Source data: U.S. National Geodetic Survey, a U.S. Government work in the public domain.
They are copied next to `proj.db` by `scripts/stage-resources.sh`, so the app shifts NAD27 data correctly without a network.

| File | Area |
|---|---|
| us_noaa_conus.tif | contiguous United States |
| us_noaa_alaska.tif | Alaska |
| us_noaa_hawaii.tif | Hawaii |
| us_noaa_prvi.tif | Puerto Rico and U.S. Virgin Islands |
| us_noaa_stgeorge.tif, us_noaa_stlrnc.tif, us_noaa_stpaul.tif | Bering Sea islands |

Canada (NTv2) and other countries' grids are not bundled; the start-up datum check reports when a shift is not available.
