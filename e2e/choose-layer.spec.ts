// Node cards: "Choose a layer" on an empty second input builds what dragging a layer in builds, as one undo step.
import { test, expect, gis, addTestLayer, addNode, frameAll, modelState, hoverCanvas } from "./fixtures";

test("NC-1 Choose a layer wires a Layer card into an empty input; undo removes both", async ({ app }) => {
  const a = await addTestLayer(app, "polygons", 10);
  const b = await addTestLayer(app, "polygons", 10);
  await gis(app, (g, id) => g.useApp.getState().setActiveLayer(id), a.id);
  const clip = await addNode(app, "clip", "clip");
  await frameAll(app);
  const card = app.locator(`.react-flow__node[data-id="${clip}"]`);
  await expect(card.locator(".bnode-row.in")).toHaveCount(2);
  // Only inputs after the first offer it.
  await expect(card.locator(".bchoose")).toHaveCount(1);
  await card.locator(".bchoose").click();
  await app.getByRole("menuitem").filter({ hasText: /polygons/ }).first().click();
  const m = await modelState(app);
  const layerNode = m.nodes.filter((n) => n.tool === "input_layer" && !n.id.startsWith("in-"))[0];
  expect(layerNode).toBeTruthy();
  expect(m.edges.some(([s, t, h]) => s === layerNode.id && t === clip && h !== "in")).toBe(true);
  await expect(card.locator(".bchoose")).toHaveCount(0);
  for (const theme of ["light", "dark"]) {
    await gis(app, (g, t) => g.useApp.setState({ dark: t === "dark" }), theme);
    await app.waitForTimeout(300);
    await hoverCanvas(app);
    await app.screenshot({ path: `${process.env.E2E_SHOTS ?? "test-results"}/nodecards-${theme}.png` });
  }
  await hoverCanvas(app);
  await app.keyboard.press("ControlOrMeta+z");
  const after = await modelState(app);
  expect(after.nodes.some((n) => n.id === layerNode.id)).toBe(false);
  expect(after.edges.some(([, t, h]) => t === clip && h !== "in")).toBe(false);
  void b;
});
