// Copyright 2026 Charles W Jackson
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

import { useMemo, useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Menu, MenuItem, Submenu, CheckMenuItem, PredefinedMenuItem } from "@tauri-apps/api/menu";
import Viewer3D from "./components/Viewer3D";
import { LogViewer } from "./components/LogViewer";
import { SolutionViewer } from "./components/SolutionViewer";
import { LoadingIndicator } from "./components/LoadingIndicator";
import { logger } from "./utils/logger";
import { groupGridsByFile } from "./utils/gridUtils";
import type { GridMetadata, SolutionMetadata } from "./types/plot3d";
import type { GridItem, GridSlice, ArbitrarySlice } from "./types/grids";
import type { ScalarField } from "./utils/solutionData";
import type { ColorScheme } from "./utils/colorMapping";
import "./App.css";

interface FileMetadata {
  fileNames: string[];
  gridCount: number;
}

type BackendScalarField =
  | 'none'
  | 'density'
  | 'velocity_magnitude'
  | 'momentum_x'
  | 'momentum_y'
  | 'momentum_z'
  | 'pressure'
  | 'energy';

type BackendPlotFamily = 'contour' | 'function_surface';

type BackendContourAttribute = 'line' | 'surface' | 'grid' | 'color_contours' | 'dots';

type ContourSpecMode = 'none' | 'automatic' | 'increment' | 'manual';

type BackendAxisView =
  | 'plus_x'
  | 'minus_x'
  | 'plus_y'
  | 'minus_y'
  | 'plus_z'
  | 'minus_z'
  | 'plane_xy'
  | 'plane_xz'
  | 'plane_yz'
  | 'plane_yx'
  | 'plane_zx'
  | 'plane_zy'
  | 'custom';

type BackendPlotUpAxis =
  | 'positive_x'
  | 'positive_y'
  | 'positive_z'
  | 'negative_x'
  | 'negative_y'
  | 'negative_z';

const AXIS_VIEW_OPTIONS: Array<{ value: BackendAxisView; label: string }> = [
  { value: 'custom', label: 'Custom' },
  { value: 'plus_x', label: '+X (Right)' },
  { value: 'minus_x', label: '-X (Left)' },
  { value: 'plus_y', label: '+Y (Top)' },
  { value: 'minus_y', label: '-Y (Bottom)' },
  { value: 'plus_z', label: '+Z (Front)' },
  { value: 'minus_z', label: '-Z (Back)' },
  { value: 'plane_xy', label: 'Plane XY (Top)' },
  { value: 'plane_xz', label: 'Plane XZ (Side)' },
  { value: 'plane_yz', label: 'Plane YZ (Front)' },
  { value: 'plane_yx', label: 'Plane YX' },
  { value: 'plane_zx', label: 'Plane ZX' },
  { value: 'plane_zy', label: 'Plane ZY' },
];

interface BackendPlotState {
  scalar_field: BackendScalarField;
  plot_family: BackendPlotFamily;
  contour_attribute: BackendContourAttribute;
  axis_view: BackendAxisView;
  plot_up?: BackendPlotUpAxis | null;
  contour_spec: unknown;
  walls: BackendGridSubset[];
  subsets: BackendGridSubset[];
  fsurface?: BackendFsurfaceSpec | null;
  text_annotations: BackendPlotText[];
  viewpoint?: { x: number; y: number; z: number } | null;
}

interface BackendFsurfaceSpec {
  value: number;
  scalar_field: BackendScalarField;
}

interface BackendPlotText {
  content: string;
  x: number;
  y: number;
}

interface BackendIndexRange {
  start: number;
  end?: number | null;
}

interface BackendGridSubset {
  grid: number;
  gui_managed?: boolean;
  i_range?: BackendIndexRange | null;
  j_range?: BackendIndexRange | null;
  k_range?: BackendIndexRange | null;
}

interface BackendDiagnostic {
  capability: string;
  severity: 'info' | 'warning' | 'error';
  message: string;
}

interface ApplyPlotActionResult {
  state: BackendPlotState;
  diagnostics: BackendDiagnostic[];
}

interface RenderIntent {
  state: BackendPlotState;
}

interface ScriptExecutionResult {
  final_state: BackendPlotState;
  intents: RenderIntent[];
  show_output: string[];
  diagnostics: BackendDiagnostic[];
}

interface ShowStatusResult {
  status: string;
  state: BackendPlotState;
  diagnostics: BackendDiagnostic[];
}

interface EditableSubset {
  id: string;
  subset: BackendGridSubset;
  editing: boolean;
}

const cloneSubset = (subset: BackendGridSubset): BackendGridSubset => ({
  grid: subset.grid,
  gui_managed: subset.gui_managed,
  i_range: subset.i_range ? { ...subset.i_range } : null,
  j_range: subset.j_range ? { ...subset.j_range } : null,
  k_range: subset.k_range ? { ...subset.k_range } : null,
});

const editableFromBackend = (items: BackendGridSubset[]): EditableSubset[] =>
  items.map((subset, idx) => ({
    id: `range-${subset.grid}-${idx}-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
    subset: cloneSubset(subset),
    editing: false,
  }));

const parseOptionalInt = (value: string): number | null => {
  const parsed = Number.parseInt(value.trim(), 10);
  return Number.isFinite(parsed) ? parsed : null;
};

const rangeStartString = (range?: BackendIndexRange | null): string =>
  range ? String(range.start) : '';

const rangeEndString = (range?: BackendIndexRange | null): string =>
  range?.end != null ? String(range.end) : '';

const compactRangeStr = (range?: BackendIndexRange | null, dim?: number): string => {
  if (!range) {
    return ':';
  }
  const isFull =
    range.start === 1 &&
    (range.end == null || range.end === -1 || (dim != null && range.end === dim));
  if (isFull) {
    return ':';
  }
  return `${range.start}:${range.end ?? ''}`;
};

const compactSubsetLabel = (
  subset: BackendGridSubset,
  dims?: { i: number; j: number; k: number }
): string =>
  `G${subset.grid} (${compactRangeStr(subset.i_range, dims?.i)}, ${compactRangeStr(subset.j_range, dims?.j)}, ${compactRangeStr(subset.k_range, dims?.k)})`;

const normalizeAxisRangeForApply = (
  range?: BackendIndexRange | null
): BackendIndexRange => {
  if (!range) {
    return { start: 1, end: -1 };
  }
  if (range.start === 1 && range.end == null) {
    return { start: 1, end: -1 };
  }
  return { ...range };
};

const normalizeSubsetForApply = (subset: BackendGridSubset): BackendGridSubset => ({
  ...cloneSubset(subset),
  i_range: normalizeAxisRangeForApply(subset.i_range),
  j_range: normalizeAxisRangeForApply(subset.j_range),
  k_range: normalizeAxisRangeForApply(subset.k_range),
});

const subsetRangeSig = (range?: BackendIndexRange | null) =>
  range ? `${range.start}:${range.end ?? ''}` : '-';

const subsetsSignature = (subsets: BackendGridSubset[]): string =>
  subsets
    .map(
      (s) =>
        `${s.grid}|${s.gui_managed ? 'gui' : 'manual'}|${subsetRangeSig(s.i_range)}|${subsetRangeSig(s.j_range)}|${subsetRangeSig(s.k_range)}`
    )
    .sort()
    .join(';');

const dedupeSubsets = (items: BackendGridSubset[]): BackendGridSubset[] => {
  const map = new Map<string, BackendGridSubset>();
  items.forEach((s) => {
    map.set(
      `${s.grid}|${s.gui_managed ? 'gui' : 'manual'}|${subsetRangeSig(s.i_range)}|${subsetRangeSig(s.j_range)}|${subsetRangeSig(s.k_range)}`,
      s
    );
  });
  return Array.from(map.values());
};

const gridSlicesSignature = (gridSlices: Record<string, GridSlice[]>): string =>
  Object.entries(gridSlices)
    .flatMap(([gridId, slices]) => slices.map((slice) => `${gridId}|${slice.plane}|${slice.index}`))
    .sort()
    .join(';');

const gridSlicesToBackendSubsets = (
  gridSlices: Record<string, GridSlice[]>,
  grids: GridItem[],
  sliceEnabled: boolean
): BackendGridSubset[] => {
  if (!sliceEnabled) {
    return [];
  }

  const subsets: BackendGridSubset[] = [];
  grids.forEach((grid) => {
    const slices = gridSlices[grid.id] || [];
    slices.forEach((slice) => {
      const fullI: BackendIndexRange = { start: 1, end: Math.max(1, grid.dimensions.i) };
      const fullJ: BackendIndexRange = { start: 1, end: Math.max(1, grid.dimensions.j) };
      const fullK: BackendIndexRange = { start: 1, end: Math.max(1, grid.dimensions.k) };

      const iPoint = Math.max(1, Math.min(Math.max(1, grid.dimensions.i), Math.floor(slice.index) + 1));
      const jPoint = Math.max(1, Math.min(Math.max(1, grid.dimensions.j), Math.floor(slice.index) + 1));
      const kPoint = Math.max(1, Math.min(Math.max(1, grid.dimensions.k), Math.floor(slice.index) + 1));

      subsets.push({
        grid: grid.gridIndex + 1,
        gui_managed: true,
        i_range: slice.plane === 'I' ? { start: iPoint, end: iPoint } : fullI,
        j_range: slice.plane === 'J' ? { start: jPoint, end: jPoint } : fullJ,
        k_range: slice.plane === 'K' ? { start: kPoint, end: kPoint } : fullK,
      });
    });
  });

  return subsets;
};

const backendSubsetsToGridSlices = (
  subsets: BackendGridSubset[],
  grids: GridItem[]
): Record<string, GridSlice[]> => {
  const gridByNumber = new Map<number, GridItem>();
  grids.forEach((grid) => {
    gridByNumber.set(grid.gridIndex + 1, grid);
  });

  const byGrid: Record<string, GridSlice[]> = {};
  let counter = 0;

  subsets.forEach((subset) => {
    if (!subset.gui_managed) {
      return;
    }
    const grid = gridByNumber.get(subset.grid);
    if (!grid) {
      return;
    }

    const axisRanges: Array<{ plane: 'I' | 'J' | 'K'; range?: BackendIndexRange | null; dim: number }> = [
      { plane: 'I', range: subset.i_range, dim: Math.max(1, grid.dimensions.i) },
      { plane: 'J', range: subset.j_range, dim: Math.max(1, grid.dimensions.j) },
      { plane: 'K', range: subset.k_range, dim: Math.max(1, grid.dimensions.k) },
    ];

    const resolveRange = (range: BackendIndexRange | null | undefined, dim: number) => {
      if (!range) {
        return { start: 1, end: dim };
      }
      const resolveOneBased = (n: number) => (n < 0 ? dim + n + 1 : n);
      const start = Math.max(1, Math.min(dim, resolveOneBased(range.start)));
      const endRaw = range.end != null ? resolveOneBased(range.end) : dim;
      const end = Math.max(1, Math.min(dim, endRaw));
      return start <= end ? { start, end } : { start: end, end: start };
    };

    const classified = axisRanges.map((a) => {
      const r = resolveRange(a.range, a.dim);
      const kind: 'point' | 'full' | 'other' =
        r.start === r.end ? 'point' : r.start === 1 && r.end === a.dim ? 'full' : 'other';
      return { ...a, resolved: r, kind };
    });

    const points = classified.filter((a) => a.kind === 'point');
    const othersAreFull = classified
      .filter((a) => a.kind !== 'point')
      .every((a) => a.kind === 'full');

    // GUI-managed slice subset must have one point axis and others full-range.
    if (points.length !== 1 || !othersAreFull) {
      return;
    }

    const selected = points[0];
    const zeroBased = Math.max(0, Math.min(selected.dim - 1, selected.resolved.start - 1));

    const id = `subset-sync-${subset.grid}-${selected.plane}-${selected.resolved.start}-${counter++}`;
    if (!byGrid[grid.id]) {
      byGrid[grid.id] = [];
    }
    byGrid[grid.id].push({ id, plane: selected.plane, index: zeroBased });
  });

  return byGrid;
};

const GRID_COLORS = [
  "#6366f1",
  "#22c55e",
  "#f97316",
  "#14b8a6",
  "#e11d48",
  "#f59e0b",
  "#0ea5e9",
  "#a855f7",
  "#84cc16",
  "#ef4444",
];

// Deprecated: Old API (kept for compatibility)
// const buildGridItems = (
//   grids: Plot3DGrid[],
//   filePath: string,
//   fileName: string,
//   colorOffset: number
// ): GridItem[] =>
//   grids.map((grid, index) => ({
//     id: `${filePath}::${index}`,
//     grid,
//     filePath,
//     fileName,
//     gridIndex: index,
//     dimensions: grid.dimensions,
//     hasIblank: !!grid.iblank,
//     color: GRID_COLORS[(index + colorOffset) % GRID_COLORS.length],
//     visible: true,
//     hasSolution: false,
//   }));

// Build grid items from metadata (v2 API - no full grid data)
const buildGridItemsFromMetadata = (
  metadataList: GridMetadata[],
  colorOffset: number
): GridItem[] =>
  metadataList.map((meta, index) => ({
    id: `${meta.file_path}::${meta.grid_index}`,
    gridCacheId: meta.id,
    filePath: meta.file_path,
    fileName: meta.file_name,
    gridIndex: meta.grid_index,
    dimensions: meta.dimensions,
    hasIblank: meta.has_iblank,
    color: GRID_COLORS[(index + colorOffset) % GRID_COLORS.length],
    visible: true,
    hasSolution: meta.has_solution,
  }));

const App = () => {
  const [error, setError] = useState("");
  const [fileMetadata, setFileMetadata] = useState<FileMetadata | null>(null);
  const [showLogs, setShowLogs] = useState(false);
  const [selectedGridIds, setSelectedGridIds] = useState<string[]>([]);
  const [isolateSelected, setIsolateSelected] = useState(false);
  const [hasSolution, setHasSolution] = useState(false);
  const [ignoreIblank, setIgnoreIblank] = useState(false);
  const [showFringePoints, setShowFringePoints] = useState(true);
  const [iblankFilterMode, setIblankFilterMode] = useState<'vertex' | 'cell'>('vertex');
  const [currentScalarField, setCurrentScalarField] = useState<ScalarField>('none');
  const [currentColorScheme, setCurrentColorScheme] = useState<ColorScheme>('viridis');
  const [showWireframe, setShowWireframe] = useState(true);
  const [shadingMode, setShadingMode] = useState<'none' | 'smooth'>('none');
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [sliceEnabled, setSliceEnabled] = useState(true);
  const [grids, setGrids] = useState<GridItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [loadingMessage, setLoadingMessage] = useState("Processing...");

  const [gridSlices, setGridSlices] = useState<Record<string, GridSlice[]>>({});
  const [subsetsDirty, setSubsetsDirty] = useState(false);
  const [sliceIndexDrafts, setSliceIndexDrafts] = useState<Record<string, string>>({});
  const [arbitrarySlices, setArbitrarySlices] = useState<ArbitrarySlice[]>([]);
  const [manualSubsetRows, setManualSubsetRows] = useState<EditableSubset[]>([]);
  const [manualWallsRows, setManualWallsRows] = useState<EditableSubset[]>([]);
  const [manualSubsetDirty, setManualSubsetDirty] = useState(false);
  const [manualWallsDirty, setManualWallsDirty] = useState(false);

  // Contour state
  const [plotFamilyState, setPlotFamilyState] = useState<BackendPlotFamily>('contour');
  const [contourAttributeState, setContourAttributeState] = useState<BackendContourAttribute>('line');
  const [contourSpecMode, setContourSpecMode] = useState<ContourSpecMode>('none');
  const [contourAutoCount, setContourAutoCount] = useState(10);
  const [contourAutoCountDraft, setContourAutoCountDraft] = useState('10');
  const [contourIncrStart, setContourIncrStart] = useState(0);
  const [contourIncrStartDraft, setContourIncrStartDraft] = useState('0');
  const [contourIncrStep, setContourIncrStep] = useState(1);
  const [contourIncrStepDraft, setContourIncrStepDraft] = useState('1');
  const [contourLevel, setContourLevel] = useState(0);
  const [contourLevelDraft, setContourLevelDraft] = useState('0');
  const [isoSurfaceOpacity, setIsoSurfaceOpacity] = useState(1.0);
  const [resolvedContourLevels, setResolvedContourLevels] = useState<number[]>([]);
  const [contourFieldMin, setContourFieldMin] = useState(0);
  const [contourFieldMax, setContourFieldMax] = useState(0);

  const dimensionsForGridNumber = (gridNumber: number) =>
    grids.find((grid) => grid.gridIndex + 1 === gridNumber)?.dimensions;

  const [backendPlotState, setBackendPlotState] = useState<BackendPlotState | null>(null);
  const [backendDiagnostics, setBackendDiagnostics] = useState<BackendDiagnostic[]>([]);
  const [showCommandWindow, setShowCommandWindow] = useState(false);
  const [commandText, setCommandText] = useState("SHOW\nPLOT/CONTOUR");
  const [comFilePath, setComFilePath] = useState("");
  const [commandWindowOutput, setCommandWindowOutput] = useState("");
  const [showStatusOutput, setShowStatusOutput] = useState("");
  const [fsurfaceEnabled, setFsurfaceEnabled] = useState(false);
  const [fsurfaceValueDraft, setFsurfaceValueDraft] = useState('0');
  const [fsurfaceField, setFsurfaceField] = useState<BackendScalarField>('pressure');
  const [textContentDraft, setTextContentDraft] = useState('');
  const [textXDraft, setTextXDraft] = useState('0.05');
  const [textYDraft, setTextYDraft] = useState('0.95');

  // Color map clipping state
  const [colorMapMin, setColorMapMin] = useState<number | null>(null);
  const [colorMapMax, setColorMapMax] = useState<number | null>(null);
  const [actualColorMapMin, setActualColorMapMin] = useState<number | null>(null);
  const [actualColorMapMax, setActualColorMapMax] = useState<number | null>(null);

  // Export workflow state
  const [lastExecutionResult, setLastExecutionResult] = useState<ScriptExecutionResult | null>(null);
  const [exportInProgress, setExportInProgress] = useState(false);
  const [exportStatus, setExportStatus] = useState('');

  const backendMappedSlicesSigRef = useRef<string | null>(null);
  const viewer3dLoadingRef = useRef(false);
  const viewer3dLoadingResolversRef = useRef<Array<() => void>>([]);

  const syncPlotStateFromBackend = async () => {
    try {
      const state = await invoke<BackendPlotState>('get_plot_state');
      setBackendPlotState(state);
    } catch (e) {
      logger.warn(`Failed to sync backend plot state: ${e}`, 'App');
    }
  };

  const updateBackendFromResult = (result: ApplyPlotActionResult) => {
    setBackendPlotState(result.state);
    setBackendDiagnostics(result.diagnostics);
  };

  const setPlotScalarField = async (field: BackendScalarField) => {
    try {
      const result = await invoke<ApplyPlotActionResult>('set_plot_scalar_field', { field });
      updateBackendFromResult(result);
    } catch (e) {
      logger.error(`Failed to set plot scalar field: ${e}`, 'App');
    }
  };

  const setPlotFamily = async (family: BackendPlotFamily) => {
    try {
      const result = await invoke<ApplyPlotActionResult>('set_plot_family', { family });
      updateBackendFromResult(result);
    } catch (e) {
      logger.error(`Failed to set plot family: ${e}`, 'App');
    }
  };

  const setPlotViewpoint = async (vp: { x: number; y: number; z: number }) => {
    try {
      const result = await invoke<ApplyPlotActionResult>('set_plot_viewpoint', { vp });
      updateBackendFromResult(result);
    } catch (e) {
      logger.error(`Failed to set plot viewpoint: ${e}`, 'App');
    }
  };

  const setPlotAxisView = async (view: BackendAxisView) => {
    try {
      const result = await invoke<ApplyPlotActionResult>('set_plot_axis_view', { view });
      updateBackendFromResult(result);
    } catch (e) {
      logger.error(`Failed to set plot axis view: ${e}`, 'App');
    }
  };

  const setPlotContourSpec = async (spec: object) => {
    try {
      const result = await invoke<ApplyPlotActionResult>('set_plot_contour_spec', { spec });
      updateBackendFromResult(result);
    } catch (e) {
      logger.error(`Failed to set contour spec: ${e}`, 'App');
    }
  };

  const setPlotContourAttributeCmd = async (attribute: BackendContourAttribute) => {
    try {
      const result = await invoke<ApplyPlotActionResult>('set_plot_contour_attribute', { attribute });
      updateBackendFromResult(result);
    } catch (e) {
      logger.error(`Failed to set contour attribute: ${e}`, 'App');
    }
  };

  const setPlotSubsets = async (subsets: BackendGridSubset[]) => {
    try {
      const result = await invoke<ApplyPlotActionResult>('set_plot_subsets', { subsets });
      updateBackendFromResult(result);
    } catch (e) {
      logger.error(`Failed to set plot subsets: ${e}`, 'App');
    }
  };

  const setPlotWalls = async (walls: BackendGridSubset[]) => {
    try {
      const result = await invoke<ApplyPlotActionResult>('set_plot_walls', { walls });
      updateBackendFromResult(result);
    } catch (e) {
      logger.error(`Failed to set plot walls: ${e}`, 'App');
    }
  };

  const setPlotFsurface = async (fsurface: BackendFsurfaceSpec | null) => {
    try {
      const result = await invoke<ApplyPlotActionResult>('set_plot_fsurface', { fsurface });
      updateBackendFromResult(result);
    } catch (e) {
      logger.error(`Failed to set FSURFACE: ${e}`, 'App');
    }
  };

  const addPlotTextAnnotation = async (text: BackendPlotText) => {
    try {
      const result = await invoke<ApplyPlotActionResult>('add_plot_text_annotation', { text });
      updateBackendFromResult(result);
    } catch (e) {
      logger.error(`Failed to add plot text annotation: ${e}`, 'App');
    }
  };

  const clearPlotTextAnnotations = async () => {
    try {
      const result = await invoke<ApplyPlotActionResult>('clear_plot_text_annotations');
      updateBackendFromResult(result);
    } catch (e) {
      logger.error(`Failed to clear plot text annotations: ${e}`, 'App');
    }
  };

  const refreshShowStatus = async () => {
    try {
      const result = await invoke<ShowStatusResult>('show_plot_status');
      setBackendPlotState(result.state);
      setBackendDiagnostics(result.diagnostics ?? []);
      setShowStatusOutput(result.status);
    } catch (e) {
      logger.error(`Failed to fetch SHOW status: ${e}`, 'App');
      setShowStatusOutput(`SHOW failed: ${e}`);
    }
  };

  const addManualRangeRow = (kind: 'subset' | 'wall') => {
    const defaultGrid = grids.length > 0 ? grids[0].gridIndex + 1 : 1;
    const row: EditableSubset = {
      id: `manual-${kind}-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
      subset: {
        grid: defaultGrid,
        gui_managed: false,
        i_range: null,
        j_range: null,
        k_range: null,
      },
      editing: true,
    };
    if (kind === 'subset') {
      setManualSubsetRows((prev) => [...prev, row]);
      setManualSubsetDirty(true);
    } else {
      setManualWallsRows((prev) => [...prev, row]);
      setManualWallsDirty(true);
    }
  };

  const updateManualRangeRow = (
    kind: 'subset' | 'wall',
    rowId: string,
    updater: (subset: BackendGridSubset) => BackendGridSubset
  ) => {
    if (kind === 'subset') {
      setManualSubsetRows((prev) =>
        prev.map((row) => (row.id === rowId ? { ...row, subset: updater(row.subset) } : row))
      );
      setManualSubsetDirty(true);
      return;
    }
    setManualWallsRows((prev) =>
      prev.map((row) => (row.id === rowId ? { ...row, subset: updater(row.subset) } : row))
    );
    setManualWallsDirty(true);
  };

  const removeManualRangeRow = (kind: 'subset' | 'wall', rowId: string) => {
    if (kind === 'subset') {
      setManualSubsetRows((prev) => prev.filter((row) => row.id !== rowId));
      setManualSubsetDirty(true);
      return;
    }
    setManualWallsRows((prev) => prev.filter((row) => row.id !== rowId));
    setManualWallsDirty(true);
  };

  const setRowEditing = (kind: 'subset' | 'wall', rowId: string, editing: boolean) => {
    if (kind === 'subset') {
      setManualSubsetRows((prev) => prev.map((row) => (row.id === rowId ? { ...row, editing } : row)));
    } else {
      setManualWallsRows((prev) => prev.map((row) => (row.id === rowId ? { ...row, editing } : row)));
    }
  };

  const updateManualAxisRange = (
    kind: 'subset' | 'wall',
    rowId: string,
    axis: 'i_range' | 'j_range' | 'k_range',
    edge: 'start' | 'end',
    rawValue: string
  ) => {
    const parsed = parseOptionalInt(rawValue);
    updateManualRangeRow(kind, rowId, (subset) => {
      const current = subset[axis] ?? { start: 1, end: null };
      const nextRange: BackendIndexRange = {
        start: edge === 'start' ? (parsed ?? current.start) : current.start,
        end: edge === 'end' ? parsed : (current.end ?? null),
      };

      if (edge === 'start' && parsed == null) {
        if ((nextRange.end ?? null) == null) {
          return { ...subset, [axis]: null };
        }
        nextRange.start = 1;
      }

      return { ...subset, [axis]: nextRange };
    });
  };

  const commitPlot = async () => {
    try {
      const result = await invoke<ApplyPlotActionResult>('commit_plot');
      updateBackendFromResult(result);
    } catch (e) {
      logger.error(`Failed to commit plot: ${e}`, 'App');
    }
  };

  const formatExecutionResult = (result: ScriptExecutionResult) => {
    const diagnostics = result.diagnostics ?? [];
    const shows = result.show_output ?? [];
    const intents = result.intents ?? [];
    const diagSummary = diagnostics.map((d) => `[${d.severity}] ${d.capability}: ${d.message}`).join('\n');

    const sections = [
      `Final plot mode: ${result.final_state?.plot_family ?? 'unknown'}`,
      `Final scalar field: ${result.final_state?.scalar_field ?? 'unknown'}`,
      `Render intents: ${intents.length}`,
      `SHOW lines: ${shows.length}`,
      '',
      'SHOW output:',
      shows.length ? shows.join('\n') : '(none)',
      '',
      'Diagnostics:',
      diagSummary || '(none)',
    ];

    return sections.join('\n');
  };

  const runCommandText = async () => {
    const commands = commandText.trim();
    if (!commands) {
      setCommandWindowOutput('Enter one or more commands first.');
      return;
    }
    try {
      const result = await invoke<ScriptExecutionResult>('execute_plot3d_commands', { commands });
      setBackendPlotState(result.final_state);
      setBackendDiagnostics(result.diagnostics ?? []);
      setCommandWindowOutput(formatExecutionResult(result));
    } catch (e) {
      setCommandWindowOutput(`Failed to execute commands:\n${e}`);
      logger.error(`Command window execute error: ${e}`, 'App');
    }
  };

  const runComFile = async () => {
    const path = comFilePath.trim();
    if (!path) {
      setCommandWindowOutput('Enter an absolute .com path first.');
      return;
    }
    try {
      const result = await invoke<ScriptExecutionResult>('execute_com_script', { path });
      setBackendPlotState(result.final_state);
      setBackendDiagnostics(result.diagnostics ?? []);
      setLastExecutionResult(result);
      setCommandWindowOutput(formatExecutionResult(result));
      setExportStatus('');
    } catch (e) {
      setCommandWindowOutput(`Failed to execute .com file:\n${e}`);
      logger.error(`.com execute error: ${e}`, 'App');
      setLastExecutionResult(null);
    }
  };

  const canvasToPngBytes = async (canvas: HTMLCanvasElement): Promise<number[]> => {
    const blob = await new Promise<Blob>((resolve, reject) => {
      canvas.toBlob((nextBlob) => {
        if (!nextBlob) {
          reject(new Error('Failed to create PNG blob from canvas'));
          return;
        }
        resolve(nextBlob);
      }, 'image/png');
    });

    const arrayBuffer = await blob.arrayBuffer();
    return Array.from(new Uint8Array(arrayBuffer));
  };

  // Export current canvas view as PNG
  const exportCurrentViewAsPNG = async () => {
    setExportInProgress(true);
    try {
      const canvas = document.querySelector('canvas') as HTMLCanvasElement;
      if (!canvas) {
        throw new Error('No canvas found. Render a visualization first.');
      }

      const filePath = await invoke<string | null>('save_png_file_dialog', {
        default_name: 'plot_view.png'
      });

      if (!filePath) {
        setExportStatus('Export cancelled');
        return;
      }

      const pngData = await canvasToPngBytes(canvas);

      const writtenPath = await invoke<string>('write_png_file', {
        path: filePath,
        pngData: pngData
      });

      setExportStatus(`Current view exported to ${writtenPath}`);
      logger.info(`Exported current view to ${writtenPath}`, 'App');
    } catch (e) {
      const errorMsg = `Failed to export current view: ${e}`;
      setExportStatus(errorMsg);
      logger.error(errorMsg, 'App');
    } finally {
      setExportInProgress(false);
    }
  };

  // Export one PNG per render intent from the last .com execution.
  // For multi-PLOT scripts each intent gets its own sequentially numbered file.
  const exportPNGsFromExecution = async () => {
    if (!lastExecutionResult || !lastExecutionResult.intents || lastExecutionResult.intents.length === 0) {
      setExportStatus('No render intents to export. Execute a .com file first.');
      return;
    }

    setExportInProgress(true);
    const intents = lastExecutionResult.intents;
    const intentCount = intents.length;

    try {
      const canvas = document.querySelector('canvas') as HTMLCanvasElement;
      if (!canvas) throw new Error('No canvas found. Render the plots first.');

      // Ask the user for the first (or only) output file path.
      const baseName = intentCount > 1 ? 'plot_output_001.png' : 'plot_output.png';
      const firstPath = await invoke<string | null>('save_png_file_dialog', {
        default_name: baseName
      });

      if (!firstPath) {
        setExportStatus('Export cancelled');
        return;
      }

      const paths = derivePngPaths(firstPath, intentCount);
      const savedPaths: string[] = [];
      const failures: string[] = [];

      // Remember the state before export so we can restore it afterwards.
      const preExportState = backendPlotState;

      for (let i = 0; i < intentCount; i++) {
        setExportStatus(`Rendering plot ${i + 1}/${intentCount}…`);

        // Apply this intent's PlotState to drive the renderer.
        setBackendPlotState(intents[i].state);

        // Give React a tick to propagate the state through the sync effect,
        // then wait for Viewer3D computations to finish and Three.js to draw.
        await waitForViewer3DStable();

        try {
          const pngData = await canvasToPngBytes(canvas);
          const writtenPath = await invoke<string>('write_png_file', {
            path: paths[i],
            pngData,
          });
          savedPaths.push(writtenPath);
          logger.info(`Exported PNG ${i + 1}/${intentCount} to ${writtenPath}`, 'App');
        } catch (e) {
          failures.push(`Plot ${i + 1}: ${e}`);
          logger.error(`Failed to export plot ${i + 1}: ${e}`, 'App');
        }
      }

      // Restore the state that was active before export.
      setBackendPlotState(preExportState);

      if (failures.length > 0 && savedPaths.length === 0) {
        setExportStatus(`Export failed:\n${failures.join('\n')}`);
      } else if (failures.length > 0) {
        setExportStatus(
          `Exported ${savedPaths.length}/${intentCount} PNGs.\nFailed:\n${failures.join('\n')}\nSaved:\n${savedPaths.join('\n')}`
        );
      } else {
        setExportStatus(
          `Successfully exported ${intentCount} PNG${intentCount > 1 ? 's' : ''}:\n${savedPaths.join('\n')}`
        );
      }
    } catch (e) {
      const errorMsg = `Failed to export PNG: ${e}`;
      setExportStatus(errorMsg);
      logger.error(errorMsg, 'App');
    } finally {
      setExportInProgress(false);
    }
  };


  // Arbitrary slice management
  const addArbitrarySlice = () => {
    const newSlice: ArbitrarySlice = {
      id: `arbitrary_${Date.now()}`,
      name: `Plane ${arbitrarySlices.length + 1}`,
      planePoint: [0, 0, 0],
      planeNormal: [0, 0, 1],
      enabled: true,
      applied: false,
      applyVersion: 0,
      dirty: true
    };
    setArbitrarySlices(prev => [...prev, newSlice]);
  };

  const removeArbitrarySlice = (sliceId: string) => {
    setArbitrarySlices(prev => prev.filter(s => s.id !== sliceId));
  };

  const updateArbitrarySlice = (sliceId: string, updates: Partial<ArbitrarySlice>) => {
    setArbitrarySlices(prev => prev.map(s => {
      if (s.id !== sliceId) return s;
      // If updating plane parameters (point/normal), mark as dirty but keep applied state
      const updatedSlice = { ...s, ...updates };
      if (updates.planePoint || updates.planeNormal) {
        updatedSlice.dirty = true;
      }
      return updatedSlice;
    }));
  };

  const toggleArbitrarySlice = (sliceId: string) => {
    setArbitrarySlices(prev => prev.map(s =>
      s.id === sliceId ? { ...s, enabled: !s.enabled } : s
    ));
  };

  const applyArbitrarySlice = (sliceId: string) => {
    setArbitrarySlices(prev => prev.map(s =>
      s.id === sliceId
        ? { ...s, applied: true, dirty: false, applyVersion: s.applyVersion + 1 }
        : s
    ));
  };

  // Grid slice management (index-based slicing)
  const getGridSlices = (gridId: string): GridSlice[] => gridSlices[gridId] || [];

  const addSliceToGrid = (gridId: string) => {
    // Find the grid to get its dimensions
    const grid = grids.find(g => g.id === gridId);
    if (!grid) return;

    const newSlice: GridSlice = {
      id: `slice_${Date.now()}`,
      plane: 'K',
      index: Math.floor(grid.dimensions.k / 2)
    };
    setSubsetsDirty(true);
    setGridSlices(prev => ({
      ...prev,
      [gridId]: [...(prev[gridId] || []), newSlice]
    }));
  };

  const removeSliceFromGrid = (gridId: string, sliceId: string) => {
    setSubsetsDirty(true);
    setGridSlices(prev => ({
      ...prev,
      [gridId]: (prev[gridId] || []).filter(s => s.id !== sliceId)
    }));
  };

  const updateGridSlice = (gridId: string, sliceId: string, updates: Partial<GridSlice>) => {
    setSubsetsDirty(true);
    setGridSlices(prev => ({
      ...prev,
      [gridId]: (prev[gridId] || []).map(s =>
        s.id === sliceId ? { ...s, ...updates } : s
      )
    }));
  };

  const commitSliceIndexDraft = (
    gridId: string,
    slice: GridSlice,
    maxIdx: number,
    options?: { applyAfterCommit?: boolean }
  ) => {
    const rawDraft = sliceIndexDrafts[slice.id];
    const parsed = Number.parseInt((rawDraft ?? '').trim(), 10);
    if (!Number.isFinite(parsed)) {
      setSliceIndexDrafts((prev) => {
        if (!(slice.id in prev)) return prev;
        const next = { ...prev };
        delete next[slice.id];
        return next;
      });
      return;
    }

    const oneBased = Math.max(1, Math.min(Math.max(1, maxIdx), parsed));
    const zeroBased = oneBased - 1;
    const nextGridSlices = zeroBased !== slice.index
      ? {
        ...gridSlices,
        [gridId]: (gridSlices[gridId] || []).map((existingSlice) =>
          existingSlice.id === slice.id ? { ...existingSlice, index: zeroBased } : existingSlice
        )
      }
      : gridSlices;

    if (zeroBased !== slice.index) {
      setSubsetsDirty(true);
      setGridSlices(nextGridSlices);
    }
    setSliceIndexDrafts((prev) => {
      if (!(slice.id in prev)) return prev;
      const next = { ...prev };
      delete next[slice.id];
      return next;
    });

    if (options?.applyAfterCommit) {
      void applyGuiManagedSubsets({ nextGridSlices });
    }
  };

  // Apply manual single contour level (called by Apply button or Enter key).
  const applyManualContourLevel = async () => {
    const parsed = Number.parseFloat(contourLevelDraft);
    const nextLevel = Number.isFinite(parsed) ? parsed : contourLevel;
    setContourLevel(nextLevel);
    setContourLevelDraft(String(nextLevel));
    await setPlotContourSpecForCurrentMode(buildContourSpecState({ mode: 'manual', level: nextLevel }));
  };

  // Build a ContourSpec-shaped object from current spec-mode state.
  const buildContourSpecState = (overrides?: Partial<{
    mode: ContourSpecMode; count: number; start: number; step: number; level: number;
  }>) => {
    const m = overrides?.mode ?? contourSpecMode;
    if (m === 'none') return { mode: 'none' as const };
    if (m === 'automatic') return { mode: 'automatic' as const, count: overrides?.count ?? contourAutoCount };
    if (m === 'increment') return { mode: 'increment' as const, start: overrides?.start ?? contourIncrStart, increment: overrides?.step ?? contourIncrStep };
    // manual
    return { mode: 'manual' as const, entries: [{ value: overrides?.level ?? contourLevel, color: null }] };
  };

  const setPlotContourSpecForCurrentMode = async (spec: ReturnType<typeof buildContourSpecState>) => {
    await setPlotContourSpec(spec);
    await commitPlot();
  };

  const applyAutomaticContourCount = async () => {
    const parsed = Number.parseFloat(contourAutoCountDraft);
    const nextCount = Math.max(1, Math.round(Number.isFinite(parsed) ? parsed : contourAutoCount));
    setContourAutoCount(nextCount);
    setContourAutoCountDraft(String(nextCount));
    await setPlotContourSpecForCurrentMode(buildContourSpecState({ mode: 'automatic', count: nextCount }));
  };

  const applyIncrementContourSpec = async () => {
    const parsedStart = Number.parseFloat(contourIncrStartDraft);
    const parsedStep = Number.parseFloat(contourIncrStepDraft);
    const nextStart = Number.isFinite(parsedStart) ? parsedStart : contourIncrStart;
    const nextStep = Number.isFinite(parsedStep) ? parsedStep : contourIncrStep;
    setContourIncrStart(nextStart);
    setContourIncrStep(nextStep);
    setContourIncrStartDraft(String(nextStart));
    setContourIncrStepDraft(String(nextStep));
    await setPlotContourSpecForCurrentMode(
      buildContourSpecState({ mode: 'increment', start: nextStart, step: nextStep })
    );
  };

  const handlePlotFamilyChange = async (family: BackendPlotFamily) => {
    setPlotFamilyState(family);
    await setPlotFamily(family);
    await commitPlot();
  };

  const handleContourAttributeChange = async (attr: BackendContourAttribute) => {
    setContourAttributeState(attr);
    await setPlotContourAttributeCmd(attr);
    await commitPlot();
  };

  // Mode selection is local-only; nothing is sent to the backend until Apply is clicked.
  const handleContourSpecModeChange = (mode: ContourSpecMode) => {
    setContourSpecMode(mode);
  };

  const applyContourSpecNone = async () => {
    await setPlotContourSpecForCurrentMode(buildContourSpecState({ mode: 'none' }));
  };

  // Debug: Log whenever loading state changes
  useEffect(() => {
    logger.info(`Loading state changed to: ${loading}`, 'App');
  }, [loading]);

  // Listen for loading events from Rust
  useEffect(() => {
    const setupListeners = async () => {
      const { listen } = await import('@tauri-apps/api/event');

      const unlistenStart = await listen<string>('loading-start', (event) => {
        logger.info(`Rust loading started: ${event.payload}`, 'App');
        setLoadingMessage(event.payload);
        setLoading(true);
      });

      const unlistenEnd = await listen('loading-end', () => {
        logger.info('Rust loading ended', 'App');
        setLoading(false);
        setLoadingMessage("Processing...");
      });

      return () => {
        unlistenStart();
        unlistenEnd();
      };
    };

    setupListeners();
  }, []);

  // Check if any grid has IBLANK data
  const hasIblankData = useMemo(() => {
    return grids.some((grid) => grid.hasIblank);
  }, [grids]);

  useEffect(() => {
    const setupMenu = async () => {
      try {
        const aboutItem = await MenuItem.new({
          id: "about",
          text: "About overview",
          action: () => {
            invoke("open_about_window").catch((err) =>
              logger.error(`Failed to open About window: ${err}`)
            );
          },
        });

        const ignoreIblankItem = await CheckMenuItem.new({
          id: "ignore-iblank",
          text: "Ignore IBLANK",
          enabled: hasIblankData,
          checked: ignoreIblank,
          action: () => {
            setIgnoreIblank((prev) => !prev);
          },
        });

        const showFringePointsItem = await CheckMenuItem.new({
          id: "show-fringe-points",
          text: "Show Fringe Points",
          enabled: hasIblankData && !ignoreIblank,
          checked: showFringePoints,
          action: () => {
            setShowFringePoints((prev) => !prev);
          },
        });

        // IBLANK filter mode selector (mutually exclusive)
        const vertexModeItem = await CheckMenuItem.new({
          id: "iblank-mode-vertex",
          text: "IBLANK Filter: Vertex Mode",
          enabled: hasIblankData,
          checked: iblankFilterMode === 'vertex',
          action: () => {
            setIblankFilterMode('vertex');
          },
        });

        const cellModeItem = await CheckMenuItem.new({
          id: "iblank-mode-cell",
          text: "IBLANK Filter: Cell Mode",
          enabled: hasIblankData,
          checked: iblankFilterMode === 'cell',
          action: () => {
            setIblankFilterMode('cell');
          },
        });

        // Wireframe option
        const wireframeItem = await CheckMenuItem.new({
          id: "show-wireframe",
          text: "Wireframe",
          checked: showWireframe,
          action: () => setShowWireframe((prev) => !prev),
        });

        // Separator
        const separator = await PredefinedMenuItem.new({ item: "Separator" });

        const smoothShadingItem = await CheckMenuItem.new({
          id: "shading-smooth",
          text: "Smooth Shading",
          checked: shadingMode === 'smooth',
          action: () => setShadingMode(shadingMode === 'smooth' ? 'none' : 'smooth'),
        });

        const fileSubmenu = await Submenu.new({
          text: "File",
          items: [aboutItem],
        });

        const viewSubmenu = await Submenu.new({
          text: "View",
          items: [
            ignoreIblankItem,
            showFringePointsItem,
            vertexModeItem,
            cellModeItem,
            separator,
            wireframeItem,
            smoothShadingItem,
          ],
        });

        const menu = await Menu.new({
          items: [fileSubmenu, viewSubmenu],
        });

        await menu.setAsAppMenu();
      } catch (err) {
        logger.error(`Failed to setup menu: ${err}`);
      }
    };

    setupMenu();
  }, [hasIblankData, ignoreIblank, showFringePoints, iblankFilterMode, showWireframe, shadingMode]);

  // Reset ignoreIblank when IBLANK data is no longer available
  useEffect(() => {
    if (!hasIblankData && ignoreIblank) {
      setIgnoreIblank(false);
    }
  }, [hasIblankData, ignoreIblank]);

  // Reset showFringePoints when IBLANK data is no longer available
  useEffect(() => {
    if (!hasIblankData && !showFringePoints) {
      setShowFringePoints(true);
    }
  }, [hasIblankData, showFringePoints]);

  const gridTree = useMemo(() => groupGridsByFile(grids), [grids]);
  const selectedGrids = useMemo(
    () => grids.filter((grid) => selectedGridIds.includes(grid.id)),
    [grids, selectedGridIds]
  );
  const anyGridHasSolution = useMemo(
    () => grids.some(grid => grid.hasSolution),
    [grids]
  );

  // Wrapper for color scheme changes to show loading indicator
  const handleColorSchemeChange = async (scheme: ColorScheme) => {
    // Rust will emit loading events
    setCurrentColorScheme(scheme);
  };

  // Wrapper for scalar field changes to show loading indicator
  const handleScalarFieldChange = async (field: ScalarField) => {
    // Rust will emit loading events
    setCurrentScalarField(field);
    await setPlotScalarField(field as BackendScalarField);
    await commitPlot();
  };

  const handleCameraCommit = async (vp: { x: number; y: number; z: number }) => {
    const current = backendPlotState?.viewpoint;
    if (current) {
      const dx = current.x - vp.x;
      const dy = current.y - vp.y;
      const dz = current.z - vp.z;
      if ((dx * dx + dy * dy + dz * dz) < 1e-8) {
        return;
      }
    }
    await setPlotViewpoint(vp);
  };

  const applyWallsRanges = async () => {
    const walls = manualWallsRows.map((row) => ({ ...normalizeSubsetForApply(row.subset), gui_managed: false }));
    await setPlotWalls(walls);
    await commitPlot();
    setManualWallsDirty(false);
  };

  const applyFsurface = async () => {
    if (!fsurfaceEnabled) {
      await setPlotFsurface(null);
      await commitPlot();
      return;
    }
    const parsedValue = Number.parseFloat(fsurfaceValueDraft);
    const value = Number.isFinite(parsedValue) ? parsedValue : 0;
    setFsurfaceValueDraft(String(value));
    await setPlotFsurface({ value, scalar_field: fsurfaceField });
    await commitPlot();
  };

  const toggleFsurfaceEnabled = async (enabled: boolean) => {
    setFsurfaceEnabled(enabled);
    if (!enabled) {
      await setPlotFsurface(null);
      await commitPlot();
      return;
    }
    const parsedValue = Number.parseFloat(fsurfaceValueDraft);
    const value = Number.isFinite(parsedValue) ? parsedValue : 0;
    setFsurfaceValueDraft(String(value));
    await setPlotFsurface({ value, scalar_field: fsurfaceField });
    await commitPlot();
  };

  const applyAddTextAnnotation = async () => {
    const content = textContentDraft.trim();
    if (!content) {
      return;
    }
    const parsedX = Number.parseFloat(textXDraft);
    const parsedY = Number.parseFloat(textYDraft);
    const x = Number.isFinite(parsedX) ? parsedX : 0.05;
    const y = Number.isFinite(parsedY) ? parsedY : 0.95;
    await addPlotTextAnnotation({ content, x, y });
    await commitPlot();
    setTextContentDraft('');
    setTextXDraft(String(x));
    setTextYDraft(String(y));
  };

  const applyGuiManagedSubsets = async (options?: {
    nextGridSlices?: Record<string, GridSlice[]>;
    nextSliceEnabled?: boolean;
  }) => {
    const effectiveGridSlices = options?.nextGridSlices ?? gridSlices;
    const effectiveSliceEnabled = options?.nextSliceEnabled ?? sliceEnabled;
    const nextSubsets = gridSlicesToBackendSubsets(effectiveGridSlices, grids, effectiveSliceEnabled);
    const manualSubsets = manualSubsetRows.map((row) => ({ ...normalizeSubsetForApply(row.subset), gui_managed: false }));
    const currentSubsets = backendPlotState?.subsets ?? [];
    const reconciledSubsets = dedupeSubsets([...manualSubsets, ...nextSubsets]);

    if (subsetsSignature(reconciledSubsets) === subsetsSignature(currentSubsets)) {
      setSubsetsDirty(false);
      return;
    }

    try {
      await setPlotSubsets(reconciledSubsets);
      await commitPlot();
      backendMappedSlicesSigRef.current = gridSlicesSignature(effectiveGridSlices);
      setSubsetsDirty(false);
      setManualSubsetDirty(false);
    } catch (e) {
      logger.error(`Failed to apply GUI-managed subsets: ${e}`, 'App');
    }
  };

  useEffect(() => {
    void syncPlotStateFromBackend();
  }, []);

  // Keep UI controls aligned with backend PlotState for migrated capabilities.
  useEffect(() => {
    if (!backendPlotState) {
      return;
    }

    setCurrentScalarField(backendPlotState.scalar_field as ScalarField);

    setPlotFamilyState(backendPlotState.plot_family ?? 'contour');
    setContourAttributeState(backendPlotState.contour_attribute ?? 'line');

    // Sync contour spec fields from backend state.
    const spec = backendPlotState.contour_spec;
    if (spec && typeof spec === 'object' && 'mode' in (spec as object)) {
      const s = spec as { mode: string; count?: number; start?: number; increment?: number; entries?: Array<{ value?: number }> };
      const mode = (s.mode as ContourSpecMode) ?? 'none';
      setContourSpecMode(mode);
      if (mode === 'automatic' && typeof s.count === 'number') {
        setContourAutoCount(s.count);
        setContourAutoCountDraft(String(s.count));
      } else if (mode === 'increment') {
        if (typeof s.start === 'number') {
          setContourIncrStart(s.start);
          setContourIncrStartDraft(String(s.start));
        }
        if (typeof s.increment === 'number') {
          setContourIncrStep(s.increment);
          setContourIncrStepDraft(String(s.increment));
        }
      } else if (mode === 'manual' && Array.isArray(s.entries) && s.entries.length > 0) {
        const val = s.entries[0]?.value;
        if (typeof val === 'number' && Number.isFinite(val)) {
          setContourLevel(val);
          setContourLevelDraft(String(val));
        }
      }
    }

    if (!subsetsDirty) {
      const backendSubsets = backendPlotState.subsets ?? [];
      const mappedSlices = backendSubsetsToGridSlices(backendSubsets, grids);
      const mappedSig = gridSlicesSignature(mappedSlices);
      backendMappedSlicesSigRef.current = mappedSig;
      if (mappedSig !== gridSlicesSignature(gridSlices)) {
        setGridSlices(mappedSlices);
      }
      if (backendSubsets.length > 0 && !sliceEnabled) {
        setSliceEnabled(true);
      }
    }

    if (!manualSubsetDirty) {
      const manual = (backendPlotState.subsets ?? []).filter((subset) => !subset.gui_managed);
      setManualSubsetRows(editableFromBackend(manual));
    }

    if (!manualWallsDirty) {
      setManualWallsRows(editableFromBackend(backendPlotState.walls ?? []));
    }

    const fs = backendPlotState.fsurface ?? null;
    if (fs) {
      setFsurfaceEnabled(true);
      setFsurfaceValueDraft(String(fs.value));
      setFsurfaceField(fs.scalar_field);
    } else {
      setFsurfaceEnabled(false);
    }
  }, [backendPlotState, grids, gridSlices, sliceEnabled, subsetsDirty, manualSubsetDirty, manualWallsDirty]);

  // Resolve contour levels for the ColorLegend whenever plot state or grids change.
  useEffect(() => {
    const spec = backendPlotState?.contour_spec;
    const refGrid = grids.find(g => g.solutionCacheId != null);
    const hasSpec = spec && typeof spec === 'object' && 'mode' in (spec as object) &&
      (spec as { mode: string }).mode !== 'none';

    if (!refGrid || !hasSpec) {
      setResolvedContourLevels([]);
      setContourFieldMin(0);
      setContourFieldMax(0);
      return;
    }

    void (async () => {
      try {
        const result = await invoke<{ levels: number[]; field_min: number; field_max: number }>(
          'resolve_contour_levels',
          { solutionId: refGrid.solutionCacheId!, scalarField: backendPlotState!.scalar_field }
        );
        setResolvedContourLevels(result.levels);
        setContourFieldMin(result.field_min);
        setContourFieldMax(result.field_max);
      } catch {
        setResolvedContourLevels([]);
      }
    })();
  }, [backendPlotState, grids]);

  const handleActualRangeChange = useCallback((min: number, max: number) => {
    setActualColorMapMin((prev) => (prev === min ? prev : min));
    setActualColorMapMax((prev) => (prev === max ? prev : max));
  }, []);

  // Callback from Viewer3D when its loading state changes.
  // Used by batch PNG export to wait for each render to settle.
  const handleViewer3DLoadingChange = (isLoading: boolean) => {
    viewer3dLoadingRef.current = isLoading;
    if (!isLoading) {
      const resolvers = viewer3dLoadingResolversRef.current.splice(0);
      resolvers.forEach(r => r());
    }
  };

  // Wait for Viewer3D to finish loading, then wait 3 animation frames for
  // Three.js to produce a stable rendered frame.
  const waitForViewer3DStable = (): Promise<void> => {
    const waitFrames = (n: number): Promise<void> =>
      new Promise(resolve => n <= 0 ? resolve() : requestAnimationFrame(() => void waitFrames(n - 1).then(resolve)));

    return new Promise<void>((resolve) => {
      const timeoutId = setTimeout(() => resolve(), 5000);
      const settle = () => { clearTimeout(timeoutId); void waitFrames(3).then(resolve); };
      if (!viewer3dLoadingRef.current) {
        settle();
      } else {
        viewer3dLoadingResolversRef.current.push(settle);
      }
    });
  };

  // Derive numbered PNG file paths from a base path chosen by the user.
  // e.g. "/out/scene.png" + 3 intents → ["/out/scene_001.png", "_002", "_003"]
  const derivePngPaths = (basePath: string, count: number): string[] => {
    if (count === 1) return [basePath];
    const withoutExt = basePath.replace(/\.png$/i, '');
    return Array.from({ length: count }, (_, i) =>
      `${withoutExt}_${String(i + 1).padStart(3, '0')}.png`
    );
  };

  async function loadFiles() {
    try {
      // Set loading state and wait for render
      logger.info('Setting loading state to TRUE', 'App');
      setLoadingMessage("Opening file dialog...");
      setLoading(true);
      setError("");

      // Use requestAnimationFrame to ensure UI updates before blocking dialog
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));

      logger.info('About to open file dialog', 'App');
      logger.info("Opening file selection dialog...");

      // Open file dialog for selecting one or more files
      const filePaths = await invoke<string[]>("open_multiple_files_dialog");

      logger.info(`File dialog returned with ${filePaths?.length || 0} files`, 'App');

      if (!filePaths || filePaths.length === 0) {
        logger.info('File dialog cancelled, setting loading to FALSE', 'App');
        setLoading(false);
        logger.debug("File dialog cancelled");
        return;
      }

      logger.info(`Loading ${filePaths.length} file(s)...`);
      setLoadingMessage(`Loading ${filePaths.length} file(s)...`);

      // Ensure UI updates
      await new Promise(resolve => requestAnimationFrame(resolve));

      // Clear backend caches before loading new files
      await invoke("clear_grid_cache");
      await invoke("clear_solution_cache_v2");

      // Try to load each file as a grid, collect successful grids
      const gridResults: { path: string; metadata: GridMetadata[]; fileName: string }[] = [];
      const potentialSolutionPaths: string[] = [];

      for (const path of filePaths) {
        try {
          const fileName = path.split(/[/\\]/).pop() || path;
          setLoadingMessage(`Parsing ${fileName}...`);
          // Ensure message renders
          await new Promise(resolve => requestAnimationFrame(resolve));
          const metadata = await invoke<GridMetadata[]>("load_plot3d_file_cached", { path });
          gridResults.push({ path, metadata, fileName });
          logger.info(`Loaded ${metadata.length} grid(s) from ${fileName}`);
        } catch (e) {
          // If it fails as a grid, it might be a solution file
          potentialSolutionPaths.push(path);
          logger.debug(`${path} is not a grid file, will try as solution`);
        }
      }

      if (gridResults.length === 0) {
        throw new Error("No valid grid files found in selection");
      }

      setLoadingMessage("Building grid structures...");
      await new Promise(resolve => requestAnimationFrame(resolve));

      // Build grid items from all loaded grid metadata
      const allGrids: GridItem[] = [];
      let colorOffset = 0;

      for (const { metadata } of gridResults) {
        const gridItems = buildGridItemsFromMetadata(metadata, colorOffset);
        allGrids.push(...gridItems);
        colorOffset += gridItems.length;
      }

      setGrids(allGrids);
      setSelectedGridIds([]);
      setIsolateSelected(false);
      setHasSolution(false);

      // Initialize gridSlices as empty - slices will be created on-demand when slicing is enabled
      setGridSlices({});

      // Try to load solution files
      if (potentialSolutionPaths.length > 0) {
        setLoadingMessage("Loading solution data...");
        await new Promise(resolve => requestAnimationFrame(resolve));
        for (const solPath of potentialSolutionPaths) {
          try {
            // Use v2 API to load and cache solutions
            const solutionMetadata = await invoke<SolutionMetadata[]>("load_plot3d_solution_cached", { path: solPath });

            const getStem = (nameOrPath: string) =>
              nameOrPath
                .split(/[/\\]/)
                .pop()
                ?.replace(/\.[^/.]+$/, '')
                .toLowerCase() ?? '';
            const solutionStem = getStem(solPath);
            const multipleGridFilesLoaded = new Set(allGrids.map((g) => g.filePath)).size > 1;

            // Build a deterministic mapping from solution metadata -> grid item ID
            const solutionToGrid = new Map<string, string>();
            const assignedGridIds = new Set<string>();

            // Validate dimensions for each grid
            for (const solMeta of solutionMetadata) {
              const candidates = allGrids.filter((g) => {
                if (g.gridIndex !== solMeta.grid_index) {
                  return false;
                }

                if (
                  solMeta.dimensions.i !== g.dimensions.i ||
                  solMeta.dimensions.j !== g.dimensions.j ||
                  solMeta.dimensions.k !== g.dimensions.k
                ) {
                  return false;
                }

                if (!multipleGridFilesLoaded) {
                  return true;
                }

                return getStem(g.fileName) === solutionStem;
              });

              const gridItem = candidates.find((candidate) => !assignedGridIds.has(candidate.id));

              if (!gridItem) {
                throw new Error(
                  `Unable to match solution grid ${solMeta.grid_index + 1} (${solMeta.dimensions.i}x${solMeta.dimensions.j}x${solMeta.dimensions.k}) to a loaded grid.`
                );
              }

              assignedGridIds.add(gridItem.id);
              solutionToGrid.set(solMeta.id, gridItem.id);
            }

            // Match solution IDs to grids
            setGrids((prevGrids) =>
              prevGrids.map((gridItem) => {
                const matchedSol = solutionMetadata.find((sol) => solutionToGrid.get(sol.id) === gridItem.id);
                if (matchedSol) {
                  return { ...gridItem, solutionCacheId: matchedSol.id, hasSolution: true };
                }
                return gridItem;
              })
            );

            setHasSolution(true);
            logger.info(`Successfully loaded ${solutionMetadata.length} solution(s) from ${solPath.split(/[/\\]/).pop()}`);
          } catch (e) {
            const errorMsg = String(e).replace(/^Error:\s*/, '');
            logger.error(errorMsg);
            throw new Error(errorMsg);
          }
        }
      }

      const metadata: FileMetadata = {
        fileNames: gridResults.map(r => r.fileName),
        gridCount: allGrids.length,
      };

      setFileMetadata(metadata);
      logger.info(`Loaded ${metadata.gridCount} total grid(s) from ${gridResults.length} file(s)`);
    } catch (e) {
      const errorMsg = String(e);
      setError(errorMsg);
      logger.error(errorMsg);
    } finally {
      logger.info('Finally block: setting loading to FALSE', 'App');
      setLoading(false);
    }
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh', overflow: 'hidden' }}>
      <header style={{
        background: '#1e293b',
        color: 'white',
        padding: '10px 20px',
        display: 'flex',
        alignItems: 'center',
        gap: '20px',
        flexWrap: 'wrap',
        flexShrink: 0
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '20px' }}>
          <h1 style={{ margin: 0, fontSize: '20px' }}>overview - PLOT3D Viewer</h1>
        </div>
        <div style={{ display: 'flex', gap: '10px' }}>
          <button
            onClick={loadFiles}
            disabled={loading}
            style={{
              padding: '8px 16px',
              cursor: loading ? 'not-allowed' : 'pointer',
              background: '#3b82f6',
              border: 'none',
              borderRadius: '4px',
              color: 'white',
              opacity: loading ? 0.7 : 1
            }}
          >
            {loading ? 'Loading...' : 'Load Files'}
          </button>
          <button
            onClick={() => setShowCommandWindow((prev) => !prev)}
            style={{
              padding: '8px 16px',
              cursor: 'pointer',
              background: showCommandWindow ? '#1d4ed8' : '#334155',
              border: 'none',
              borderRadius: '4px',
              color: 'white',
            }}
          >
            {showCommandWindow ? 'Hide Command Sidebar' : 'Show Command Sidebar'}
          </button>
          <button
            onClick={() => void exportCurrentViewAsPNG()}
            disabled={loading || exportInProgress}
            style={{
              padding: '8px 16px',
              cursor: (loading || exportInProgress) ? 'not-allowed' : 'pointer',
              background: '#f97316',
              border: 'none',
              borderRadius: '4px',
              color: 'white',
              opacity: (loading || exportInProgress) ? 0.6 : 1,
            }}
            title="Export current view as PNG"
          >
            {exportInProgress ? 'Exporting...' : 'Export View'}
          </button>
          {hasSolution && (
            <span style={{
              display: 'flex',
              alignItems: 'center',
              gap: '6px',
              padding: '8px 12px',
              background: '#10b981',
              borderRadius: '4px',
              color: 'white',
              fontSize: '13px'
            }}>
              ✓ Solution loaded
            </span>
          )}
        </div>
        {error && <span style={{ color: '#ef4444', fontSize: '14px' }}>{error}</span>}
        {fileMetadata && (
          <div style={{
            marginLeft: 'auto',
            fontSize: '14px',
          }}>
            <div>
              <strong>Files:</strong>{' '}
              {fileMetadata.fileNames.length === 1
                ? fileMetadata.fileNames[0]
                : `${fileMetadata.fileNames[0]} +${fileMetadata.fileNames.length - 1}`}
            </div>
            <div><strong>Grids:</strong> {fileMetadata.gridCount}</div>
          </div>
        )}
      </header>

      <main style={{ flex: 1, position: 'relative', display: 'flex', flexDirection: 'column', overflow: 'hidden', minHeight: 0 }}>
        <div style={{ flex: 1, position: 'relative', overflow: 'hidden', display: 'flex', minHeight: 0 }}>
          <aside
            style={{
              width: sidebarCollapsed ? '50px' : '280px',
              background: '#0f172a',
              color: '#e2e8f0',
              borderRight: '1px solid #1f2937',
              display: 'flex',
              flexDirection: 'column',
              padding: sidebarCollapsed ? '10px 6px' : '10px 14px 10px 10px',
              gap: '10px',
              overflow: 'auto',
              scrollbarGutter: 'stable both-edges',
              transition: 'width 0.3s ease'
            }}
          >
            <button
              onClick={() => setSidebarCollapsed(!sidebarCollapsed)}
              style={{
                background: 'transparent',
                border: 'none',
                color: '#cbd5e1',
                cursor: 'pointer',
                padding: '4px',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                fontSize: '16px',
                height: '32px'
              }}
              title={sidebarCollapsed ? 'Expand sidebar' : 'Collapse sidebar'}
            >
              {sidebarCollapsed ? '→' : '←'}
            </button>

            {!sidebarCollapsed && (
              <>
                {grids.length > 0 && (
                  <div>
                    <SolutionViewer
                      selectedGrid={anyGridHasSolution ? (grids.find(g => g.solution) || grids[0]) : null}
                      selectedField={currentScalarField}
                      selectedColorScheme={currentColorScheme}
                      onScalarFieldChange={handleScalarFieldChange}
                      onColorSchemeChange={handleColorSchemeChange}
                      contourLevels={resolvedContourLevels.length > 0 ? resolvedContourLevels : undefined}
                      contourFieldMin={contourFieldMin}
                      contourFieldMax={contourFieldMax}
                      colorMapMin={colorMapMin}
                      colorMapMax={colorMapMax}
                      actualMin={actualColorMapMin}
                      actualMax={actualColorMapMax}
                      onColorMapMinChange={setColorMapMin}
                      onColorMapMaxChange={setColorMapMax}
                    />
                  </div>
                )}

                {/* Contour Controls Section */}
                {hasSolution && (
                  <div style={{ marginBottom: '12px', paddingBottom: '12px', borderBottom: '2px solid #334155' }}>
                    <div style={{
                      fontSize: '10px',
                      fontWeight: '600',
                      color: '#cbd5e1',
                      textTransform: 'uppercase',
                      letterSpacing: '0.05em',
                      marginBottom: '6px',
                      paddingBottom: '4px',
                      borderBottom: '1px solid #334155'
                    }}>
                      Contours
                    </div>

                    <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', fontSize: '11px' }}>

                      {/* PLOT family */}
                      <label style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                        <span style={{ fontSize: '10px', color: '#94a3b8' }}>PLOT Family:</span>
                        <select
                          value={plotFamilyState}
                          onChange={(e) => {
                            void handlePlotFamilyChange(e.target.value as BackendPlotFamily);
                          }}
                          style={{ padding: '4px 6px', background: '#1a2640', color: '#e2e8f0', border: '1px solid #334155', borderRadius: '3px', fontSize: '11px' }}
                        >
                          <option value="contour">CONTOUR</option>
                          <option value="function_surface">SURFACE/CARPET/LINE</option>
                        </select>
                      </label>

                      {plotFamilyState === 'contour' && (
                        <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>

                          {/* Contour Attribute */}
                          <label style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                            <span style={{ fontSize: '10px', color: '#94a3b8' }}>CONTOURS Attribute:</span>
                            <select
                              value={contourAttributeState}
                              onChange={(e) => {
                                void handleContourAttributeChange(e.target.value as BackendContourAttribute);
                              }}
                              style={{ padding: '4px 6px', background: '#1a2640', color: '#e2e8f0', border: '1px solid #334155', borderRadius: '3px', fontSize: '11px' }}
                            >
                              <option value="line">LINE</option>
                              <option value="surface">SURFACE</option>
                              <option value="grid">GRID</option>
                              <option value="color_contours">COLOR CONTOURS</option>
                              <option value="dots">DOTS</option>
                            </select>
                          </label>

                          {/* Contour Spec Mode */}
                          <label style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                            <span style={{ fontSize: '10px', color: '#94a3b8' }}>CONTOURS Levels:</span>
                            <select
                              value={contourSpecMode}
                              onChange={(e) => {
                                handleContourSpecModeChange(e.target.value as ContourSpecMode);
                              }}
                              style={{ padding: '4px 6px', background: '#1a2640', color: '#e2e8f0', border: '1px solid #334155', borderRadius: '3px', fontSize: '11px' }}
                            >
                              <option value="none">NONE</option>
                              <option value="automatic">AUTOMATIC</option>
                              <option value="increment">INCREMENT</option>
                              <option value="manual">MANUAL</option>
                            </select>
                          </label>

                          {/* None mode: explicit Apply so the backend spec is cleared */}
                          {contourSpecMode === 'none' && (
                            <button
                              type="button"
                              onClick={() => { void applyContourSpecNone(); }}
                              style={{ padding: '4px 6px', background: '#334155', color: '#e2e8f0', border: '1px solid #475569', borderRadius: '3px', fontSize: '11px', cursor: 'pointer' }}
                            >
                              Apply
                            </button>
                          )}

                          {/* Automatic mode: count */}
                          {contourSpecMode === 'automatic' && (
                            <label style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                              <span style={{ fontSize: '10px', color: '#94a3b8' }}>Count:</span>
                              <input
                                type="number"
                                min="1"
                                step="1"
                                value={contourAutoCountDraft}
                                onChange={(e) => {
                                  setContourAutoCountDraft(e.target.value);
                                }}
                                onKeyDown={(e) => {
                                  if (e.key === 'Enter') {
                                    void applyAutomaticContourCount();
                                  }
                                }}
                                style={{ padding: '4px 6px', background: '#1a2640', color: '#e2e8f0', border: '1px solid #334155', borderRadius: '3px', fontSize: '11px' }}
                              />
                              <button
                                type="button"
                                onClick={() => {
                                  void applyAutomaticContourCount();
                                }}
                                style={{ padding: '4px 6px', background: '#334155', color: '#e2e8f0', border: '1px solid #475569', borderRadius: '3px', fontSize: '11px', cursor: 'pointer' }}
                              >
                                Apply
                              </button>
                            </label>
                          )}

                          {/* Increment mode: start + step */}
                          {contourSpecMode === 'increment' && (
                            <>
                              <label style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                                <span style={{ fontSize: '10px', color: '#94a3b8' }}>Start:</span>
                                <input
                                  type="number"
                                  step="any"
                                  value={contourIncrStartDraft}
                                  onChange={(e) => {
                                    setContourIncrStartDraft(e.target.value);
                                  }}
                                  onKeyDown={(e) => {
                                    if (e.key === 'Enter') {
                                      void applyIncrementContourSpec();
                                    }
                                  }}
                                  style={{ padding: '4px 6px', background: '#1a2640', color: '#e2e8f0', border: '1px solid #334155', borderRadius: '3px', fontSize: '11px' }}
                                />
                              </label>
                              <label style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                                <span style={{ fontSize: '10px', color: '#94a3b8' }}>Step:</span>
                                <input
                                  type="number"
                                  min="0.000001"
                                  step="any"
                                  value={contourIncrStepDraft}
                                  onChange={(e) => {
                                    setContourIncrStepDraft(e.target.value);
                                  }}
                                  onKeyDown={(e) => {
                                    if (e.key === 'Enter') {
                                      void applyIncrementContourSpec();
                                    }
                                  }}
                                  style={{ padding: '4px 6px', background: '#1a2640', color: '#e2e8f0', border: '1px solid #334155', borderRadius: '3px', fontSize: '11px' }}
                                />
                              </label>
                              <button
                                type="button"
                                onClick={() => {
                                  void applyIncrementContourSpec();
                                }}
                                style={{ padding: '4px 6px', background: '#334155', color: '#e2e8f0', border: '1px solid #475569', borderRadius: '3px', fontSize: '11px', cursor: 'pointer' }}
                              >
                                Apply
                              </button>
                            </>
                          )}

                          {/* Manual mode: single level value — only applied on click/Enter */}
                          {contourSpecMode === 'manual' && (
                            <label style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                              <span style={{ fontSize: '10px', color: '#94a3b8' }}>Level:</span>
                              <input
                                type="number"
                                step="any"
                                value={contourLevelDraft}
                                onChange={(e) => { setContourLevelDraft(e.target.value); }}
                                onKeyDown={(e) => { if (e.key === 'Enter') { void applyManualContourLevel(); } }}
                                style={{ padding: '4px 6px', background: '#1a2640', color: '#e2e8f0', border: '1px solid #334155', borderRadius: '3px', fontSize: '11px' }}
                              />
                              <button
                                type="button"
                                onClick={() => { void applyManualContourLevel(); }}
                                style={{ padding: '4px 6px', background: '#334155', color: '#e2e8f0', border: '1px solid #475569', borderRadius: '3px', fontSize: '11px', cursor: 'pointer' }}
                              >
                                Apply
                              </button>
                            </label>
                          )}

                          {/* Surface opacity slider (SURFACE / COLOR CONTOURS attributes only) */}
                          {(contourAttributeState === 'surface' || contourAttributeState === 'color_contours') && (
                            <label style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                              <span style={{ fontSize: '10px', color: '#94a3b8' }}>SURFACE Opacity: {Math.round(isoSurfaceOpacity * 100)}%</span>
                              <input
                                type="range"
                                min="0"
                                max="1"
                                step="0.05"
                                value={isoSurfaceOpacity}
                                onChange={(e) => { setIsoSurfaceOpacity(parseFloat(e.target.value)); }}
                                style={{ accentColor: '#3b82f6' }}
                              />
                            </label>
                          )}

                        </div>
                      )}
                    </div>
                  </div>
                )}

                {/* Camera Controls Section */}
                <div style={{ marginBottom: '12px', paddingBottom: '12px', borderBottom: '2px solid #334155' }}>
                  <div
                    style={{
                      fontSize: '10px',
                      fontWeight: '600',
                      color: '#cbd5e1',
                      textTransform: 'uppercase',
                      letterSpacing: '0.05em',
                      marginBottom: '6px',
                      paddingBottom: '4px',
                      borderBottom: '1px solid #334155',
                    }}
                  >
                    Camera
                  </div>
                  <label style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                    <span style={{ fontSize: '10px', color: '#94a3b8' }}>View Preset:</span>
                    <select
                      value={backendPlotState?.axis_view ?? 'custom'}
                      onChange={(e) => {
                        const next = e.target.value as BackendAxisView;
                        void (async () => {
                          await setPlotAxisView(next);
                          await commitPlot();
                        })();
                      }}
                      style={{
                        padding: '4px 6px',
                        background: '#1a2640',
                        color: '#e2e8f0',
                        border: '1px solid #334155',
                        borderRadius: '3px',
                        fontSize: '11px',
                      }}
                    >
                      {AXIS_VIEW_OPTIONS.map((option) => (
                        <option key={option.value} value={option.value}>
                          {option.label}
                        </option>
                      ))}
                    </select>
                  </label>
                </div>

                {/* Arbitrary Planes Section */}
                <div style={{ marginBottom: '12px', paddingBottom: '12px', borderBottom: '2px solid #334155' }}>
                  <div style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                    marginBottom: '6px',
                    paddingBottom: '4px',
                    borderBottom: '1px solid #334155'
                  }}>
                    <span style={{ fontSize: '10px', fontWeight: '600', color: '#cbd5e1', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
                      Arbitrary Planes
                    </span>
                    <button
                      onClick={addArbitrarySlice}
                      style={{
                        padding: '2px 6px',
                        fontSize: '9px',
                        background: '#059669',
                        border: 'none',
                        color: 'white',
                        borderRadius: '3px',
                        cursor: 'pointer'
                      }}
                    >
                      +
                    </button>
                  </div>

                  {arbitrarySlices.length === 0 && (
                    <div style={{ fontSize: '9px', color: '#64748b', fontStyle: 'italic', padding: '2px 0' }}>
                      No planes
                    </div>
                  )}

                  {arbitrarySlices.map((slice) => (
                    <div
                      key={slice.id}
                      style={{
                        background: '#0a0e1a',
                        borderRadius: '3px',
                        padding: '4px',
                        marginBottom: '4px',
                        border: slice.enabled ? '1px solid #3b82f6' : '1px solid #334155'
                      }}
                    >
                      <div style={{ display: 'flex', gap: '3px', alignItems: 'center', marginBottom: '4px' }}>
                        <input
                          type="text"
                          value={slice.name}
                          onChange={(e) => updateArbitrarySlice(slice.id, { name: e.target.value })}
                          style={{
                            flex: 1,
                            padding: '1px 4px',
                            background: '#1a2640',
                            color: '#e2e8f0',
                            border: '1px solid #334155',
                            borderRadius: '2px',
                            fontSize: '9px',
                            minWidth: 0
                          }}
                        />
                        <button
                          onClick={() => toggleArbitrarySlice(slice.id)}
                          style={{
                            padding: '1px 5px',
                            fontSize: '9px',
                            background: slice.enabled ? '#3b82f6' : '#475569',
                            border: 'none',
                            color: 'white',
                            borderRadius: '2px',
                            cursor: 'pointer',
                            lineHeight: 1
                          }}
                        >
                          {slice.enabled ? '👁' : '⚫'}
                        </button>
                        <button
                          onClick={() => removeArbitrarySlice(slice.id)}
                          style={{
                            padding: '1px 4px',
                            background: 'transparent',
                            border: 'none',
                            color: '#ef4444',
                            cursor: 'pointer',
                            fontSize: '11px',
                            lineHeight: 1
                          }}
                        >
                          ✕
                        </button>
                      </div>

                      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: '2px', marginBottom: '3px' }}>
                        {['X', 'Y', 'Z'].map((axis, idx) => (
                          <input
                            key={axis}
                            type="text"
                            inputMode="decimal"
                            defaultValue={slice.planePoint[idx]}
                            onBlur={(e) => {
                              const parsed = parseFloat(e.target.value);
                              if (!isNaN(parsed)) {
                                const newPoint = [...slice.planePoint] as [number, number, number];
                                newPoint[idx] = parsed;
                                updateArbitrarySlice(slice.id, { planePoint: newPoint });
                              } else {
                                // Reset to current value if invalid
                                e.target.value = slice.planePoint[idx].toString();
                              }
                            }}
                            onKeyDown={(e) => {
                              if (e.key === 'Enter') {
                                const parsed = parseFloat(e.currentTarget.value);
                                if (!isNaN(parsed)) {
                                  const newPoint = [...slice.planePoint] as [number, number, number];
                                  newPoint[idx] = parsed;
                                  updateArbitrarySlice(slice.id, { planePoint: newPoint });
                                  applyArbitrarySlice(slice.id);
                                } else {
                                  e.currentTarget.value = slice.planePoint[idx].toString();
                                }
                              }
                            }}
                            placeholder={`P${axis}`}
                            title={`Point ${axis}`}
                            style={{
                              padding: '1px 2px',
                              background: '#1a2640',
                              color: '#e2e8f0',
                              border: '1px solid #334155',
                              borderRadius: '2px',
                              fontSize: '8px',
                              minWidth: 0,
                              textAlign: 'center'
                            }}
                          />
                        ))}
                      </div>

                      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: '2px' }}>
                        {['X', 'Y', 'Z'].map((axis, idx) => (
                          <input
                            key={axis}
                            type="text"
                            inputMode="decimal"
                            defaultValue={slice.planeNormal[idx]}
                            onBlur={(e) => {
                              const parsed = parseFloat(e.target.value);
                              if (!isNaN(parsed)) {
                                const newNormal = [...slice.planeNormal] as [number, number, number];
                                newNormal[idx] = parsed;
                                updateArbitrarySlice(slice.id, { planeNormal: newNormal });
                              } else {
                                // Reset to current value if invalid
                                e.target.value = slice.planeNormal[idx].toString();
                              }
                            }}
                            onKeyDown={(e) => {
                              if (e.key === 'Enter') {
                                const parsed = parseFloat(e.currentTarget.value);
                                if (!isNaN(parsed)) {
                                  const newNormal = [...slice.planeNormal] as [number, number, number];
                                  newNormal[idx] = parsed;
                                  updateArbitrarySlice(slice.id, { planeNormal: newNormal });
                                  applyArbitrarySlice(slice.id);
                                } else {
                                  e.currentTarget.value = slice.planeNormal[idx].toString();
                                }
                              }
                            }}
                            placeholder={`N${axis}`}
                            title={`Normal ${axis}`}
                            style={{
                              padding: '1px 2px',
                              background: '#1a2640',
                              color: '#e2e8f0',
                              border: '1px solid #334155',
                              borderRadius: '2px',
                              fontSize: '8px',
                              minWidth: 0,
                              textAlign: 'center'
                            }}
                          />
                        ))}
                      </div>

                      <button
                        onClick={() => {
                          if (slice.dirty) applyArbitrarySlice(slice.id);
                        }}
                        disabled={!slice.dirty}
                        style={{
                          width: '100%',
                          marginTop: '4px',
                          padding: '3px 6px',
                          fontSize: '9px',
                          background: slice.applied && !slice.dirty ? '#10b981' : '#059669',
                          border: 'none',
                          color: 'white',
                          borderRadius: '2px',
                          cursor: slice.dirty ? 'pointer' : 'not-allowed',
                          fontWeight: slice.applied && !slice.dirty ? 'bold' : 'normal',
                          opacity: slice.dirty ? 1 : 0.7
                        }}
                      >
                        {slice.applied && !slice.dirty ? '✓ Applied' : 'Apply'}
                      </button>
                    </div>
                  ))}
                </div>

                <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                  <strong style={{ fontSize: '14px', textTransform: 'uppercase', letterSpacing: '0.08em' }}>Grids</strong>
                  <div style={{ fontSize: '12px', color: '#94a3b8' }}>
                    {fileMetadata ? `${fileMetadata.gridCount} grid(s) loaded` : 'No grids loaded'}
                  </div>
                </div>

                <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                  <label style={{ display: 'flex', alignItems: 'center', gap: '8px', fontSize: '13px' }}>
                    <input
                      type="checkbox"
                      checked={isolateSelected}
                      onChange={(e) => setIsolateSelected(e.target.checked)}
                      disabled={selectedGridIds.length === 0}
                    />
                    Isolate selected
                  </label>
                  <button
                    onClick={() => {
                      setGrids((prev) => prev.map((grid) => ({ ...grid, visible: true })));
                      setIsolateSelected(false);
                      setSelectedGridIds([]);
                    }}
                    style={{
                      padding: '6px 10px',
                      fontSize: '12px',
                      background: '#1d4ed8',
                      border: 'none',
                      color: 'white',
                      borderRadius: '6px'
                    }}
                  >
                    Clear selection
                  </button>
                </div>

                {/* Slicing controls */}
                <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                  <strong style={{ fontSize: '14px', textTransform: 'uppercase', letterSpacing: '0.08em' }}>Slicing</strong>
                  <label style={{ display: 'flex', alignItems: 'center', gap: '8px', fontSize: '13px' }}>
                    <input
                      type="checkbox"
                      checked={sliceEnabled}
                      onChange={(e) => {
                        const next = e.target.checked;
                        setSliceEnabled(next);
                        setSubsetsDirty(true);
                      }}
                    />
                    Slicing {sliceEnabled ? '(enabled)' : '(disabled)'}
                  </label>
                  <button
                    onClick={() => {
                      void applyGuiManagedSubsets();
                    }}
                    disabled={!subsetsDirty}
                    style={{
                      width: '100%',
                      padding: '6px 10px',
                      fontSize: '12px',
                      background: subsetsDirty ? '#0284c7' : '#475569',
                      border: 'none',
                      color: 'white',
                      borderRadius: '6px',
                      cursor: subsetsDirty ? 'pointer' : 'not-allowed',
                    }}
                  >
                    {subsetsDirty ? 'Apply Slicing to PlotState' : 'Slicing in sync'}
                  </button>
                </div>

                <div style={{ marginTop: '10px', background: '#0b1120', padding: '10px', borderRadius: '8px', fontSize: '11px', display: 'flex', flexDirection: 'column', gap: '8px' }}>
                  <div style={{ fontWeight: 600 }}>Range-Based SUBSETS</div>
                  {manualSubsetRows.map((row) => (
                    <div key={row.id}>
                      {!row.editing ? (
                        <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
                          <span style={{ flex: 1, fontFamily: 'monospace', fontSize: '10px', background: '#1e293b', padding: '3px 6px', borderRadius: '4px', border: '1px solid #334155', color: '#cbd5e1', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                            {compactSubsetLabel(row.subset, dimensionsForGridNumber(row.subset.grid))}
                          </span>
                          <button
                            onClick={() => setRowEditing('subset', row.id, true)}
                            title="Edit"
                            style={{ padding: '2px 6px', background: '#1d4ed8', color: 'white', border: 'none', borderRadius: '3px', cursor: 'pointer', fontSize: '11px', flexShrink: 0 }}
                          >✎</button>
                          <button
                            onClick={() => removeManualRangeRow('subset', row.id)}
                            title="Remove"
                            style={{ padding: '2px 6px', background: '#7f1d1d', color: 'white', border: 'none', borderRadius: '3px', cursor: 'pointer', fontSize: '11px', flexShrink: 0 }}
                          >✕</button>
                        </div>
                      ) : (
                        <div style={{ border: '1px solid #334155', borderRadius: '6px', padding: '6px', display: 'grid', gap: '4px', overflow: 'hidden' }}>
                          <label style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
                            <span style={{ minWidth: '30px' }}>Grid</span>
                            <input
                              type="number"
                              min={1}
                              value={row.subset.grid}
                              onChange={(e) => {
                                const next = Math.max(1, Number.parseInt(e.target.value || '1', 10));
                                updateManualRangeRow('subset', row.id, (subset) => ({ ...subset, grid: next }));
                              }}
                              style={{ width: '50px', padding: '2px 4px', background: '#1a2640', color: '#e2e8f0', border: '1px solid #334155', borderRadius: '3px', fontSize: '11px' }}
                            />
                          </label>
                          {(['i_range', 'j_range', 'k_range'] as const).map((axis) => (
                            <div key={axis} style={{ display: 'grid', gridTemplateColumns: '16px 1fr 1fr', gap: '3px', alignItems: 'center' }}>
                              <span style={{ fontSize: '10px' }}>{axis[0].toUpperCase()}</span>
                              <input
                                type="number"
                                placeholder="start"
                                value={rangeStartString(row.subset[axis])}
                                onChange={(e) => updateManualAxisRange('subset', row.id, axis, 'start', e.target.value)}
                                style={{ width: '100%', minWidth: 0, padding: '2px 4px', background: '#1a2640', color: '#e2e8f0', border: '1px solid #334155', borderRadius: '3px', fontSize: '11px', boxSizing: 'border-box' }}
                              />
                              <input
                                type="number"
                                placeholder="end"
                                value={rangeEndString(row.subset[axis])}
                                onChange={(e) => updateManualAxisRange('subset', row.id, axis, 'end', e.target.value)}
                                style={{ width: '100%', minWidth: 0, padding: '2px 4px', background: '#1a2640', color: '#e2e8f0', border: '1px solid #334155', borderRadius: '3px', fontSize: '11px', boxSizing: 'border-box' }}
                              />
                            </div>
                          ))}
                          <div style={{ display: 'flex', gap: '4px' }}>
                            <button
                              onClick={() => setRowEditing('subset', row.id, false)}
                              style={{ flex: 1, padding: '3px 6px', background: '#374151', color: 'white', border: 'none', borderRadius: '4px', cursor: 'pointer', fontSize: '11px' }}
                            >Done</button>
                            <button
                              onClick={() => removeManualRangeRow('subset', row.id)}
                              style={{ flex: 1, padding: '3px 6px', background: '#7f1d1d', color: 'white', border: 'none', borderRadius: '4px', cursor: 'pointer', fontSize: '11px' }}
                            >Remove</button>
                          </div>
                        </div>
                      )}
                    </div>
                  ))}
                  <div style={{ display: 'flex', gap: '6px' }}>
                    <button
                      onClick={() => addManualRangeRow('subset')}
                      style={{ flex: 1, padding: '5px 6px', background: '#1d4ed8', color: 'white', border: 'none', borderRadius: '4px', cursor: 'pointer', fontSize: '11px' }}
                    >
                      Add Subset Range
                    </button>
                    <button
                      onClick={() => void applyGuiManagedSubsets()}
                      disabled={!manualSubsetDirty && !subsetsDirty}
                      style={{ flex: 1, padding: '5px 6px', background: (!manualSubsetDirty && !subsetsDirty) ? '#475569' : '#0284c7', color: 'white', border: 'none', borderRadius: '4px', cursor: (!manualSubsetDirty && !subsetsDirty) ? 'not-allowed' : 'pointer', fontSize: '11px' }}
                    >
                      Apply SUBSETS
                    </button>
                  </div>
                </div>

                <div style={{ marginTop: '10px', background: '#0b1120', padding: '10px', borderRadius: '8px', fontSize: '11px', display: 'flex', flexDirection: 'column', gap: '8px' }}>
                  <div style={{ fontWeight: 600 }}>Range-Based WALLS</div>
                  {manualWallsRows.map((row) => (
                    <div key={row.id}>
                      {!row.editing ? (
                        <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
                          <span style={{ flex: 1, fontFamily: 'monospace', fontSize: '10px', background: '#1e293b', padding: '3px 6px', borderRadius: '4px', border: '1px solid #334155', color: '#cbd5e1', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                            {compactSubsetLabel(row.subset, dimensionsForGridNumber(row.subset.grid))}
                          </span>
                          <button
                            onClick={() => setRowEditing('wall', row.id, true)}
                            title="Edit"
                            style={{ padding: '2px 6px', background: '#1d4ed8', color: 'white', border: 'none', borderRadius: '3px', cursor: 'pointer', fontSize: '11px', flexShrink: 0 }}
                          >✎</button>
                          <button
                            onClick={() => removeManualRangeRow('wall', row.id)}
                            title="Remove"
                            style={{ padding: '2px 6px', background: '#7f1d1d', color: 'white', border: 'none', borderRadius: '3px', cursor: 'pointer', fontSize: '11px', flexShrink: 0 }}
                          >✕</button>
                        </div>
                      ) : (
                        <div style={{ border: '1px solid #334155', borderRadius: '6px', padding: '6px', display: 'grid', gap: '4px', overflow: 'hidden' }}>
                          <label style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
                            <span style={{ minWidth: '30px' }}>Grid</span>
                            <input
                              type="number"
                              min={1}
                              value={row.subset.grid}
                              onChange={(e) => {
                                const next = Math.max(1, Number.parseInt(e.target.value || '1', 10));
                                updateManualRangeRow('wall', row.id, (subset) => ({ ...subset, grid: next }));
                              }}
                              style={{ width: '50px', padding: '2px 4px', background: '#1a2640', color: '#e2e8f0', border: '1px solid #334155', borderRadius: '3px', fontSize: '11px' }}
                            />
                          </label>
                          {(['i_range', 'j_range', 'k_range'] as const).map((axis) => (
                            <div key={axis} style={{ display: 'grid', gridTemplateColumns: '16px 1fr 1fr', gap: '3px', alignItems: 'center' }}>
                              <span style={{ fontSize: '10px' }}>{axis[0].toUpperCase()}</span>
                              <input
                                type="number"
                                placeholder="start"
                                value={rangeStartString(row.subset[axis])}
                                onChange={(e) => updateManualAxisRange('wall', row.id, axis, 'start', e.target.value)}
                                style={{ width: '100%', minWidth: 0, padding: '2px 4px', background: '#1a2640', color: '#e2e8f0', border: '1px solid #334155', borderRadius: '3px', fontSize: '11px', boxSizing: 'border-box' }}
                              />
                              <input
                                type="number"
                                placeholder="end"
                                value={rangeEndString(row.subset[axis])}
                                onChange={(e) => updateManualAxisRange('wall', row.id, axis, 'end', e.target.value)}
                                style={{ width: '100%', minWidth: 0, padding: '2px 4px', background: '#1a2640', color: '#e2e8f0', border: '1px solid #334155', borderRadius: '3px', fontSize: '11px', boxSizing: 'border-box' }}
                              />
                            </div>
                          ))}
                          <div style={{ display: 'flex', gap: '4px' }}>
                            <button
                              onClick={() => setRowEditing('wall', row.id, false)}
                              style={{ flex: 1, padding: '3px 6px', background: '#374151', color: 'white', border: 'none', borderRadius: '4px', cursor: 'pointer', fontSize: '11px' }}
                            >Done</button>
                            <button
                              onClick={() => removeManualRangeRow('wall', row.id)}
                              style={{ flex: 1, padding: '3px 6px', background: '#7f1d1d', color: 'white', border: 'none', borderRadius: '4px', cursor: 'pointer', fontSize: '11px' }}
                            >Remove</button>
                          </div>
                        </div>
                      )}
                    </div>
                  ))}
                  <div style={{ display: 'flex', gap: '6px' }}>
                    <button
                      onClick={() => addManualRangeRow('wall')}
                      style={{ flex: 1, padding: '5px 6px', background: '#1d4ed8', color: 'white', border: 'none', borderRadius: '4px', cursor: 'pointer', fontSize: '11px' }}
                    >
                      Add Wall Range
                    </button>
                    <button
                      onClick={() => { void applyWallsRanges(); }}
                      disabled={!manualWallsDirty}
                      style={{ flex: 1, padding: '5px 6px', background: manualWallsDirty ? '#0284c7' : '#475569', color: 'white', border: 'none', borderRadius: '4px', cursor: manualWallsDirty ? 'pointer' : 'not-allowed', fontSize: '11px' }}
                    >
                      Apply WALLS
                    </button>
                  </div>
                </div>

                {hasSolution && (
                  <div style={{ marginTop: '10px', background: '#0b1120', padding: '10px', borderRadius: '8px', fontSize: '11px', display: 'flex', flexDirection: 'column', gap: '8px' }}>
                    <div style={{ fontWeight: 600 }}>FSURFACE</div>
                    <div style={{ fontSize: '10px', color: '#94a3b8' }}>
                      Legacy note: this control currently sets an iso-level plus scalar field, not full legacy FSURFACE axis-property semantics.
                    </div>
                    <label style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                      <input
                        type="checkbox"
                        checked={fsurfaceEnabled}
                        onChange={(e) => {
                          void toggleFsurfaceEnabled(e.target.checked);
                        }}
                      />
                      Enabled
                    </label>
                    <label style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                      <span style={{ fontSize: '10px', color: '#94a3b8' }}>Iso-Level (current)</span>
                      <input
                        type="number"
                        step="any"
                        value={fsurfaceValueDraft}
                        onChange={(e) => setFsurfaceValueDraft(e.target.value)}
                        style={{ padding: '4px 6px', background: '#1a2640', color: '#e2e8f0', border: '1px solid #334155', borderRadius: '3px', fontSize: '11px' }}
                      />
                    </label>
                    <label style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                      <span style={{ fontSize: '10px', color: '#94a3b8' }}>FUNCTION (scalar field)</span>
                      <select
                        value={fsurfaceField}
                        onChange={(e) => setFsurfaceField(e.target.value as BackendScalarField)}
                        style={{ padding: '4px 6px', background: '#1a2640', color: '#e2e8f0', border: '1px solid #334155', borderRadius: '3px', fontSize: '11px' }}
                      >
                        <option value="density">Density</option>
                        <option value="velocity_magnitude">Velocity Magnitude</option>
                        <option value="momentum_x">Momentum X</option>
                        <option value="momentum_y">Momentum Y</option>
                        <option value="momentum_z">Momentum Z</option>
                        <option value="pressure">Pressure</option>
                        <option value="energy">Energy</option>
                      </select>
                    </label>
                    <button
                      onClick={() => { void applyFsurface(); }}
                      style={{ padding: '5px 6px', background: '#0369a1', color: 'white', border: 'none', borderRadius: '4px', cursor: 'pointer', fontSize: '11px' }}
                    >
                      Apply FSURFACE
                    </button>
                  </div>
                )}

                <div style={{ marginTop: '10px', background: '#0b1120', padding: '10px', borderRadius: '8px', fontSize: '11px', display: 'flex', flexDirection: 'column', gap: '10px' }}>
                  <div style={{ fontWeight: 600 }}>TEXT Annotations</div>
                  <input
                    type="text"
                    value={textContentDraft}
                    onChange={(e) => setTextContentDraft(e.target.value)}
                    placeholder="Annotation text"
                    style={{ width: '100%', minWidth: 0, boxSizing: 'border-box', minHeight: '34px', lineHeight: 1.35, padding: '6px 8px', background: '#1a2640', color: '#e2e8f0', border: '1px solid #334155', borderRadius: '3px', fontSize: '12px' }}
                  />
                  <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '6px' }}>
                    <input
                      type="number"
                      step="any"
                      value={textXDraft}
                      onChange={(e) => setTextXDraft(e.target.value)}
                      placeholder="X (0..1)"
                      style={{ width: '100%', minWidth: 0, boxSizing: 'border-box', minHeight: '34px', lineHeight: 1.35, padding: '6px 8px', background: '#1a2640', color: '#e2e8f0', border: '1px solid #334155', borderRadius: '3px', fontSize: '12px' }}
                    />
                    <input
                      type="number"
                      step="any"
                      value={textYDraft}
                      onChange={(e) => setTextYDraft(e.target.value)}
                      placeholder="Y (0..1)"
                      style={{ width: '100%', minWidth: 0, boxSizing: 'border-box', minHeight: '34px', lineHeight: 1.35, padding: '6px 8px', background: '#1a2640', color: '#e2e8f0', border: '1px solid #334155', borderRadius: '3px', fontSize: '12px' }}
                    />
                  </div>
                  <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '6px' }}>
                    <button
                      onClick={() => { void applyAddTextAnnotation(); }}
                      style={{ width: '100%', minWidth: 0, minHeight: '34px', padding: '6px 8px', background: '#1d4ed8', color: 'white', border: 'none', borderRadius: '4px', cursor: 'pointer', fontSize: '12px' }}
                    >
                      Add TEXT
                    </button>
                    <button
                      onClick={() => {
                        void (async () => {
                          await clearPlotTextAnnotations();
                          await commitPlot();
                        })();
                      }}
                      disabled={(backendPlotState?.text_annotations?.length ?? 0) === 0}
                      style={{ width: '100%', minWidth: 0, minHeight: '34px', padding: '6px 8px', background: (backendPlotState?.text_annotations?.length ?? 0) > 0 ? '#7f1d1d' : '#475569', color: 'white', border: 'none', borderRadius: '4px', cursor: (backendPlotState?.text_annotations?.length ?? 0) > 0 ? 'pointer' : 'not-allowed', fontSize: '12px' }}
                    >
                      Clear TEXT
                    </button>
                  </div>
                </div>

                <div style={{ marginTop: '10px', background: '#0b1120', padding: '10px', borderRadius: '8px', fontSize: '11px', display: 'flex', flexDirection: 'column', gap: '8px' }}>
                  <div style={{ fontWeight: 600 }}>SHOW Status</div>
                  <button
                    onClick={() => { void refreshShowStatus(); }}
                    style={{ padding: '5px 6px', background: '#0f766e', color: 'white', border: 'none', borderRadius: '4px', cursor: 'pointer', fontSize: '11px' }}
                  >
                    Refresh SHOW
                  </button>
                  <pre style={{ margin: 0, whiteSpace: 'pre-wrap', wordBreak: 'break-word', color: '#cbd5e1', background: '#020617', border: '1px solid #334155', borderRadius: '6px', padding: '8px' }}>
                    {showStatusOutput || 'No SHOW snapshot yet.'}
                  </pre>
                </div>

                {gridTree.length === 0 ? (
                  <div style={{ fontSize: '12px', color: '#94a3b8' }}>Load a PLOT3D file to view grids.</div>
                ) : (
                  gridTree.map((group) => {
                    const allVisible = group.grids.every((grid) => grid.visible);
                    return (
                      <details key={group.filePath} open={sliceEnabled} style={{ background: '#111827', borderRadius: '8px', padding: '8px' }}>
                        <summary style={{ cursor: 'pointer', listStyle: 'none' }}>
                          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: '8px' }}>
                            <div style={{ display: 'flex', flexDirection: 'column', gap: '2px' }}>
                              <span style={{ fontSize: '13px', fontWeight: 600 }}>{group.fileName}</span>
                              <span style={{ fontSize: '11px', color: '#94a3b8' }}>{group.grids.length} grid(s)</span>
                            </div>
                            <label style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '11px', color: '#cbd5f5' }}>
                              <input
                                type="checkbox"
                                checked={allVisible}
                                onChange={(e) => {
                                  const checked = e.target.checked;
                                  setGrids((prev) =>
                                    prev.map((grid) =>
                                      grid.filePath === group.filePath
                                        ? { ...grid, visible: checked }
                                        : grid
                                    )
                                  );
                                }}
                              />
                              All
                            </label>
                          </div>
                        </summary>
                        <div style={{ marginTop: '8px', display: 'flex', flexDirection: 'column', gap: '6px' }}>
                          {group.grids.map((grid) => {
                            const isSelected = selectedGridIds.includes(grid.id);
                            const dims = grid.dimensions;
                            return (
                              <div
                                key={grid.id}
                                style={{
                                  display: 'flex',
                                  flexDirection: 'column',
                                  gap: '4px',
                                  padding: '4px',
                                  borderRadius: '6px',
                                  background: isSelected ? 'rgba(148, 163, 184, 0.2)' : 'transparent',
                                }}
                              >
                                {/* Index-based slices dropdown */}
                                {sliceEnabled ? (
                                  <details className="slice-details">
                                    <summary style={{
                                      cursor: 'pointer',
                                      display: 'flex',
                                      alignItems: 'center',
                                      gap: '6px',
                                      listStyle: 'none',
                                      userSelect: 'none'
                                    }}>
                                      <span style={{
                                        fontSize: '10px',
                                        color: '#64748b',
                                        transition: 'transform 0.2s',
                                        display: 'inline-block',
                                        width: '12px'
                                      }}
                                        className="disclosure-arrow">▶</span>
                                      <input
                                        type="checkbox"
                                        checked={grid.visible}
                                        onChange={(e) => {
                                          e.stopPropagation();
                                          const checked = e.target.checked;
                                          setGrids((prev) =>
                                            prev.map((item) =>
                                              item.id === grid.id
                                                ? { ...item, visible: checked }
                                                : item
                                            )
                                          );
                                        }}
                                        onClick={(e) => e.stopPropagation()}
                                      />
                                      <button
                                        onClick={(e) => {
                                          e.stopPropagation();
                                          setSelectedGridIds((prev) => {
                                            // Toggle selection: if already selected, remove it; otherwise add it
                                            if (prev.includes(grid.id)) {
                                              return prev.filter(id => id !== grid.id);
                                            } else {
                                              return [...prev, grid.id];
                                            }
                                          });
                                        }}
                                        style={{
                                          flex: 1,
                                          background: 'transparent',
                                          border: 'none',
                                          color: '#e2e8f0',
                                          textAlign: 'left',
                                          padding: 0,
                                          cursor: 'pointer',
                                          fontSize: '12px'
                                        }}
                                      >
                                        <span style={{ display: 'inline-flex', alignItems: 'center', gap: '8px' }}>
                                          <span
                                            style={{
                                              width: '10px',
                                              height: '10px',
                                              borderRadius: '999px',
                                              background: grid.color,
                                              boxShadow: '0 0 0 1px rgba(15, 23, 42, 0.6)'
                                            }}
                                          />
                                          Grid {grid.gridIndex + 1}
                                        </span>
                                      </button>
                                      <span style={{ fontSize: '10px', color: '#64748b', whiteSpace: 'nowrap' }}>
                                        {getGridSlices(grid.id).length} index slice{getGridSlices(grid.id).length !== 1 ? 's' : ''}
                                      </span>
                                    </summary>
                                    <div style={{
                                      marginTop: '4px',
                                      display: 'flex',
                                      flexDirection: 'column',
                                      gap: '4px',
                                      padding: '6px',
                                      paddingRight: '12px',
                                      background: '#0a0e1a',
                                      borderRadius: '4px'
                                    }}>
                                      {getGridSlices(grid.id).map((slice) => {
                                        const maxIdx = slice.plane === 'I' ? dims.i : slice.plane === 'J' ? dims.j : dims.k;
                                        return (
                                          <div key={slice.id} style={{ display: 'flex', gap: '4px', alignItems: 'center', fontSize: '11px', color: '#cbd5e1' }}>
                                            <select
                                              value={slice.plane}
                                              onChange={(e) => updateGridSlice(grid.id, slice.id, { plane: e.target.value as 'I' | 'J' | 'K' })}
                                              style={{
                                                padding: '2px 4px',
                                                background: '#1a2640',
                                                color: '#e2e8f0',
                                                border: '1px solid #334155',
                                                borderRadius: '3px',
                                                fontSize: '10px'
                                              }}
                                            >
                                              <option value="I">I</option>
                                              <option value="J">J</option>
                                              <option value="K">K</option>
                                            </select>
                                            <input
                                              type="number"
                                              min={1}
                                              max={Math.max(1, maxIdx)}
                                              value={sliceIndexDrafts[slice.id] ?? String(slice.index + 1)}
                                              onChange={(e) => {
                                                const next = e.target.value;
                                                setSliceIndexDrafts((prev) => ({
                                                  ...prev,
                                                  [slice.id]: next,
                                                }));
                                              }}
                                              onKeyDown={(e) => {
                                                if (e.key === 'Enter') {
                                                  e.preventDefault();
                                                  commitSliceIndexDraft(grid.id, slice, maxIdx, { applyAfterCommit: true });
                                                }
                                                if (e.key === 'Escape') {
                                                  e.preventDefault();
                                                  setSliceIndexDrafts((prev) => {
                                                    if (!(slice.id in prev)) return prev;
                                                    const next = { ...prev };
                                                    delete next[slice.id];
                                                    return next;
                                                  });
                                                }
                                              }}
                                              onBlur={() => {
                                                // Per UX request, only commit on Enter; blur discards draft edits.
                                                setSliceIndexDrafts((prev) => {
                                                  if (!(slice.id in prev)) return prev;
                                                  const next = { ...prev };
                                                  delete next[slice.id];
                                                  return next;
                                                });
                                              }}
                                              style={{
                                                flex: 1,
                                                minWidth: '80px',
                                                padding: '2px 4px',
                                                background: '#1a2640',
                                                color: '#e2e8f0',
                                                border: '1px solid #334155',
                                                borderRadius: '3px',
                                                fontSize: '10px'
                                              }}
                                            />
                                            <span style={{ minWidth: '34px', textAlign: 'right', fontSize: '10px', color: '#94a3b8' }}>/ {Math.max(1, maxIdx)}</span>
                                            <button
                                              type="button"
                                              onClick={(e) => {
                                                e.preventDefault();
                                                e.stopPropagation();
                                                removeSliceFromGrid(grid.id, slice.id);
                                              }}
                                              style={{
                                                flex: '0 0 18px',
                                                background: 'transparent',
                                                border: 'none',
                                                color: '#ef4444',
                                                cursor: 'pointer',
                                                padding: '0 4px',
                                                fontSize: '12px'
                                              }}
                                            >
                                              ✕
                                            </button>
                                          </div>
                                        );
                                      })}
                                      <button
                                        onClick={() => addSliceToGrid(grid.id)}
                                        style={{
                                          marginTop: '4px',
                                          padding: '2px 6px',
                                          fontSize: '10px',
                                          background: '#1d4ed8',
                                          border: 'none',
                                          color: 'white',
                                          borderRadius: '3px',
                                          cursor: 'pointer'
                                        }}
                                      >
                                        + Add slice
                                      </button>
                                    </div>
                                  </details>
                                ) : (
                                  <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                                    <input
                                      type="checkbox"
                                      checked={grid.visible}
                                      onChange={(e) => {
                                        const checked = e.target.checked;
                                        setGrids((prev) =>
                                          prev.map((item) =>
                                            item.id === grid.id
                                              ? { ...item, visible: checked }
                                              : item
                                          )
                                        );
                                      }}
                                    />
                                    <button
                                      onClick={() => {
                                        setSelectedGridIds((prev) => {
                                          // Toggle selection: if already selected, remove it; otherwise add it
                                          if (prev.includes(grid.id)) {
                                            return prev.filter(id => id !== grid.id);
                                          } else {
                                            return [...prev, grid.id];
                                          }
                                        });
                                      }}
                                      style={{
                                        flex: 1,
                                        background: 'transparent',
                                        border: 'none',
                                        color: '#e2e8f0',
                                        textAlign: 'left',
                                        padding: 0,
                                        cursor: 'pointer',
                                        fontSize: '12px'
                                      }}
                                    >
                                      <span style={{ display: 'inline-flex', alignItems: 'center', gap: '8px' }}>
                                        <span
                                          style={{
                                            width: '10px',
                                            height: '10px',
                                            borderRadius: '999px',
                                            background: grid.color,
                                            boxShadow: '0 0 0 1px rgba(15, 23, 42, 0.6)'
                                          }}
                                        />
                                        Grid {grid.gridIndex + 1}
                                      </span>
                                    </button>
                                  </div>
                                )}
                              </div>
                            );
                          })}
                        </div>
                      </details>
                    );
                  })
                )}

                {selectedGrids.length > 0 && (
                  <div style={{ marginTop: 'auto', background: '#0b1120', padding: '10px', borderRadius: '8px', fontSize: '12px' }}>
                    <div style={{ fontWeight: 600, marginBottom: '6px' }}>
                      Selected {selectedGrids.length > 1 ? `grids (${selectedGrids.length})` : 'grid'}
                    </div>
                    {selectedGrids.map((grid, idx) => (
                      <div key={grid.id} style={{ marginBottom: idx < selectedGrids.length - 1 ? '8px' : '0', paddingBottom: idx < selectedGrids.length - 1 ? '8px' : '0', borderBottom: idx < selectedGrids.length - 1 ? '1px solid #1e293b' : 'none' }}>
                        <div style={{ color: '#cbd5f5' }}>File: {grid.fileName}</div>
                        <div style={{ color: '#cbd5f5' }}>Grid: {grid.gridIndex + 1}</div>
                        <div style={{ color: '#cbd5f5' }}>
                          Dimensions: {grid.dimensions.i}x{grid.dimensions.j}x{grid.dimensions.k}
                        </div>
                        {grid.hasSolution && (
                          <div style={{ color: '#10b981', marginTop: '4px', fontSize: '11px' }}>
                            ✓ Solution data loaded
                          </div>
                        )}
                      </div>
                    ))}
                  </div>
                )}

                <div style={{ marginTop: '10px', background: '#0b1120', padding: '10px', borderRadius: '8px', fontSize: '11px' }}>
                  <div style={{ fontWeight: 600, marginBottom: '6px' }}>Backend PlotState (Dev)</div>
                  <button
                    onClick={() => void syncPlotStateFromBackend()}
                    style={{
                      marginBottom: '8px',
                      width: '100%',
                      padding: '5px 6px',
                      background: '#1f2937',
                      color: '#e2e8f0',
                      border: '1px solid #334155',
                      borderRadius: '4px',
                      cursor: 'pointer',
                      fontSize: '11px',
                    }}
                  >
                    Refresh
                  </button>
                  <pre style={{ margin: 0, whiteSpace: 'pre-wrap', wordBreak: 'break-word', color: '#94a3b8' }}>
                    {backendPlotState ? JSON.stringify(backendPlotState, null, 2) : 'No backend state loaded yet.'}
                  </pre>
                  {backendDiagnostics.length > 0 && (
                    <div style={{ marginTop: '8px', color: '#facc15' }}>
                      Last diagnostics: {backendDiagnostics.length}
                    </div>
                  )}
                </div>
              </>
            )}

          </aside>

          <div style={{ flex: 1, position: 'relative', overflow: 'hidden' }}>
            <Viewer3D
              grids={grids}
              selectedGridIds={selectedGridIds}
              isolateSelected={isolateSelected}
              ignoreIblank={ignoreIblank}
              showFringePoints={showFringePoints}
              iblankFilterMode={iblankFilterMode}
              scalarField={currentScalarField}
              colorScheme={currentColorScheme}
              showWireframe={showWireframe}
              shadingMode={shadingMode}
              sliceEnabled={sliceEnabled}
              subsets={backendPlotState?.subsets ?? []}
              arbitrarySlices={arbitrarySlices}
              plotFamily={plotFamilyState}
              contourAttribute={contourAttributeState}
              contourSpec={backendPlotState?.contour_spec}
              isoSurfaceOpacity={isoSurfaceOpacity}
              cameraAxisView={backendPlotState?.axis_view ?? 'custom'}
              cameraViewpoint={backendPlotState?.viewpoint ?? null}
              cameraPlotUp={backendPlotState?.plot_up ?? null}
              onCameraCommit={handleCameraCommit}
              onLoadingChange={handleViewer3DLoadingChange}
              colorMapMin={colorMapMin}
              colorMapMax={colorMapMax}
              onActualRangeChange={handleActualRangeChange}
            />
          </div>

          {showCommandWindow && (
            <aside
              style={{
                width: 'min(420px, 42vw)',
                minWidth: '300px',
                maxWidth: '520px',
                background: '#0b1220',
                borderLeft: '1px solid #1f2937',
                color: '#e2e8f0',
                display: 'flex',
                flexDirection: 'column',
                padding: '12px',
                gap: '10px',
                overflow: 'auto',
              }}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <h2 style={{ margin: 0, fontSize: '16px' }}>PLOT3D Command Sidebar</h2>
                <button
                  onClick={() => setShowCommandWindow(false)}
                  style={{
                    padding: '6px 10px',
                    background: '#1f2937',
                    color: '#e2e8f0',
                    border: '1px solid #334155',
                    borderRadius: '4px',
                    cursor: 'pointer',
                  }}
                >
                  Close
                </button>
              </div>

              <div>
                <div style={{ fontSize: '12px', marginBottom: '6px', color: '#93c5fd' }}>Type commands:</div>
                <textarea
                  value={commandText}
                  onChange={(e) => setCommandText(e.target.value)}
                  spellCheck={false}
                  style={{
                    width: '100%',
                    minHeight: '170px',
                    boxSizing: 'border-box',
                    padding: '10px',
                    background: '#020617',
                    color: '#e2e8f0',
                    border: '1px solid #334155',
                    borderRadius: '6px',
                    fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace',
                    fontSize: '12px',
                  }}
                />
                <button
                  onClick={() => void runCommandText()}
                  style={{
                    marginTop: '8px',
                    padding: '7px 12px',
                    background: '#2563eb',
                    color: 'white',
                    border: 'none',
                    borderRadius: '4px',
                    cursor: 'pointer',
                  }}
                >
                  Execute Commands
                </button>
              </div>

              <div style={{ borderTop: '1px solid #334155', paddingTop: '10px' }}>
                <div style={{ fontSize: '12px', marginBottom: '6px', color: '#93c5fd' }}>Or run .com file:</div>
                <input
                  type="text"
                  value={comFilePath}
                  onChange={(e) => setComFilePath(e.target.value)}
                  placeholder="/absolute/path/to/script.com"
                  style={{
                    width: '100%',
                    boxSizing: 'border-box',
                    padding: '8px',
                    background: '#020617',
                    color: '#e2e8f0',
                    border: '1px solid #334155',
                    borderRadius: '4px',
                    fontSize: '12px',
                  }}
                />
                <button
                  onClick={() => void runComFile()}
                  style={{
                    marginTop: '8px',
                    padding: '7px 12px',
                    background: '#0f766e',
                    color: 'white',
                    border: 'none',
                    borderRadius: '4px',
                    cursor: 'pointer',
                  }}
                >
                  Execute .com File
                </button>
              </div>

              <div style={{ borderTop: '1px solid #334155', paddingTop: '10px' }}>
                <div style={{ fontSize: '12px', marginBottom: '6px', color: '#93c5fd' }}>Export PNGs:</div>
                {lastExecutionResult && lastExecutionResult.intents && lastExecutionResult.intents.length > 0 ? (
                  <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                    <div style={{ fontSize: '11px', color: '#cbd5e1' }}>
                      Ready to export {lastExecutionResult.intents.length} plot(s)
                    </div>
                    <button
                      onClick={() => void exportPNGsFromExecution()}
                      disabled={exportInProgress}
                      style={{
                        padding: '7px 12px',
                        background: exportInProgress ? '#475569' : '#7c2d12',
                        color: 'white',
                        border: 'none',
                        borderRadius: '4px',
                        cursor: exportInProgress ? 'not-allowed' : 'pointer',
                        fontSize: '12px',
                      }}
                    >
                      {exportInProgress ? 'Exporting...' : 'Export to PNG'}
                    </button>
                    {exportStatus && (
                      <div style={{
                        fontSize: '11px',
                        color: exportStatus.startsWith('Failed') ? '#ef4444' : '#86efac',
                        background: '#0a0a0a',
                        padding: '6px',
                        borderRadius: '4px',
                        border: '1px solid #334155',
                        wordBreak: 'break-word'
                      }}>
                        {exportStatus}
                      </div>
                    )}
                  </div>
                ) : (
                  <div style={{ fontSize: '11px', color: '#94a3b8' }}>
                    Execute a .com file to enable PNG export
                  </div>
                )}
              </div>

              <div style={{ borderTop: '1px solid #334155', paddingTop: '10px' }}>
                <div style={{ fontSize: '12px', marginBottom: '6px', color: '#93c5fd' }}>Output:</div>
                <pre style={{
                  margin: 0,
                  minHeight: '90px',
                  whiteSpace: 'pre-wrap',
                  wordBreak: 'break-word',
                  background: '#020617',
                  border: '1px solid #334155',
                  borderRadius: '6px',
                  padding: '10px',
                  color: '#cbd5e1',
                  fontSize: '12px',
                }}>
                  {commandWindowOutput || 'No output yet.'}
                </pre>
              </div>
            </aside>
          )}
        </div>
        <LogViewer isOpen={showLogs} onToggle={setShowLogs} />
      </main>

      <LoadingIndicator isLoading={loading} message={loadingMessage} />
    </div>
  );
}

export default App;
