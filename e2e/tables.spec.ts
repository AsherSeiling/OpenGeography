import { test, expect, gis, ribbon } from "./fixtures";

const layers = () => (page: any) => gis(page, (g) => Object.values(g.useApp.getState().layers).map((l: any) => ({ id: l.id, name: l.name, type: l.geomType, n: l.featureCount, fields: l.fields.map((f: any) => `${f.name}:${f.type}`) }))) as Promise<any[]>;

test("UR-14 New Table makes a typed empty table; rows are added, edited, deleted and put back in the Attribute Table", async ({ app }) => {
  await app.locator(".rtab", { hasText: /^Map$/ }).click();
  await ribbon(app, "Map", /^New Table$/);
  const dlg = app.getByRole("dialog");
  await expect(dlg).toContainText("New Table");
  // The button says what is wrong before the engine does.
  await dlg.getByLabel("Table name").fill("People");
  await dlg.getByLabel("Field 1 name").fill("geom");
  await expect(dlg).toContainText("reserved");
  await expect(dlg.getByRole("button", { name: "Create table" })).toBeDisabled();
  await dlg.getByLabel("Field 1 name").fill("name");
  await dlg.getByRole("button", { name: "Add field" }).click();
  await dlg.getByLabel("Field 2 name").fill("age");
  await dlg.getByLabel("Field 2 type").click();
  await app.getByRole("option", { name: "Whole number" }).click();
  await dlg.getByRole("button", { name: "Create table" }).click();
  await expect(app.getByRole("dialog")).toHaveCount(0);

  const t = (await layers()(app)).find((l) => l.name === "People");
  expect(t).toMatchObject({ type: "Table", n: 0 });
  expect(t.fields).toEqual(["name:VARCHAR", "age:BIGINT"]);

  // Add two rows with the Row button, fill one in.
  const row = app.getByRole("button", { name: /Add a blank row/ });
  await row.click();
  await expect.poll(async () => (await layers()(app)).find((l) => l.name === "People").n).toBe(1);
  await row.click();
  await expect.poll(async () => (await layers()(app)).find((l) => l.name === "People").n).toBe(2);
  await gis(app, (g, id) => g.api.updateCell(id, 1, "name", "Ann"), t.id);
  // Select the first row (row header) and delete it: no confirmation for a table; Undo puts it back with its values.
  const nOf = async () => (await layers()(app)).find((l) => l.name === "People").n;
  await app.locator(".grid-row .cell.fid", { hasText: /^1$/ }).first().click();
  await app.getByRole("button", { name: /^Delete 1 selected row/ }).click();
  await expect.poll(nOf).toBe(1);
  await app.getByRole("button", { name: /^Undo the last edit/ }).click();
  await expect.poll(nOf).toBe(2);
  const rows = (await gis(app, (g, id) => g.api.rowsByFid(id, [1, 2]), t.id)) as { fid: number; values: (string | null)[] }[];
  expect(rows).toEqual([{ fid: 1, values: ["Ann", null] }, { fid: 2, values: [null, null] }]);
  // Undo again removes the second row, which was added after the first edit... the stack runs backwards: the cell edit was made
  // through the API, so the next Undo is the second "add row".
  await app.getByRole("button", { name: /^Undo the last edit/ }).click();
  await expect.poll(nOf).toBe(1);
});

test("UR-14 a table can be exported to Excel, OpenDocument and the other formats from the Export dialog's file types, and the files read back", async ({ app }) => {
  // The file-type list for a table (what Choose file… offers); the engine side is covered by tables_ur14.rs.
  const t = (await gis(app, async (g) => {
    const info = await g.api.createTable("Plants", [["species", "text"], ["height", "double"]]);
    g.useApp.getState().addLayers([info]);
    for (const [s, h] of [["oak", "12.5"], ["pine", "20"]]) { const r = await g.api.addRow(info.id); await g.api.updateCell(info.id, r.fid, "species", s); await g.api.updateCell(info.id, r.fid, "height", h); }
    return info;
  })) as any;
  const dir = (await import("node:os")).tmpdir();
  for (const ext of ["csv", "tsv", "xlsx", "ods", "parquet", "json", "dbf"]) {
    const path = `${dir}/gis-e2e-plants-${Date.now()}.${ext}`;
    const fmt = { csv: "CSV", tsv: "TSV", xlsx: "XLSX", ods: "ODS", parquet: "PARQUET", json: "JSON", dbf: "DBF" }[ext];
    const out = (await gis(app, async (g, a: any) => {
      const res = await g.api.runGraph({ nodes: [{ id: "in", tool: "input_layer", params: { layer: a.id } }, { id: "out", tool: "export", params: { path: a.path, format: a.fmt } }], edges: [{ source: "in", target: "out", targetHandle: "in" }] });
      const o = res.find((r: any) => r.id === "out");
      const back = o.error ? [] : await g.api.importFiles([a.path]);
      return { error: o.error, back: back.map((b: any) => ({ type: b.geomType, n: b.featureCount })) };
    }, { id: t.id, path, fmt })) as any;
    expect(out.error ?? null, ext).toBeNull();
    expect(out.back, ext).toEqual([{ type: "Table", n: 2 }]);
  }
});
