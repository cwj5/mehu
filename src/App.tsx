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

import { useMemo, useState, useEffect } from "react";
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
  const [arbitrarySlices, setArbitrarySlices] = useState<ArbitrarySlice[]>([]);

  // Contour state
  const [contoursEnabled, setContoursEnabled] = useState(false);
  const [contourLevel, setContourLevel] = useState(0.5);
  const [contourLevelWarning, setContourLevelWarning] = useState("");
  const [contourDisplayMode, setContourDisplayMode] = useState<'surfaces' | 'lines' | 'both'>('both');


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
    setGridSlices(prev => ({
      ...prev,
      [gridId]: [...(prev[gridId] || []), newSlice]
    }));
  };

  const removeSliceFromGrid = (gridId: string, sliceId: string) => {
    setGridSlices(prev => ({
      ...prev,
      [gridId]: (prev[gridId] || []).filter(s => s.id !== sliceId)
    }));
  };

  const updateGridSlice = (gridId: string, sliceId: string, updates: Partial<GridSlice>) => {
    setGridSlices(prev => ({
      ...prev,
      [gridId]: (prev[gridId] || []).map(s =>
        s.id === sliceId ? { ...s, ...updates } : s
      )
    }));
  };

  // Contour level validation and clamping
  const handleContourLevelChange = (value: number) => {
    if (value < 0 || value > 1) {
      const clamped = Math.max(0, Math.min(1, value));
      setContourLevel(clamped);
      setContourLevelWarning(`Level clamped to valid range [0, 1]`);
      setTimeout(() => setContourLevelWarning(""), 3000);
    } else {
      setContourLevel(value);
      setContourLevelWarning("");
    }
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
  };

  // Callback from Viewer3D when it's done loading meshes
  const handleViewer3DLoadingChange = () => {
    // Viewer3D loading is now controlled by Rust events, ignore this callback
    logger.debug('Ignoring Viewer3D loading change (controlled by Rust)', 'App');
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
                      onScalarFieldChange={handleScalarFieldChange}
                      onColorSchemeChange={handleColorSchemeChange}
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
                      <label style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                        <input
                          type="checkbox"
                          checked={contoursEnabled}
                          onChange={(e) => setContoursEnabled(e.target.checked)}
                        />
                        Enable Contours
                      </label>

                      {contoursEnabled && (
                        <div style={{ paddingLeft: '8px', display: 'flex', flexDirection: 'column', gap: '8px' }}>
                          <label style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                            <span style={{ fontSize: '10px', color: '#94a3b8' }}>
                              Display:
                            </span>
                            <select
                              value={contourDisplayMode}
                              onChange={(e) => setContourDisplayMode(e.target.value as 'surfaces' | 'lines' | 'both')}
                              style={{
                                padding: '4px 6px',
                                background: '#1a2640',
                                color: '#e2e8f0',
                                border: '1px solid #334155',
                                borderRadius: '3px',
                                fontSize: '11px',
                              }}
                            >
                              <option value="both">Surfaces & Lines</option>
                              <option value="surfaces">Surfaces Only</option>
                              <option value="lines">Lines Only</option>
                            </select>
                          </label>
                          <label style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                            <span style={{ fontSize: '10px', color: '#94a3b8' }}>
                              Contour Level (0-1):
                            </span>
                            <input
                              type="number"
                              min="0"
                              max="1"
                              step="0.01"
                              value={contourLevel}
                              onChange={(e) => handleContourLevelChange(parseFloat(e.target.value) || 0)}
                              style={{
                                padding: '4px 6px',
                                background: '#1a2640',
                                color: '#e2e8f0',
                                border: '1px solid #334155',
                                borderRadius: '3px',
                                fontSize: '11px',
                              }}
                            />
                          </label>
                          {contourLevelWarning && (
                            <div style={{
                              marginTop: '4px',
                              padding: '4px',
                              background: '#422006',
                              color: '#fbbf24',
                              fontSize: '9px',
                              borderRadius: '2px',
                              border: '1px solid #78350f'
                            }}>
                              ⚠ {contourLevelWarning}
                            </div>
                          )}
                        </div>
                      )}
                    </div>
                  </div>
                )}

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
                      onChange={(e) => setSliceEnabled(e.target.checked)}
                    />
                    Slicing {sliceEnabled ? '(enabled)' : '(disabled)'}
                  </label>
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
                                              type="range"
                                              min={0}
                                              max={Math.max(0, maxIdx - 1)}
                                              value={slice.index}
                                              onChange={(e) => updateGridSlice(grid.id, slice.id, { index: parseInt(e.target.value) })}
                                              style={{ flex: 1, height: '12px', minWidth: '80px' }}
                                            />
                                            <span style={{ minWidth: '18px', textAlign: 'right' }}>{slice.index + 1}</span>
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
              gridSlices={gridSlices}
              arbitrarySlices={arbitrarySlices}
              contoursEnabled={contoursEnabled}
              contourLevel={contourLevel}
              contourDisplayMode={contourDisplayMode}
              onSlicesChange={setGridSlices}
              onLoadingChange={handleViewer3DLoadingChange}
            />
          </div>
        </div>
        <LogViewer isOpen={showLogs} onToggle={setShowLogs} />
      </main>
      <LoadingIndicator isLoading={loading} message={loadingMessage} />
    </div>
  );
}

export default App;
