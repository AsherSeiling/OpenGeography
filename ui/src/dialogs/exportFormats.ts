// The formats of Export Features (UR-14, UR-15) and what each one can hold, in one place for the dialog and the save box.

export interface ExportFormat {
  /** The engine's `format` parameter. */
  id: string;
  label: string;
  /** File extension without the dot. */
  ext: string;
  /** What the format cannot do, shown under the choice. */
  note?: string;
  /** Holds attributes without shapes only (a table file). */
  tableOnly?: boolean;
  /** Shapes are not written at all (table formats), or as text. */
  noShapes?: boolean;
}

export const VECTOR_FORMATS: ExportFormat[] = [
  { id: "GPKG", label: "GeoPackage (.gpkg)", ext: "gpkg" },
  { id: "ESRI Shapefile", label: "Shapefile (.shp)", ext: "shp", note: "Field names are cut to 10 characters and text to 254; the export lists what changed." },
  { id: "GeoJSON", label: "GeoJSON (.geojson)", ext: "geojson", note: "Standard GeoJSON is always WGS 84 longitude/latitude." },
  { id: "FlatGeobuf", label: "FlatGeobuf (.fgb)", ext: "fgb" },
  { id: "PARQUET", label: "GeoParquet (.parquet)", ext: "parquet" },
  { id: "CSV", label: "CSV with the shape as WKT text (.csv)", ext: "csv" },
  { id: "KML", label: "Google Earth KML (.kml)", ext: "kml", note: "Always WGS 84 longitude/latitude." },
  { id: "GPX", label: "GPX for GPS devices (.gpx)", ext: "gpx", note: "Points (waypoints) and lines (routes) only; always WGS 84. Columns called name, desc, ele, time … become GPX's own elements; the rest are extensions." },
  { id: "DXF", label: "AutoCAD DXF (.dxf)", ext: "dxf", note: "Shapes only: DXF has no attribute table." },
];

export const TABLE_FORMATS: ExportFormat[] = [
  { id: "CSV", label: "CSV (.csv)", ext: "csv", tableOnly: true },
  { id: "TSV", label: "Tab-separated text (.tsv)", ext: "tsv", tableOnly: true },
  { id: "XLSX", label: "Excel workbook (.xlsx)", ext: "xlsx", tableOnly: true },
  { id: "ODS", label: "OpenDocument spreadsheet (.ods)", ext: "ods", tableOnly: true },
  { id: "PARQUET", label: "Parquet (.parquet)", ext: "parquet", tableOnly: true },
  { id: "JSON", label: "JSON records (.json)", ext: "json", tableOnly: true },
  { id: "DBF", label: "dBASE table (.dbf)", ext: "dbf", tableOnly: true },
  { id: "GPKG", label: "GeoPackage (.gpkg)", ext: "gpkg", tableOnly: true },
];

/** The formats on offer for a layer of this geometry type ("Table" = no shapes). */
export function formatsFor(geomType: string): ExportFormat[] {
  return geomType === "Table" ? TABLE_FORMATS : VECTOR_FORMATS;
}

/** Why a format can't be used for this layer, or null. */
export function refusal(format: string, geomType: string): string | null {
  if (format === "GPX" && geomType === "Polygon") return "GPX holds points and lines only; this layer has polygons. Choose KML or GeoPackage, or turn the polygons into lines first.";
  if (format === "GPX" && geomType === "Mixed") return "GPX holds points and lines only; this layer has several kinds of shapes.";
  return null;
}

export const formatById = (geomType: string, id: string): ExportFormat => formatsFor(geomType).find((f) => f.id === id) ?? formatsFor(geomType)[0];

/** What the dialog passes to the export node besides path and format. Empty choices are left out so the node keeps its defaults. */
export interface ExportChoices {
  crs?: string;
  /** null = every field. */
  fields?: string[] | null;
  /** west,south,east,north in degrees. */
  clipExtent?: string;
  clipLayer?: string;
}

export function exportParams(c: ExportChoices): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  if (c.crs) out.crs = c.crs;
  if (c.fields) out.fields = c.fields;
  if (c.clipExtent) out.clip_extent = c.clipExtent;
  if (c.clipLayer) out.clip_layer = c.clipLayer;
  return out;
}
