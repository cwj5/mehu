import { useState, useEffect, useRef } from 'react';
import { SCALAR_FIELDS, type ScalarField, formatValue } from '../utils/solutionData';
import { type ColorScheme } from '../utils/colorMapping';
import { ColorLegend } from './ColorLegend';
import type { GridItem } from '../types/grids';
import type { Plot3DSolution } from '../types/plot3d';
import './SolutionViewer.css';

interface SolutionViewerProps {
    selectedGrid: GridItem | null;
    selectedField?: ScalarField;
    selectedColorScheme?: ColorScheme;
    onScalarFieldChange?: (field: ScalarField) => void;
    onColorSchemeChange?: (scheme: ColorScheme) => void;
}

export function SolutionViewer({
    selectedGrid,
    selectedField: controlledField,
    selectedColorScheme: controlledColorScheme,
    onScalarFieldChange,
    onColorSchemeChange,
}: SolutionViewerProps) {
    const [localSelectedField, setLocalSelectedField] = useState<ScalarField>('none');
    const [localColorScheme, setLocalColorScheme] = useState<ColorScheme>('viridis');
    const [fieldStats, setFieldStats] = useState<{ min: number, max: number, mean: number, stdDev: number } | null>(null);
    const statsRequestRef = useRef(0);

    const hasSolution = selectedGrid?.hasSolution === true;

    useEffect(() => {
        if (controlledField !== undefined) {
            setLocalSelectedField(controlledField);
        }
    }, [controlledField]);

    useEffect(() => {
        if (controlledColorScheme !== undefined) {
            setLocalColorScheme(controlledColorScheme);
        }
    }, [controlledColorScheme]);

    // Compute field stats in chunks to keep the UI responsive on large grids
    useEffect(() => {
        if (!hasSolution || localSelectedField === 'none') {
            setFieldStats(null);
            return;
        }

        // If we have the full solution data (v1 API), compute stats
        if (!selectedGrid?.solution) {
            // For v2 API (cached backend), stats computation is deferred
            // The stats will be computed on the backend when needed
            setFieldStats(null);
            return;
        }

        const solution = selectedGrid.solution as Plot3DSolution;
        const requestId = statsRequestRef.current + 1;
        statsRequestRef.current = requestId;
        setFieldStats(null);

        const totalPoints = solution.rho.length;
        if (totalPoints === 0) {
            setFieldStats({ min: 0, max: 0, mean: 0, stdDev: 0 });
            return;
        }
        const chunkSize = 50000;
        const defaultGamma = 1.4;

        let min = Number.POSITIVE_INFINITY;
        let max = Number.NEGATIVE_INFINITY;
        let sum = 0;
        let sumSquared = 0;
        let index = 0;

        const getValue = (i: number): number => {
            switch (localSelectedField) {
                case 'density':
                    return solution.rho[i];
                case 'velocity_magnitude': {
                    const rho = solution.rho[i];
                    if (rho > 0) {
                        const u = solution.rhou[i] / rho;
                        const v = solution.rhov[i] / rho;
                        const w = solution.rhow[i] / rho;
                        return Math.sqrt(u * u + v * v + w * w);
                    }
                    return 0;
                }
                case 'pressure': {
                    const rho = solution.rho[i];
                    if (rho > 0) {
                        const gamma = solution.gamma ? solution.gamma[i] : defaultGamma;
                        const u = solution.rhou[i] / rho;
                        const v = solution.rhov[i] / rho;
                        const w = solution.rhow[i] / rho;
                        const kinetic = 0.5 * rho * (u * u + v * v + w * w);
                        const internal = solution.rhoe[i] - kinetic;
                        return (gamma - 1) * internal;
                    }
                    return 0;
                }
                case 'momentum_x':
                    return solution.rhou[i];
                case 'momentum_y':
                    return solution.rhov[i];
                case 'momentum_z':
                    return solution.rhow[i];
                case 'energy':
                    return solution.rhoe[i];
                default:
                    return solution.rho[i];
            }
        };

        const processChunk = () => {
            if (statsRequestRef.current !== requestId) {
                return;
            }

            const end = Math.min(index + chunkSize, totalPoints);
            for (let i = index; i < end; i += 1) {
                const v = getValue(i);
                if (!Number.isFinite(v)) {
                    continue;
                }
                if (v < min) min = v;
                if (v > max) max = v;
                sum += v;
                sumSquared += v * v;
            }
            index = end;

            if (index < totalPoints) {
                setTimeout(processChunk, 0);
                return;
            }

            if (!Number.isFinite(min) || !Number.isFinite(max)) {
                setFieldStats({ min: 0, max: 0, mean: 0, stdDev: 0 });
                return;
            }

            const mean = sum / totalPoints;
            const variance = Math.max(0, sumSquared / totalPoints - mean * mean);
            setFieldStats({ min, max, mean, stdDev: Math.sqrt(variance) });
        };

        setTimeout(processChunk, 0);
        return () => {
            // Cancel any in-flight stats computation for stale selections
            if (statsRequestRef.current === requestId) {
                statsRequestRef.current += 1;
            }
        };
    }, [localSelectedField, hasSolution, selectedGrid]);

    const handleFieldChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
        const field = e.target.value as ScalarField;
        setLocalSelectedField(field);
        onScalarFieldChange?.(field);
    };

    const handleColorSchemeChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
        const scheme = e.target.value as ColorScheme;
        setLocalColorScheme(scheme);
        onColorSchemeChange?.(scheme);
    };

    if (!selectedGrid) {
        return (
            <div style={{
                padding: '12px',
                background: '#1f2937',
                borderRadius: '6px',
                fontSize: '12px',
                color: '#94a3b8'
            }}>
                <strong style={{ display: 'block', marginBottom: '6px' }}>Solution Visualization</strong>
                Load a solution file to plot the solution
            </div>
        );
    }

    if (!hasSolution) {
        return (
            <div style={{
                padding: '12px',
                background: '#1f2937',
                borderRadius: '6px',
                fontSize: '12px',
                color: '#94a3b8'
            }}>
                <strong style={{ display: 'block', marginBottom: '6px' }}>Solution Visualization</strong>
                No solution data loaded for this grid
            </div>
        );
    }

    return (
        <div style={{
            display: 'flex',
            flexDirection: 'column',
            gap: '12px',
            padding: '12px',
            background: '#1f2937',
            borderRadius: '6px',
            fontSize: '12px'
        }}>
            <strong style={{ textTransform: 'uppercase', letterSpacing: '0.08em', fontSize: '11px' }}>
                Solution Visualization
            </strong>

            <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                <label style={{ fontSize: '12px', color: '#cbd5e1' }}>
                    <strong>Field:</strong>
                </label>
                <select
                    value={localSelectedField}
                    onChange={handleFieldChange}
                    style={{
                        padding: '6px',
                        background: '#111827',
                        color: '#e2e8f0',
                        border: '1px solid #374151',
                        borderRadius: '4px',
                        fontSize: '12px',
                        cursor: 'pointer'
                    }}
                >
                    {SCALAR_FIELDS.map(field => (
                        <option key={field.field} value={field.field}>
                            {field.name} ({field.unit})
                        </option>
                    ))}
                </select>
            </div>

            <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                <label style={{ fontSize: '12px', color: '#cbd5e1' }}>
                    <strong>Color Scheme:</strong>
                </label>
                <select
                    value={localColorScheme}
                    onChange={handleColorSchemeChange}
                    style={{
                        padding: '6px',
                        background: '#111827',
                        color: '#e2e8f0',
                        border: '1px solid #374151',
                        borderRadius: '4px',
                        fontSize: '12px',
                        cursor: 'pointer'
                    }}
                >
                    <option value="viridis">Viridis (Perceptual)</option>
                    <option value="turbo">Turbo (Google)</option>
                    <option value="rainbow">Rainbow</option>
                    <option value="hot">Hot (Fire)</option>
                    <option value="grayscale">Grayscale</option>
                </select>
            </div>

            {fieldStats && localSelectedField !== 'none' && (
                <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
                    <ColorLegend
                        min={fieldStats.min}
                        max={fieldStats.max}
                        colorScheme={localColorScheme}
                        orientation="horizontal"
                        numTicks={5}
                        label={SCALAR_FIELDS.find(f => f.field === localSelectedField)?.name}
                    />

                    <div style={{
                        display: 'grid',
                        gridTemplateColumns: '1fr 1fr',
                        gap: '8px',
                        fontSize: '11px'
                    }}>
                        <div>
                            <div style={{ color: '#94a3b8' }}>Min</div>
                            <div style={{ color: '#e2e8f0', fontWeight: 'bold' }}>
                                {formatValue(fieldStats.min)}
                            </div>
                        </div>
                        <div>
                            <div style={{ color: '#94a3b8' }}>Max</div>
                            <div style={{ color: '#e2e8f0', fontWeight: 'bold' }}>
                                {formatValue(fieldStats.max)}
                            </div>
                        </div>
                        <div>
                            <div style={{ color: '#94a3b8' }}>Mean</div>
                            <div style={{ color: '#e2e8f0', fontWeight: 'bold' }}>
                                {formatValue(fieldStats.mean)}
                            </div>
                        </div>
                        <div>
                            <div style={{ color: '#94a3b8' }}>Std Dev</div>
                            <div style={{ color: '#e2e8f0', fontWeight: 'bold' }}>
                                {formatValue(fieldStats.stdDev)}
                            </div>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
