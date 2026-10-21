// Dev builds only: expose state for scripted end-to-end checks (window.__gis).
import * as actions from "./actions";
import { api } from "./ipc/api";
import { getMap } from "./map/mapController";
import { useModel, useRun } from "./store/model";
import { useApp } from "./store/store";

if (import.meta.env.DEV) {
  (window as unknown as { __gis: unknown }).__gis = { api, actions, useApp, useModel, useRun, getMap };
}

