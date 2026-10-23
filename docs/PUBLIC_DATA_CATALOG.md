# Public data toolkit — dataset catalog

A Living Atlas–style collection of free, public datasets that GIS Studio can
pull for an area of interest (clipped, projected, cited automatically).
Status: ✅ integrated · ⬜ planned · Priority **P1** (next), **P2**, **P3**.
Last reviewed 2026-10-23. Availability of federal services changes — the
2025 shutdowns of HIFLD Open and EPA EJScreen are noted below — so each
connector must verify its endpoint and fail with a clear message.

## How to build it: connectors, then catalog entries

Most datasets below are served through a handful of protocols. Building one
connector per protocol turns each dataset into a small catalog entry (JSON:
endpoint, layer, fields, citation, license, default style) instead of code —
the way the Living Atlas scales.

| Connector | Protocol | Unlocks | Status |
|---|---|---|---|
| ArcGIS REST query | FeatureServer/MapServer `query` (Esri JSON → GeoJSON, paging, retries) | TIGERweb, The National Map, FEMA NFHL, PAD-US, NTAD, NIFC fires, USFWS, many state portals | ✅ built for TNM (`tnm_vector.rs`); generalize to a catalog |
| ArcGIS ImageServer export | `exportImage` | 3DEP, NAIP, NLCD-type rasters | ✅ for 3DEP (`tnm.rs`); generalize |
| STAC search + COG read | STAC API → Cloud-Optimized GeoTIFFs via GDAL `/vsicurl/` | Landsat, Sentinel-2, NAIP, Copernicus DEM, HLS, ESA WorldCover | ⬜ P1 |
| OGC API – Features / WFS | REST JSON | USGS Water Data, NOAA, state SDIs | ⬜ P1 |
| WMS / WMTS / XYZ | tiles for display | NASA GIBS, NOAA charts, basemaps | ✅ XYZ basemaps; ⬜ WMS/WMTS P2 |
| Cloud GeoParquet | DuckDB `read_parquet` over HTTPS/S3 with bbox filter | Overture Maps, Source Cooperative mirrors (incl. HIFLD archive), Microsoft buildings | ⬜ P1 (DuckDB already does this natively) |
| Tabular APIs | JSON/CSV joined to geography by GEOID/FIPS | Census ACS, CDC PLACES/SVI, NASA POWER, USDA NASS | ⬜ P1 (Census) |
| Token-authenticated | user-entered key, OS keychain | Census API, NASA Earthdata, NOAA CDO, USDA NASS, recreation.gov | ⬜ with keychain support |

Every connector reuses existing pieces: progress + cancel (`progress.rs`),
clip to area, download cache, and a citation entry (`sources.rs`).

## Catalog by theme

### Boundaries and administrative
| Dataset | Provider | Coverage | Access | License | Status |
|---|---|---|---|---|---|
| TIGER/Line boundaries (states → blocks, places, ZCTAs), by year 2010–current | U.S. Census Bureau | US | ArcGIS REST (TIGERweb) | Public domain | ✅ |
| National Boundary Dataset (federal lands, gov units) | USGS TNM | US | ArcGIS REST | Public domain | ✅ |
| PAD-US protected areas (ownership, GAP status, access) | USGS Gap Analysis Project | US | ArcGIS REST / GeoPackage | Public domain | ⬜ P1 |
| School districts | NCES EDGE | US | ArcGIS REST | Public domain | ⬜ P2 |
| BLM Surface Management Agency | BLM | US West | ArcGIS REST | Public domain | ⬜ P2 |
| Natural Earth admin & physical (small scale) | Natural Earth | Global | Download (GeoPackage) | Public domain | ⬜ P2 |
| geoBoundaries (ADM0–ADM5) | William & Mary geoLab | Global | REST API + GeoJSON | CC BY 4.0 | ⬜ P2 |

### Elevation, terrain and bathymetry
| Dataset | Provider | Coverage | Access | License | Status |
|---|---|---|---|---|---|
| 3DEP DEM (best available, 1 m–30 m) | USGS | US | ImageServer exportImage | Public domain | ✅ |
| 3DEP lidar point clouds (LAZ / EPT) | USGS | US | AWS open data (`usgs-lidar-public`), TNM Access API | Public domain | ⬜ P2 (needs point-cloud support) |
| Copernicus DEM GLO-30 | ESA / Copernicus | Global | COG on AWS open data / STAC | Copernicus licence (free, attribution) | ⬜ P1 |
| CUDEM / coastal DEMs and bathymetry | NOAA NCEI | US coasts | ArcGIS ImageServer / THREDDS | Public domain | ⬜ P2 |
| GEBCO global bathymetry | GEBCO / IHO–IOC | Global | Download (GeoTIFF/netCDF), WMS | Public domain (attribution requested) | ⬜ P3 |

### Imagery and remote sensing
| Dataset | Provider | Coverage | Access | License | Status |
|---|---|---|---|---|---|
| NAIP 0.3–1 m aerial imagery | USDA FPAC | US | USGS ImageServer; STAC (Planetary Computer) | Public domain | ⬜ P1 (planned) |
| Landsat Collection 2 Level-2 | USGS / NASA | Global, 1982– | STAC (USGS LandsatLook, Earth Search) + COG | Public domain | ⬜ P1 |
| Sentinel-2 L2A (10 m) | ESA / Copernicus | Global, 2015– | STAC (Earth Search) + COG | Copernicus licence | ⬜ P1 |
| Harmonized Landsat Sentinel-2 (HLS) | NASA LP DAAC | Global | STAC (CMR-STAC) + Earthdata login | Public domain | ⬜ P2 |
| GIBS daily global imagery (MODIS, VIIRS…) | NASA | Global | WMTS | Public domain (NASA) | ⬜ P1 (planned) |
| Black Marble night lights | NASA | Global | Earthdata / GIBS | Public domain | ⬜ P3 |
| Earthdata search (any NASA collection) | NASA CMR | Global | CMR / CMR-STAC + token | Varies by collection | ⬜ P2 (planned) |

### Land cover, vegetation and agriculture
| Dataset | Provider | Coverage | Access | License | Status |
|---|---|---|---|---|---|
| NLCD / Annual NLCD land cover, impervious, canopy | USGS / MRLC | US | WMS/WCS, COG downloads | Public domain | ⬜ P1 |
| ESA WorldCover 10 m | ESA | Global (2020, 2021) | COG on AWS / STAC | CC BY 4.0 | ⬜ P2 |
| Cropland Data Layer | USDA NASS | US, annual | CroplandCROS / WMS / downloads | Public domain | ⬜ P2 |
| LANDFIRE vegetation, fuels, disturbance | USGS / USFS | US | ImageServer / downloads | Public domain | ⬜ P2 |
| Global tree cover loss (Hansen) | UMD / GFW | Global | Tiles / COG | CC BY 4.0 | ⬜ P3 |
| USDA NASS Quick Stats (crop statistics) | USDA NASS | US counties | API + key | Public domain | ⬜ P3 |

### Water and hydrology
| Dataset | Provider | Coverage | Access | License | Status |
|---|---|---|---|---|---|
| NHD flowlines, waterbodies, areas, points | USGS | US | ArcGIS REST | Public domain | ✅ |
| Watershed Boundary Dataset (HUC 8/10/12) | USGS / NRCS | US | ArcGIS REST | Public domain | ✅ |
| NHDPlus HR (network attributes, catchments) | USGS | US | ArcGIS REST / downloads | Public domain | ⬜ P2 |
| Streamgages and water data (discharge, levels) | USGS Water Data | US | OGC API – Features (`api.waterdata.usgs.gov`) | Public domain | ⬜ P1 |
| National Flood Hazard Layer | FEMA | US | ArcGIS REST | Public domain | ⬜ P1 |
| National Wetlands Inventory | USFWS | US | ArcGIS REST / WMS | Public domain | ⬜ P1 |
| Water Quality Portal | USGS / EPA / NWQMC | US | REST (CSV/GeoJSON) | Public domain | ⬜ P3 |

### Geology, soils and hazards
| Dataset | Provider | Coverage | Access | License | Status |
|---|---|---|---|---|---|
| Earthquakes (ComCat) | USGS | Global | FDSN event API (GeoJSON) | Public domain | ⬜ P1 |
| State Geologic Map Compilation (SGMC) | USGS | US | Downloads / ArcGIS REST | Public domain | ⬜ P1 |
| Quaternary faults | USGS | US | ArcGIS REST | Public domain | ⬜ P2 |
| SSURGO / gSSURGO soils | USDA NRCS | US | Soil Data Access (SQL web service) | Public domain | ⬜ P1 |
| Mineral Resources Data System | USGS | Global | Downloads / WFS | Public domain | ⬜ P3 |
| Wildfire perimeters (WFIGS) | NIFC | US | ArcGIS REST | Public domain | ⬜ P2 |
| National Risk Index | FEMA | US | ArcGIS REST / CSV | Public domain | ⬜ P2 |
| Wildfire Risk to Communities | USFS | US | Downloads | Public domain | ⬜ P3 |
| Hurricane tracks (IBTrACS) | NOAA NCEI | Global | CSV / shapefile | Public domain | ⬜ P3 |

### Climate and weather
| Dataset | Provider | Coverage | Access | License | Status |
|---|---|---|---|---|---|
| POWER daily/monthly climate at a point or region | NASA | Global | REST API | Public domain | ⬜ P1 (planned) |
| Weather alerts, forecasts | NOAA NWS | US | `api.weather.gov` | Public domain | ⬜ P2 |
| Daymet daily surface weather (1 km) | ORNL DAAC | North America | THREDDS / single-pixel API | Public domain (cite) | ⬜ P2 |
| U.S. Drought Monitor | NDMC / USDA / NOAA | US weekly | GeoJSON / shapefile | Public domain (attribution) | ⬜ P2 |
| PRISM climate normals | Oregon State Univ. | US | Downloads | Free with terms (attribution) | ⬜ P3 |
| Climate Data Online station data | NOAA NCEI | Global | API + token | Public domain | ⬜ P3 |

### People, health and economy (tables joined to geography)
| Dataset | Provider | Coverage | Access | License | Status |
|---|---|---|---|---|---|
| ACS 5-year estimates / 2020 Census DHC | U.S. Census Bureau | US | Census Data API + user key | Public domain | ⬜ P1 (planned "Join Census Data") |
| Social Vulnerability Index | CDC / ATSDR | US tracts | ArcGIS REST / CSV | Public domain | ⬜ P2 |
| PLACES local health estimates | CDC | US tracts | Socrata API | Public domain | ⬜ P2 |
| Food Access Research Atlas | USDA ERS | US tracts | Downloads | Public domain | ⬜ P3 |
| EJScreen (environmental justice indicators) | EPA (offline since 2025) | US | Community mirrors (e.g. Public Environmental Data Partners) | Public domain data; check mirror terms | ⬜ P3 — use archived copies only |
| Global population (GHSL / WorldPop) | EC JRC / WorldPop | Global | COG downloads | CC BY 4.0 | ⬜ P3 |

### Infrastructure, transportation and places
| Dataset | Provider | Coverage | Access | License | Status |
|---|---|---|---|---|---|
| Roads, trails, rail, airports; structures; place names | USGS TNM, Census TIGER | US | ArcGIS REST | Public domain | ✅ |
| Overture Maps (buildings, places, transportation, addresses) | Overture Maps Foundation | Global, monthly | GeoParquet on S3/Azure (DuckDB) | ODbL / CDLA-Permissive (by theme) | ⬜ P1 |
| National Transportation Atlas Database | USDOT BTS | US | ArcGIS REST | Public domain | ⬜ P2 |
| Building footprints | Microsoft / FEMA USA Structures | US / global | GeoParquet / downloads | ODbL (Microsoft); public domain (FEMA) | ⬜ P2 |
| Energy infrastructure (plants, lines, pipelines) | EIA | US | ArcGIS REST | Public domain | ⬜ P2 |
| HIFLD Open layers (critical infrastructure) | DHS (Open portal shut down Aug 2025) | US | DataLumos archive / Source Cooperative mirrors; many layers via original agencies | Public domain (archived) | ⬜ P3 — archived snapshot; prefer agency sources |
| OpenStreetMap features | OSM contributors | Global | Overpass API / extracts | ODbL | ⬜ P2 |

### Ecology and biodiversity
| Dataset | Provider | Coverage | Access | License | Status |
|---|---|---|---|---|---|
| Species occurrences | GBIF | Global | Occurrence API | CC0 / CC BY (per record) | ⬜ P2 |
| Critical habitat | USFWS ECOS | US | ArcGIS REST | Public domain | ⬜ P2 |
| Ecoregions (Level III/IV) | EPA | US | Downloads / ArcGIS REST | Public domain | ⬜ P2 |
| GAP species ranges and habitat | USGS | US | Downloads | Public domain | ⬜ P3 |

### Oceans and coasts
| Dataset | Provider | Coverage | Access | License | Status |
|---|---|---|---|---|---|
| Nautical charts (ENC) | NOAA OCS | US waters | WMS / downloads | Public domain (not for navigation) | ⬜ P3 |
| Marine Cadastre (ocean planning layers) | BOEM / NOAA | US waters | ArcGIS REST | Public domain | ⬜ P3 |
| Sea surface temperature (OISST) | NOAA | Global | THREDDS / ERDDAP | Public domain | ⬜ P3 |

### Public lands and recreation
| Dataset | Provider | Coverage | Access | License | Status |
|---|---|---|---|---|---|
| National forest roads, trails, boundaries | USFS Enterprise Data Warehouse | US | ArcGIS REST | Public domain | ⬜ P2 |
| National Park boundaries | NPS | US | ArcGIS REST | Public domain | ⬜ P2 |
| Recreation facilities (RIDB) | recreation.gov | US | REST API + key | Public domain | ⬜ P3 |

## Recommended order
1. **Generic ArcGIS REST catalog** (reuse `tnm_vector.rs`) → PAD-US, FEMA NFHL, NWI, SGMC, faults, NIFC, NTAD, EIA come almost for free.
2. **Census Data API join** (already planned; keychain for the key).
3. **STAC + COG** → NAIP, Landsat, Sentinel-2, Copernicus DEM, NLCD/WorldCover.
4. **Cloud GeoParquet** → Overture Maps (buildings/places/roads anywhere in the world).
5. **USGS Water Data + earthquakes + NASA POWER** (simple JSON APIs, high value for geoscience).
6. **Soils (SSURGO via Soil Data Access)**.
7. WMS/WMTS display layers (NASA GIBS, NOAA charts).

## Sources consulted for availability (2026-10-23)
- HIFLD Open shutdown and archives: [NAPSG Foundation](https://www.napsgfoundation.org/hifld_open/), [Navigating the HIFLD transition](https://www.napsgfoundation.org/navigating-the-hifld-transition/), [Data Rescue Project](https://www.datarescueproject.org/hifld-data-saved/)
- EJScreen removal and mirrors: [EDGI](https://envirodatagov.org/?p=11456), [Harvard EELP tracker](https://eelp.law.harvard.edu/?p=8168), [Wikipedia: EJScreen](https://en.wikipedia.org/wiki/EJScreen)
- Overture Maps releases and access: [Overture docs — getting data](https://docs.overturemaps.org/getting-data), [2026-08-19 release notes](https://docs.overturemaps.org/blog/2026/08/19/release-notes/)
