/**
 * Unit tests for solution data utilities
 */

import { describe, it, expect } from 'vitest';
import { computeScalarField, getFieldStats, formatValue, getFieldInfo, SCALAR_FIELDS } from './solutionData';
import type { Plot3DSolution } from '../types/plot3d';

describe('solutionData', () => {
    // Helper to create a simple test solution
    const createTestSolution = (size: number = 4, includeGamma: boolean = false): Plot3DSolution => {
        const solution: Plot3DSolution = {
            grid_index: 0,
            dimensions: { i: 2, j: 2, k: 1 },
            rho: new Array(size).fill(0),
            rhou: new Array(size).fill(0),
            rhov: new Array(size).fill(0),
            rhow: new Array(size).fill(0),
            rhoe: new Array(size).fill(0),
        };

        // Fill with test data
        for (let i = 0; i < size; i++) {
            solution.rho[i] = 1.0 + i * 0.1;      // 1.0, 1.1, 1.2, 1.3
            solution.rhou[i] = 0.5 * solution.rho[i];  // 0.5, 0.55, 0.6, 0.65
            solution.rhov[i] = 0.3 * solution.rho[i];  // 0.3, 0.33, 0.36, 0.39
            solution.rhow[i] = 0.2 * solution.rho[i];  // 0.2, 0.22, 0.24, 0.26
            solution.rhoe[i] = 2.5 * solution.rho[i];  // 2.5, 2.75, 3.0, 3.25
        }

        if (includeGamma) {
            solution.gamma = new Array(size).fill(0).map((_, i) => 1.4 + i * 0.01);
        }

        return solution;
    };

    describe('computeScalarField', () => {
        it('should compute density field', () => {
            const solution = createTestSolution(4);
            const result = computeScalarField(solution, 'density');

            expect(result.length).toBe(4);
            expect(result[0]).toBeCloseTo(1.0);
            expect(result[1]).toBeCloseTo(1.1);
            expect(result[2]).toBeCloseTo(1.2);
            expect(result[3]).toBeCloseTo(1.3);
        });

        it('should compute velocity magnitude', () => {
            const solution = createTestSolution(4);
            const result = computeScalarField(solution, 'velocity_magnitude');

            expect(result.length).toBe(4);
            // For point 0: u=0.5, v=0.3, w=0.2 -> |V| = sqrt(0.25 + 0.09 + 0.04) = sqrt(0.38) ≈ 0.6164
            expect(result[0]).toBeCloseTo(0.6164, 3);
        });

        it('should compute pressure with gamma from solution', () => {
            const solution = createTestSolution(4, true);
            const result = computeScalarField(solution, 'pressure');

            expect(result.length).toBe(4);

            // For point 0: rho=1.0, u=0.5, v=0.3, w=0.2, rhoe=2.5, gamma=1.4
            // KE = 0.5 * 1.0 * (0.25 + 0.09 + 0.04) = 0.19
            // IE = 2.5 - 0.19 = 2.31
            // p = (1.4 - 1) * 2.31 = 0.924
            expect(result[0]).toBeCloseTo(0.924, 2);
        });

        it('should compute pressure with default gamma when not provided', () => {
            const solution = createTestSolution(4, false);
            const result = computeScalarField(solution, 'pressure');

            expect(result.length).toBe(4);

            // Should use DEFAULT_GAMMA = 1.4
            // Same calculation as above
            expect(result[0]).toBeCloseTo(0.924, 2);
        });

        it('should handle zero density gracefully', () => {
            const solution = createTestSolution(2);
            solution.rho[0] = 0;

            const velocity = computeScalarField(solution, 'velocity_magnitude');
            expect(velocity[0]).toBe(0);

            const pressure = computeScalarField(solution, 'pressure');
            expect(pressure[0]).toBe(0);
        });

        it('should compute momentum components', () => {
            const solution = createTestSolution(4);

            const momX = computeScalarField(solution, 'momentum_x');
            expect(momX[0]).toBeCloseTo(0.5);

            const momY = computeScalarField(solution, 'momentum_y');
            expect(momY[0]).toBeCloseTo(0.3);

            const momZ = computeScalarField(solution, 'momentum_z');
            expect(momZ[0]).toBeCloseTo(0.2);
        });

        it('should compute energy field', () => {
            const solution = createTestSolution(4);
            const result = computeScalarField(solution, 'energy');

            expect(result.length).toBe(4);
            expect(result[0]).toBeCloseTo(2.5);
            expect(result[3]).toBeCloseTo(3.25);
        });
    });

    describe('getFieldStats', () => {
        it('should compute correct statistics', () => {
            const values = new Float32Array([1.0, 2.0, 3.0, 4.0, 5.0]);
            const stats = getFieldStats(values);

            expect(stats.min).toBe(1.0);
            expect(stats.max).toBe(5.0);
            expect(stats.mean).toBe(3.0);
            expect(stats.stdDev).toBeCloseTo(1.4142, 3);
        });

        it('should handle single value', () => {
            const values = new Float32Array([42.0]);
            const stats = getFieldStats(values);

            expect(stats.min).toBe(42.0);
            expect(stats.max).toBe(42.0);
            expect(stats.mean).toBe(42.0);
            expect(stats.stdDev).toBe(0.0);
        });

        it('should handle empty array', () => {
            const values = new Float32Array([]);
            const stats = getFieldStats(values);

            expect(stats.min).toBe(0);
            expect(stats.max).toBe(0);
            expect(stats.mean).toBe(0);
            expect(stats.stdDev).toBe(0);
        });

        it('should handle uniform values', () => {
            const values = new Float32Array([3.14, 3.14, 3.14, 3.14]);
            const stats = getFieldStats(values);

            expect(stats.min).toBeCloseTo(3.14, 2);
            expect(stats.max).toBeCloseTo(3.14, 2);
            expect(stats.mean).toBeCloseTo(3.14, 2);
            expect(stats.stdDev).toBeCloseTo(0, 6);
        });

        it('should handle negative values', () => {
            const values = new Float32Array([-10, -5, 0, 5, 10]);
            const stats = getFieldStats(values);

            expect(stats.min).toBe(-10);
            expect(stats.max).toBe(10);
            expect(stats.mean).toBe(0);
        });

        it('should handle empty array', () => {
            const values = new Float32Array([]);
            const stats = getFieldStats(values);

            expect(stats.min).toBe(0);
            expect(stats.max).toBe(0);
            expect(stats.mean).toBe(0);
            expect(stats.stdDev).toBe(0);
        });
    });

    describe('formatValue', () => {
        it('should format zero correctly', () => {
            expect(formatValue(0)).toBe('0');
        });

        it('should format very small values in scientific notation', () => {
            const result = formatValue(0.000001);
            expect(result).toContain('e');
        });

        it('should format small values with decimals', () => {
            const result = formatValue(0.5, 3);
            const parsed = parseFloat(result);
            expect(parsed).toBeCloseTo(0.5, 1);
        });

        it('should format normal values appropriately', () => {
            const result = formatValue(12.3456, 3);
            const parsed = parseFloat(result);
            expect(parsed).toBeCloseTo(12.3456, 1);
        });

        it('should format large values in scientific notation', () => {
            const result = formatValue(12345.6);
            expect(result).toContain('e');
        });

        it('should handle NaN and Infinity', () => {
            expect(formatValue(NaN)).toBe('N/A');
            expect(formatValue(Infinity)).toBe('N/A');
            expect(formatValue(-Infinity)).toBe('N/A');
        });

        it('should respect decimals parameter', () => {
            const result1 = formatValue(0.123456, 2);
            const result2 = formatValue(0.123456, 5);
            // With fewer decimals, we should lose precision
            expect(result1.length).toBeLessThanOrEqual(result2.length);
        });
    });

    describe('getFieldInfo', () => {
        it('should return info for valid field', () => {
            const info = getFieldInfo('density');
            expect(info.field).toBe('density');
            expect(info.name).toBeDefined();
            expect(info.name.length).toBeGreaterThan(0);
        });

        it('should return default info for invalid field', () => {
            const info = getFieldInfo('invalid' as any);
            expect(info).toBeDefined();
            expect(info.field).toBe('none');
        });

        it('should have different info for each field', () => {
            const density = getFieldInfo('density');
            const pressure = getFieldInfo('pressure');

            expect(density.name).not.toBe(pressure.name);
            expect(density.unit).not.toBe(pressure.unit);
        });
    });

    describe('SCALAR_FIELDS', () => {
        it('should define all expected fields', () => {
            const fieldNames = SCALAR_FIELDS.map(f => f.field);

            expect(fieldNames).toContain('density');
            expect(fieldNames).toContain('pressure');
            expect(fieldNames).toContain('velocity_magnitude');
            expect(fieldNames).toContain('momentum_x');
            expect(fieldNames).toContain('momentum_y');
            expect(fieldNames).toContain('momentum_z');
            expect(fieldNames).toContain('energy');
        });

        it('should have proper metadata for each field', () => {
            SCALAR_FIELDS.forEach(field => {
                expect(field.field).toBeDefined();
                expect(field.name).toBeDefined();
                expect(field.unit).toBeDefined();
                expect(field.description).toBeDefined();
                expect(field.name.length).toBeGreaterThan(0);
                expect(field.description.length).toBeGreaterThan(0);
            });
        });

        it('should include all new scalar fields from the 100-199 range', () => {
            const fieldNames = SCALAR_FIELDS.map(f => f.field);
            // Spot-check each major family
            expect(fieldNames).toContain('mach_number');
            expect(fieldNames).toContain('stagnation_pressure');
            expect(fieldNames).toContain('temperature');
            expect(fieldNames).toContain('enthalpy');
            expect(fieldNames).toContain('internal_energy');
            expect(fieldNames).toContain('entropy');
            expect(fieldNames).toContain('vorticity_magnitude');
            expect(fieldNames).toContain('pressure_gradient_magnitude');
        });
    });

    // ── Equation tests for new scalar fields ──────────────────────────────────

    /** Build a single-point solution with fully specified primitive state. */
    const pointSolution = (
        rho: number, u: number, v: number, w: number, p: number, gamma = 1.4,
        fsmach?: number
    ): Plot3DSolution => {
        const v2 = u * u + v * v + w * w;
        const rhoe = p / (gamma - 1) + 0.5 * rho * v2;
        return {
            grid_index: 0,
            dimensions: { i: 1, j: 1, k: 1 },
            rho: [rho],
            rhou: [rho * u],
            rhov: [rho * v],
            rhow: [rho * w],
            rhoe: [rhoe],
            gamma: [gamma],
            ...(fsmach !== undefined ? { metadata: { fsmach } } : {}),
        };
    };

    describe('equation tests — new scalar fields', () => {
        it('temperature = p/ρ', () => {
            const sol = pointSolution(1.2, 0.5, 0.3, 0.0, 0.5);
            const t = computeScalarField(sol, 'temperature');
            expect(t[0]).toBeCloseTo(0.5 / 1.2, 5);
        });

        it('mach_number = |V|/c', () => {
            const rho = 1.2, u = 0.5, v = 0.3, p = 0.5, gamma = 1.4;
            const sol = pointSolution(rho, u, v, 0, p, gamma);
            const mach = computeScalarField(sol, 'mach_number');
            const vmag = Math.sqrt(u * u + v * v);
            const c = Math.sqrt(gamma * p / rho);
            expect(mach[0]).toBeCloseTo(vmag / c, 4);
        });

        it('speed_of_sound = sqrt(γp/ρ)', () => {
            const rho = 1.2, u = 0.3, p = 0.5, gamma = 1.4;
            const sol = pointSolution(rho, u, 0, 0, p, gamma);
            const c = computeScalarField(sol, 'speed_of_sound');
            expect(c[0]).toBeCloseTo(Math.sqrt(gamma * p / rho), 4);
        });

        it('stagnation_pressure uses isentropic formula', () => {
            const rho = 1.2, u = 0.3, p = 0.5, gamma = 1.4;
            const sol = pointSolution(rho, u, 0, 0, p, gamma);
            const p0Field = computeScalarField(sol, 'stagnation_pressure');
            const c = Math.sqrt(gamma * p / rho);
            const mach = u / c;
            const base = 1 + (gamma - 1) / 2 * mach * mach;
            const expected = p * Math.pow(base, gamma / (gamma - 1));
            expect(p0Field[0]).toBeCloseTo(expected, 4);
        });

        it('stagnation_temperature = T*(1 + (γ-1)/2*M²)', () => {
            const rho = 1.2, u = 0.3, p = 0.5, gamma = 1.4;
            const sol = pointSolution(rho, u, 0, 0, p, gamma);
            const t0Field = computeScalarField(sol, 'stagnation_temperature');
            const c = Math.sqrt(gamma * p / rho);
            const mach = u / c;
            const t = p / rho;
            const expected = t * (1 + (gamma - 1) / 2 * mach * mach);
            expect(t0Field[0]).toBeCloseTo(expected, 4);
        });

        it('pressure_coefficient is zero at freestream conditions', () => {
            const gamma = 1.4, minf = 0.8;
            const pInf = 1.0 / gamma;
            const u = minf; // M∞*c∞ = M∞ (c∞=1)
            const sol = pointSolution(1.0, u, 0, 0, pInf, gamma, minf);
            const cp = computeScalarField(sol, 'pressure_coefficient');
            expect(cp[0]).toBeCloseTo(0, 4);
        });

        it('pitot_pressure equals stagnation_pressure for subsonic flow', () => {
            const sol = pointSolution(1.2, 0.3, 0, 0, 0.5);
            const pitot = computeScalarField(sol, 'pitot_pressure');
            const p0 = computeScalarField(sol, 'stagnation_pressure');
            expect(pitot[0]).toBeCloseTo(p0[0], 4);
        });

        it('pitot_pressure is less than isentropic p0 for supersonic flow', () => {
            // c=1.0 when γ=1.4, ρ=1, p=1/γ; u=1.5 gives M=1.5
            const sol = pointSolution(1.0, 1.5, 0, 0, 1.0 / 1.4);
            const pitot = computeScalarField(sol, 'pitot_pressure');
            const p0 = computeScalarField(sol, 'stagnation_pressure');
            expect(pitot[0]).toBeLessThan(p0[0]);
        });

        it('dynamic_pressure = ½ρV²', () => {
            const rho = 1.2, u = 0.5, v = 0.3;
            const sol = pointSolution(rho, u, v, 0, 0.5);
            const q = computeScalarField(sol, 'dynamic_pressure');
            expect(q[0]).toBeCloseTo(0.5 * rho * (u * u + v * v), 4);
        });

        it('entropy_measure_s1 is zero at non-dimensional freestream reference', () => {
            const gamma = 1.4;
            const sol = pointSolution(1.0, 0.5, 0, 0, 1.0 / gamma, gamma);
            const s1 = computeScalarField(sol, 'entropy_measure_s1');
            expect(s1[0]).toBeCloseTo(0, 4);
        });

        it('internal_energy + kinetic_energy = total_energy (stagnation_energy)', () => {
            const sol = pointSolution(1.2, 0.5, 0.3, 0, 0.5);
            const ei = computeScalarField(sol, 'internal_energy');
            const ke = computeScalarField(sol, 'kinetic_energy');
            const e0 = computeScalarField(sol, 'stagnation_energy');
            expect(ei[0] + ke[0]).toBeCloseTo(e0[0], 4);
        });

        it('cross_flow_velocity = sqrt(v² + w²)', () => {
            const sol = pointSolution(1.0, 10.0, 0.3, 0.4, 1.0 / 1.4);
            const cf = computeScalarField(sol, 'cross_flow_velocity');
            expect(cf[0]).toBeCloseTo(Math.sqrt(0.3 * 0.3 + 0.4 * 0.4), 4);
        });

        it('log_normalized_density = ln(ρ)', () => {
            const rho = 1.5;
            const sol = pointSolution(rho, 0.3, 0, 0, 0.5);
            const lnRho = computeScalarField(sol, 'log_normalized_density');
            expect(lnRho[0]).toBeCloseTo(Math.log(rho), 5);
        });

        it('stagnation_enthalpy = e0 + p/ρ', () => {
            const rho = 1.2, u = 0.3, p = 0.5, gamma = 1.4;
            const v2 = u * u;
            const rhoe = p / (gamma - 1) + 0.5 * rho * v2;
            const e0 = rhoe / rho;
            const sol = pointSolution(rho, u, 0, 0, p, gamma);
            const h0 = computeScalarField(sol, 'stagnation_enthalpy');
            expect(h0[0]).toBeCloseTo(e0 + p / rho, 4);
        });

        it('derivative-based fields return zeros without grid', () => {
            const sol = pointSolution(1.2, 0.5, 0.3, 0, 0.5);
            const derivFields = [
                'vorticity_x', 'vorticity_y', 'vorticity_z', 'vorticity_magnitude',
                'velocity_divergence', 'pressure_gradient_magnitude',
                'density_gradient_magnitude', 'shock_function_pressure_gradient',
            ] as const;
            for (const field of derivFields) {
                const result = computeScalarField(sol, field);
                expect(result[0]).toBe(0);
            }
        });
    });
});
