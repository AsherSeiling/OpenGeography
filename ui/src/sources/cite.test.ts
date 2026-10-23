import { describe, expect, it } from "vitest";
import type { SourceEntry } from "../ipc/api";
import { longDate, referenceText, toBibtex, toMarkdown } from "./cite";

const nhd: SourceEntry = {
  citation: {
    key: "usgs-nhd",
    author: "U.S. Geological Survey",
    year: "n.d.",
    title: "National Hydrography Dataset (NHD)",
    version: "nhd MapServer",
    publisher: "U.S. Geological Survey, The National Map",
    url: "https://hydro.nationalmap.gov/arcgis/rest/services/nhd/MapServer",
    kind: "Web service",
    license: "Public domain.",
    annotation: "Surface water.",
    complete: true,
  },
  firstUsed: "2026-10-22",
  uses: [
    { layerId: "L1", layerName: "Streams — OKC", at: "2026-10-22" },
    { layerId: "L4", layerName: "Streams — OKC", at: "2026-10-23" },
  ],
};

const file: SourceEntry = {
  citation: { key: "file:/data/roads_2024.shp", author: "", year: "", title: "roads_2024.shp", version: "", publisher: "", url: "", kind: "Local file", license: "", annotation: "SHP file.", complete: false },
  firstUsed: "2026-10-21",
  uses: [{ layerId: "L2", layerName: "roads_2024", at: "2026-10-21" }],
};

describe("citations", () => {
  it("formats an APA-style reference with the access date", () => {
    expect(longDate("2026-10-22")).toBe("22 October 2026");
    expect(referenceText(nhd.citation, nhd.firstUsed)).toBe(
      "U.S. Geological Survey. (n.d.). National Hydrography Dataset (NHD) [nhd MapServer]. U.S. Geological Survey, The National Map. Retrieved 22 October 2026, from https://hydro.nationalmap.gov/arcgis/rest/services/nhd/MapServer",
    );
  });

  it("marks what a local file citation is missing", () => {
    expect(referenceText(file.citation, file.firstUsed)).toBe("[Author unknown]. (n.d.). roads_2024.shp. Local file, used 21 October 2026.");
  });

  it("writes an annotated bibliography with each layer listed once", () => {
    const md = toMarkdown([nhd, file], "Test project");
    expect(md).toContain("*National Hydrography Dataset (NHD)*");
    expect(md).toContain("> Surface water. Used for: Streams — OKC. License: Public domain.");
    expect(md.indexOf("U.S. Geological Survey")).toBeLessThan(md.indexOf("[Author unknown]"));
  });

  it("writes BibTeX with escaped fields and unique keys", () => {
    const bib = toBibtex([nhd, file, { ...file, citation: { ...file.citation, key: "file:/other/roads_2024.shp" } }]);
    expect(bib).toContain("@misc{usgs-nhd,");
    expect(bib).toContain("author = {{U.S. Geological Survey}}");
    expect(bib).toContain("urldate = {2026-10-22}");
    expect(bib).toContain("@misc{file-roads-2024-shp,");
    expect(bib).toContain("@misc{file-roads-2024-shp-2,");
    expect(bib).toContain("title = {roads\\_2024.shp}");
  });
});
