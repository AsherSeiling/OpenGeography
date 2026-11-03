// Just enough of Node's fs/path/url for the theme tests to read CSS from disk (Vitest returns "" for `.css?raw`).
// Declared here instead of adding @types/node as a dependency.
declare module "node:fs" {
  export function readFileSync(path: string | URL, encoding: "utf8"): string;
  export function readdirSync(path: string): string[];
  export function statSync(path: string): { isDirectory(): boolean };
}
declare module "node:path" {
  export function join(...parts: string[]): string;
  export function relative(from: string, to: string): string;
  export const sep: string;
}
declare module "node:url" {
  export function fileURLToPath(url: string | URL): string;
}
