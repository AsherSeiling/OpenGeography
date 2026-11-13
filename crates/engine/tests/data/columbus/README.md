Columbus, Ohio crime data: 49 neighbourhoods (CRIME, INC, HOVAL and others) with polygon boundaries.

Source: Anselin, L. (1988). Spatial Econometrics: Methods and Models. Kluwer, Table 12.1. Distributed "as is" by the GeoDa Center
(https://geodacenter.github.io/data-and-lab/columbus/), which publishes it for teaching and as a sample data set; it is also the data set of
R's spdep package (`columbus`). `columbus.geojson` and `columbus.csv` are the files from that page, unchanged. The polygon coordinates are in
the data set's own arbitrary units (no CRS is defined; GeoJSON readers treat them as degrees, which only matters for distances: the tests use
contiguity, which is topological).

Used by `tests/stat_columbus.rs` to check Moran's I, OLS and the spatial lag and error models against the values published for this data set
in the GeoDa and spdep documentation.
