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
import { resolveCameraUpVector, type CameraPlotUpAxis } from './cameraUp';

interface MeshGeometry {
    vertices: number[];
    indices: number[];
    triangle_indices: number[];
    normals: number[];
    vertex_count: number;
    face_count: number;
    colors?: number[];
    scalar_values?: number[]; // Raw scalar field values (1 per vertex)
    probe_components?: number[]; // Interleaved per-vertex components: rho,rhou,rhov,rhow,rhoe,gamma
    probe_ijk?: number[]; // Interleaved per-vertex indices: i,j,k (1-based)
}

type ProbeMode = 'off' | 'interpolated' | 'snap';

interface ProbeFields {
    density: number;
    momentum_x: number;
    momentum_y: number;
    momentum_z: number;
    energy: number;
    u_velocity: number;
    v_velocity: number;
    w_velocity: number;
    velocity_magnitude: number;
    pressure: number;
    gamma: number;
}

interface ProbeInfo {
    position: [number, number, number];
    scalarValue: number | null;
    gridId: string;
    worldPosition: [number, number, number];
    ijkIndex: [number, number, number] | null;
    mode: Exclude<ProbeMode, 'off'>;
    fields: ProbeFields | null;
}

interface ProbeTarget {
    probeId: string;
    displayGridId: string;
    mesh: MeshGeometry;
}

const PROBE_COMPONENT_STRIDE = 6;
const PROBE_IJK_STRIDE = 3;

function computeProbeFieldsFromComponents(components: [number, number, number, number, number, number]): ProbeFields {
    const [rho, rhou, rhov, rhow, rhoe, gammaIn] = components;
    const gamma = Number.isFinite(gammaIn) && gammaIn > 0 ? gammaIn : 1.4;

    const safeRho = Number.isFinite(rho) ? rho : 0;
    const safeRhou = Number.isFinite(rhou) ? rhou : 0;
    const safeRhov = Number.isFinite(rhov) ? rhov : 0;
    const safeRhow = Number.isFinite(rhow) ? rhow : 0;
    const safeRhoe = Number.isFinite(rhoe) ? rhoe : 0;

    const u = safeRho > 0 ? safeRhou / safeRho : 0;
    const v = safeRho > 0 ? safeRhov / safeRho : 0;
    const w = safeRho > 0 ? safeRhow / safeRho : 0;
    const velocityMagnitude = Math.sqrt(u * u + v * v + w * w);
    const kineticEnergy = 0.5 * safeRho * (u * u + v * v + w * w);
    const pressure = safeRho > 0 ? (gamma - 1.0) * (safeRhoe - kineticEnergy) : 0;

    return {
        density: safeRho,
        momentum_x: safeRhou,
        momentum_y: safeRhov,
        momentum_z: safeRhow,
        energy: safeRhoe,
        u_velocity: u,
        v_velocity: v,
        w_velocity: w,
        velocity_magnitude: velocityMagnitude,
        pressure,
        gamma,
    };
}

function formatProbeNumeric(value: number): string {
    if (!Number.isFinite(value)) {
        return 'NaN';
    }
    const abs = Math.abs(value);
    if (abs > 0 && (abs < 0.001 || abs >= 10000)) {
        return value.toExponential(6);
    }
    return value.toFixed(6);
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
    walls?: BackendGridSubset[];
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
    cameraPlotUp?: CameraPlotUpAxis | null;
    onCameraCommit?: (vp: { x: number; y: number; z: number }) => void;
    onLoadingChange?: (isLoading: boolean) => void;
    colorMapMin?: number | null;
    colorMapMax?: number | null;
    onActualRangeChange?: (min: number, max: number) => void;
    vectors?: {
        scalar_function?: number | null;
        scalar_function_disabled: boolean;
        length_scale?: number | null;
        attributes_enabled?: boolean | null;
    } | null;
    rakes?: {
        coordinate_mode?: 'ijk' | 'xyz' | null;
        add: boolean;
        attributes_enabled?: boolean | null;
        io_mode?: { kind: 'read' | 'write'; path: string } | null;
        time_mode?: 'plus' | 'minus' | 'plus_minus' | null;
        max_points?: number | null;
        scalar_function?: number | null;
        scalar_function_disabled: boolean;
    } | null;
}

function CameraViewpointSync({
    cameraAxisView,
    cameraViewpoint,
    cameraPlotUp,
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
    cameraPlotUp?: CameraPlotUpAxis | null;
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

    const plotUpDirection = (plotUp: CameraPlotUpAxis | null): THREE.Vector3 | null => {
        switch (plotUp) {
            case 'positive_x':
                return new THREE.Vector3(1, 0, 0);
            case 'positive_y':
                return new THREE.Vector3(0, 1, 0);
            case 'positive_z':
                return new THREE.Vector3(0, 0, 1);
            case 'negative_x':
                return new THREE.Vector3(-1, 0, 0);
            case 'negative_y':
                return new THREE.Vector3(0, -1, 0);
            case 'negative_z':
                return new THREE.Vector3(0, 0, -1);
            default:
                return null;
        }
    };

    const applyCameraUpForPosition = (position: THREE.Vector3) => {
        const requestedUp = plotUpDirection(cameraPlotUp ?? null);
        const nextUp = resolveCameraUpVector(position, requestedUp);
        if (camera.up.distanceToSquared(nextUp) < 1e-8) {
            return;
        }
        camera.up.copy(nextUp);
    };

    useEffect(() => {
        applyCameraUpForPosition(camera.position);
        if (controlsRef.current) {
            controlsRef.current.update();
        } else {
            camera.lookAt(0, 0, 0);
        }
    }, [camera, cameraPlotUp, controlsRef]);

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
        applyCameraUpForPosition(camera.position);
        if (controlsRef.current) {
            controlsRef.current.target.set(0, 0, 0);
            controlsRef.current.update();
        } else {
            camera.lookAt(0, 0, 0);
        }
        lastAppliedAxisViewRef.current = null;
    }, [camera, cameraViewpoint, cameraPlotUp, controlsRef, isUserNavigatingRef]);

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
        applyCameraUpForPosition(camera.position);
        if (controlsRef.current) {
            controlsRef.current.target.set(0, 0, 0);
            controlsRef.current.update();
        } else {
            camera.lookAt(0, 0, 0);
        }
        lastAppliedAxisViewRef.current = cameraAxisView;
    }, [camera, cameraAxisView, cameraViewpoint, cameraPlotUp, controlsRef, isUserNavigatingRef]);

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

// Iso-surface renderer — current implementation detail for SURFACE-style contours.
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

function VectorFieldRenderer({
    meshGeometry,
    colorScheme,
    vectors,
}: {
    meshGeometry: MeshGeometry;
    colorScheme: ColorScheme;
    vectors: {
        scalar_function?: number | null;
        scalar_function_disabled: boolean;
        length_scale?: number | null;
        attributes_enabled?: boolean | null;
    };
}) {
    const lineGeometry = useMemo(() => {
        const probe = meshGeometry.probe_components;
        const vertexCount = meshGeometry.vertex_count;
        if (!probe || vertexCount <= 0 || probe.length !== vertexCount * PROBE_COMPONENT_STRIDE) {
            return null;
        }

        const targetArrowCount = 450;
        const sampleStride = Math.max(1, Math.floor(vertexCount / targetArrowCount));

        let minX = Number.POSITIVE_INFINITY;
        let minY = Number.POSITIVE_INFINITY;
        let minZ = Number.POSITIVE_INFINITY;
        let maxX = Number.NEGATIVE_INFINITY;
        let maxY = Number.NEGATIVE_INFINITY;
        let maxZ = Number.NEGATIVE_INFINITY;
        for (let i = 0; i < meshGeometry.vertices.length; i += 3) {
            const x = meshGeometry.vertices[i];
            const y = meshGeometry.vertices[i + 1];
            const z = meshGeometry.vertices[i + 2];
            minX = Math.min(minX, x);
            minY = Math.min(minY, y);
            minZ = Math.min(minZ, z);
            maxX = Math.max(maxX, x);
            maxY = Math.max(maxY, y);
            maxZ = Math.max(maxZ, z);
        }
        const dx = maxX - minX;
        const dy = maxY - minY;
        const dz = maxZ - minZ;
        const bboxDiag = Math.sqrt(dx * dx + dy * dy + dz * dz);

        const sampled: Array<{ index: number; vx: number; vy: number; vz: number; mag: number }> = [];
        let maxMag = 0;
        let minMag = Number.POSITIVE_INFINITY;

        for (let vertexIndex = 0; vertexIndex < vertexCount; vertexIndex += sampleStride) {
            const start = vertexIndex * PROBE_COMPONENT_STRIDE;
            const rho = probe[start];
            const rhou = probe[start + 1];
            const rhov = probe[start + 2];
            const rhow = probe[start + 3];

            if (!Number.isFinite(rho) || Math.abs(rho) < 1e-12) {
                continue;
            }

            const vx = rhou / rho;
            const vy = rhov / rho;
            const vz = rhow / rho;
            const mag = Math.sqrt(vx * vx + vy * vy + vz * vz);
            if (!Number.isFinite(mag) || mag <= 1e-10) {
                continue;
            }

            sampled.push({ index: vertexIndex, vx, vy, vz, mag });
            maxMag = Math.max(maxMag, mag);
            minMag = Math.min(minMag, mag);
        }

        if (sampled.length === 0 || maxMag <= 0) {
            return null;
        }

        const lengthScale = Math.max(0.01, vectors.length_scale ?? 1.0);
        const baseArrowLength = Math.max(1e-4, bboxDiag * 0.04 * lengthScale);
        const useAttributes = vectors.attributes_enabled !== false;

        const positions: number[] = [];
        const colors: number[] = [];

        for (const sample of sampled) {
            const p = sample.index * 3;
            const x0 = meshGeometry.vertices[p];
            const y0 = meshGeometry.vertices[p + 1];
            const z0 = meshGeometry.vertices[p + 2];

            const dirScale = baseArrowLength * (sample.mag / maxMag);
            const invMag = 1.0 / sample.mag;
            const x1 = x0 + sample.vx * invMag * dirScale;
            const y1 = y0 + sample.vy * invMag * dirScale;
            const z1 = z0 + sample.vz * invMag * dirScale;

            positions.push(x0, y0, z0, x1, y1, z1);

            if (useAttributes) {
                const t = normalizeValue(sample.mag, minMag, maxMag);
                const rgb = mapValueToColor(t, colorScheme);
                colors.push(rgb.r / 255, rgb.g / 255, rgb.b / 255, rgb.r / 255, rgb.g / 255, rgb.b / 255);
            }
        }

        if (positions.length === 0) {
            return null;
        }

        const geo = new BufferGeometry();
        geo.setAttribute('position', new BufferAttribute(new Float32Array(positions), 3));
        if (useAttributes && colors.length === positions.length) {
            geo.setAttribute('color', new BufferAttribute(new Float32Array(colors), 3));
        }
        geo.computeBoundingSphere();
        return geo;
    }, [colorScheme, meshGeometry, vectors.attributes_enabled, vectors.length_scale]);

    if (!lineGeometry) {
        return null;
    }

    const useAttributes = vectors.attributes_enabled !== false;
    return (
        <lineSegments geometry={lineGeometry} frustumCulled={true}>
            <lineBasicMaterial
                color={useAttributes ? '#ffffff' : '#f59e0b'}
                vertexColors={useAttributes}
                transparent={false}
                depthTest={true}
                depthWrite={true}
            />
        </lineSegments>
    );
}

function RakeFieldRenderer({
    meshGeometry,
    colorScheme,
    rakes,
}: {
    meshGeometry: MeshGeometry;
    colorScheme: ColorScheme;
    rakes: {
        coordinate_mode?: 'ijk' | 'xyz' | null;
        add: boolean;
        attributes_enabled?: boolean | null;
        io_mode?: { kind: 'read' | 'write'; path: string } | null;
        time_mode?: 'plus' | 'minus' | 'plus_minus' | null;
        max_points?: number | null;
        scalar_function?: number | null;
        scalar_function_disabled: boolean;
    };
}) {
    const lineGeometry = useMemo(() => {
        const probe = meshGeometry.probe_components;
        const vertexCount = meshGeometry.vertex_count;
        if (!probe || vertexCount <= 0 || probe.length !== vertexCount * PROBE_COMPONENT_STRIDE) {
            return null;
        }

        let minX = Number.POSITIVE_INFINITY;
        let minY = Number.POSITIVE_INFINITY;
        let minZ = Number.POSITIVE_INFINITY;
        let maxX = Number.NEGATIVE_INFINITY;
        let maxY = Number.NEGATIVE_INFINITY;
        let maxZ = Number.NEGATIVE_INFINITY;
        for (let i = 0; i < meshGeometry.vertices.length; i += 3) {
            const x = meshGeometry.vertices[i];
            const y = meshGeometry.vertices[i + 1];
            const z = meshGeometry.vertices[i + 2];
            minX = Math.min(minX, x);
            minY = Math.min(minY, y);
            minZ = Math.min(minZ, z);
            maxX = Math.max(maxX, x);
            maxY = Math.max(maxY, y);
            maxZ = Math.max(maxZ, z);
        }
        const dx = maxX - minX;
        const dy = maxY - minY;
        const dz = maxZ - minZ;
        const bboxDiag = Math.sqrt(dx * dx + dy * dy + dz * dz);

        const maxSeeds = Math.max(8, Math.min(600, rakes.max_points ?? 120));
        const fieldTarget = Math.max(200, Math.min(2200, maxSeeds * 4));
        const fieldStride = Math.max(1, Math.floor(vertexCount / fieldTarget));

        const sampled: Array<{ x: number; y: number; z: number; vx: number; vy: number; vz: number; mag: number }> = [];
        let maxMag = 0;
        let minMag = Number.POSITIVE_INFINITY;

        for (let vertexIndex = 0; vertexIndex < vertexCount; vertexIndex += fieldStride) {
            const start = vertexIndex * PROBE_COMPONENT_STRIDE;
            const rho = probe[start];
            const rhou = probe[start + 1];
            const rhov = probe[start + 2];
            const rhow = probe[start + 3];

            if (!Number.isFinite(rho) || Math.abs(rho) < 1e-12) {
                continue;
            }

            const vx = rhou / rho;
            const vy = rhov / rho;
            const vz = rhow / rho;
            const mag = Math.sqrt(vx * vx + vy * vy + vz * vz);
            if (!Number.isFinite(mag) || mag <= 1e-10) {
                continue;
            }

            const p = vertexIndex * 3;
            const x = meshGeometry.vertices[p];
            const y = meshGeometry.vertices[p + 1];
            const z = meshGeometry.vertices[p + 2];

            sampled.push({ x, y, z, vx, vy, vz, mag });
            maxMag = Math.max(maxMag, mag);
            minMag = Math.min(minMag, mag);
        }

        if (sampled.length === 0 || maxMag <= 0) {
            return null;
        }

        const seedStride = Math.max(1, Math.floor(sampled.length / maxSeeds));
        const seedSamples = sampled.filter((_, idx) => idx % seedStride === 0).slice(0, maxSeeds);
        const lengthScale = Math.max(0.01, rakes.max_points != null ? Math.min(4, Math.max(0.25, rakes.max_points / 120)) : 1);
        const baseStepLength = Math.max(1e-4, bboxDiag * 0.008 * lengthScale);
        const stepCount = 12;
        const useAttributes = rakes.attributes_enabled !== false;

        const directions: number[] =
            rakes.time_mode === 'minus'
                ? [-1]
                : rakes.time_mode === 'plus_minus'
                    ? [1, -1]
                    : [1];

        const positions: number[] = [];
        const colors: number[] = [];

        const sampleVelocityAt = (x: number, y: number, z: number): { vx: number; vy: number; vz: number; mag: number } | null => {
            const nearest: Array<{ d2: number; sample: (typeof sampled)[number] }> = [];
            const k = 6;

            for (const s of sampled) {
                const dxs = s.x - x;
                const dys = s.y - y;
                const dzs = s.z - z;
                const d2 = dxs * dxs + dys * dys + dzs * dzs;
                if (!Number.isFinite(d2)) {
                    continue;
                }
                if (d2 < 1e-14) {
                    return { vx: s.vx, vy: s.vy, vz: s.vz, mag: s.mag };
                }

                if (nearest.length < k) {
                    nearest.push({ d2, sample: s });
                    nearest.sort((a, b) => a.d2 - b.d2);
                } else if (d2 < nearest[k - 1].d2) {
                    nearest[k - 1] = { d2, sample: s };
                    nearest.sort((a, b) => a.d2 - b.d2);
                }
            }

            if (nearest.length === 0) {
                return null;
            }

            let wSum = 0;
            let vx = 0;
            let vy = 0;
            let vz = 0;
            for (const entry of nearest) {
                const w = 1.0 / Math.max(1e-12, entry.d2);
                wSum += w;
                vx += entry.sample.vx * w;
                vy += entry.sample.vy * w;
                vz += entry.sample.vz * w;
            }

            if (wSum <= 0 || !Number.isFinite(wSum)) {
                return null;
            }

            vx /= wSum;
            vy /= wSum;
            vz /= wSum;
            const mag = Math.sqrt(vx * vx + vy * vy + vz * vz);
            if (!Number.isFinite(mag) || mag <= 1e-10) {
                return null;
            }
            return { vx, vy, vz, mag };
        };

        for (const seed of seedSamples) {

            for (const sign of directions) {
                let px = seed.x;
                let py = seed.y;
                let pz = seed.z;

                for (let step = 0; step < stepCount; step += 1) {
                    const v1 = sampleVelocityAt(px, py, pz);
                    if (!v1) {
                        break;
                    }

                    const v1Inv = 1.0 / v1.mag;
                    const h = baseStepLength * (0.35 + 0.65 * (v1.mag / maxMag));
                    const midX = px + sign * v1.vx * v1Inv * h * 0.5;
                    const midY = py + sign * v1.vy * v1Inv * h * 0.5;
                    const midZ = pz + sign * v1.vz * v1Inv * h * 0.5;

                    const v2 = sampleVelocityAt(midX, midY, midZ);
                    if (!v2) {
                        break;
                    }
                    const v2Inv = 1.0 / v2.mag;
                    const nx = px + sign * v2.vx * v2Inv * h;
                    const ny = py + sign * v2.vy * v2Inv * h;
                    const nz = pz + sign * v2.vz * v2Inv * h;

                    if (
                        nx < minX || nx > maxX ||
                        ny < minY || ny > maxY ||
                        nz < minZ || nz > maxZ
                    ) {
                        break;
                    }

                    positions.push(px, py, pz, nx, ny, nz);

                    if (useAttributes) {
                        const t = normalizeValue(v2.mag, minMag, maxMag);
                        const rgb = mapValueToColor(t, colorScheme);
                        colors.push(
                            rgb.r / 255,
                            rgb.g / 255,
                            rgb.b / 255,
                            rgb.r / 255,
                            rgb.g / 255,
                            rgb.b / 255
                        );
                    } else {
                        colors.push(
                            34 / 255,
                            197 / 255,
                            94 / 255,
                            34 / 255,
                            197 / 255,
                            94 / 255
                        );
                    }

                    px = nx;
                    py = ny;
                    pz = nz;
                }
            }
        }

        if (positions.length === 0) {
            return null;
        }

        const geo = new BufferGeometry();
        geo.setAttribute('position', new BufferAttribute(new Float32Array(positions), 3));
        if (colors.length === positions.length) {
            geo.setAttribute('color', new BufferAttribute(new Float32Array(colors), 3));
        }
        geo.computeBoundingSphere();
        return geo;
    }, [colorScheme, meshGeometry, rakes.attributes_enabled, rakes.max_points, rakes.time_mode]);

    if (!lineGeometry) {
        return null;
    }

    const useAttributes = rakes.attributes_enabled !== false;
    return (
        <lineSegments geometry={lineGeometry} frustumCulled={true}>
            <lineBasicMaterial
                color={useAttributes ? '#ffffff' : '#22c55e'}
                vertexColors={true}
                transparent={false}
                depthTest={true}
                depthWrite={true}
            />
        </lineSegments>
    );
}

// Point probe interaction handler component
function PointerInteractionHandler({
    probeTargets,
    probeMode,
    sampleRequestToken,
    raycasterRef,
    pointerRef,
    onProbe,
}: {
    probeTargets: ProbeTarget[];
    probeMode: ProbeMode;
    sampleRequestToken: number;
    raycasterRef: MutableRefObject<THREE.Raycaster>;
    pointerRef: MutableRefObject<THREE.Vector2>;
    onProbe: (info: ProbeInfo | null) => void;
}) {
    const { camera, gl } = useThree();
    const groupRef = useRef<THREE.Group>(null);
    const hasPointerSampleRef = useRef(false);

    const toVertex = (mesh: MeshGeometry, idx: number): THREE.Vector3 => {
        return new THREE.Vector3(
            mesh.vertices[idx * 3],
            mesh.vertices[idx * 3 + 1],
            mesh.vertices[idx * 3 + 2]
        );
    };

    const toProbeComponents = (
        probeComponents: number[] | undefined,
        idxA: number,
        idxB: number,
        idxC: number,
        wA: number,
        wB: number,
        wC: number
    ): [number, number, number, number, number, number] | null => {
        if (!probeComponents || probeComponents.length === 0) {
            return null;
        }

        const startA = idxA * PROBE_COMPONENT_STRIDE;
        const startB = idxB * PROBE_COMPONENT_STRIDE;
        const startC = idxC * PROBE_COMPONENT_STRIDE;
        if (
            startA + (PROBE_COMPONENT_STRIDE - 1) >= probeComponents.length ||
            startB + (PROBE_COMPONENT_STRIDE - 1) >= probeComponents.length ||
            startC + (PROBE_COMPONENT_STRIDE - 1) >= probeComponents.length
        ) {
            return null;
        }

        const out: number[] = [];
        for (let i = 0; i < PROBE_COMPONENT_STRIDE; i += 1) {
            out.push(
                wA * probeComponents[startA + i] +
                wB * probeComponents[startB + i] +
                wC * probeComponents[startC + i]
            );
        }
        return out as [number, number, number, number, number, number];
    };

    const probeIjkAtVertex = (probeIjk: number[] | undefined, idx: number): [number, number, number] | null => {
        if (!probeIjk || probeIjk.length === 0) {
            return null;
        }
        const start = idx * PROBE_IJK_STRIDE;
        if (start + (PROBE_IJK_STRIDE - 1) >= probeIjk.length) {
            return null;
        }
        return [probeIjk[start], probeIjk[start + 1], probeIjk[start + 2]];
    };

    const sampleProbeAtCurrentPointer = () => {
        if (probeMode === 'off') {
            return;
        }

        // Convert screen coordinates to normalized device coordinates
        // Update raycaster
        raycasterRef.current.setFromCamera(pointerRef.current, camera);

        // Test intersections with rendered probe targets
        if (groupRef.current && groupRef.current.children.length > 0) {
            const intersects = raycasterRef.current.intersectObjects(groupRef.current.children, true);

            if (intersects.length > 0) {
                const hit = intersects[0];
                const mesh = hit.object as THREE.Mesh;

                const targetId = String(mesh.userData.probeId ?? '');
                const matchedTarget = probeTargets.find((target) => target.probeId === targetId);

                if (matchedTarget) {
                    const face = hit.face;
                    if (face) {
                        const matchedMesh = matchedTarget.mesh;

                        // Triangle vertex indices
                        const a = face.a;
                        const b = face.b;
                        const c = face.c;

                        const point = hit.point;
                        const va = toVertex(matchedMesh, a);
                        const vb = toVertex(matchedMesh, b);
                        const vc = toVertex(matchedMesh, c);

                        // Compute barycentric coordinates
                        const v0 = vc.clone().sub(va);
                        const v1 = vb.clone().sub(va);
                        const v2 = point.clone().sub(va);

                        const dot00 = v0.dot(v0);
                        const dot01 = v0.dot(v1);
                        const dot02 = v0.dot(v2);
                        const dot11 = v1.dot(v1);
                        const dot12 = v1.dot(v2);

                        const denom = dot00 * dot11 - dot01 * dot01;
                        if (Math.abs(denom) < 1e-12) {
                            onProbe(null);
                            return;
                        }
                        const invDenom = 1 / denom;
                        const baryU = (dot11 * dot02 - dot01 * dot12) * invDenom;
                        const baryV = (dot00 * dot12 - dot01 * dot02) * invDenom;
                        const baryW = 1 - baryU - baryV;

                        let sampledPosition = point;
                        let scalarValue: number | null = null;
                        let fields: ProbeFields | null = null;
                        let ijkIndex: [number, number, number] | null = null;

                        if (probeMode === 'snap') {
                            const dA = point.distanceToSquared(va);
                            const dB = point.distanceToSquared(vb);
                            const dC = point.distanceToSquared(vc);
                            let snappedIdx = a;
                            let snappedPos = va;
                            if (dB < dA && dB <= dC) {
                                snappedIdx = b;
                                snappedPos = vb;
                            } else if (dC < dA && dC < dB) {
                                snappedIdx = c;
                                snappedPos = vc;
                            }

                            sampledPosition = snappedPos;

                            if (matchedMesh.scalar_values && snappedIdx < matchedMesh.scalar_values.length) {
                                scalarValue = matchedMesh.scalar_values[snappedIdx];
                            }

                            if (matchedMesh.probe_components) {
                                const start = snappedIdx * PROBE_COMPONENT_STRIDE;
                                if (start + (PROBE_COMPONENT_STRIDE - 1) < matchedMesh.probe_components.length) {
                                    fields = computeProbeFieldsFromComponents([
                                        matchedMesh.probe_components[start],
                                        matchedMesh.probe_components[start + 1],
                                        matchedMesh.probe_components[start + 2],
                                        matchedMesh.probe_components[start + 3],
                                        matchedMesh.probe_components[start + 4],
                                        matchedMesh.probe_components[start + 5],
                                    ]);
                                }
                            }

                            ijkIndex = probeIjkAtVertex(matchedMesh.probe_ijk, snappedIdx);
                        } else {
                            if (matchedMesh.scalar_values) {
                                const scalarValues = matchedMesh.scalar_values;
                                if (a < scalarValues.length && b < scalarValues.length && c < scalarValues.length) {
                                    scalarValue =
                                        baryW * scalarValues[a] + baryU * scalarValues[b] + baryV * scalarValues[c];
                                }
                            }

                            const components = toProbeComponents(
                                matchedMesh.probe_components,
                                a,
                                b,
                                c,
                                baryW,
                                baryU,
                                baryV
                            );
                            if (components) {
                                fields = computeProbeFieldsFromComponents(components);
                            }

                            let representativeIdx = a;
                            let representativeWeight = baryW;
                            if (baryU > representativeWeight) {
                                representativeIdx = b;
                                representativeWeight = baryU;
                            }
                            if (baryV > representativeWeight) {
                                representativeIdx = c;
                            }
                            ijkIndex = probeIjkAtVertex(matchedMesh.probe_ijk, representativeIdx);
                        }

                        onProbe({
                            position: [sampledPosition.x, sampledPosition.y, sampledPosition.z],
                            scalarValue,
                            gridId: matchedTarget.displayGridId,
                            worldPosition: [sampledPosition.x, sampledPosition.y, sampledPosition.z],
                            ijkIndex,
                            mode: probeMode,
                            fields,
                        });
                    }
                } else {
                    onProbe(null);
                }
            } else {
                // No hit - clear probe
                onProbe(null);
            }
        }
    };

    const handlePointerMove = (event: PointerEvent) => {
        const rect = gl.domElement.getBoundingClientRect();
        pointerRef.current.x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
        pointerRef.current.y = -((event.clientY - rect.top) / rect.height) * 2 + 1;
        hasPointerSampleRef.current = true;
    };

    useEffect(() => {
        const canvas = gl.domElement;
        canvas.addEventListener('pointermove', handlePointerMove);
        return () => {
            canvas.removeEventListener('pointermove', handlePointerMove);
        };
    }, [gl, camera, probeTargets, probeMode]);

    useEffect(() => {
        if (!hasPointerSampleRef.current) {
            logger.info('[Probe] sample request ignored: no pointer position sampled yet', 'Viewer3D');
            void invoke('frontend_log', {
                message: '[Viewer3D][Probe] sample request ignored: no pointer position sampled yet'
            }).catch(() => {
                // Ignore logging transport failures.
            });
            return;
        }
        logger.info(`[Probe] sample request token received -> sampling at current pointer (mode=${probeMode})`, 'Viewer3D');
        void invoke('frontend_log', {
            message: `[Viewer3D][Probe] sample request token received -> sampling at current pointer (mode=${probeMode})`
        }).catch(() => {
            // Ignore logging transport failures.
        });
        sampleProbeAtCurrentPointer();
    }, [sampleRequestToken]);

    // Create dummy geometry to ensure this component renders and children are set
    return (
        <group ref={groupRef}>
            {/* Invisible meshes for raycasting - built from raw vertex/index data */}
            {probeTargets.map((target) => {
                const meshData = target.mesh;
                if (!meshData || !meshData.triangle_indices || meshData.triangle_indices.length === 0) {
                    return null;
                }

                const geo = new THREE.BufferGeometry();
                geo.setAttribute('position', new THREE.BufferAttribute(new Float32Array(meshData.vertices), 3));
                geo.setIndex(new THREE.BufferAttribute(new Uint32Array(meshData.triangle_indices), 1));

                return (
                    <mesh key={`raycast-${target.probeId}`} geometry={geo} userData={{ probeId: target.probeId }}>
                        <meshBasicMaterial side={THREE.DoubleSide} transparent opacity={0} depthWrite={false} />
                    </mesh>
                );
            })}
        </group>
    );
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
    walls = [],
    subsets = [],
    arbitrarySlices = [],
    plotFamily = 'contour',
    contourAttribute = 'line' as 'line' | 'surface' | 'grid' | 'color_contours' | 'dots',
    contourSpec,
    isoSurfaceOpacity = 1.0,
    cameraAxisView = 'custom',
    cameraViewpoint,
    cameraPlotUp,
    onCameraCommit,
    onLoadingChange,
    colorMapMin,
    colorMapMax,
    onActualRangeChange,
    vectors = null,
    rakes = null,
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
            return 'CONTOURS levels/attributes are ignored when PLOT/SURFACE (CARPET/LINE) is active (current MVP behavior).';
        }
        if (isContourPlotFamily && (contourAttribute === 'grid' || contourAttribute === 'dots')) {
            return `${contourAttribute.toUpperCase()} CONTOURS attribute is not fully implemented yet; rendering LINE contours as a first-pass fallback.`;
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
    const lastReportedActualRangeRef = useRef<{ min: number; max: number } | null>(null);
    const requestIdRef = useRef(0);
    const isUserNavigatingRef = useRef(false);
    const controlsRef = useRef<any>(null);

    // Point probe state
    const [probeMode, setProbeMode] = useState<ProbeMode>('off');
    const [probeSampleRequestToken, setProbeSampleRequestToken] = useState(0);
    const [probeInfo, setProbeInfo] = useState<ProbeInfo | null>(null);
    const raycasterRef = useRef(new THREE.Raycaster());
    const pointerRef = useRef(new THREE.Vector2());
    const probeWindowReadyRef = useRef(false);

    const probeLog = (message: string) => {
        logger.info(`[Probe] ${message}`, 'Viewer3D');
        void invoke('frontend_log', { message: `[Viewer3D][Probe] ${message}` }).catch(() => {
            // Ignore logging transport failures so probe UX is unaffected.
        });
    };

    const ensureProbePopup = async () => {
        probeLog('ensureProbePopup start (tauri window)');
        try {
            await invoke('open_probe_window');
            probeWindowReadyRef.current = true;
            probeLog('open_probe_window succeeded');
        } catch (err) {
            probeWindowReadyRef.current = false;
            probeLog(`open_probe_window failed: ${err}`);
        }
    };

    useEffect(() => {
        probeLog('probe keydown listener installed');
        const handleKeyDown = (event: KeyboardEvent) => {
            if (event.key === 'Escape') {
                probeLog('Escape pressed -> disabling probe');
                setProbeMode('off');
                setProbeInfo(null);
                return;
            }

            if (event.key !== 'p' && event.key !== 'P') {
                return;
            }

            event.preventDefault();
            const wantsSnap = event.shiftKey || event.key === 'P';
            const nextMode: ProbeMode = wantsSnap ? 'snap' : 'interpolated';
            probeLog(`probe key pressed: key=${event.key} shift=${event.shiftKey ? '1' : '0'} nextMode=${nextMode}`);
            if (!probeWindowReadyRef.current) {
                void ensureProbePopup();
            }
            setProbeMode(nextMode);
            setProbeSampleRequestToken((prev) => prev + 1);
        };

        window.addEventListener('keydown', handleKeyDown);
        return () => {
            window.removeEventListener('keydown', handleKeyDown);
            probeLog('probe keydown listener removed');
        };
    }, []);

    useEffect(() => {
        probeLog(`probeMode changed -> ${probeMode}`);
        if (probeMode === 'off') {
            probeLog('closing probe popup because mode is off');
            probeWindowReadyRef.current = false;
            void invoke('close_probe_window').catch((err) => {
                probeLog(`close_probe_window failed: ${err}`);
            });
        }
    }, [probeMode]);

    useEffect(() => {
        return () => {
            probeLog('component unmount -> closing probe popup');
            probeWindowReadyRef.current = false;
            void invoke('close_probe_window').catch((err) => {
                probeLog(`close_probe_window on unmount failed: ${err}`);
            });
        };
    }, []);

    useEffect(() => {
        if (probeMode === 'off') {
            return;
        }

        if (!probeWindowReadyRef.current) {
            probeLog(`popup update skipped: probe window not ready (mode=${probeMode})`);
            return;
        }

        const modeText = probeMode === 'snap' ? 'SNAP (nearest grid point)' : 'INTERPOLATED';
        const fields = probeInfo?.fields;

        const fieldRows = fields
            ? [
                ['density', fields.density],
                ['pressure', fields.pressure],
                ['velocity_magnitude', fields.velocity_magnitude],
                ['u_velocity', fields.u_velocity],
                ['v_velocity', fields.v_velocity],
                ['w_velocity', fields.w_velocity],
                ['momentum_x', fields.momentum_x],
                ['momentum_y', fields.momentum_y],
                ['momentum_z', fields.momentum_z],
                ['energy', fields.energy],
                ['gamma', fields.gamma],
            ]
                .map(([label, value]) => `<tr><td>${label}</td><td>${formatProbeNumeric(value as number)}</td></tr>`)
                .join('')
            : '<tr><td colspan="2">No solution values at this point</td></tr>';

        const scalarLabel = scalarField === 'none' ? 'selected_scalar' : scalarField;
        const scalarValueText =
            probeInfo?.scalarValue == null ? 'N/A' : formatProbeNumeric(probeInfo.scalarValue);

        const popupHtml = `
            <style>
                body { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; margin: 0; padding: 14px; background: #111827; color: #e5e7eb; }
                .meta { margin-bottom: 10px; font-size: 12px; line-height: 1.5; }
                .title { font-size: 14px; font-weight: 700; margin-bottom: 6px; color: #93c5fd; }
                table { width: 100%; border-collapse: collapse; font-size: 12px; }
                td { border-top: 1px solid #374151; padding: 6px 4px; vertical-align: top; }
                td:first-child { color: #9ca3af; width: 46%; }
                td:last-child { color: #f9fafb; text-align: right; }
            </style>
            <div class="title">Point Probe</div>
            <div class="meta">Mode: ${modeText}</div>
            <div class="meta">Grid: ${probeInfo?.gridId ?? '---'}</div>
            <div class="meta">I,J,K: ${probeInfo?.ijkIndex ? `(${probeInfo.ijkIndex.join(', ')})` : '---'}</div>
            <div class="meta">Position: ${probeInfo
                ? `(${probeInfo.worldPosition.map((v) => formatProbeNumeric(v)).join(', ')})`
                : '---'}</div>
            <div class="meta">${scalarLabel}: ${scalarValueText}</div>
            <table>
                <tbody>${fieldRows}</tbody>
            </table>
        `;
        void invoke('update_probe_window_html', { html: popupHtml })
            .then(() => {
                probeLog(`popup content updated: mode=${probeMode} hasProbe=${probeInfo ? 'yes' : 'no'}`);
            })
            .catch((err) => {
                probeWindowReadyRef.current = false;
                probeLog(`popup content update failed: ${err}`);
            });
    }, [probeInfo, probeMode, scalarField]);

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

    const wallsContentKey = useMemo(
        () => (walls || [])
            .map((w) => {
                const i = w.i_range ? `${w.i_range.start}:${w.i_range.end ?? ''}` : '-';
                const j = w.j_range ? `${w.j_range.start}:${w.j_range.end ?? ''}` : '-';
                const k = w.k_range ? `${w.k_range.start}:${w.k_range.end ?? ''}` : '-';
                return `${w.grid}|${i}|${j}|${k}`;
            })
            .sort()
            .join(';'),
        [walls]
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

    const wallsByGridId = useMemo(() => {
        const byGrid: Record<string, BackendGridSubset[]> = {};
        for (const wall of walls) {
            const grid = grids.find((g) => g.gridIndex + 1 === wall.grid);
            if (!grid) {
                continue;
            }
            if (!byGrid[grid.id]) {
                byGrid[grid.id] = [];
            }
            byGrid[grid.id].push(wall);
        }
        return byGrid;
    }, [grids, wallsContentKey]);

    const effectiveRangesByGridId = useMemo(() => {
        if (sliceEnabled) {
            return subsetsByGridId;
        }
        return wallsByGridId;
    }, [sliceEnabled, subsetsByGridId, wallsByGridId]);

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

        const currentColorKey = `${scalarField}|${colorScheme}|${colorMapMin ?? ''}|${colorMapMax ?? ''}`;
        // Only include APPLIED slices in the slice key to avoid reprocessing while editing
        const sliceKey = `${sliceEnabled}|${ignoreIblank}|${showFringePoints}|${iblankFilterMode}|${subsetsContentKey}|${wallsContentKey}|${appliedSlicesKey}`;
        const shouldRecolor = lastColorKeyRef.current !== currentColorKey;
        const shouldReslice = lastSliceKeyRef.current !== sliceKey;

        void invoke('frontend_log', {
            message: `[Viewer3D] Color key check: last="${lastColorKeyRef.current}" current="${currentColorKey}" shouldRecolor=${shouldRecolor}`
        });
        void invoke('frontend_log', {
            message: `[Viewer3D] Slice key check: shouldReslice=${shouldReslice}`
        });

        const gridsWithRanges = grids.filter((grid) => (effectiveRangesByGridId[grid.id]?.length ?? 0) > 0);
        const hasEffectiveRanges = gridsWithRanges.length > 0;
        const hasAppliedArbitrarySlices = (arbitrarySlices || []).some(s => s.applied);

        if (sliceEnabled || hasEffectiveRanges) {
            // If there are no backend subsets, fall back to full-grid rendering.
            if (gridsWithRanges.length === 0 && !hasAppliedArbitrarySlices) {
                void invoke('frontend_log', {
                    message: '[Viewer3D] No effective ranges available; using full-grid fallback rendering'
                });
            }

            // Clean up subset meshes for grids without active subsets.
            if (gridsWithRanges.length > 0) {
                const gridsWithoutRanges = grids.filter((grid) => (effectiveRangesByGridId[grid.id]?.length ?? 0) === 0);
                const hasStaleMeshes = gridsWithoutRanges.some((grid) => meshById[grid.id]);
                if (hasStaleMeshes) {
                    setMeshById((prev) => {
                        const next = { ...prev };
                        gridsWithoutRanges.forEach((grid) => {
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
        const targetGrids = hasEffectiveRanges ? gridsWithRanges : grids;

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
                        const isFiniteRange = Number.isFinite(globalMin) && Number.isFinite(globalMax);
                        const lastReported = lastReportedActualRangeRef.current;
                        const rangeChanged = !lastReported
                            || lastReported.min !== globalMin
                            || lastReported.max !== globalMax;
                        if (isFiniteRange && rangeChanged) {
                            lastReportedActualRangeRef.current = { min: globalMin, max: globalMax };
                            onActualRangeChange?.(globalMin, globalMax);
                        }
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
                                            globalMin: colorMapMin ?? range?.min,
                                            globalMax: colorMapMax ?? range?.max,
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

                    // Apply effective backend ranges. GUI slice mode uses SUBSETS;
                    // non-slice script mode falls back to WALLS.
                    const gridRanges = effectiveRangesByGridId[gridItem.id] || [];
                    if (gridRanges.length > 0) {
                        try {
                            // Generate meshes for each range and merge them for display.
                            const subsetMeshes = await Promise.all(
                                gridRanges.map(async (subset, subsetIndex) => {
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
                                                globalMin: colorMapMin ?? range?.min,
                                                globalMax: colorMapMax ?? range?.max,
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
                                    scalar_values: undefined,
                                    probe_components: undefined,
                                    probe_ijk: undefined,
                                    vertex_count: 0,
                                    face_count: 0,
                                };

                                // Check if all slices have colors before processing them
                                const allHaveColors = subsetMeshes.every(({ mesh }) => mesh.colors && mesh.colors.length > 0);
                                const allHaveScalars = subsetMeshes.every(({ mesh }) => mesh.scalar_values && mesh.scalar_values.length > 0);
                                const allHaveProbeComponents = subsetMeshes.every(({ mesh }) => mesh.probe_components && mesh.probe_components.length > 0);
                                const allHaveProbeIjk = subsetMeshes.every(({ mesh }) => mesh.probe_ijk && mesh.probe_ijk.length > 0);
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
                                if (allHaveScalars) {
                                    mergedMesh.scalar_values = [];
                                }
                                if (allHaveProbeComponents) {
                                    mergedMesh.probe_components = [];
                                }
                                if (allHaveProbeIjk) {
                                    mergedMesh.probe_ijk = [];
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
                                    if (mergedMesh.scalar_values && sliceMesh.scalar_values && sliceMesh.scalar_values.length > 0) {
                                        mergedMesh.scalar_values.push(...sliceMesh.scalar_values);
                                    }
                                    if (mergedMesh.probe_components && sliceMesh.probe_components && sliceMesh.probe_components.length > 0) {
                                        mergedMesh.probe_components.push(...sliceMesh.probe_components);
                                    }
                                    if (mergedMesh.probe_ijk && sliceMesh.probe_ijk && sliceMesh.probe_ijk.length > 0) {
                                        mergedMesh.probe_ijk.push(...sliceMesh.probe_ijk);
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

                                if (mergedMesh.scalar_values && mergedMesh.scalar_values.length !== mergedMesh.vertex_count) {
                                    logger.warn(
                                        `Scalar array length mismatch on merged subset mesh: have ${mergedMesh.scalar_values.length} need ${mergedMesh.vertex_count}. Discarding scalar probe values.`,
                                        'Viewer3D'
                                    );
                                    mergedMesh.scalar_values = undefined;
                                }

                                if (
                                    mergedMesh.probe_components &&
                                    mergedMesh.probe_components.length !== mergedMesh.vertex_count * PROBE_COMPONENT_STRIDE
                                ) {
                                    logger.warn(
                                        `Probe component length mismatch on merged subset mesh: have ${mergedMesh.probe_components.length} need ${mergedMesh.vertex_count * PROBE_COMPONENT_STRIDE}. Discarding probe components.`,
                                        'Viewer3D'
                                    );
                                    mergedMesh.probe_components = undefined;
                                }

                                if (
                                    mergedMesh.probe_ijk &&
                                    mergedMesh.probe_ijk.length !== mergedMesh.vertex_count * PROBE_IJK_STRIDE
                                ) {
                                    logger.warn(
                                        `Probe IJK length mismatch on merged subset mesh: have ${mergedMesh.probe_ijk.length} need ${mergedMesh.vertex_count * PROBE_IJK_STRIDE}. Discarding IJK metadata.`,
                                        'Viewer3D'
                                    );
                                    mergedMesh.probe_ijk = undefined;
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
                                await fieldRangesPromise;
                                const range = displayedGlobalRange ?? fieldRangeMap.get(gridItem.solutionCacheId);
                                mesh = await invoke<MeshGeometry>('compute_solution_colors', {
                                    gridId: gridItem.gridCacheId!,
                                    solutionId: gridItem.solutionCacheId,
                                    field: scalarField,
                                    colorScheme: colorScheme,
                                    respectIblank: !ignoreIblank,
                                    showFringePoints: showFringePoints,
                                    iblankFilterMode: iblankFilterMode,
                                    globalMin: colorMapMin ?? range?.min,
                                    globalMax: colorMapMax ?? range?.max,
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
    }, [
        grids,
        ignoreIblank,
        showFringePoints,
        iblankFilterMode,
        scalarField,
        colorScheme,
        colorMapMin,
        colorMapMax,
        onActualRangeChange,
        sliceEnabled,
        subsetsContentKey,
        wallsContentKey,
        subsetsByGridId,
        wallsByGridId,
        effectiveRangesByGridId,
        appliedSlicesKey,
    ]);
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

    const probeTargets = useMemo<ProbeTarget[]>(() => {
        const targets: ProbeTarget[] = [];

        for (const gridItem of visibleGrids) {
            const mesh = meshById[gridItem.id];
            if (!mesh) {
                continue;
            }
            targets.push({
                probeId: gridItem.id,
                displayGridId: gridItem.id,
                mesh,
            });
        }

        for (const [id, mesh] of Object.entries(meshById)) {
            if (!id.startsWith('arbitrary::')) {
                continue;
            }
            const parts = id.split('::');
            const sliceId = parts[1];
            const gridId = parts[2] ?? id;
            if (!enabledArbitraryIds.has(sliceId)) {
                continue;
            }

            targets.push({
                probeId: id,
                displayGridId: gridId,
                mesh,
            });
        }

        return targets;
    }, [enabledArbitraryIds, meshById, visibleGrids]);

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
            <Canvas camera={{ position: [5, 5, 5], fov: 50 }} gl={{ preserveDrawingBuffer: true }}>
                <ambientLight intensity={0.5} />
                <directionalLight position={[10, 10, 5]} intensity={1} />
                <CameraViewpointSync
                    cameraAxisView={cameraAxisView}
                    cameraViewpoint={cameraViewpoint}
                    cameraPlotUp={cameraPlotUp}
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
                    // Keep neutral context geometry only when no scalar field is selected.
                    const shouldForceSolidContext = isContourPlotFamily && scalarField === 'none';
                    const displayColor = shouldForceSolidContext ? '#808080' : gridItem.color;

                    return (
                        <group key={gridItem.id}>
                            {/* Render smooth shaded surface */}
                            {shadingMode === 'smooth' && (
                                <SolidMeshRenderer
                                    meshGeometry={mesh}
                                    color={displayColor}
                                    dimmed={dimmed}
                                    forceSolidColor={shouldForceSolidContext}
                                />
                            )}
                            {/* Render wireframe */}
                            {showWireframe && (
                                <MeshRenderer
                                    meshGeometry={mesh}
                                    color={displayColor}
                                    dimmed={dimmed}
                                    forceSolidColor={shouldForceSolidContext}
                                />
                            )}
                            {vectors && (
                                <VectorFieldRenderer
                                    meshGeometry={mesh}
                                    colorScheme={colorScheme}
                                    vectors={vectors}
                                />
                            )}
                            {rakes && (
                                <RakeFieldRenderer
                                    meshGeometry={mesh}
                                    colorScheme={colorScheme}
                                    rakes={rakes}
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
                        const shouldForceSolidContext = isContourPlotFamily && scalarField === 'none';
                        const sliceColor = shouldForceSolidContext ? '#808080' : '#60a5fa';
                        return (
                            <group key={id}>
                                {shadingMode === 'smooth' && (
                                    <SolidMeshRenderer
                                        meshGeometry={mesh}
                                        color={sliceColor}
                                        dimmed={false}
                                        forceSolidColor={shouldForceSolidContext}
                                    />
                                )}
                                {showWireframe && (
                                    <MeshRenderer
                                        meshGeometry={mesh}
                                        color={sliceColor}
                                        dimmed={false}
                                        forceSolidColor={shouldForceSolidContext}
                                    />
                                )}
                                {vectors && (
                                    <VectorFieldRenderer
                                        meshGeometry={mesh}
                                        colorScheme={colorScheme}
                                        vectors={vectors}
                                    />
                                )}
                                {rakes && (
                                    <RakeFieldRenderer
                                        meshGeometry={mesh}
                                        colorScheme={colorScheme}
                                        rakes={rakes}
                                    />
                                )}
                            </group>
                        );
                    })}

                {/* Render iso-surfaces (SURFACE and COLOR CONTOURS attributes) */}
                {isContourPlotFamily && (contourAttribute === 'surface' || contourAttribute === 'color_contours') &&
                    Object.entries(isoSurfaceGeometries).map(([id, iso]) => (
                        <group key={`iso::${id}`}>
                            <IsoSurfaceRenderer meshGeometry={iso.mesh} color={iso.color} opacity={isoSurfaceOpacity} />
                        </group>
                    ))
                }

                {/* Render contour lines (LINE attribute, or GRID/DOTS as first-pass fallback) */}
                {isContourPlotFamily && (contourAttribute === 'line' || contourAttribute === 'grid' || contourAttribute === 'dots') &&
                    mergedContourLinesByColor.map((contour, idx) => (
                        <group key={`contour-color::${contour.color}::${idx}`}>
                            <ContourLineRenderer lineData={contour.lineData} color={contour.color} />
                        </group>
                    ))
                }

                {/* Point probe interaction */}
                <PointerInteractionHandler
                    probeTargets={probeTargets}
                    probeMode={probeMode}
                    sampleRequestToken={probeSampleRequestToken}
                    raycasterRef={raycasterRef}
                    pointerRef={pointerRef}
                    onProbe={setProbeInfo}
                />

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
                        <br />
                        Probe: {probeMode === 'off' ? 'OFF (press p or P)' : probeMode === 'snap' ? 'SNAP (P)' : 'INTERPOLATED (p)'}
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
