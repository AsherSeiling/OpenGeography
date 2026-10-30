// Topo Map Builder presets (no imports, so browser tests can read them too).
/** The four ready-made sheets: scale, paper, units and the National Map data worth adding for each. */
export interface TopoPreset {
  id: string;
  label: string;
  scale: number;
  paper: "letter" | "tabloid" | "a4" | "a3";
  landscape: boolean;
  units: "imperial" | "metric";
  /** National Map dataset ids (`api.tnmDatasets()`). */
  data: string[];
}
const LAND_DATA = ["nhd_flowlines", "nhd_waterbodies", "roads_major", "roads_local", "roads_4wd", "trails", "gn_places", "gn_landforms", "gn_water"];
export const TOPO_PRESETS: TopoPreset[] = [
  { id: "land24", label: "Land Nav 1:24,000 Tabloid", scale: 24000, paper: "tabloid", landscape: true, units: "imperial", data: LAND_DATA },
  { id: "land25", label: "Land Nav 1:25,000 Letter", scale: 25000, paper: "letter", landscape: true, units: "metric", data: LAND_DATA },
  { id: "land50", label: "Land Nav 1:50,000 Tabloid", scale: 50000, paper: "tabloid", landscape: true, units: "metric", data: ["nhd_flowlines", "nhd_waterbodies", "roads_major", "roads_local", "trails", "gn_places", "gn_landforms"] },
  { id: "trail", label: "Trail map 1:25,000 Letter portrait", scale: 25000, paper: "letter", landscape: false, units: "imperial", data: ["trails", "roads_4wd", "roads_local", "nhd_flowlines", "nhd_waterbodies", "gn_landforms", "gn_water"] },
];
