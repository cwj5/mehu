import { Canvas } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';
import { useState, useEffect, useMemo, useRef } from 'react';
import * as THREE from 'three';
import { BufferGeometry, BufferAttribute, ShaderMaterial } from 'three';
import { LineSegments2 } from 'three/examples/jsm/lines/LineSegments2.js';
import { LineSegmentsGeometry } from 'three/examples/jsm/lines/LineSegmentsGeometry.js';
import { LineMaterial } from 'three/examples/jsm/lines/LineMaterial.js';
import { invoke } from '@tauri-apps/api/core';
import { logger } from '../utils/logger';
import type { GridItem, GridSlice, ArbitrarySlice } from '../types/grids';
import type { ColorScheme } from '../utils/colorMapping';
import type { ScalarField } from '../utils/solutionData';
import { getVisibleGridItems } from '../utils/gridUtils';

interface MeshGeometry {
    vertices: number[];
    indices: number[];
    triangle_indices: number[];
    normals: number[];
    vertex_count: number;
    face_count: number;
    colors?: number[];
}

interface SerializableGrid {
    dimensions: { i: number; j: number; k: number };
    x_coords: number[];
    y_coords: number[];
    z_coords: number[];
    iblank?: number[];
    original_indices?: number[]; // Maps sliced points back to original grid indices
}

interface Viewer3DProps {
    grids: GridItem[];
    selectedGridIds: string[];
    isolateSelected: boolean;
    ignoreIblank: boolean;
    showFringePoints: boolean;
    iblankFilterMode: 'vertex' | 'cell';
    scalarField?: ScalarField;
    colorScheme?: ColorScheme;
    showWireframe?: boolean;
    shadingMode?: 'none' | 'smooth';
    sliceEnabled?: boolean;
    gridSlices?: Record<string, GridSlice[]>;
    arbitrarySlices?: ArbitrarySlice[];
    contoursEnabled?: boolean;
    contourLevel?: number;
    contourDisplayMode?: 'surfaces' | 'lines' | 'both';
    onSlicesChange?: (slices: Record<string, GridSlice[]>) => void;
    onLoadingChange?: (isLoading: boolean) => void;
}

function SolidMeshRenderer({
    meshGeometry,
    color,
    dimmed,
}: {
    meshGeometry: MeshGeometry;
    color: string;
    dimmed: boolean;
}) {
    // Shader for field quantity colors (vertex colors) - both sides equally visible
    const vertexColorMaterial = useMemo(() => {
        return new ShaderMaterial({
            transparent: false,
            depthWrite: true,
            depthTest: true,
            side: 2, // DoubleSide
            uniforms: {
                opacity: { value: 1.0 },
            },
            vertexShader: `
                attribute vec3 color;
                varying vec3 vColor;
                varying vec3 vNormal;
                void main() {
                    vColor = color;
                    vNormal = normalize(normalMatrix * normal);
                    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
                }
            `,
            fragmentShader: `
                uniform float opacity;
                varying vec3 vColor;
                varying vec3 vNormal;
                void main() {
                    // Multiple light sources for better global illumination
                    vec3 light1 = normalize(vec3(0.5, 0.5, 1.0));
                    vec3 light2 = normalize(vec3(-0.5, -0.3, 0.8));
                    vec3 light3 = normalize(vec3(0.0, 1.0, 0.3));
                    vec3 normal = normalize(vNormal);
                    
                    // Check if this is a backface
                    float facing = gl_FrontFacing ? 1.0 : -1.0;
                    normal *= facing;
                    
                    // Apply lighting from multiple sources
                    float diffuse1 = max(dot(normal, light1), 0.0);
                    float diffuse2 = max(dot(normal, light2), 0.0) * 0.5;
                    float diffuse3 = max(dot(normal, light3), 0.0) * 0.3;
                    float diffuse = diffuse1 + diffuse2 + diffuse3;
                    
                    // Both sides equally visible for field quantity visualization
                    diffuse = max(diffuse, 0.7);
                    
                    vec3 finalColor = vColor * diffuse;
                    gl_FragColor = vec4(finalColor, opacity);
                }
            `,
        });
    }, []);

    // Shader for grid ID colors (solid color) - backfaces darker for depth perception
    const solidColorMaterial = useMemo(() => {
        const hexColor = parseInt(color.replace('#', ''), 16);
        const r = ((hexColor >> 16) & 255) / 255;
        const g = ((hexColor >> 8) & 255) / 255;
        const b = (hexColor & 255) / 255;

        return new ShaderMaterial({
            transparent: false,
            depthWrite: true,
            depthTest: true,
            side: 2, // DoubleSide
            uniforms: {
                opacity: { value: 1.0 },
                baseColor: { value: [r, g, b] },
            },
            vertexShader: `
                varying vec3 vNormal;
                void main() {
                    vNormal = normalize(normalMatrix * normal);
                    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
                }
            `,
            fragmentShader: `
                uniform float opacity;
                uniform vec3 baseColor;
                varying vec3 vNormal;
                void main() {
                    // Multiple light sources for better global illumination
                    vec3 light1 = normalize(vec3(0.5, 0.5, 1.0));
                    vec3 light2 = normalize(vec3(-0.5, -0.3, 0.8));
                    vec3 light3 = normalize(vec3(0.0, 1.0, 0.3));
                    vec3 normal = normalize(vNormal);
                    
                    // Check if this is a backface
                    float facing = gl_FrontFacing ? 1.0 : -1.0;
                    normal *= facing;
                    
                    // Apply lighting from multiple sources
                    float diffuse1 = max(dot(normal, light1), 0.0);
                    float diffuse2 = max(dot(normal, light2), 0.0) * 0.5;
                    float diffuse3 = max(dot(normal, light3), 0.0) * 0.3;
                    float diffuse = diffuse1 + diffuse2 + diffuse3;
                    
                    // Differentiate front and back faces for depth perception
                    if (gl_FrontFacing) {
                        diffuse = max(diffuse, 0.7); // Front faces have ambient
                    } else {
                        diffuse *= 0.3; // Backfaces are darker
                    }
                    
                    vec3 finalColor = baseColor * diffuse;
                    gl_FragColor = vec4(finalColor, opacity);
                }
            `,
        });
    }, [color]);

    useEffect(() => {
        vertexColorMaterial.transparent = dimmed;
        vertexColorMaterial.depthWrite = !dimmed;
        vertexColorMaterial.uniforms.opacity.value = dimmed ? 0.35 : 1.0;
        vertexColorMaterial.needsUpdate = true;
    }, [dimmed, vertexColorMaterial]);

    useEffect(() => {
        solidColorMaterial.transparent = dimmed;
        solidColorMaterial.depthWrite = !dimmed;
        solidColorMaterial.uniforms.opacity.value = dimmed ? 0.35 : 1.0;
        solidColorMaterial.needsUpdate = true;
    }, [dimmed, solidColorMaterial]);

    const geometry = useMemo(() => {
        const geo = new BufferGeometry();
        geo.setAttribute(
            'position',
            new BufferAttribute(new Float32Array(meshGeometry.vertices), 3)
        );

        // Add normals for smooth shading
        geo.setAttribute(
            'normal',
            new BufferAttribute(new Float32Array(meshGeometry.normals), 3)
        );

        const colors = meshGeometry.colors;
        const hasColors = !!colors && colors.length === meshGeometry.vertices.length;

        // Add vertex colors if available and length matches vertices
        if (hasColors) {
            let colorArray = colors;

            // Detect 0-255 color data and normalize to 0-1 if needed
            let maxSample = 0;
            const sampleCount = Math.min(colors.length, 3000);
            for (let i = 0; i < sampleCount; i += 1) {
                const v = colors[i];
                if (v > maxSample) maxSample = v;
            }

            if (maxSample > 1.0) {
                const normalized = new Float32Array(colors.length);
                for (let i = 0; i < colors.length; i += 1) {
                    normalized[i] = colors[i] / 255.0;
                }
                colorArray = Array.from(normalized);
            }

            geo.setAttribute(
                'color',
                new BufferAttribute(new Float32Array(colorArray), 3)
            );
        }

        geo.setIndex(new BufferAttribute(new Uint32Array(meshGeometry.triangle_indices), 1));

        // Compute bounding sphere for frustum culling
        geo.computeBoundingSphere();

        return geo;
    }, [meshGeometry]);

    // Use vertex colors if available, otherwise use single color
    const hasColors = !!meshGeometry.colors && meshGeometry.colors.length === meshGeometry.vertices.length;

    return (
        <mesh geometry={geometry} frustumCulled={true}>
            {hasColors ? (
                <primitive object={vertexColorMaterial} attach="material" />
            ) : (
                <primitive object={solidColorMaterial} attach="material" />
            )}
        </mesh>
    );
}

function MeshRenderer({
    meshGeometry,
    color,
    dimmed,
}: {
    meshGeometry: MeshGeometry;
    color: string;
    dimmed: boolean;
}) {
    const vertexColorMaterial = useMemo(() => {
        return new ShaderMaterial({
            transparent: true,
            uniforms: {
                opacity: { value: 1.0 },
            },
            vertexShader: `
                attribute vec3 color;
                varying vec3 vColor;
                void main() {
                    vColor = color;
                    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
                }
            `,
            fragmentShader: `
                uniform float opacity;
                varying vec3 vColor;
                void main() {
                    gl_FragColor = vec4(vColor, opacity);
                }
            `,
        });
    }, []);

    useEffect(() => {
        vertexColorMaterial.uniforms.opacity.value = dimmed ? 0.35 : 1.0;
        vertexColorMaterial.needsUpdate = true;
    }, [dimmed, vertexColorMaterial]);

    const geometry = useMemo(() => {
        const geo = new BufferGeometry();
        geo.setAttribute(
            'position',
            new BufferAttribute(new Float32Array(meshGeometry.vertices), 3)
        );

        const colors = meshGeometry.colors;
        const hasColors = !!colors && colors.length === meshGeometry.vertices.length;

        // Add vertex colors if available and length matches vertices
        if (hasColors) {
            let colorArray = colors;

            // Detect 0-255 color data and normalize to 0-1 if needed
            let maxSample = 0;
            const sampleCount = Math.min(colors.length, 3000);
            for (let i = 0; i < sampleCount; i += 1) {
                const v = colors[i];
                if (v > maxSample) maxSample = v;
            }

            if (maxSample > 1.0) {
                const normalized = new Float32Array(colors.length);
                for (let i = 0; i < colors.length; i += 1) {
                    normalized[i] = colors[i] / 255.0;
                }
                colorArray = Array.from(normalized);
                logger.warn('Detected 0-255 color data. Normalizing to 0-1.', 'MeshRenderer');
            }

            geo.setAttribute(
                'color',
                new BufferAttribute(new Float32Array(colorArray), 3)
            );
        } else if (colors && colors.length > 0) {
            logger.warn(
                `Color array length (${colors.length}) does not match vertex array length (${meshGeometry.vertices.length}). Ignoring colors.`,
                'MeshRenderer'
            );
        }

        geo.setIndex(new BufferAttribute(new Uint32Array(meshGeometry.indices), 1));

        // Compute bounding sphere for frustum culling
        geo.computeBoundingSphere();

        return geo;
    }, [meshGeometry]);

    // Use vertex colors if available, otherwise use single color
    const hasColors = !!meshGeometry.colors && meshGeometry.colors.length === meshGeometry.vertices.length;

    return (
        <lineSegments geometry={geometry} frustumCulled={true}>
            {hasColors ? (
                <primitive object={vertexColorMaterial} attach="material" />
            ) : (
                <lineBasicMaterial
                    color={color}
                    transparent={dimmed}
                    opacity={dimmed ? 0.35 : 1}
                />
            )}
        </lineSegments>
    );
}

// Iso-surface renderer (solid single-color mesh)
function IsoSurfaceRenderer({ meshGeometry, color }: { meshGeometry: MeshGeometry; color: string }) {
    const geometry = useMemo(() => {
        const geo = new BufferGeometry();
        geo.setAttribute('position', new BufferAttribute(new Float32Array(meshGeometry.vertices), 3));
        geo.setAttribute('normal', new BufferAttribute(new Float32Array(meshGeometry.normals), 3));
        geo.setIndex(new BufferAttribute(new Uint32Array(meshGeometry.triangle_indices), 1));
        geo.computeBoundingSphere();
        return geo;
    }, [meshGeometry]);

    return (
        <mesh geometry={geometry} frustumCulled={true}>
            <meshStandardMaterial color={color} />
        </mesh>
    );
}

// Contour line renderer (thick line segments using LineSegments2)
function ContourLineRenderer({ lineData, color }: { lineData: Float32Array; color: string }) {
    const lineSegments = useMemo(() => {
        const geometry = new LineSegmentsGeometry();
        geometry.setPositions(lineData);

        const material = new LineMaterial({
            color: color,
            linewidth: 3, // in pixels
            resolution: new THREE.Vector2(window.innerWidth, window.innerHeight),
        });

        const segments = new LineSegments2(geometry, material);
        segments.computeLineDistances();
        return segments;
    }, [lineData, color]);

    useEffect(() => {
        const handleResize = () => {
            if (lineSegments.material instanceof LineMaterial) {
                lineSegments.material.resolution.set(window.innerWidth, window.innerHeight);
            }
        };
        window.addEventListener('resize', handleResize);
        return () => window.removeEventListener('resize', handleResize);
    }, [lineSegments]);

    return <primitive object={lineSegments} frustumCulled={true} />;
}

export default function Viewer3D({
    grids,
    selectedGridIds,
    isolateSelected,
    ignoreIblank,
    showFringePoints,
    iblankFilterMode,
    scalarField = 'none',
    colorScheme = 'viridis',
    showWireframe = true,
    shadingMode = 'none',
    sliceEnabled = false,
    gridSlices = {},
    arbitrarySlices = [],
    contoursEnabled = false,
    contourLevel = 0.5,
    contourDisplayMode = 'both',
    onSlicesChange,
    onLoadingChange
}: Viewer3DProps) {
    const [meshById, setMeshById] = useState<Record<string, MeshGeometry>>({});
    const [loadingById, setLoadingById] = useState<Record<string, number>>({});
    const [error, setError] = useState<string | null>(null);
    const autoCreatedSlicesRef = useRef(false);

    // Contour state
    const [isoSurfaceGeometries, setIsoSurfaceGeometries] = useState<Record<string, MeshGeometry>>({});
    const [contourLineGeometries, setContourLineGeometries] = useState<Record<string, Float32Array>>({});

    type MeshResult = { id: string; mesh: MeshGeometry } | { id: string; error: string };

    // Notify parent when loading state changes
    useEffect(() => {
        const isLoading = Object.keys(loadingById).length > 0;
        onLoadingChange?.(isLoading);
    }, [loadingById, onLoadingChange]);

    const gridIdKey = useMemo(() => grids.map((grid) => grid.id).join('|'), [grids]);

    // Clear meshes when grids change
    useEffect(() => {
        autoCreatedSlicesRef.current = false;
        if (grids.length === 0) {
            setMeshById({});
            setLoadingById({});
            setError(null);
        }
    }, [gridIdKey, grids.length]);

    const lastColorKeyRef = useRef<string>('');
    const lastSliceKeyRef = useRef<string>('');
    const requestIdRef = useRef(0);

    // Memoize a key based on applied slices (updates only on Apply)
    const appliedSlicesKey = useMemo(
        () => (arbitrarySlices || [])
            .filter(s => s.applied)
            .map(s => `${s.id}:${s.applyVersion}`)
            .join('|'),
        [arbitrarySlices]
    );

    // Generate or regenerate meshes as needed
    // When field/scheme changes, regenerate grids with solutions
    useEffect(() => {
        const effectStart = performance.now();
        void invoke('frontend_log', {
            message: `[Viewer3D] effect start grids=${grids.length} field=${scalarField} scheme=${colorScheme} ignoreIblank=${ignoreIblank} mode=${iblankFilterMode}`
        });
        if (grids.length === 0) {
            return;
        }

        const currentColorKey = `${scalarField}|${colorScheme}`;
        // Only include APPLIED slices in the slice key to avoid reprocessing while editing
        const sliceKey = `${sliceEnabled}|${ignoreIblank}|${showFringePoints}|${iblankFilterMode}|${JSON.stringify(gridSlices)}|${appliedSlicesKey}`;
        const shouldRecolor = lastColorKeyRef.current !== currentColorKey;
        const shouldReslice = lastSliceKeyRef.current !== sliceKey;

        void invoke('frontend_log', {
            message: `[Viewer3D] Color key check: last="${lastColorKeyRef.current}" current="${currentColorKey}" shouldRecolor=${shouldRecolor}`
        });
        void invoke('frontend_log', {
            message: `[Viewer3D] Slice key check: shouldReslice=${shouldReslice}`
        });

        const hasSlices = Object.keys(gridSlices).length > 0;

        // Auto-create default K=1 slices only once when a new grid set is first loaded
        if (!hasSlices && !autoCreatedSlicesRef.current) {
            void invoke('frontend_log', { message: '[Viewer3D] Creating default K=1 slices on initial grid load' });
            const newSlices: Record<string, GridSlice[]> = {};
            grids.forEach(grid => {
                newSlices[grid.id] = [{
                    id: `${grid.id}_default`,
                    plane: 'K',
                    index: 0
                }];
            });
            autoCreatedSlicesRef.current = true;
            onSlicesChange?.(newSlices);
            return; // Return to let the effect re-run with the new slices
        }

        // No slices means nothing is rendered, even if slicing is disabled
        if (!hasSlices) {
            requestIdRef.current += 1; // Cancel any in-flight mesh work
            setLoadingById({});
            setMeshById((prev) => (Object.keys(prev).length > 0 ? {} : prev));
            lastSliceKeyRef.current = sliceKey;
            return;
        }

        // Clear meshes when slicing is disabled - no rendering when slices are off
        if (!sliceEnabled) {
            requestIdRef.current += 1; // Cancel any in-flight mesh work
            setLoadingById({});
            setMeshById((prev) => (Object.keys(prev).length > 0 ? {} : prev));
            lastSliceKeyRef.current = sliceKey;
            return;
        }

        const gridsWithSlices = grids.filter((grid) => (gridSlices[grid.id]?.length ?? 0) > 0);
        const hasAppliedArbitrarySlices = (arbitrarySlices || []).some(s => s.applied);

        if (sliceEnabled) {
            // Only return early if there are NO slices at all (neither I/J/K nor arbitrary)
            if (gridsWithSlices.length === 0 && !hasAppliedArbitrarySlices) {
                if (Object.keys(meshById).length > 0) {
                    setMeshById({});
                }
                lastSliceKeyRef.current = sliceKey;
                return;
            }

            // Clean up I/J/K meshes for grids without slices
            const gridsWithoutSlices = grids.filter((grid) => (gridSlices[grid.id]?.length ?? 0) === 0);
            const hasStaleMeshes = gridsWithoutSlices.some((grid) => meshById[grid.id]);
            if (hasStaleMeshes) {
                setMeshById((prev) => {
                    const next = { ...prev };
                    gridsWithoutSlices.forEach((grid) => {
                        delete next[grid.id];
                    });
                    return next;
                });
            }

            // Clean up arbitrary meshes only when slices are removed or no longer applied
            const appliedArbitraryIds = new Set((arbitrarySlices || []).filter(s => s.applied).map(s => s.id));
            const staleArbitraryMeshes = Object.keys(meshById).filter(id => {
                if (id.startsWith('arbitrary::')) {
                    const parts = id.split('::');
                    const sliceId = parts[1];
                    return !appliedArbitraryIds.has(sliceId);
                }
                // Legacy format: arbitrary_${sliceId}_${gridId} (cannot reliably parse), remove
                if (id.startsWith('arbitrary_')) {
                    return true;
                }
                return false;
            });

            if (staleArbitraryMeshes.length > 0) {
                setMeshById((prev) => {
                    const next = { ...prev };
                    staleArbitraryMeshes.forEach((id: string) => {
                        delete next[id];
                    });
                    return next;
                });
            }
        }

        // targetGrids: grids that need I/J/K slice processing
        // For arbitrary slices, we always process ALL grids regardless of I/J/K slices
        const targetGrids = sliceEnabled ? gridsWithSlices : grids;

        // Determine which grids need to be regenerated
        // 1. On color/field change: regenerate all grids to avoid stale colors
        // 2. On slice change: regenerate all grids affected by slice changes
        // 3. Otherwise: only grids without any mesh
        let missing = shouldRecolor
            ? targetGrids
            : shouldReslice
                ? targetGrids  // Regenerate all grids when slice config changes
                : targetGrids.filter((grid) => !meshById[grid.id]);

        void invoke('frontend_log', {
            message: `[Viewer3D] Missing grids: ${missing.length} of ${targetGrids.length} (shouldRecolor=${shouldRecolor}, shouldReslice=${shouldReslice})`
        });

        // Regenerate arbitrary slices if config changed, field/color changed, or they're newly enabled
        const needArbitraryRegen = hasAppliedArbitrarySlices && (shouldReslice || shouldRecolor);

        void invoke('frontend_log', {
            message: `[Viewer3D] Arbitrary check: applied=${hasAppliedArbitrarySlices} shouldReslice=${shouldReslice} shouldRecolor=${shouldRecolor} needRegen=${needArbitraryRegen}`
        });

        if (missing.length === 0 && !needArbitraryRegen) {
            void invoke('frontend_log', { message: '[Viewer3D] effect no-op (missing=0, no reslice needed)' });
            lastSliceKeyRef.current = sliceKey;
            return;
        }

        let isCancelled = false;
        const requestId = requestIdRef.current + 1;
        requestIdRef.current = requestId;
        setError(null);
        setLoadingById((prev) => {
            const next = { ...prev };
            missing.forEach((grid) => {
                next[grid.id] = requestId;
            });
            return next;
        });

        // If arbitrary planes need regeneration, clear existing arbitrary meshes first to avoid remnants
        if (needArbitraryRegen) {
            setMeshById((prev) => {
                const next = { ...prev };
                Object.keys(next).forEach((id) => {
                    if (id.startsWith('arbitrary::') || id.startsWith('arbitrary_')) {
                        delete next[id];
                    }
                });
                return next;
            });
        }

        // Compute field ranges for consistent coloring across all slices
        // For each unique solution with a selected scalar field, get the global min/max
        interface FieldRange {
            min: number;
            max: number;
        }
        const fieldRangeMap = new Map<string, FieldRange>();
        let displayedGlobalRange: FieldRange | null = null;

        const fieldRangesPromise = scalarField !== 'none'
            ? (() => {
                const solsToProcess = new Set<string>();
                missing.forEach((grid) => {
                    if (grid.hasSolution && grid.solutionCacheId) {
                        solsToProcess.add(grid.solutionCacheId);
                    }
                });
                if (needArbitraryRegen) {
                    grids.forEach((grid) => {
                        if (grid.hasSolution && grid.solutionCacheId) {
                            solsToProcess.add(grid.solutionCacheId);
                        }
                    });
                }

                const rangePromises = Array.from(solsToProcess).map(async (solId) => {
                    try {
                        const range = await invoke<FieldRange>('get_solution_field_range', {
                            solutionId: solId,
                            field: scalarField
                        });
                        fieldRangeMap.set(solId, range);
                        void invoke('frontend_log', {
                            message: `[Viewer3D][color-range] solutionId=${solId} field=${scalarField} normalize=[${range.min}, ${range.max}]`
                        });
                    } catch (err) {
                        void invoke('frontend_log', {
                            message: `[Viewer3D] Failed to get field range for solution ${solId}: ${err}`
                        });
                        logger.warn(`Failed to compute field range for solution ${solId}: ${err}`, 'Viewer3D');
                    }
                });

                return Promise.all(rangePromises).then(() => {
                    if (fieldRangeMap.size > 0) {
                        const ranges = Array.from(fieldRangeMap.values());
                        const globalMin = Math.min(...ranges.map(r => r.min));
                        const globalMax = Math.max(...ranges.map(r => r.max));
                        displayedGlobalRange = { min: globalMin, max: globalMax };
                        void invoke('frontend_log', {
                            message: `[Viewer3D][color-range] displayed-global field=${scalarField} normalize=[${globalMin}, ${globalMax}] solutions=${fieldRangeMap.size}`
                        });
                    }
                    void invoke('frontend_log', {
                        message: `[Viewer3D] Computed field ranges for ${fieldRangeMap.size} solutions`
                    });
                });
            })()
            : Promise.resolve();

        // Process arbitrary cutting planes (global - affect ALL grids, not just those with I/J/K slices)
        // Only process if they don't exist yet or if slices have changed
        const arbitrarySlicePromises = needArbitraryRegen
            ? (arbitrarySlices || [])
                .filter((slice) => slice.enabled && slice.applied)
                .map((arbitrarySlice) => {
                    void invoke('frontend_log', {
                        message: `[Viewer3D] Processing arbitrary slice: ${arbitrarySlice.name} against ${grids.length} grids`
                    });
                    return Promise.all(
                        grids.map(async (gridItem) => {
                            try {
                                let mesh: MeshGeometry;

                                // Try to apply solution colors if available
                                if (gridItem.hasSolution && scalarField !== 'none' && gridItem.solutionCacheId) {
                                    try {
                                        logger.debug(
                                            `Attempting solution coloring for arbitrary plane '${arbitrarySlice.name}' on grid ${gridItem.id}`,
                                            'Viewer3D'
                                        );

                                        await fieldRangesPromise;
                                        const range = displayedGlobalRange ?? fieldRangeMap.get(gridItem.solutionCacheId);
                                        mesh = await invoke<MeshGeometry>('compute_solution_colors_arbitrary_plane', {
                                            gridId: gridItem.gridCacheId!,
                                            solutionId: gridItem.solutionCacheId,
                                            field: scalarField,
                                            colorScheme: colorScheme,
                                            planePoint: arbitrarySlice.planePoint,
                                            planeNormal: arbitrarySlice.planeNormal,
                                            respectIblank: !ignoreIblank,
                                            showFringePoints: showFringePoints,
                                            iblankFilterMode: iblankFilterMode,
                                            globalMin: range?.min,
                                            globalMax: range?.max,
                                        });

                                        const hasColors = mesh.colors && mesh.colors.length > 0;
                                        void invoke('frontend_log', {
                                            message: `[Viewer3D] Arbitrary plane '${arbitrarySlice.name}': Colors ${hasColors ? 'YES' : 'NO'} (${mesh.colors?.length || 0} values)`
                                        });
                                    } catch (colorErr) {
                                        // Fall back to non-colored geometry if solution coloring fails
                                        void invoke('frontend_log', {
                                            message: `[Viewer3D] Solution coloring FAILED on arbitrary plane '${arbitrarySlice.name}': ${colorErr}`
                                        });
                                        logger.error(`Solution coloring on arbitrary plane failed: ${colorErr}`, 'Viewer3D');

                                        mesh = await invoke<MeshGeometry>('slice_arbitrary_plane_by_id', {
                                            gridId: gridItem.gridCacheId!,
                                            planePoint: arbitrarySlice.planePoint,
                                            planeNormal: arbitrarySlice.planeNormal,
                                            respectIblank: !ignoreIblank,
                                            showFringePoints: showFringePoints,
                                            iblankFilterMode: iblankFilterMode,
                                        });
                                    }
                                } else {
                                    // No solution data - use base geometry
                                    mesh = await invoke<MeshGeometry>('slice_arbitrary_plane_by_id', {
                                        gridId: gridItem.gridCacheId!,
                                        planePoint: arbitrarySlice.planePoint,
                                        planeNormal: arbitrarySlice.planeNormal,
                                        respectIblank: !ignoreIblank,
                                        showFringePoints: showFringePoints,
                                        iblankFilterMode: iblankFilterMode,
                                    });
                                }

                                logger.debug(
                                    `Arbitrary plane '${arbitrarySlice.name}' intersected grid ${gridItem.id}`,
                                    'Viewer3D'
                                );
                                // Use special ID format for arbitrary slices
                                return {
                                    id: `arbitrary::${arbitrarySlice.id}::${gridItem.id}`,
                                    mesh,
                                };
                            } catch (err) {
                                // plane doesn't intersect this grid - expected, not an error
                                return null;
                            }
                        })
                    ).then((results) => results.filter((r) => r !== null));
                })
            : [];

        void invoke('frontend_log', {
            message: `[Viewer3D] Arbitrary slice promises: ${arbitrarySlicePromises.length}`
        });

        // Process per-grid I/J/K slices
        const gridSlicePromises = Promise.all(
            missing.map(async (gridItem) => {
                try {
                    const gridStart = performance.now();
                    let mesh: MeshGeometry;

                    // Apply all I/J/K slices for this grid if enabled
                    if (sliceEnabled && gridSlices[gridItem.id] && gridSlices[gridItem.id].length > 0) {
                        try {
                            // Generate meshes for each slice with solution colors if available
                            const sliceMeshes = await Promise.all(
                                gridSlices[gridItem.id].map(async (slice) => {
                                    let sliceMesh: MeshGeometry;

                                    // Try to apply solution colors to sliced geometry
                                    if (gridItem.hasSolution && scalarField !== 'none' && gridItem.solutionCacheId) {
                                        try {
                                            logger.debug(`Attempting solution coloring for slice ${slice.plane}${slice.index}...`, 'Viewer3D');

                                            await fieldRangesPromise;
                                            const range = displayedGlobalRange ?? fieldRangeMap.get(gridItem.solutionCacheId);
                                            sliceMesh = await invoke<MeshGeometry>('compute_solution_colors_sliced', {
                                                gridId: gridItem.gridCacheId!,
                                                solutionId: gridItem.solutionCacheId,
                                                slicePlane: slice.plane,
                                                sliceIndex: slice.index,
                                                field: scalarField,
                                                colorScheme: colorScheme,
                                                respectIblank: !ignoreIblank,
                                                showFringePoints: showFringePoints,
                                                iblankFilterMode: iblankFilterMode,
                                                globalMin: range?.min,
                                                globalMax: range?.max,
                                            });

                                            const hasColors = sliceMesh.colors && sliceMesh.colors.length > 0;
                                            void invoke('frontend_log', {
                                                message: `[Viewer3D] Slice ${slice.plane}${slice.index}: Colors ${hasColors ? 'YES' : 'NO'} (${sliceMesh.colors?.length || 0} values for ${sliceMesh.vertices.length} bytes vertices)`
                                            });
                                        } catch (colorErr) {
                                            // Fall back to non-colored geometry if solution coloring fails
                                            void invoke('frontend_log', {
                                                message: `[Viewer3D] Solution coloring FAILED on slice ${slice.plane}${slice.index}: ${colorErr}`
                                            });
                                            logger.error(`Solution coloring on slice (${slice.plane}${slice.index}) failed: ${colorErr}`, 'Viewer3D');

                                            const slicedGrid = await invoke<SerializableGrid>('slice_grid_by_id', {
                                                gridId: gridItem.gridCacheId!,
                                                plane: slice.plane,
                                                index: slice.index,
                                            });
                                            sliceMesh = await invoke<MeshGeometry>('convert_grid_to_mesh', {
                                                grid: slicedGrid,
                                                respectIblank: !ignoreIblank,
                                                showFringePoints: showFringePoints,
                                                iblankFilterMode: iblankFilterMode
                                            });
                                        }
                                    } else {
                                        // No solution data - use base geometry
                                        const slicedGrid = await invoke<SerializableGrid>('slice_grid_by_id', {
                                            gridId: gridItem.gridCacheId!,
                                            plane: slice.plane,
                                            index: slice.index,
                                        });
                                        sliceMesh = await invoke<MeshGeometry>('convert_grid_to_mesh', {
                                            grid: slicedGrid,
                                            respectIblank: !ignoreIblank,
                                            showFringePoints: showFringePoints,
                                            iblankFilterMode: iblankFilterMode
                                        });
                                    }
                                    return { sliceId: slice.id, mesh: sliceMesh };
                                })
                            );

                            // Merge all slice meshes into one
                            if (sliceMeshes.length > 0) {
                                const mergedMesh: MeshGeometry = {
                                    vertices: [],
                                    indices: [],
                                    triangle_indices: [],
                                    normals: [],
                                    colors: undefined,
                                    vertex_count: 0,
                                    face_count: 0,
                                };

                                // Check if all slices have colors before processing them
                                const allHaveColors = sliceMeshes.every(({ mesh }) => mesh.colors && mesh.colors.length > 0);
                                void invoke('frontend_log', {
                                    message: `[Viewer3D] Merging ${sliceMeshes.length} slices, allHaveColors=${allHaveColors}`
                                });
                                sliceMeshes.forEach((sm, idx) => {
                                    void invoke('frontend_log', {
                                        message: `[Viewer3D]  Slice ${idx}: verts=${sm.mesh.vertices.length / 3 | 0}, colors=${sm.mesh.colors?.length || 0}`
                                    });
                                });

                                if (allHaveColors) {
                                    mergedMesh.colors = [];
                                }

                                for (const { mesh: sliceMesh } of sliceMeshes) {
                                    const vertexOffset = mergedMesh.vertices.length / 3;

                                    // Append vertices and normals
                                    mergedMesh.vertices.push(...sliceMesh.vertices);
                                    mergedMesh.normals.push(...sliceMesh.normals);

                                    // Append colors only if we're collecting them from all slices
                                    if (mergedMesh.colors && sliceMesh.colors && sliceMesh.colors.length > 0) {
                                        mergedMesh.colors.push(...sliceMesh.colors);
                                    }

                                    // Append indices (offset by vertex count)
                                    mergedMesh.indices.push(...sliceMesh.indices.map(idx => idx + vertexOffset));
                                    mergedMesh.triangle_indices.push(...sliceMesh.triangle_indices.map(idx => idx + vertexOffset));

                                    // Update counts
                                    mergedMesh.vertex_count += sliceMesh.vertex_count;
                                    mergedMesh.face_count += sliceMesh.face_count;
                                }

                                // Verify colors array matches vertices
                                logger.debug(`Merged: vertices=${mergedMesh.vertices.length}, colors=${mergedMesh.colors?.length ?? 0}`, 'Viewer3D');
                                if (mergedMesh.colors && mergedMesh.colors.length > 0) {
                                    const expectedColorLength = mergedMesh.vertices.length;
                                    void invoke('frontend_log', {
                                        message: `[Viewer3D] Color validation: have ${mergedMesh.colors.length} need ${expectedColorLength}`
                                    });
                                    if (mergedMesh.colors.length !== expectedColorLength) {
                                        void invoke('frontend_log', {
                                            message: `[Viewer3D] MISMATCH: discarding colors`
                                        });
                                        logger.warn(`Color array length mismatch: have ${mergedMesh.colors.length} but need ${expectedColorLength}. This likely means slices have different vertex counts or color computation failed. Discarding colors.`, 'Viewer3D');
                                        mergedMesh.colors = undefined;
                                    } else {
                                        void invoke('frontend_log', {
                                            message: `[Viewer3D] Color validation PASSED`
                                        });
                                        logger.debug(`Color validation PASSED: ${mergedMesh.colors.length} colors for ${mergedMesh.vertices.length} vertices`, 'Viewer3D');
                                    }
                                } else {
                                    void invoke('frontend_log', {
                                        message: `[Viewer3D] No colors in merged mesh`
                                    });
                                    logger.debug(`No colors in merged mesh (expected for uncolored slices)`, 'Viewer3D');
                                }

                                mesh = mergedMesh;
                            } else {
                                throw new Error('No slice meshes generated');
                            }
                        } catch (sliceErr) {
                            const sliceMsg = String(sliceErr);
                            logger.error(`Slicing failed: ${sliceMsg}`, 'Viewer3D');
                            throw sliceErr;
                        }
                    } else {
                        // No slicing - use the original grid (fallback path, shouldn't normally be reached)
                        if (gridItem.hasSolution && scalarField !== 'none' && gridItem.solutionCacheId) {
                            try {
                                mesh = await invoke<MeshGeometry>('compute_solution_colors', {
                                    gridId: gridItem.gridCacheId!,
                                    solutionId: gridItem.solutionCacheId,
                                    field: scalarField,
                                    colorScheme: colorScheme,
                                    respectIblank: !ignoreIblank,
                                    showFringePoints: showFringePoints,
                                    iblankFilterMode: iblankFilterMode,
                                });
                            } catch (invokeErr) {
                                const invokeMsg = String(invokeErr);
                                logger.error(`[${gridItem.id}] compute_solution_colors FAILED: ${invokeMsg}`, 'Viewer3D');
                                throw invokeErr;
                            }
                        } else {
                            mesh = await invoke<MeshGeometry>('convert_grid_to_mesh_by_id', {
                                gridId: gridItem.gridCacheId!,
                                respectIblank: !ignoreIblank,
                                showFringePoints: showFringePoints,
                                iblankFilterMode: iblankFilterMode
                            });
                        }
                    }

                    void invoke('frontend_log', {
                        message: `[Viewer3D] grid done id=${gridItem.id} ms=${Math.round(performance.now() - gridStart)}`
                    });

                    return { id: gridItem.id, mesh };
                } catch (err) {
                    const errorMsg = String(err);
                    logger.error(`Grid ${gridItem.id} FAILED: ${errorMsg}`, 'Viewer3D');
                    return { id: gridItem.id, error: errorMsg };
                }
            })
        );

        // Wait for both per-grid slices and arbitrary slices to complete
        Promise.all([gridSlicePromises, Promise.all(arbitrarySlicePromises)])
            .then(([gridResults, arbitraryResults]: [MeshResult[], MeshResult[][]]) => {
                if (isCancelled || requestId !== requestIdRef.current) {
                    return;
                }

                // Flatten arbitrary results (array of arrays)
                const flatArbitraryResults = arbitraryResults.flat();

                void invoke('frontend_log', {
                    message: `[Viewer3D] effect done ms=${Math.round(performance.now() - effectStart)} gridResults=${gridResults.length} arbitraryResults=${flatArbitraryResults.length}`
                });

                lastColorKeyRef.current = currentColorKey;
                lastSliceKeyRef.current = sliceKey;

                // Combine both result sets
                const allResults = [...gridResults, ...flatArbitraryResults];

                const errors = allResults.filter((result) => "error" in result) as { id: string; error: string }[];
                if (errors.length > 0) {
                    const errorDetails = errors.map(e => `${e.id}: ${e.error}`).join('\n');
                    const errorMsg = `Failed to convert ${errors.length} grid(s) to mesh:\n${errorDetails}`;
                    logger.error(errorMsg, 'Viewer3D');
                    setError(errorMsg);
                }

                setMeshById((prev) => {
                    const next = { ...prev };
                    allResults.forEach((result) => {
                        if ("mesh" in result) {
                            next[result.id] = result.mesh;
                        }
                    });
                    return next;
                });

                setLoadingById((prev) => {
                    const next = { ...prev };
                    allResults.forEach((result) => {
                        if (next[result.id] === requestId) {
                            delete next[result.id];
                        }
                    });
                    return next;
                });
            });

        return () => {
            isCancelled = true;
            setLoadingById((prev) => {
                const next = { ...prev };
                missing.forEach((grid) => {
                    if (next[grid.id] === requestId) {
                        delete next[grid.id];
                    }
                });
                return next;
            });
            void invoke('frontend_log', {
                message: `[Viewer3D] effect cancelled ms=${Math.round(performance.now() - effectStart)}`
            });
        };
    }, [grids, ignoreIblank, showFringePoints, iblankFilterMode, scalarField, colorScheme, sliceEnabled, gridSlices, appliedSlicesKey]);
    const visibleGrids = useMemo(
        () => getVisibleGridItems(grids, selectedGridIds, isolateSelected),
        [grids, isolateSelected, selectedGridIds]
    );


    // Contour extraction effect
    useEffect(() => {
        if (!contoursEnabled || scalarField === 'none') {
            // Clear contours when disabled or no field selected
            setIsoSurfaceGeometries({});
            setContourLineGeometries({});
            return;
        }

        const gridsWithSolution = grids.filter(g => g.hasSolution && g.solutionCacheId && g.gridCacheId);
        if (gridsWithSolution.length === 0) {
            return;
        }

        logger.info(`Extracting contours at level ${contourLevel} for field ${scalarField}`, 'Viewer3D');

        // Extract iso-surfaces for each grid with solution
        const isoSurfacePromises = gridsWithSolution.map(async (gridItem) => {
            try {
                const mesh = await invoke<MeshGeometry>('extract_iso_surface_by_id', {
                    gridId: gridItem.gridCacheId!,
                    solutionId: gridItem.solutionCacheId!,
                    scalarField: scalarField,
                    levelNormalized: contourLevel,
                    respectIblank: !ignoreIblank,
                    showFringePoints: showFringePoints,
                    iblankFilterMode: iblankFilterMode,
                });
                return { id: gridItem.id, mesh };
            } catch (err) {
                logger.warn(`Failed to extract iso-surface for grid ${gridItem.id}: ${err}`, 'Viewer3D');
                return null;
            }
        });

        // Extract contour lines for slices (only visible grids)
        const visibleGridIds = new Set(visibleGrids.map(g => g.id));
        const sliceContourPromises = gridsWithSolution
            .filter(gridItem => visibleGridIds.has(gridItem.id))
            .flatMap((gridItem) => {
                const slices = gridSlices[gridItem.id] || [];
                return slices.map(async (slice) => {
                    try {
                        const lineData = await invoke<number[]>('extract_slice_contours_by_id', {
                            gridId: gridItem.gridCacheId!,
                            solutionId: gridItem.solutionCacheId!,
                            plane: slice.plane,
                            index: slice.index,
                            scalarField: scalarField,
                            levelNormalized: contourLevel,
                            respectIblank: !ignoreIblank,
                            showFringePoints: showFringePoints,
                            iblankFilterMode: iblankFilterMode,
                        });
                        return { id: `slice::${gridItem.id}::${slice.id}`, lineData: new Float32Array(lineData) };
                    } catch (err) {
                        logger.warn(`Failed to extract slice contours for ${gridItem.id}/${slice.id}: ${err}`, 'Viewer3D');
                        return null;
                    }
                });
            });

        // Extract contour lines for arbitrary planes (only visible grids)
        const arbitraryContourPromises = (arbitrarySlices || [])
            .filter(s => s.enabled && s.applied)
            .flatMap((slice) => {
                return gridsWithSolution
                    .filter(gridItem => visibleGridIds.has(gridItem.id))
                    .map(async (gridItem) => {
                        try {
                            const lineData = await invoke<number[]>('extract_arbitrary_plane_contours_by_id', {
                                gridId: gridItem.gridCacheId!,
                                solutionId: gridItem.solutionCacheId!,
                                planePoint: slice.planePoint,
                                planeNormal: slice.planeNormal,
                                scalarField: scalarField,
                                levelNormalized: contourLevel,
                                respectIblank: !ignoreIblank,
                                showFringePoints: showFringePoints,
                                iblankFilterMode: iblankFilterMode,
                            });
                            return { id: `arbitrary::${slice.id}::${gridItem.id}`, lineData: new Float32Array(lineData) };
                        } catch (err) {
                            // Plane may not intersect this grid - expected
                            return null;
                        }
                    });
            });

        Promise.all([
            Promise.all(isoSurfacePromises),
            Promise.all(sliceContourPromises),
            Promise.all(arbitraryContourPromises),
        ]).then(([isoResults, sliceResults, arbitraryResults]) => {
            // Update iso-surfaces
            const newIsoSurfaces: Record<string, MeshGeometry> = {};
            isoResults.forEach(result => {
                if (result && result.mesh.vertex_count > 0) {
                    newIsoSurfaces[result.id] = result.mesh;
                }
            });
            setIsoSurfaceGeometries(newIsoSurfaces);

            // Update contour lines
            const newContourLines: Record<string, Float32Array> = {};
            [...sliceResults, ...arbitraryResults].forEach(result => {
                if (result && result.lineData.length > 0) {
                    newContourLines[result.id] = result.lineData;
                }
            });
            setContourLineGeometries(newContourLines);

            logger.info(
                `Contours extracted: ${Object.keys(newIsoSurfaces).length} iso-surfaces, ${Object.keys(newContourLines).length} contour lines`,
                'Viewer3D'
            );
        }).catch(err => {
            logger.error(`Failed to extract contours: ${err}`, 'Viewer3D');
        });
    }, [contoursEnabled, contourLevel, scalarField, grids, gridSlices, arbitrarySlices, ignoreIblank, showFringePoints, iblankFilterMode, visibleGrids]);

    const enabledArbitraryIds = useMemo(
        () => new Set((arbitrarySlices || []).filter(s => s.applied && s.enabled).map(s => s.id)),
        [arbitrarySlices]
    );

    const stats = useMemo(() => {
        return visibleGrids.reduce(
            (acc, grid) => {
                const mesh = meshById[grid.id];
                if (mesh) {
                    acc.vertices += mesh.vertex_count;
                    acc.edges += mesh.face_count;
                }
                return acc;
            },
            { vertices: 0, edges: 0 }
        );
    }, [meshById, visibleGrids]);

    const isLoading = Object.keys(loadingById).length > 0;

    return (
        <div style={{ width: '100%', height: '100%', position: 'relative' }}>
            <Canvas camera={{ position: [5, 5, 5], fov: 50 }}>
                <ambientLight intensity={0.5} />
                <directionalLight position={[10, 10, 5]} intensity={1} />

                {/* Render mesh based on selected mode */}
                {visibleGrids.map((gridItem) => {
                    const mesh = meshById[gridItem.id];
                    if (!mesh) {
                        return null;
                    }
                    const dimmed = selectedGridIds.length > 0 && !selectedGridIds.includes(gridItem.id) && !isolateSelected;
                    // Use grey color when contours are enabled, otherwise use grid color
                    const displayColor = contoursEnabled ? '#808080' : gridItem.color;

                    return (
                        <group key={gridItem.id}>
                            {/* Render smooth shaded surface */}
                            {shadingMode === 'smooth' && (
                                <SolidMeshRenderer
                                    meshGeometry={mesh}
                                    color={displayColor}
                                    dimmed={dimmed}
                                />
                            )}
                            {/* Render wireframe */}
                            {showWireframe && (
                                <MeshRenderer
                                    meshGeometry={mesh}
                                    color={displayColor}
                                    dimmed={dimmed}
                                />
                            )}
                        </group>
                    );
                })}

                {/* Render arbitrary cutting plane meshes */}
                {Object.entries(meshById)
                    .filter(([id]) => {
                        if (!id.startsWith('arbitrary::')) return false;
                        const parts = id.split('::');
                        const sliceId = parts[1];
                        return enabledArbitraryIds.has(sliceId);
                    })
                    .map(([id, mesh]) => {
                        // Use grey when contours enabled, otherwise light blue
                        const sliceColor = contoursEnabled ? '#808080' : '#60a5fa';
                        return (
                            <group key={id}>
                                {shadingMode === 'smooth' && (
                                    <SolidMeshRenderer
                                        meshGeometry={mesh}
                                        color={sliceColor}
                                        dimmed={false}
                                    />
                                )}
                                {showWireframe && (
                                    <MeshRenderer
                                        meshGeometry={mesh}
                                        color={sliceColor}
                                        dimmed={false}
                                    />
                                )}
                            </group>
                        );
                    })}

                {/* Render iso-surfaces */}
                {contoursEnabled && (contourDisplayMode === 'surfaces' || contourDisplayMode === 'both') &&
                    Object.entries(isoSurfaceGeometries).map(([id, mesh]) => (
                        <group key={`iso::${id}`}>
                            <IsoSurfaceRenderer meshGeometry={mesh} color="#3b82f6" />
                        </group>
                    ))
                }

                {/* Render contour lines */}
                {contoursEnabled && (contourDisplayMode === 'lines' || contourDisplayMode === 'both') &&
                    Object.entries(contourLineGeometries).map(([id, lineData]) => (
                        <group key={`contour::${id}`}>
                            <ContourLineRenderer lineData={lineData} color="#000000" />
                        </group>
                    ))
                }

                {/* Camera controls */}
                <OrbitControls enableDamping dampingFactor={0.05} />
            </Canvas>

            {/* UI Controls */}
            <div
                style={{
                    position: 'absolute',
                    top: 10,
                    right: 10,
                    background: 'rgba(0,0,0,0.7)',
                    padding: '10px',
                    borderRadius: '5px',
                    color: 'white',
                    zIndex: 10,
                }}
            >
                {isLoading && <div>Loading mesh...</div>}

                {visibleGrids.length > 0 && (
                    <div style={{ marginTop: isLoading ? '10px' : '0', fontSize: '0.9em' }}>
                        Visible grids: {visibleGrids.length}
                        <br />
                        Vertices: {stats.vertices}
                        <br />
                        Edges: {stats.edges}
                    </div>
                )}
            </div>

            {/* Error Modal/Popup */}
            {error && (
                <div
                    style={{
                        position: 'fixed',
                        top: 0,
                        left: 0,
                        right: 0,
                        bottom: 0,
                        backgroundColor: 'rgba(0, 0, 0, 0.5)',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        zIndex: 1000,
                    }}
                    onClick={() => setError(null)}
                >
                    <div
                        style={{
                            backgroundColor: 'white',
                            borderRadius: '8px',
                            padding: '20px',
                            maxWidth: '500px',
                            boxShadow: '0 4px 12px rgba(0, 0, 0, 0.15)',
                        }}
                        onClick={(e) => e.stopPropagation()}
                    >
                        <div style={{ marginBottom: '15px', fontWeight: 'bold', color: '#333' }}>
                            Error
                        </div>
                        <div style={{ marginBottom: '20px', color: '#666', whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}>
                            {error}
                        </div>
                        <button
                            onClick={() => setError(null)}
                            style={{
                                padding: '8px 16px',
                                backgroundColor: '#ef4444',
                                color: 'white',
                                border: 'none',
                                borderRadius: '4px',
                                cursor: 'pointer',
                                float: 'right',
                            }}
                        >
                            Close
                        </button>
                        <div style={{ clear: 'both' }} />
                    </div>
                </div>
            )}
        </div>
    );
}
