import { describe, expect, it } from "vitest";
import type { SourceEntry } from "../ipc/api";
import { bibliographyFiles, longDate, mlaDate, referenceText, toBibtex, toHtml, toMarkdown, toRis, usDate } from "./cite";

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

const census: SourceEntry = {
  citation: { ...nhd.citation, key: "census-2020-pl", author: "U.S. Census Bureau", year: "2021", title: "2020 Census Redistricting Data (Public Law 94-171) Summary File", version: "", publisher: "U.S. Census Bureau", url: "https://www.census.gov/data.html", annotation: "Population counts." },
  firstUsed: "2026-10-23",
  uses: [{ layerId: "L2", layerName: "Tracts", at: "2026-10-23" }],
};

const file: SourceEntry = {
  citation: { key: "file:/data/roads_2024.shp", author: "", year: "", title: "roads_2024.shp", version: "", publisher: "", url: "", kind: "Local file", license: "", annotation: "SHP file.", complete: false },
  firstUsed: "2026-10-21",
  uses: [{ layerId: "L3", layerName: "roads_2024", at: "2026-10-21" }],
};

describe("dates", () => {
  it("formats access dates per style", () => {
    expect(usDate("2026-10-22")).toBe("October 22, 2026");
    expect(mlaDate("2026-09-05")).toBe("5 Sept. 2026");
    expect(longDate("2026-10-22")).toBe("22 October 2026");
  });
});

describe("APA 7", () => {
  it("formats a data set with retrieval date", () => {
    expect(referenceText(nhd.citation, nhd.firstUsed, "apa")).toBe(
      "U.S. Geological Survey. (n.d.). National Hydrography Dataset (NHD) (nhd MapServer) [Data set]. U.S. Geological Survey, The National Map. Retrieved October 22, 2026, from https://hydro.nationalmap.gov/arcgis/rest/services/nhd/MapServer",
    );
  });
  it("omits the publisher when it is the author", () => {
    expect(referenceText(census.citation, census.firstUsed, "apa")).toBe(
      "U.S. Census Bureau. (2021). 2020 Census Redistricting Data (Public Law 94-171) Summary File [Data set]. Retrieved October 23, 2026, from https://www.census.gov/data.html",
    );
  });
  it("moves the title first when there is no author", () => {
    expect(referenceText(file.citation, file.firstUsed, "apa")).toBe("roads_2024.shp [Unpublished data set]. (n.d.).");
  });
});

describe("MLA 9", () => {
  it("skips an organization that is also the publisher and drops https://", () => {
    expect(referenceText(census.citation, census.firstUsed, "mla")).toBe(
      "2020 Census Redistricting Data (Public Law 94-171) Summary File. U.S. Census Bureau, 2021, www.census.gov/data.html. Accessed 23 Oct. 2026.",
    );
    expect(referenceText(nhd.citation, nhd.firstUsed, "mla")).toBe(
      "U.S. Geological Survey. National Hydrography Dataset (NHD). nhd MapServer, U.S. Geological Survey, The National Map, hydro.nationalmap.gov/arcgis/rest/services/nhd/MapServer. Accessed 22 Oct. 2026.",
    );
  });
  it("marks unpublished local data", () => {
    expect(referenceText(file.citation, file.firstUsed, "mla")).toBe("roads_2024.shp. Unpublished data set, used 21 Oct. 2026.");
  });
});

describe("Chicago 17 (author-date)", () => {
  it("puts the year after the author and the access date before the URL", () => {
    expect(referenceText(nhd.citation, nhd.firstUsed, "chicago")).toBe(
      "U.S. Geological Survey. n.d. National Hydrography Dataset (NHD). nhd MapServer. U.S. Geological Survey, The National Map. Accessed October 22, 2026. https://hydro.nationalmap.gov/arcgis/rest/services/nhd/MapServer.",
    );
    expect(referenceText(file.citation, file.firstUsed, "chicago")).toBe("roads_2024.shp. n.d. Accessed October 21, 2026.");
  });
});

describe("annotated bibliographies and exports", () => {
  it("Markdown: italic titles, one annotation each, sorted, incomplete last", () => {
    const md = toMarkdown([file, nhd, census], "Test project", "apa");
    expect(md).toContain("*National Hydrography Dataset (NHD)*");
    expect(md).toContain("> Surface water. Used for: Streams — OKC. License: Public domain.");
    const order = ["U.S. Census Bureau", "U.S. Geological Survey", "roads_2024.shp"].map((s) => md.indexOf(s));
    expect(order).toEqual([...order].sort((a, b) => a - b));
    expect(toMarkdown([nhd], "P", "mla")).toContain("MLA 9th edition");
  });

  it("HTML: hanging indents, italics, escaped text", () => {
    const html = toHtml([nhd, { ...file, citation: { ...file.citation, title: "a<b>.shp" } }], "P & Q", "chicago");
    expect(html).toContain("<i>National Hydrography Dataset (NHD)</i>");
    expect(html).toContain("text-indent: -0.5in");
    expect(html).toContain("a&lt;b&gt;.shp");
    expect(html).toContain("P &amp; Q");
  });

  it("BibTeX: escaped fields and unique keys", () => {
    const bib = toBibtex([nhd, file, { ...file, citation: { ...file.citation, key: "file:/other/roads_2024.shp" } }]);
    expect(bib).toContain("@misc{usgs-nhd,");
    expect(bib).toContain("author = {{U.S. Geological Survey}}");
    expect(bib).toContain("urldate = {2026-10-22}");
    expect(bib).toContain("@misc{file-roads-2024-shp,");
    expect(bib).toContain("@misc{file-roads-2024-shp-2,");
    expect(bib).toContain("title = {roads\\_2024.shp}");
  });

  it("RIS: data records with access dates", () => {
    const ris = toRis([census, file]);
    expect(ris).toContain("TY  - DATA\r\nAU  - U.S. Census Bureau");
    expect(ris).toContain("PY  - 2021");
    expect(ris).toContain("Y2  - 2026/10/23");
    expect(ris.match(/ER {2}- /g)).toHaveLength(2);
    expect(ris).not.toContain("AU  - \r\n");
  });

  it("writes every format for auto-export", () => {
    const files = bibliographyFiles([nhd, file], "P");
    expect(Object.keys(files).sort()).toEqual([
      "Data sources (APA).html",
      "Data sources (APA).md",
      "Data sources (Chicago).html",
      "Data sources (Chicago).md",
      "Data sources (MLA).html",
      "Data sources (MLA).md",
      "Data sources.bib",
      "Data sources.ris",
    ]);
  });
});
