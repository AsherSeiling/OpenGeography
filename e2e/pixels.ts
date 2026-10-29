// Helpers for pixel-level checks: decode a PNG in the page, sample it, and write tiny GeoTIFF / ASCII-grid fixtures.
import type { Page } from "@playwright/test";
import * as fs from "node:fs";
import * as zlib from "node:zlib";

export type RGBA = [number, number, number, number];

/** Decode a base64 PNG in the page; returns a sampler by pixel position plus the image size. */
export async function decodePng(page: Page, b64: string, points: [number, number][]): Promise<{ w: number; h: number; px: RGBA[]; distinct: number }> {
  return page.evaluate(async ({ b64, points }) => {
    const bin = atob(b64);
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    const bmp = await createImageBitmap(new Blob([bytes], { type: "image/png" }));
    const c = document.createElement("canvas");
    c.width = bmp.width;
    c.height = bmp.height;
    const ctx = c.getContext("2d", { willReadFrequently: true })!;
    ctx.drawImage(bmp, 0, 0);
    const all = ctx.getImageData(0, 0, c.width, c.height).data;
    const colours = new Set<number>();
    for (let i = 0; i < all.length; i += 4) colours.add((all[i] << 24) | (all[i + 1] << 16) | (all[i + 2] << 8) | all[i + 3]);
    const px = points.map(([x, y]) => Array.from(ctx.getImageData(Math.round(x), Math.round(y), 1, 1).data) as [number, number, number, number]);
    return { w: c.width, h: c.height, px, distinct: colours.size };
  }, { b64, points });
}

/** Web Mercator tile containing lon/lat at zoom z, and the pixel (0–256) of a lon/lat inside it. */
export function tileOf(lon: number, lat: number, z: number) {
  const n = 2 ** z;
  const x = ((lon + 180) / 360) * n;
  const rad = (lat * Math.PI) / 180;
  const y = ((1 - Math.log(Math.tan(rad) + 1 / Math.cos(rad)) / Math.PI) / 2) * n;
  return { x: Math.floor(x), y: Math.floor(y), fx: x, fy: y };
}

export function tilePixel(lon: number, lat: number, z: number, tx: number, ty: number): [number, number] {
  const t = tileOf(lon, lat, z);
  return [(t.fx - tx) * 256, (t.fy - ty) * 256];
}

const WGS84_PRJ = 'GEOGCS["GCS_WGS_1984",DATUM["D_WGS_1984",SPHEROID["WGS_1984",6378137.0,298.257223563]],PRIMEM["Greenwich",0.0],UNIT["Degree",0.0174532925199433]]';

/** A one-band ASCII grid in EPSG:4326 (with its .prj) whose value is `f(col, row)`; origin = lower-left corner. */
export function writeAsc(path: string, cols: number, rows: number, lon0: number, lat0: number, cell: number, f: (c: number, r: number) => number) {
  const lines = [`ncols ${cols}`, `nrows ${rows}`, `xllcorner ${lon0}`, `yllcorner ${lat0}`, `cellsize ${cell}`, "NODATA_value -9999"];
  for (let r = 0; r < rows; r++) lines.push(Array.from({ length: cols }, (_, c) => f(c, r)).join(" "));
  fs.writeFileSync(path, lines.join("\n") + "\n");
  fs.writeFileSync(path.replace(/\.asc$/, ".prj"), WGS84_PRJ);
}

/** An uncompressed 8-bit GeoTIFF in EPSG:4326, `bands` channels, pixel-interleaved; `f(col,row,band)` gives 0–255. */
export function writeGeoTiff(path: string, w: number, h: number, bands: number, lon0: number, latTop: number, cell: number, f: (c: number, r: number, b: number) => number) {
  const data = Buffer.alloc(w * h * bands);
  for (let r = 0; r < h; r++) for (let c = 0; c < w; c++) for (let b = 0; b < bands; b++) data[(r * w + c) * bands + b] = Math.max(0, Math.min(255, Math.round(f(c, r, b))));
  type Entry = { tag: number; type: number; count: number; value: Buffer | number };
  const extra: Buffer[] = [];
  const entries: Entry[] = [];
  const short = (v: number[]) => { const b = Buffer.alloc(v.length * 2); v.forEach((x, i) => b.writeUInt16LE(x, i * 2)); return b; };
  const doubles = (v: number[]) => { const b = Buffer.alloc(v.length * 8); v.forEach((x, i) => b.writeDoubleLE(x, i * 8)); return b; };
  const headerSize = 8;
  const nEntries = 12;
  const ifdSize = 2 + nEntries * 12 + 4;
  let offset = headerSize + ifdSize;
  const place = (buf: Buffer) => { const o = offset; extra.push(buf); offset += buf.length + (buf.length % 2); if (buf.length % 2) extra.push(Buffer.alloc(1)); return o; };
  const bitsOff = place(short(Array(bands).fill(8)));
  const scaleOff = place(doubles([cell, cell, 0]));
  const tieOff = place(doubles([0, 0, 0, lon0, latTop, 0]));
  const geoKeys = short([1, 1, 0, 3, 1024, 0, 1, 2, 1025, 0, 1, 1, 2048, 0, 1, 4326]);
  const geoOff = place(geoKeys);
  const dataOff = place(data);
  const e = (tag: number, type: number, count: number, value: number) => entries.push({ tag, type, count, value });
  e(256, 3, 1, w);
  e(257, 3, 1, h);
  e(258, 3, bands, bands === 1 ? 8 : bitsOff);
  e(262, 3, 1, bands >= 3 ? 2 : 1);
  e(273, 4, 1, dataOff);
  e(277, 3, 1, bands);
  e(278, 3, 1, h);
  e(279, 4, 1, data.length);
  e(284, 3, 1, 1);
  e(33550, 12, 3, scaleOff);
  e(33922, 12, 6, tieOff);
  e(34735, 3, geoKeys.length / 2, geoOff);
  const head = Buffer.alloc(headerSize + ifdSize);
  head.write("II", 0);
  head.writeUInt16LE(42, 2);
  head.writeUInt32LE(8, 4);
  head.writeUInt16LE(entries.length, 8);
  entries.sort((a, b) => a.tag - b.tag).forEach((en, i) => {
    const o = 10 + i * 12;
    head.writeUInt16LE(en.tag, o);
    head.writeUInt16LE(en.type, o + 2);
    head.writeUInt32LE(en.count, o + 4);
    if (en.type === 3 && en.count === 1) head.writeUInt16LE(en.value as number, o + 8);
    else head.writeUInt32LE(en.value as number, o + 8);
  });
  fs.writeFileSync(path, Buffer.concat([head, ...extra]));
}

export type Query = { px: [number, number] } | { count: [number, number, number, number]; rgb: [number, number, number]; tol?: number } | { nonwhite: [number, number, number, number] } | { size: true };

/** Answer pixel questions about a base64 PNG, in the PNG's own pixel coordinates. */
export async function analyse(page: Page, b64: string, queries: Query[]): Promise<any[]> {
  return page.evaluate(async ({ b64, queries }) => {
    const bin = atob(b64);
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    const bmp = await createImageBitmap(new Blob([bytes], { type: "image/png" }));
    const c = document.createElement("canvas");
    c.width = bmp.width;
    c.height = bmp.height;
    const ctx = c.getContext("2d", { willReadFrequently: true })!;
    ctx.drawImage(bmp, 0, 0);
    return queries.map((q: any) => {
      if (q.size) return [c.width, c.height];
      if (q.px) return Array.from(ctx.getImageData(Math.round(q.px[0]), Math.round(q.px[1]), 1, 1).data);
      const [x, y, w, h] = (q.count ?? q.nonwhite).map((v: number) => Math.max(0, Math.round(v)));
      const d = ctx.getImageData(x, y, Math.max(1, Math.min(w, c.width - x)), Math.max(1, Math.min(h, c.height - y))).data;
      let n = 0;
      for (let i = 0; i < d.length; i += 4) {
        if (q.count) {
          const tol = q.tol ?? 24;
          if (d[i + 3] > 200 && Math.abs(d[i] - q.rgb[0]) <= tol && Math.abs(d[i + 1] - q.rgb[1]) <= tol && Math.abs(d[i + 2] - q.rgb[2]) <= tol) n++;
        } else if (d[i + 3] > 20 && (d[i] < 235 || d[i + 1] < 235 || d[i + 2] < 235)) n++;
      }
      return n;
    });
  }, { b64, queries });
}

/** A solid-colour RGBA PNG as a base64 data URI (for picture elements). */
export function solidPngUri(w: number, h: number, rgba: [number, number, number, number]): string {
  const raw = Buffer.alloc((w * 4 + 1) * h);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) Buffer.from(rgba).copy(raw, y * (w * 4 + 1) + 1 + x * 4);
  const crcTable = Array.from({ length: 256 }, (_, n) => { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c >>> 0; });
  const crc = (b: Buffer) => { let c = 0xffffffff; for (const x of b) c = crcTable[(c ^ x) & 255] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; };
  const chunk = (type: string, data: Buffer) => {
    const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
    const td = Buffer.concat([Buffer.from(type), data]);
    const c = Buffer.alloc(4); c.writeUInt32BE(crc(td));
    return Buffer.concat([len, td, c]);
  };
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 6;
  const png = Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ihdr), chunk("IDAT", zlib.deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]);
  return `data:image/png;base64,${png.toString("base64")}`;
}
