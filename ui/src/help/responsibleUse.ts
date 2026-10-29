// The limitations and responsible-use statement shown in View ▸ Help and from the Coordinate System tab. docs/RESPONSIBLE_USE.md carries the
// same text (a test keeps them in step). CRS accuracy plan D3; wording approved by the user 2026-10-29.
export const RESPONSIBLE_USE_TITLE = "Limitations and responsible use";

export const RESPONSIBLE_USE: { heading: string; text: string }[] = [
  {
    heading: "Coordinate conversions",
    text: "GIS Studio converts coordinates between coordinate systems and datums using the PROJ library and the datum grids installed on this computer. Where a precise grid is installed, shifts are accurate to a few centimetres to about 2 metres, depending on the datum pair. Where it is not, the program says so (read the \"Datum shift\" line on a layer's Coordinate System tab and the note on a reprojected result) and uses a rougher method that can be off by metres, and for some older datums by tens of metres. Get more grids from Map ▸ Datum Grids. Turn on Strict mode to stop work that would rely on a rough shift.",
  },
  {
    heading: "Heights",
    text: "Elevations have a vertical datum (for example NAVD88 or the EGM2008 geoid). Mixing datums changes heights by up to metres. USGS 3DEP downloads are recorded as NAVD88 and Copernicus DEM as EGM2008; use Define Vertical Datum for other rasters, and read the warning before combining rasters on different datums. The program does not convert between height references.",
  },
  {
    heading: "Distances and areas",
    text: "Lengths and areas measured in a map projection are distorted. The program measures on the ellipsoid by default and warns when a tool uses map units in a projection that stretches them over your data. Work in a local projected coordinate system (a UTM zone or State Plane) for analysis, and note that the two kinds of foot (international and US survey) differ by 2 parts per million.",
  },
  {
    heading: "Not a survey instrument",
    text: "GIS Studio is not a survey instrument. Do not use its output as the legal record of a boundary, for engineering design, or for emergency decisions unless a qualified person has verified it against the source data and the official transformation for your jurisdiction. Statistical and interpolated results (hot spots, Moran's I, kriging, classification) are estimates; read their uncertainty before acting on them.",
  },
  {
    heading: "Sensitive locations",
    text: "Coordinates you export can reveal sensitive locations. Remove, round or aggregate them before sharing when people, protected species or cultural sites could be harmed.",
  },
  {
    heading: "Data and licences",
    text: "Downloaded data keeps the terms of its provider; the Sources pane records where each layer came from. Some data (for example OpenStreetMap-based maps and some Overture themes) needs attribution or must be shared under the same licence. Open-source software and data used by GIS Studio are listed in docs/OPEN_SOURCE.md and THIRD_PARTY_NOTICES.md.",
  },
];
