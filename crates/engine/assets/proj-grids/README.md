# Bundled PROJ datum grids

NOAA NADCON grids for NAD27 ↔ NAD83 and the state High Accuracy Reference Network (HPGN/HARN) grids for NAD83 ↔ WGS 84 in the United States (converted to GeoTIFF by the PROJ-data project, https://cdn.proj.org).
Source data: U.S. National Geodetic Survey, a U.S. Government work in the public domain.
They are copied next to `proj.db` by `scripts/stage-resources.sh`, so the app shifts NAD27 data correctly without a network.

| File | Area |
|---|---|
| us_noaa_conus.tif | contiguous United States |
| us_noaa_alaska.tif | Alaska |
| us_noaa_hawaii.tif | Hawaii |
| us_noaa_prvi.tif | Puerto Rico and U.S. Virgin Islands |
| us_noaa_stgeorge.tif, us_noaa_stlrnc.tif, us_noaa_stpaul.tif | Bering Sea islands |
| us_noaa_??hpgn.tif (51 files, 444 KB) | NAD83(HARN) ↔ NAD83 by state; PROJ's best WGS 84 ↔ NAD83 operation inside a state uses its file (for example Oklahoma: us_noaa_okhpgn.tif). Added 2026-10-29: CI found the datum reference tests need them. |

Canada (NTv2) and other countries' grids are not bundled; the start-up datum check reports when a shift is not available.
