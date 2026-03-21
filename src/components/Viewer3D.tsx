import { Canvas, useThree } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';
import { useState, useEffect, useMemo, useRef, type MutableRefObject } from 'react';
import * as THREE from 'three';
import { BufferGeometry, BufferAttribute, ShaderMaterial } from 'three';
import { LineSegments2 } from 'three/examples/jsm/lines/LineSegments2.js';
import { LineSegmentsGeometry } from 'three/examples/jsm/lines/LineSegmentsGeometry.js';
import { LineMaterial } from 'three/examples/jsm/lines/LineMaterial.js';
import { invoke } from '@tauri-apps/api/core';
import { logger } from '../utils/logger';
import type { GridItem, GridSlice, ArbitrarySlice } from '../types/grids';
import type { ColorScheme } from '../utils/colorMapping';
import { mapValueToColor, normalizeValue, rgbToHex } from '../utils/colorMapping';
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

interface BackendIndexRange {
    start: number;
    end?: number | null;
}

interface BackendGridSubset {
    grid: number;
    i_range?: BackendIndexRange | null;
    j_range?: BackendIndexRange | null;
    k_range?: BackendIndexRange | null;
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
    subsets?: BackendGridSubset[];
    arbitrarySlices?: ArbitrarySlice[];
    plotFamily?: 'contour' | 'function_surface';
    contourAttribute?: 'line' | 'surface' | 'grid' | 'color_contours' | 'dots';
    contourSpec?: unknown;
    isoSurfaceOpacity?: number;
    cameraAxisView?:
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
    cameraViewpoint?: { x: number; y: number; z: number } | null;
    onCameraCommit?: (vp: { x: number; y: number; z: number }) => void;
    onLoadingChange?: (isLoading: boolean) => void;
}

function CameraViewpointSync({
    cameraAxisView,
    cameraViewpoint,
    isUserNavigatingRef,
    controlsRef,
}: {
    cameraAxisView?:
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
    cameraViewpoint?: { x: number; y: number; z: number } | null;
    isUserNavigatingRef: MutableRefObject<boolean>;
    controlsRef: MutableRefObject<any>;
}) {
    const { camera } = useThree();
    const lastAppliedAxisViewRef = useRef<string | null>(null);

    const axisDirection = (axisView: NonNullable<typeof cameraAxisView>): THREE.Vector3 | null => {
        switch (axisView) {
            case 'plus_x':
                return new THREE.Vector3(1, 0, 0);
            case 'minus_x':
                return new THREE.Vector3(-1, 0, 0);
            case 'plus_y':
                return new THREE.Vector3(0, 1, 0);
            case 'minus_y':
                return new THREE.Vector3(0, -1, 0);
            case 'plus_z':
                return new THREE.Vector3(0, 0, 1);
            case 'minus_z':
                return new THREE.Vector3(0, 0, -1);
            // Legacy plane aliases map to canonical orthogonal camera axes.
            // TOP(XY)->+Z, SIDE(XZ)->+Y, FRONT(YZ)->+X.
            case 'plane_xy':
            case 'plane_yx':
                return new THREE.Vector3(0, 0, 1);
            case 'plane_xz':
            case 'plane_zx':
                return new THREE.Vector3(0, 1, 0);
            case 'plane_yz':
            case 'plane_zy':
                return new THREE.Vector3(1, 0, 0);
            case 'custom':
            default:
                return null;
        }
    };

    useEffect(() => {
        if (!cameraViewpoint) {
            return;
        }
        // Never fight active user interaction; only sync from backend when idle.
        if (isUserNavigatingRef.current) {
            return;
        }

        const dx = camera.position.x - cameraViewpoint.x;
        const dy = camera.position.y - cameraViewpoint.y;
        const dz = camera.position.z - cameraViewpoint.z;
        const dist2 = dx * dx + dy * dy + dz * dz;
        // Avoid tiny corrective snaps that feel like jitter.
        if (dist2 < 1e-8) {
            return;
        }

        camera.position.set(cameraViewpoint.x, cameraViewpoint.y, cameraViewpoint.z);
        if (controlsRef.current) {
            controlsRef.current.target.set(0, 0, 0);
            controlsRef.current.update();
        } else {
            camera.lookAt(0, 0, 0);
        }
        lastAppliedAxisViewRef.current = null;
    }, [camera, cameraViewpoint, controlsRef, isUserNavigatingRef]);

    useEffect(() => {
        if (!cameraAxisView || cameraAxisView === 'custom') {
            return;
        }
        if (isUserNavigatingRef.current) {
            return;
        }
        if (lastAppliedAxisViewRef.current === cameraAxisView) {
            return;
        }

        const dir = axisDirection(cameraAxisView);
        if (!dir) {
            return;
        }

        // Preserve current camera radius when switching to a named axis view.
        const distance = Math.max(camera.position.length(), 1.0);
        const next = dir.multiplyScalar(distance);
        camera.position.set(next.x, next.y, next.z);
        if (controlsRef.current) {
            controlsRef.current.target.set(0, 0, 0);
            controlsRef.current.update();
        } else {
            camera.lookAt(0, 0, 0);
        }
        lastAppliedAxisViewRef.current = cameraAxisView;
    }, [camera, cameraAxisView, cameraViewpoint, controlsRef, isUserNavigatingRef]);

    return null;
}

function CameraCommitControls({
    onCameraCommit,
    isUserNavigatingRef,
    controlsRef,
}: {
    onCameraCommit?: (vp: { x: number; y: number; z: number }) => void;
    isUserNavigatingRef: MutableRefObject<boolean>;
    controlsRef: MutableRefObject<any>;
}) {
    const { camera } = useThree();
    const settleTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

    const enableDamping = true;
    const dampingFactor = 0.15;

    // Derive "idle" debounce from OrbitControls damping settings.
    // Smaller damping factors decay more slowly, so we wait longer.
    const settleDelayMs = enableDamping
        ? Math.max(80, Math.min(350, Math.round(30 / Math.max(dampingFactor, 0.001))))
        : 40;

    const clearSettleTimer = () => {
        if (settleTimerRef.current) {
            clearTimeout(settleTimerRef.current);
            settleTimerRef.current = null;
        }
    };

    const scheduleCommitAfterIdle = () => {
        clearSettleTimer();
        settleTimerRef.current = setTimeout(() => {
            isUserNavigatingRef.current = false;
            if (onCameraCommit) {
                onCameraCommit({
                    x: camera.position.x,
                    y: camera.position.y,
                    z: camera.position.z,
                });
            }
        }, settleDelayMs);
    };

    const handleStart = () => {
        isUserNavigatingRef.current = true;
        clearSettleTimer();
    };

    const handleChange = () => {
        // Ignore programmatic camera changes (preset/viewpoint sync). We only
        // want to commit when the user is actively interacting with controls.
        if (!isUserNavigatingRef.current) {
            return;
        }
        scheduleCommitAfterIdle();
    };

    const handleEnd = () => {
        if (!enableDamping) {
            isUserNavigatingRef.current = false;
            if (onCameraCommit) {
                onCameraCommit({
                    x: camera.position.x,
                    y: camera.position.y,
                    z: camera.position.z,
                });
            }
            return;
        }
        // Damping continues after pointer release; wait for "change" events to
        // go quiet before committing.
        isUserNavigatingRef.current = true;
        scheduleCommitAfterIdle();
    };

    useEffect(() => {
        return () => {
            clearSettleTimer();
        };
    }, []);

    return (
        <OrbitControls
            ref={controlsRef}
            enableDamping={enableDamping}
            dampingFactor={dampingFactor}
            onStart={handleStart}
            onChange={handleChange}
            onEnd={handleEnd}
        />
    );
}

function SolidMeshRenderer({
    meshGeometry,
    color,
    dimmed,
    forceSolidColor = false,
}: {
    meshGeometry: MeshGeometry;
    color: string;
    dimmed: boolean;
    forceSolidColor?: boolean;
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
    const hasColors = !forceSolidColor && !!meshGeometry.colors && meshGeometry.colors.length === meshGeometry.vertices.length;

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
    forceSolidColor = false,
}: {
    meshGeometry: MeshGeometry;
    color: string;
    dimmed: boolean;
    forceSolidColor?: boolean;
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
        const hasColors = !forceSolidColor && !!colors && colors.length === meshGeometry.vertices.length;

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
    }, [forceSolidColor, meshGeometry]);

    // Use vertex colors if available, otherwise use single color
    const hasColors = !forceSolidColor && !!meshGeometry.colors && meshGeometry.colors.length === meshGeometry.vertices.length;

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

// Iso-surface renderer — double-sided with optional transparency.
function IsoSurfaceRenderer({ meshGeometry, color, opacity = 1.0 }: {
    meshGeometry: MeshGeometry;
    color: string;
    opacity?: number;
}) {
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
            <meshStandardMaterial
                color={color}
                side={THREE.DoubleSide}
                transparent={opacity < 1.0}
                opacity={opacity}
                depthWrite={opacity >= 1.0}
            />
        </mesh>
    );
}

// Contour line renderer (thick line segments using LineSegments2)
function ContourLineRenderer({ lineData, color }: { lineData: Float32Array; color: string }) {
    const { size } = useThree();

    const lineSegments = useMemo(() => {
        const geometry = new LineSegmentsGeometry();
        geometry.setPositions(lineData);

        const material = new LineMaterial({
            color: color,
            linewidth: 2, // in pixels
            resolution: new THREE.Vector2(size.width, size.height),
            depthTest: true,
            depthWrite: true,
            transparent: false,
            opacity: 1.0,
        });

        const segments = new LineSegments2(geometry, material);
        segments.computeLineDistances();
        return segments;
    }, [lineData, color, size.height, size.width]);

    useEffect(() => {
        if (lineSegments.material instanceof LineMaterial) {
            lineSegments.material.resolution.set(size.width, size.height);
        }
    }, [lineSegments, size.height, size.width]);

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
    subsets = [],
    arbitrarySlices = [],
    plotFamily = 'contour',
    contourAttribute = 'line' as 'line' | 'surface' | 'grid' | 'color_contours' | 'dots',
    contourSpec,
    isoSurfaceOpacity = 1.0,
    cameraAxisView = 'custom',
    cameraViewpoint,
    onCameraCommit,
    onLoadingChange
}: Viewer3DProps) {
    type IsoSurfaceGeometry = {
        mesh: MeshGeometry;
        level: number;
        color: string;
    };

    type ContourLineGeometry = {
        lineData: Float32Array;
        color: string;
    };

    const isContourPlotFamily = plotFamily === 'contour';
    const contourSpecMode =
        contourSpec && typeof contourSpec === 'object' && 'mode' in contourSpec
            ? String((contourSpec as { mode?: unknown }).mode ?? 'none')
            : 'none';

    const renderNotice = useMemo(() => {
        if (!isContourPlotFamily && contourSpecMode !== 'none') {
            return 'Contour levels/attributes are ignored in Function Surface mode (MVP behavior).';
        }
        if (isContourPlotFamily && (contourAttribute === 'grid' || contourAttribute === 'dots')) {
            return `${contourAttribute.toUpperCase()} contour attribute is not fully implemented yet; rendering line contours as a first-pass fallback.`;
        }
        return null;
    }, [contourAttribute, contourSpecMode, isContourPlotFamily]);

    const [meshById, setMeshById] = useState<Record<string, MeshGeometry>>({});
    const [loadingById, setLoadingById] = useState<Record<string, number>>({});
    const [error, setError] = useState<string | null>(null);

    // Contour state
    const [isoSurfaceGeometries, setIsoSurfaceGeometries] = useState<Record<string, IsoSurfaceGeometry>>({});
    const [contourLineGeometries, setContourLineGeometries] = useState<Record<string, ContourLineGeometry>>({});

    const mergedContourLinesByColor = useMemo(() => {
        const buckets = new Map<string, Float32Array[]>();

        Object.values(contourLineGeometries).forEach((contour) => {
            if (!buckets.has(contour.color)) {
                buckets.set(contour.color, []);
            }
            buckets.get(contour.color)!.push(contour.lineData);
        });

        const mergeArrays = (arrays: Float32Array[]): Float32Array => {
            if (arrays.length === 1) {
                return arrays[0];
            }
            const totalLength = arrays.reduce((sum, arr) => sum + arr.length, 0);
            const merged = new Float32Array(totalLength);
            let offset = 0;
            arrays.forEach((arr) => {
                merged.set(arr, offset);
                offset += arr.length;
            });
            return merged;
        };

        return Array.from(buckets.entries()).map(([color, chunks]) => ({
            color,
            lineData: mergeArrays(chunks),
        }));
    }, [contourLineGeometries]);

    type MeshResult = { id: string; mesh: MeshGeometry } | { id: string; error: string };

    // Notify parent when loading state changes
    useEffect(() => {
        const isLoading = Object.keys(loadingById).length > 0;
        onLoadingChange?.(isLoading);
    }, [loadingById, onLoadingChange]);

    const gridIdKey = useMemo(() => grids.map((grid) => grid.id).join('|'), [grids]);

    // Clear meshes when grids change
    useEffect(() => {
        if (grids.length === 0) {
            setMeshById({});
            setLoadingById({});
            setError(null);
        }
    }, [gridIdKey, grids.length]);

    const lastColorKeyRef = useRef<string>('');
    const lastSliceKeyRef = useRef<string>('');
    const requestIdRef = useRef(0);
    const isUserNavigatingRef = useRef(false);
    const controlsRef = useRef<any>(null);

    // Memoize a key based on applied slices (updates only on Apply)
    const appliedSlicesKey = useMemo(
        () => (arbitrarySlices || [])
            .filter(s => s.applied)
            .map(s => `${s.id}:${s.applyVersion}`)
            .join('|'),
        [arbitrarySlices]
    );

    const subsetsContentKey = useMemo(
        () => (subsets || [])
            .map((s) => {
                const i = s.i_range ? `${s.i_range.start}:${s.i_range.end ?? ''}` : '-';
                const j = s.j_range ? `${s.j_range.start}:${s.j_range.end ?? ''}` : '-';
                const k = s.k_range ? `${s.k_range.start}:${s.k_range.end ?? ''}` : '-';
                return `${s.grid}|${i}|${j}|${k}`;
            })
            .sort()
            .join(';'),
        [subsets]
    );

    const contourSpecKey = useMemo(() => JSON.stringify(contourSpec), [contourSpec]);

    const contourArbitrarySlicesKey = useMemo(
        () => (arbitrarySlices || [])
            .filter((s) => s.enabled && s.applied)
            .map((s) => `${s.id}|${s.planePoint.join(',')}|${s.planeNormal.join(',')}|${s.applyVersion}`)
            .sort()
            .join(';'),
        [arbitrarySlices]
    );

    const subsetsByGridId = useMemo(() => {
        const byGrid: Record<string, BackendGridSubset[]> = {};
        for (const subset of subsets) {
            const grid = grids.find((g) => g.gridIndex + 1 === subset.grid);
            if (!grid) {
                continue;
            }
            if (!byGrid[grid.id]) {
                byGrid[grid.id] = [];
            }
            byGrid[grid.id].push(subset);
        }
        return byGrid;
    }, [grids, subsetsContentKey]);

    const subsetSlicesByGridId = useMemo(() => {
        const byGrid: Record<string, GridSlice[]> = {};
        const resolveRange = (range: BackendIndexRange | null | undefined, dim: number) => {
            if (!range) {
                return { start: 1, end: dim };
            }
            const resolve = (n: number) => (n < 0 ? dim + n + 1 : n);
            const start = Math.max(1, Math.min(dim, resolve(range.start)));
            const endRaw = range.end != null ? resolve(range.end) : dim;
            const end = Math.max(1, Math.min(dim, endRaw));
            return start <= end ? { start, end } : { start: end, end: start };
        };

        for (const grid of grids) {
            const gridSubsets = subsetsByGridId[grid.id] || [];
            const slices: GridSlice[] = [];

            for (let idx = 0; idx < gridSubsets.length; idx += 1) {
                const subset = gridSubsets[idx];
                const i = resolveRange(subset.i_range, grid.dimensions.i);
                const j = resolveRange(subset.j_range, grid.dimensions.j);
                const k = resolveRange(subset.k_range, grid.dimensions.k);
                const axes = [
                    { plane: 'I' as const, range: i, dim: Math.max(1, grid.dimensions.i) },
                    { plane: 'J' as const, range: j, dim: Math.max(1, grid.dimensions.j) },
                    { plane: 'K' as const, range: k, dim: Math.max(1, grid.dimensions.k) },
                ];

                const classified = axes.map((axis) => {
                    const isPoint = axis.range.start === axis.range.end;
                    const isFull = axis.range.start === 1 && axis.range.end === axis.dim;
                    return { ...axis, isPoint, isFull };
                });

                const points = classified.filter((axis) => axis.isPoint);
                const othersAreFull = classified
                    .filter((axis) => !axis.isPoint)
                    .every((axis) => axis.isFull);

                // Represent GUI-style slicing: one point axis, others full-range.
                if (points.length !== 1 || !othersAreFull) {
                    continue;
                }
                const active = points[0];
                slices.push({
                    id: `subset-slice-${grid.id}-${idx}`,
                    plane: active.plane,
                    index: active.range.start - 1,
                });
            }

            if (slices.length > 0) {
                byGrid[grid.id] = slices;
            }
        }

        return byGrid;
    }, [grids, subsetsByGridId]);

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
        const sliceKey = `${sliceEnabled}|${ignoreIblank}|${showFringePoints}|${iblankFilterMode}|${subsetsContentKey}|${appliedSlicesKey}`;
        const shouldRecolor = lastColorKeyRef.current !== currentColorKey;
        const shouldReslice = lastSliceKeyRef.current !== sliceKey;

        void invoke('frontend_log', {
            message: `[Viewer3D] Color key check: last="${lastColorKeyRef.current}" current="${currentColorKey}" shouldRecolor=${shouldRecolor}`
        });
        void invoke('frontend_log', {
            message: `[Viewer3D] Slice key check: shouldReslice=${shouldReslice}`
        });

        const gridsWithSubsets = grids.filter((grid) => (subsetsByGridId[grid.id]?.length ?? 0) > 0);
        const hasAppliedArbitrarySlices = (arbitrarySlices || []).some(s => s.applied);

        if (sliceEnabled) {
            // If there are no backend subsets, fall back to full-grid rendering.
            if (gridsWithSubsets.length === 0 && !hasAppliedArbitrarySlices) {
                void invoke('frontend_log', {
                    message: '[Viewer3D] No backend subsets available; using full-grid fallback rendering'
                });
            }

            // Clean up subset meshes for grids without active subsets.
            if (gridsWithSubsets.length > 0) {
                const gridsWithoutSubsets = grids.filter((grid) => (subsetsByGridId[grid.id]?.length ?? 0) === 0);
                const hasStaleMeshes = gridsWithoutSubsets.some((grid) => meshById[grid.id]);
                if (hasStaleMeshes) {
                    setMeshById((prev) => {
                        const next = { ...prev };
                        gridsWithoutSubsets.forEach((grid) => {
                            delete next[grid.id];
                        });
                        return next;
                    });
                }
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

        // targetGrids: grids that need subset processing
        // For arbitrary slices, we always process ALL grids regardless of subsets
        const targetGrids = sliceEnabled && gridsWithSubsets.length > 0 ? gridsWithSubsets : grids;

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

                    // Apply all backend subsets for this grid when slicing is enabled.
                    const gridSubsets = subsetsByGridId[gridItem.id] || [];
                    if (sliceEnabled && gridSubsets.length > 0) {
                        try {
                            // Generate meshes for each subset and merge them for display.
                            const subsetMeshes = await Promise.all(
                                gridSubsets.map(async (subset, subsetIndex) => {
                                    const norm = (range?: BackendIndexRange | null, dim?: number) => {
                                        if (!range || !dim || dim <= 0) {
                                            return { start: undefined as number | undefined, end: undefined as number | undefined };
                                        }
                                        const resolve = (n: number) => (n < 0 ? dim + n + 1 : n);
                                        const s = Math.max(1, Math.min(dim, resolve(range.start)));
                                        const eRaw = range.end != null ? resolve(range.end) : dim;
                                        const e = Math.max(1, Math.min(dim, eRaw));
                                        return s <= e ? { start: s, end: e } : { start: e, end: s };
                                    };

                                    const i = norm(subset.i_range, gridItem.dimensions.i);
                                    const j = norm(subset.j_range, gridItem.dimensions.j);
                                    const k = norm(subset.k_range, gridItem.dimensions.k);

                                    let subsetMesh: MeshGeometry;

                                    if (gridItem.hasSolution && scalarField !== 'none' && gridItem.solutionCacheId) {
                                        try {
                                            await fieldRangesPromise;
                                            const range = displayedGlobalRange ?? fieldRangeMap.get(gridItem.solutionCacheId);
                                            subsetMesh = await invoke<MeshGeometry>('compute_solution_colors_subset_by_id', {
                                                gridId: gridItem.gridCacheId!,
                                                solutionId: gridItem.solutionCacheId,
                                                iStart: i.start,
                                                iEnd: i.end,
                                                jStart: j.start,
                                                jEnd: j.end,
                                                kStart: k.start,
                                                kEnd: k.end,
                                                field: scalarField,
                                                colorScheme: colorScheme,
                                                respectIblank: !ignoreIblank,
                                                showFringePoints: showFringePoints,
                                                iblankFilterMode: iblankFilterMode,
                                                globalMin: range?.min,
                                                globalMax: range?.max,
                                            });
                                        } catch (colorErr) {
                                            logger.error(`Solution coloring on subset failed: ${colorErr}`, 'Viewer3D');
                                            const subsetGrid = await invoke<SerializableGrid>('subset_grid_by_id', {
                                                gridId: gridItem.gridCacheId!,
                                                iStart: i.start,
                                                iEnd: i.end,
                                                jStart: j.start,
                                                jEnd: j.end,
                                                kStart: k.start,
                                                kEnd: k.end,
                                            });
                                            subsetMesh = await invoke<MeshGeometry>('convert_grid_to_mesh', {
                                                grid: subsetGrid,
                                                respectIblank: !ignoreIblank,
                                                showFringePoints: showFringePoints,
                                                iblankFilterMode: iblankFilterMode,
                                            });
                                        }
                                    } else {
                                        const subsetGrid = await invoke<SerializableGrid>('subset_grid_by_id', {
                                            gridId: gridItem.gridCacheId!,
                                            iStart: i.start,
                                            iEnd: i.end,
                                            jStart: j.start,
                                            jEnd: j.end,
                                            kStart: k.start,
                                            kEnd: k.end,
                                        });
                                        subsetMesh = await invoke<MeshGeometry>('convert_grid_to_mesh', {
                                            grid: subsetGrid,
                                            respectIblank: !ignoreIblank,
                                            showFringePoints: showFringePoints,
                                            iblankFilterMode: iblankFilterMode,
                                        });
                                    }
                                    return { subsetId: `${subset.grid}:${subsetIndex}`, mesh: subsetMesh };
                                })
                            );

                            // Merge all subset meshes into one
                            if (subsetMeshes.length > 0) {
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
                                const allHaveColors = subsetMeshes.every(({ mesh }) => mesh.colors && mesh.colors.length > 0);
                                void invoke('frontend_log', {
                                    message: `[Viewer3D] Merging ${subsetMeshes.length} subsets, allHaveColors=${allHaveColors}`
                                });
                                subsetMeshes.forEach((sm, idx) => {
                                    void invoke('frontend_log', {
                                        message: `[Viewer3D]  Subset ${idx}: verts=${sm.mesh.vertices.length / 3 | 0}, colors=${sm.mesh.colors?.length || 0}`
                                    });
                                });

                                if (allHaveColors) {
                                    mergedMesh.colors = [];
                                }

                                for (const { mesh: sliceMesh } of subsetMeshes) {
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
                                throw new Error('No subset meshes generated');
                            }
                        } catch (subsetErr) {
                            const subsetMsg = String(subsetErr);
                            logger.error(`Subset rendering failed: ${subsetMsg}`, 'Viewer3D');
                            throw subsetErr;
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
    }, [grids, ignoreIblank, showFringePoints, iblankFilterMode, scalarField, colorScheme, sliceEnabled, subsetsContentKey, subsetsByGridId, appliedSlicesKey]);
    const visibleGrids = useMemo(
        () => getVisibleGridItems(grids, selectedGridIds, isolateSelected),
        [grids, isolateSelected, selectedGridIds]
    );


    // Contour extraction effect
    useEffect(() => {
        let isCancelled = false;
        if (!isContourPlotFamily || scalarField === 'none') {
            // Clear contours when disabled or no field selected
            setIsoSurfaceGeometries({});
            setContourLineGeometries({});
            return;
        }

        const wantsIsoSurfaces = contourAttribute === 'surface' || contourAttribute === 'color_contours';
        const wantsContourLines = contourAttribute === 'line' || contourAttribute === 'grid' || contourAttribute === 'dots';

        const gridsWithSolution = grids.filter(g => g.hasSolution && g.solutionCacheId && g.gridCacheId);
        if (gridsWithSolution.length === 0) {
            return;
        }

        const resolveContourLevels = async (): Promise<number[]> => {
            const refSolution = gridsWithSolution.find(g => g.solutionCacheId != null);
            if (!refSolution) {
                return [];
            }
            try {
                const result = await invoke<{ levels: number[]; diagnostics: unknown[] }>(
                    'resolve_contour_levels',
                    { solutionId: refSolution.solutionCacheId!, scalarField }
                );
                return result.levels;
            } catch (err) {
                logger.warn(`resolve_contour_levels failed: ${err}`, 'Viewer3D');
                return [];
            }
        };

        void (async () => {
            const absoluteLevels = await resolveContourLevels();
            if (isCancelled) {
                return;
            }

            if (absoluteLevels.length === 0) {
                setIsoSurfaceGeometries({});
                setContourLineGeometries({});
                return;
            }

            const uniqueLevels = Array.from(new Set(absoluteLevels)).sort((a, b) => a - b);

            logger.info(
                `Extracting contours at ${uniqueLevels.length} level(s) for field ${scalarField}`,
                'Viewer3D'
            );

            // Extract iso-surfaces for each grid and each resolved level.
            const isoSurfacePromises = wantsIsoSurfaces
                ? gridsWithSolution.flatMap((gridItem) =>
                    uniqueLevels.map(async (level, levelIndex) => {
                        try {
                            const mesh = await invoke<MeshGeometry>('extract_iso_surface_by_id', {
                                gridId: gridItem.gridCacheId!,
                                solutionId: gridItem.solutionCacheId!,
                                scalarField: scalarField,
                                levelAbsolute: level,
                                respectIblank: !ignoreIblank,
                                showFringePoints: showFringePoints,
                                iblankFilterMode: iblankFilterMode,
                            });
                            return { id: `${gridItem.id}::lvl${levelIndex}`, mesh, level };
                        } catch (err) {
                            logger.warn(`Failed to extract iso-surface for grid ${gridItem.id}: ${err}`, 'Viewer3D');
                            return null;
                        }
                    })
                )
                : [];

            // Extract contour lines for slices (only visible grids).
            const visibleGridIds = new Set(visibleGrids.map(g => g.id));
            const sliceContourPromises = wantsContourLines
                ? gridsWithSolution
                    .filter(gridItem => visibleGridIds.has(gridItem.id))
                    .flatMap((gridItem) => {
                        const slices = subsetSlicesByGridId[gridItem.id] || [];
                        return slices.flatMap((slice) =>
                            uniqueLevels.map(async (level, levelIndex) => {
                                try {
                                    const lineData = await invoke<number[]>('extract_slice_contours_by_id', {
                                        gridId: gridItem.gridCacheId!,
                                        solutionId: gridItem.solutionCacheId!,
                                        plane: slice.plane,
                                        index: slice.index,
                                        scalarField: scalarField,
                                        levelAbsolute: level,
                                        respectIblank: !ignoreIblank,
                                        showFringePoints: showFringePoints,
                                        iblankFilterMode: iblankFilterMode,
                                    });
                                    return {
                                        id: `slice::${gridItem.id}::${slice.id}::lvl${levelIndex}`,
                                        level,
                                        lineData: new Float32Array(lineData)
                                    };
                                } catch (err) {
                                    logger.warn(`Failed to extract slice contours for ${gridItem.id}/${slice.id}: ${err}`, 'Viewer3D');
                                    return null;
                                }
                            })
                        );
                    })
                : [];

            // Extract contour lines for arbitrary planes (only visible grids).
            const arbitraryContourPromises = wantsContourLines
                ? (arbitrarySlices || [])
                    .filter(s => s.enabled && s.applied)
                    .flatMap((slice) => {
                        return gridsWithSolution
                            .flatMap((gridItem) =>
                                [
                                    (async () => {
                                        try {
                                            const lineSets = await invoke<number[][]>('extract_arbitrary_plane_contours_multi_by_id', {
                                                gridId: gridItem.gridCacheId!,
                                                solutionId: gridItem.solutionCacheId!,
                                                planePoint: slice.planePoint,
                                                planeNormal: slice.planeNormal,
                                                scalarField: scalarField,
                                                levelsAbsolute: uniqueLevels,
                                                respectIblank: !ignoreIblank,
                                                showFringePoints: showFringePoints,
                                                iblankFilterMode: iblankFilterMode,
                                            });

                                            return uniqueLevels.map((level, levelIndex) => ({
                                                id: `arbitrary::${slice.id}::${gridItem.id}::lvl${levelIndex}`,
                                                level,
                                                lineData: new Float32Array(lineSets[levelIndex] ?? []),
                                            }));
                                        } catch {
                                            // Plane may not intersect this grid - expected.
                                            return [] as Array<{ id: string; level: number; lineData: Float32Array }>;
                                        }
                                    })(),
                                ]
                            );
                    })
                : [];

            Promise.all([
                Promise.all(isoSurfacePromises),
                Promise.all(sliceContourPromises),
                Promise.all(arbitraryContourPromises),
            ]).then(([isoResults, sliceResults, arbitraryResults]) => {
                if (isCancelled) {
                    return;
                }
                // Update iso-surfaces.
                const minLevel = uniqueLevels[0];
                const maxLevel = uniqueLevels[uniqueLevels.length - 1];
                const newIsoSurfaces: Record<string, IsoSurfaceGeometry> = {};
                isoResults.forEach(result => {
                    if (result && result.mesh.vertex_count > 0) {
                        let color = '#3b82f6';
                        if (contourAttribute === 'color_contours') {
                            const normalized = normalizeValue(result.level, minLevel, maxLevel);
                            const rgb = mapValueToColor(normalized, colorScheme);
                            color = rgbToHex(rgb);
                        }
                        newIsoSurfaces[result.id] = { mesh: result.mesh, level: result.level, color };
                    }
                });
                setIsoSurfaceGeometries(newIsoSurfaces);

                // Update contour lines.
                const newContourLines: Record<string, ContourLineGeometry> = {};
                const flattenedArbitraryResults = arbitraryResults.flat();
                [...sliceResults, ...flattenedArbitraryResults].forEach(result => {
                    if (result && result.lineData.length > 0) {
                        const normalized = normalizeValue(result.level, minLevel, maxLevel);
                        const rgb = mapValueToColor(normalized, colorScheme);
                        newContourLines[result.id] = {
                            lineData: result.lineData,
                            color: rgbToHex(rgb),
                        };
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
        })();

        return () => {
            isCancelled = true;
        };
    }, [
        isContourPlotFamily,
        contourAttribute,
        contourSpecKey,
        scalarField,
        grids,
        subsetSlicesByGridId,
        contourArbitrarySlicesKey,
        ignoreIblank,
        showFringePoints,
        iblankFilterMode,
        visibleGrids,
        colorScheme,
    ]);

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
                <CameraViewpointSync
                    cameraAxisView={cameraAxisView}
                    cameraViewpoint={cameraViewpoint}
                    isUserNavigatingRef={isUserNavigatingRef}
                    controlsRef={controlsRef}
                />

                {/* Render mesh based on selected mode */}
                {visibleGrids.map((gridItem) => {
                    const mesh = meshById[gridItem.id];
                    if (!mesh) {
                        return null;
                    }
                    const dimmed = selectedGridIds.length > 0 && !selectedGridIds.includes(gridItem.id) && !isolateSelected;
                    // Contour family uses neutral context geometry; function-surface keeps field coloring.
                    const displayColor = isContourPlotFamily ? '#808080' : gridItem.color;

                    return (
                        <group key={gridItem.id}>
                            {/* Render smooth shaded surface */}
                            {shadingMode === 'smooth' && (
                                <SolidMeshRenderer
                                    meshGeometry={mesh}
                                    color={displayColor}
                                    dimmed={dimmed}
                                    forceSolidColor={isContourPlotFamily}
                                />
                            )}
                            {/* Render wireframe */}
                            {showWireframe && (
                                <MeshRenderer
                                    meshGeometry={mesh}
                                    color={displayColor}
                                    dimmed={dimmed}
                                    forceSolidColor={isContourPlotFamily}
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
                        // Contour family uses neutral context geometry; function-surface keeps contrasty slice color.
                        const sliceColor = isContourPlotFamily ? '#808080' : '#60a5fa';
                        return (
                            <group key={id}>
                                {shadingMode === 'smooth' && (
                                    <SolidMeshRenderer
                                        meshGeometry={mesh}
                                        color={sliceColor}
                                        dimmed={false}
                                        forceSolidColor={isContourPlotFamily}
                                    />
                                )}
                                {showWireframe && (
                                    <MeshRenderer
                                        meshGeometry={mesh}
                                        color={sliceColor}
                                        dimmed={false}
                                        forceSolidColor={isContourPlotFamily}
                                    />
                                )}
                            </group>
                        );
                    })}

                {/* Render iso-surfaces (surface-based contour attributes) */}
                {isContourPlotFamily && (contourAttribute === 'surface' || contourAttribute === 'color_contours') &&
                    Object.entries(isoSurfaceGeometries).map(([id, iso]) => (
                        <group key={`iso::${id}`}>
                            <IsoSurfaceRenderer meshGeometry={iso.mesh} color={iso.color} opacity={isoSurfaceOpacity} />
                        </group>
                    ))
                }

                {/* Render contour lines (line attribute, or grid/dots as first-pass fallback) */}
                {isContourPlotFamily && (contourAttribute === 'line' || contourAttribute === 'grid' || contourAttribute === 'dots') &&
                    mergedContourLinesByColor.map((contour, idx) => (
                        <group key={`contour-color::${contour.color}::${idx}`}>
                            <ContourLineRenderer lineData={contour.lineData} color={contour.color} />
                        </group>
                    ))
                }

                {/* Camera controls */}
                <CameraCommitControls
                    onCameraCommit={onCameraCommit}
                    isUserNavigatingRef={isUserNavigatingRef}
                    controlsRef={controlsRef}
                />
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

                {renderNotice && (
                    <div style={{ marginTop: isLoading ? '8px' : '0', color: '#facc15', maxWidth: '240px' }}>
                        {renderNotice}
                    </div>
                )}

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
