import { describe, expect, it } from "vitest";
import { filterGrids, sortGrids } from "./DatumGridsDialog";
import type { DatumGrid } from "../ipc/api";

const g = (o: Partial<DatumGrid>): DatumGrid => ({ name: "x.tif", country: "Canada", continent: "North America", source: "s", sourceId: "s", kind: "horizontal", area: "", from: "NAD27", to: "NAD83", description: "NAD27 to NAD83", size: 1000, sha256: "", url: "", bundled: false, installed: false, location: "", ...o });

describe("datum grid list", () => {
  it("sorts by region, country, purpose, then name", () => {
    const rows = sortGrids([
      g({ name: "d.tif", continent: "Oceania", country: "Australia" }),
      g({ name: "c.tif", continent: "Europe", country: "Germany", kind: "vertical" }),
      g({ name: "b.tif", continent: "Europe", country: "Germany", kind: "horizontal" }),
      g({ name: "a.tif", continent: "North America", country: "Canada" }),
      g({ name: "z.tif", continent: "Mystery", country: "Nowhere" }),
    ]);
    expect(rows.map((r) => r.name)).toEqual(["a.tif", "b.tif", "c.tif", "d.tif", "z.tif"]);
  });

  it("filters by text, region, purpose and status", () => {
    const all = [
      g({ name: "ca_ntv2.tif", installed: true, bundled: true, location: "app" }),
      g({ name: "de_beta.tif", continent: "Europe", country: "Germany", from: "DHDN", to: "ETRS89", description: "DHDN to ETRS89", installed: false }),
      g({ name: "au_geoid.tif", continent: "Oceania", country: "Australia", kind: "vertical", installed: true, location: "downloaded" }),
    ];
    const q = { text: "", continent: "all", kind: "all", show: "all" };
    expect(filterGrids(all, q)).toHaveLength(3);
    expect(filterGrids(all, { ...q, text: "dhdn" }).map((r) => r.name)).toEqual(["de_beta.tif"]);
    expect(filterGrids(all, { ...q, continent: "Oceania" }).map((r) => r.name)).toEqual(["au_geoid.tif"]);
    expect(filterGrids(all, { ...q, kind: "vertical" }).map((r) => r.name)).toEqual(["au_geoid.tif"]);
    expect(filterGrids(all, { ...q, show: "missing" }).map((r) => r.name)).toEqual(["de_beta.tif"]);
    expect(filterGrids(all, { ...q, show: "shipped" }).map((r) => r.name)).toEqual(["ca_ntv2.tif"]);
    expect(filterGrids(all, { ...q, show: "installed" })).toHaveLength(2);
  });
});
