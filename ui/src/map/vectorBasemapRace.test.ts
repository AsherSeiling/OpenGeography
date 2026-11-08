import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "../ipc/api";
import { MercatorRenderer, vectorPictureFor } from "./vectorBasemap";

const extent: [number, number, number, number] = [0, 0, 1000, 1000];
const merc: [number, number, number, number] = [0, 0, 100, 100];

afterEach(() => vi.restoreAllMocks());

describe("vector basemap pictures that are no longer wanted", () => {
  it("stop at once when already stale, and after the extent answer, so nothing is rendered or warped", async () => {
    const extentCall = vi.spyOn(api, "basemapMercatorExtent").mockResolvedValue(merc);
    const warp = vi.spyOn(api, "warpBasemapPicture").mockResolvedValue("");
    const render = vi.fn();
    expect(await vectorPictureFor({ render }, "EPSG:3857", extent, 1, 1, () => false)).toBeNull();
    expect(extentCall).not.toHaveBeenCalled();
    let current = true;
    extentCall.mockImplementation(async () => { current = false; return merc; });
    expect(await vectorPictureFor({ render }, "EPSG:3857", extent, 1, 1, () => current)).toBeNull();
    expect(render).not.toHaveBeenCalled();
    expect(warp).not.toHaveBeenCalled();
  });

  it("drop a picture that became stale while the renderer drew it, without warping it", async () => {
    vi.spyOn(api, "basemapMercatorExtent").mockResolvedValue(merc);
    const warp = vi.spyOn(api, "warpBasemapPicture").mockResolvedValue("");
    let current = true;
    const render = vi.fn(async () => { current = false; return ({} as HTMLCanvasElement); });
    expect(await vectorPictureFor({ render }, "EPSG:3857", extent, 1, 1, () => current)).toBeNull();
    expect(warp).not.toHaveBeenCalled();
  });

  it("a renderer disposed before or during a render resolves to nothing instead of throwing", async () => {
    // No DOM in this test environment: a renderer with a stand-in map that records what it is asked to do.
    const calls: string[] = [];
    const view = { setCenter: () => {}, setResolution: () => {} };
    const map = {
      getView: () => view,
      updateSize: () => {},
      once: () => {},
      renderSync: () => calls.push("renderSync"),
      setTarget: () => calls.push("setTarget"),
      getLayers: () => ({ clear: () => {} }),
    };
    const r = Object.assign(Object.create(MercatorRenderer.prototype), { map, host: { style: {}, remove: () => {} }, gone: false, disposers: [], background: null }) as MercatorRenderer;
    const inFlight = r.render(merc, 64, 64);
    expect(calls).toEqual(["renderSync"]);
    r.dispose(); // the basemap changed while the picture was being drawn
    expect(await inFlight).toBeNull();
    expect(await r.render(merc, 64, 64)).toBeNull();
    expect(calls).toEqual(["renderSync", "setTarget"]); // nothing drawn after disposal
    r.dispose(); // twice is harmless
    expect(r.disposed).toBe(true);
  });
});
