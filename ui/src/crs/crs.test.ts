import { describe, expect, it } from "vitest";
import { loadCatalog, searchCatalog, statePlaneFor, utmFor } from "./crs";

describe("CRS catalog", () => {
  it("finds a code exactly", async () => {
    const list = await loadCatalog();
    expect(searchCatalog(list, "26914")[0].id).toBe("EPSG:26914");
    expect(searchCatalog(list, "epsg:4326")[0].name).toBe("WGS 84");
  });

  it("filters by location", async () => {
    const list = await loadCatalog();
    // Dallas, TX: Texas North Central State Plane should be offered; Oregon's should not.
    const res = searchCatalog(list, "texas north central", { at: [-96.8, 32.8] });
    expect(res.some((e) => e.id === "EPSG:2276")).toBe(true);
    expect(searchCatalog(list, "oregon", { at: [-96.8, 32.8] }).length).toBe(0);
  });

  it("lists State Plane zones for every state", async () => {
    const list = await loadCatalog();
    const ok = statePlaneFor(list, "Oklahoma").map((e) => e.id);
    expect(ok).toContain("EPSG:2267"); // NAD83 / Oklahoma North (ftUS)
    expect(ok).toContain("EPSG:6553"); // NAD83(2011) / Oklahoma North (ftUS)
    expect(statePlaneFor(list, "Hawaii").some((e) => e.name.startsWith("NAD83(PA11) / Hawaii zone"))).toBe(true);
    const states = ["Alabama", "Alaska", "Arizona", "Arkansas", "California", "Colorado", "Connecticut", "Delaware", "Florida", "Georgia", "Hawaii", "Idaho", "Illinois", "Indiana", "Iowa", "Kansas", "Kentucky", "Louisiana", "Maine", "Maryland", "Massachusetts", "Michigan", "Minnesota", "Mississippi", "Missouri", "Montana", "Nebraska", "Nevada", "New Hampshire", "New Jersey", "New Mexico", "New York", "North Carolina", "North Dakota", "Ohio", "Oklahoma", "Oregon", "Pennsylvania", "Rhode Island", "South Carolina", "South Dakota", "Tennessee", "Texas", "Utah", "Vermont", "Virginia", "Washington", "West Virginia", "Wisconsin", "Wyoming", "Puerto Rico"];
    for (const st of states) expect(statePlaneFor(list, st).length, st).toBeGreaterThan(0);
  });

  it("picks UTM zones", () => {
    expect(utmFor(-97.5, 35.2)).toBe("EPSG:32614");
    expect(utmFor(-97.5, 35.2, "NAD83")).toBe("EPSG:26914");
    expect(utmFor(151.2, -33.9)).toBe("EPSG:32756");
  });
});
