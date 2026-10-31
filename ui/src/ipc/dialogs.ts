// File dialogs: the desktop app's native ones; in a browser (dev server, tests) the plugin throws, so ask for a path instead (TP-15).
import { open as nativeOpen, save as nativeSave } from "@tauri-apps/plugin-dialog";

type SaveOptions = Parameters<typeof nativeSave>[0];
type OpenOptions = Parameters<typeof nativeOpen>[0];

const inBrowser = () => typeof window !== "undefined" && !("__TAURI_INTERNALS__" in window);

function askPath(title: string | undefined, def: string | undefined): string | null {
  const p = window.prompt(`${title ?? "File"}: path on the machine running the dev server`, def ?? "");
  return p && p.trim() ? p.trim() : null;
}

export async function save(options?: SaveOptions): Promise<string | null> {
  try {
    return await nativeSave(options);
  } catch (e) {
    if (!inBrowser()) throw e;
    return askPath(options?.title, options?.defaultPath);
  }
}

export async function open(options?: OpenOptions): Promise<string | string[] | null> {
  try {
    return (await nativeOpen(options)) as string | string[] | null;
  } catch (e) {
    if (!inBrowser()) throw e;
    const p = askPath(options?.title, options?.defaultPath);
    if (!p) return null;
    return options?.multiple ? [p] : p;
  }
}
