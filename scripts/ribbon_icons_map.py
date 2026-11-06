"""Which Fluent one-colour icon each ribbon command used before UR-23, and which new icon replaces it."""
FLUENT2ICON = {
 'AddCircleRegular':'add-data','AddSquareRegular':'add-tool','AlignCenterHorizontalRegular':'layout-guide-h','AlignCenterVerticalRegular':'layout-guide-v',
 'ArrowExportRegular':'export','ArrowMaximizeRegular':'fit-page','ArrowResetRegular':'reset-panes','ArrowRightRegular':'layout-arrow','BookInformationRegular':'layout-sources',
 'BorderAllRegular':'usng-grid','CalculatorRegular':'calc-field','ChartMultipleRegular':'layout-chart','CircleRegular':'ellipse','ColorRegular':'palette',
 'CompassNorthwestRegular':'declination','CopyRegular':'duplicate','CursorClickRegular':'select','DarkThemeRegular':'theme','DataUsageRegular':'test-data',
 'DatabaseRegular':'datum-grids','DeleteRegular':'remove','DismissCircleRegular':'clear-selection','DocumentAddRegular':'new','DocumentPdfRegular':'pdf','EarthRegular':'globe',
 'FilterRegular':'definition-query','FolderOpenRegular':'folder-open','FullScreenMaximizeRegular':'full-extent','GlobeLocationRegular':'define-projection','GlobeRegular':'coordinate-system',
 'GridRegular':'graticule','GroupRegular':'layout-group','HandLeftRegular':'explore','ImageAddRegular':'layout-picture-add','ImageRegular':'image','InfoRegular':'identify',
 'LayerDiagonalAddRegular':'layer-add','LineHorizontal1Regular':'layout-line','LocationRegular':'select-location','LocationTargetSquareRegular':'frame-from-map','LockClosedRegular':'layout-lock',
 'Map16Regular':'overview','MapRegular':'map','MountainTrailRegular':'topo-map','MyLocationRegular':'resection','NavigationRegular':'north-arrow','PaintBrushRegular':'symbology',
 'PlayRegular':'run-model','RectangleLandscapeRegular':'rectangle','RulerRegular':'measure','SaveEditRegular':'save-as','SaveRegular':'save','ScanObjectRegular':'zoom-selection',
 'SelectAllOnRegular':'select-attributes','SettingsRegular':'properties','ShapesRegular':'shapes','TableAddRegular':'table-add-col','TableRegular':'attribute-table','TargetArrowRegular':'intersect',
 'TaskListSquareLtrRegular':'preflight','TextBulletListRegular':'layout-legend','TextTRegular':'layout-text','TopSpeedRegular':'fps','TriangleRegular':'triangle','ZoomFitRegular':'zoom-layer',
 'ZoomInRegular':'zoom-in','ZoomOutRegular':'zoom-out',
}
# per command id where the Fluent icon was shared by commands that mean different things
CMD = {
 'tissot':'tissot','layoutEllipse':'ellipse','exportTable':'export-table','exportLayer':'export','layoutGuidesClear':'clear-guides','removeLayer':'remove','newLayout':'new-layout','layoutPdf':'pdf',
 'open':'open-project','catalog':'catalog','mapCrs':'coordinate-system','layoutFrameCrs':'frame-crs','worldView':'globe','worldOutline':'world-outline','newMap':'map-add','layoutMapFrame':'map-frame',
 'paneMap':'map','responsibleUse':'info','identify':'identify','splitByAttribute':'split','layerFromSelection':'layer-add','selectBox':'select-box','layoutRect':'rectangle','measure':'measure',
 'calcGeometry':'calc-geometry','layoutScaleBar':'scale-bar','measureArea':'measure-area','layoutSvg':'svg','census':'people','addField':'table-add-col','newTable':'table-new','layoutTable':'attribute-table',
 'openTable':'attribute-table','layoutActual':'zoom-actual','zoomLayer':'zoom-layer','layoutUngroup':'layout-ungroup','layoutGroup':'layout-group','layoutPng':'image','projectionBuilder':'projection-builder',
 'layerProps':'properties','layoutFromMap':'frame-from-map','layoutOverview':'overview','layoutNorthArrow':'north-arrow','layoutDeclination':'declination','fullExtent':'full-extent','benchmark':'fps','fps':'fps',
 'layoutFit':'fit-page','layoutSources':'layout-sources','save':'save','saveAs':'save-as','new':'new',
}
