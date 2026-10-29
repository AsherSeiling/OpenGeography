# Large-area elevation download (phase D0, bug D-1)

**Design (2026-10-28).** One cell grid for the whole job; the grid is cut into requests of at most the service's image size
(read from `ImageServer?f=json`, never above 4000 × 4000 cells = 64 MB). Tiles are fetched one at a time, retried three times
with doubling back-off, kept in `store_dir()/dem_jobs/<hash>/` so a failed job resumes, then written into one tiled, DEFLATE-compressed
GeoTIFF without holding more than one tile in memory, imported (overviews) and cited once.

**Output CRS (DEM-5).** A box at most 6° wide and between 84° S and 84° N: the local UTM zone, metre cells snapped to the cell size.
Anything wider, or beyond 84°: EPSG:4326 with square degree cells (the metre size is taken along a meridian at the mid-latitude);
the plan says so. Why not the zone of the centre for wide boxes: the far edges would be 6° or more from the central meridian and the
cells no longer true.

**Size policy.** At most 10⁹ cells (4 GB of F32). Above that the cell size grows 5 % at a time until it fits; the dialog and result name
the cell size used. Beyond 1000 m cells the download is refused (a global model is the better source). More than 500 MB needs the
user's confirmation (enforced in the engine, checkbox in the dialog) before any request.

**Why not the National Map staged COG products.** They need a product search per 1°/10 m tile and a different licence note per dataset;
the ImageServer already serves every resolution through one URL and resamples consistently. Revisit if the service limits tighten.

**Not done / needs the real service.** DEM-0 (reproduction ladder with timings) and DEM-11 (Mt Whitney) are network cases
(`downloads::dem_11…`, ignored). The service's real tile time is unknown; the 180 s per-request timeout and 4000-cell tile
are the conservative defaults.
