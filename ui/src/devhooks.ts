// Dev builds only: expose state for scripted end-to-end checks (window.__gis).
import * as actions from "./actions";
import { api } from "./ipc/api";
import * as mapController from "./map/mapController";
import { useMapView } from "./panes/MapView";
import { useModel, useRun } from "./store/model";
import * as layoutActions from "./layout/actions";
import { startLayoutTrace } from "./layout/trace";
import { useLayouts } from "./store/layout";
import { useApp } from "./store/store";
import * as panes from "./panes/panes";
import * as mapActions from "./maps/mapActions";
import * as editActions from "./edit/editActions";
import { useClip, useEdit } from "./edit/editStore";
import { useGeoref } from "./georef/store";

if (import.meta.env.DEV) {
  (window as unknown as { __gis: unknown }).__gis = { api, actions, useApp, useModel, useRun, getMap: mapController.getMap, mapController, useMapView, useLayouts, layoutActions, panes, edit: { ...editActions, useEdit, useClip }, mapActions, useGeoref };
  startLayoutTrace();
}

