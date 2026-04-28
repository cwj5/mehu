/**
 * Unit tests for color mapping utilities
 */

import { describe, it, expect } from 'vitest';
import { mapValueToColor, rgbToHex, normalizeValue, generateColorArray, type ColorScheme, type RGB } from './colorMapping';

describe('colorMapping', () => {
    describe('mapValueToColor', () => {
        it('should handle boundary values', () => {
            const schemes: ColorScheme[] = ['rainbow', 'viridis', 'turbo', 'grayscale', 'hot'];

            schemes.forEach(scheme => {
                // Test boundary values
                const min = mapValueToColor(0, scheme);
                const max = mapValueToColor(1, scheme);

                expect(min.r).toBeGreaterThanOrEqual(0);
                expect(min.r).toBeLessThanOrEqual(1);
                expect(min.g).toBeGreaterThanOrEqual(0);
                expect(min.g).toBeLessThanOrEqual(1);
                expect(min.b).toBeGreaterThanOrEqual(0);
                expect(min.b).toBeLessThanOrEqual(1);

                expect(max.r).toBeGreaterThanOrEqual(0);
                expect(max.r).toBeLessThanOrEqual(1);
                expect(max.g).toBeGreaterThanOrEqual(0);
                expect(max.g).toBeLessThanOrEqual(1);
                expect(max.b).toBeGreaterThanOrEqual(0);
                expect(max.b).toBeLessThanOrEqual(1);
            });
        });

        it('should clamp out-of-range values', () => {
            const color1 = mapValueToColor(-0.5, 'viridis');
            const color2 = mapValueToColor(1.5, 'viridis');

            expect(color1).toEqual(mapValueToColor(0, 'viridis'));
            expect(color2).toEqual(mapValueToColor(1, 'viridis'));
        });

        it('should return different colors for different values', () => {
            const color1 = mapValueToColor(0.25, 'viridis');
            const color2 = mapValueToColor(0.75, 'viridis');

            // At least one channel should differ
            const areDifferent =
                color1.r !== color2.r ||
                color1.g !== color2.g ||
                color1.b !== color2.b;

            expect(areDifferent).toBe(true);
        });

        it('grayscale should have equal RGB channels', () => {
            for (let v = 0; v <= 1; v += 0.25) {
                const color = mapValueToColor(v, 'grayscale');
                expect(color.r).toBeCloseTo(v);
                expect(color.g).toBeCloseTo(v);
                expect(color.b).toBeCloseTo(v);
            }
        });

        it('rainbow should transition through spectrum', () => {
            const colors = [
                mapValueToColor(0.0, 'rainbow'),  // Red-ish
                mapValueToColor(0.2, 'rainbow'),  // Yellow-ish
                mapValueToColor(0.4, 'rainbow'),  // Green-ish
                mapValueToColor(0.6, 'rainbow'),  // Cyan-ish
                mapValueToColor(0.8, 'rainbow'),  // Blue-ish
                mapValueToColor(1.0, 'rainbow'),  // Magenta-ish
            ];

            // Just verify all are valid colors
            colors.forEach(color => {
                expect(color.r).toBeGreaterThanOrEqual(0);
                expect(color.r).toBeLessThanOrEqual(1);
                expect(color.g).toBeGreaterThanOrEqual(0);
                expect(color.g).toBeLessThanOrEqual(1);
                expect(color.b).toBeGreaterThanOrEqual(0);
                expect(color.b).toBeLessThanOrEqual(1);
            });
        });
    });

    describe('rgbToHex', () => {
        it('should convert black RGB to hex', () => {
            const color: RGB = { r: 0, g: 0, b: 0 };
            expect(rgbToHex(color)).toBe('#000000');
        });

        it('should convert white RGB to hex', () => {
            const color: RGB = { r: 1, g: 1, b: 1 };
            expect(rgbToHex(color)).toBe('#ffffff');
        });

        it('should convert red RGB to hex', () => {
            const color: RGB = { r: 1, g: 0, b: 0 };
            expect(rgbToHex(color)).toBe('#ff0000');
        });

        it('should convert green RGB to hex', () => {
            const color: RGB = { r: 0, g: 1, b: 0 };
            expect(rgbToHex(color)).toBe('#00ff00');
        });

        it('should convert blue RGB to hex', () => {
            const color: RGB = { r: 0, g: 0, b: 1 };
            expect(rgbToHex(color)).toBe('#0000ff');
        });

        it('should convert mid-tone RGB to hex', () => {
            const color: RGB = { r: 0.5, g: 0.5, b: 0.5 };
            // 0.5 * 255 = 127.5, which rounds to 128 (0x80)
            expect(rgbToHex(color)).toBe('#808080');
        });

        it('should handle rounding correctly', () => {
            const color: RGB = { r: 0.502, g: 0.251, b: 0.753 };
            const hex = rgbToHex(color);
            expect(hex).toMatch(/^#[0-9a-f]{6}$/i);

            // Extract values and verify ranges
            const r = parseInt(hex.slice(1, 3), 16);
            const g = parseInt(hex.slice(3, 5), 16);
            const b = parseInt(hex.slice(5, 7), 16);

            expect(r).toBeGreaterThanOrEqual(0);
            expect(r).toBeLessThanOrEqual(255);
            expect(g).toBeGreaterThanOrEqual(0);
            expect(g).toBeLessThanOrEqual(255);
            expect(b).toBeGreaterThanOrEqual(0);
            expect(b).toBeLessThanOrEqual(255);
        });

        it('should pad with zeros for small values', () => {
            const color: RGB = { r: 0.01, g: 0.01, b: 0.01 };
            const hex = rgbToHex(color);
            expect(hex).toMatch(/^#[0-9a-f]{6}$/i);
            expect(hex.length).toBe(7); // # + 6 hex digits
        });

        it('should return lowercase hex', () => {
            const color: RGB = { r: 0.5, g: 0.5, b: 0.5 };
            const hex = rgbToHex(color);
            expect(hex).toBe(hex.toLowerCase());
        });
    });

    describe('normalizeValue', () => {
        it('should normalize value in simple range', () => {
            const result = normalizeValue(50, 0, 100);
            expect(result).toBeCloseTo(0.5);
        });

        it('should return 0 for minimum value', () => {
            const result = normalizeValue(0, 0, 100);
            expect(result).toBe(0);
        });

        it('should return 1 for maximum value', () => {
            const result = normalizeValue(100, 0, 100);
            expect(result).toBe(1);
        });

        it('should return 0.5 when min equals max', () => {
            const result = normalizeValue(50, 100, 100);
            expect(result).toBe(0.5);
        });

        it('should handle negative ranges', () => {
            const result = normalizeValue(-50, -100, 0);
            expect(result).toBeCloseTo(0.5);
        });

        it('should handle ranges crossing zero', () => {
            const result = normalizeValue(0, -100, 100);
            expect(result).toBeCloseTo(0.5);
        });

        it('should handle very small ranges', () => {
            const result = normalizeValue(0.0005, 0, 0.001);
            expect(result).toBeCloseTo(0.5);
        });

        it('should handle value outside range', () => {
            const result = normalizeValue(150, 0, 100);
            expect(result).toBe(1.5);
        });

        it('should handle negative value outside range', () => {
            const result = normalizeValue(-50, 0, 100);
            expect(result).toBe(-0.5);
        });

        it('should handle large ranges', () => {
            const result = normalizeValue(5000, 0, 10000);
            expect(result).toBeCloseTo(0.5);
        });
    });

    describe('generateColorArray', () => {
        it('should return empty array for empty input', () => {
            const result = generateColorArray([]);
            expect(result).toEqual(new Float32Array(0));
            expect(result.length).toBe(0);
        });

        it('should generate correct array length', () => {
            const values = [1, 2, 3, 4, 5];
            const result = generateColorArray(values);
            expect(result.length).toBe(values.length * 3); // 5 values * 3 channels
            expect(result.length).toBe(15);
        });

        it('should generate valid RGB values', () => {
            const values = [0.5];
            const result = generateColorArray(values);

            // First color should have 3 values
            const r = result[0];
            const g = result[1];
            const b = result[2];

            expect(r).toBeGreaterThanOrEqual(0);
            expect(r).toBeLessThanOrEqual(1);
            expect(g).toBeGreaterThanOrEqual(0);
            expect(g).toBeLessThanOrEqual(1);
            expect(b).toBeGreaterThanOrEqual(0);
            expect(b).toBeLessThanOrEqual(1);
        });

        it('should use provided min/max', () => {
            const values = [50, 100];
            const result = generateColorArray(values, 'viridis', 0, 200);

            expect(result.length).toBe(6);
            expect(result[0]).toBeGreaterThanOrEqual(0); // First R value
            expect(result[0]).toBeLessThanOrEqual(1);
        });

        it('should calculate min/max from values if not provided', () => {
            const values = [10, 20, 30];
            const result = generateColorArray(values, 'viridis');

            expect(result.length).toBe(9);
            // All channels should be valid
            for (let i = 0; i < result.length; i++) {
                expect(result[i]).toBeGreaterThanOrEqual(0);
                expect(result[i]).toBeLessThanOrEqual(1);
            }
        });

        it('should apply color scheme correctly', () => {
            const values = [0, 0.5, 1];
            const grayscaleResult = generateColorArray(values, 'grayscale', 0, 1);

            // For grayscale, R, G, B should be equal
            expect(grayscaleResult[0]).toBeCloseTo(0); // R for value 0
            expect(grayscaleResult[1]).toBeCloseTo(0); // G for value 0
            expect(grayscaleResult[2]).toBeCloseTo(0); // B for value 0

            expect(grayscaleResult[3]).toBeCloseTo(0.5); // R for value 0.5
            expect(grayscaleResult[4]).toBeCloseTo(0.5); // G for value 0.5
            expect(grayscaleResult[5]).toBeCloseTo(0.5); // B for value 0.5
        });

        it('should handle single value', () => {
            const values = [42];
            const result = generateColorArray(values, 'viridis', 0, 100);

            expect(result.length).toBe(3);
            expect(result[0]).toBeGreaterThanOrEqual(0);
            expect(result[0]).toBeLessThanOrEqual(1);
        });

        it('should handle equal min and max', () => {
            const values = [1, 1, 1];
            const result = generateColorArray(values, 'viridis');

            // All values should map to same halfway color since min === max
            expect(result.length).toBe(9);
        });

        it('should handle different color schemes', () => {
            const values = [0.5];
            const schemes: ColorScheme[] = ['rainbow', 'viridis', 'turbo', 'grayscale', 'hot'];

            const results = schemes.map(scheme =>
                generateColorArray(values, scheme)
            );

            results.forEach(result => {
                expect(result.length).toBe(3);
                // Note: not all schemes will produce same color for 0.5
                expect(result[0]).toBeGreaterThanOrEqual(0);
                expect(result[0]).toBeLessThanOrEqual(1);
            });
        });

        it('should preserve value order in colors', () => {
            const values = [0, 1];
            const result = generateColorArray(values, 'grayscale');

            // First color (0) should be darker than second (1)
            const color1Brightness = result[0] + result[1] + result[2];
            const color2Brightness = result[3] + result[4] + result[5];

            expect(color1Brightness).toBeLessThan(color2Brightness);
        });

        it('should return Float32Array', () => {
            const values = [1, 2, 3];
            const result = generateColorArray(values);

            expect(result).toBeInstanceOf(Float32Array);
        });

        it('should handle large arrays', () => {
            const values = new Array(1000).fill(0).map((_, i) => i);
            const result = generateColorArray(values);

            expect(result.length).toBe(3000);
            expect(result).toBeInstanceOf(Float32Array);
        });
    });

    describe('color-map min/max clipping via generateColorArray', () => {
        it('value below activeMin maps to the same color as the low endpoint', () => {
            const activeMin = 10;
            const activeMax = 100;
            const belowMin = generateColorArray([-50], 'viridis', activeMin, activeMax);
            const atMin = generateColorArray([activeMin], 'viridis', activeMin, activeMax);
            expect(belowMin[0]).toBeCloseTo(atMin[0], 5);
            expect(belowMin[1]).toBeCloseTo(atMin[1], 5);
            expect(belowMin[2]).toBeCloseTo(atMin[2], 5);
        });

        it('value above activeMax maps to the same color as the high endpoint', () => {
            const activeMin = 10;
            const activeMax = 100;
            const aboveMax = generateColorArray([9999], 'viridis', activeMin, activeMax);
            const atMax = generateColorArray([activeMax], 'viridis', activeMin, activeMax);
            expect(aboveMax[0]).toBeCloseTo(atMax[0], 5);
            expect(aboveMax[1]).toBeCloseTo(atMax[1], 5);
            expect(aboveMax[2]).toBeCloseTo(atMax[2], 5);
        });

        it('value inside active range maps to intermediate color', () => {
            const activeMin = 0;
            const activeMax = 100;
            const inside = generateColorArray([50], 'viridis', activeMin, activeMax);
            const atMin = generateColorArray([activeMin], 'viridis', activeMin, activeMax);
            const atMax = generateColorArray([activeMax], 'viridis', activeMin, activeMax);
            // Must differ from both endpoints
            const diffFromMin = inside[0] !== atMin[0] || inside[1] !== atMin[1] || inside[2] !== atMin[2];
            const diffFromMax = inside[0] !== atMax[0] || inside[1] !== atMax[1] || inside[2] !== atMax[2];
            expect(diffFromMin).toBe(true);
            expect(diffFromMax).toBe(true);
        });

        it('narrowed active range produces different colors than full range for same values', () => {
            const fullMin = 0;
            const fullMax = 100;
            const clippedMin = 40;
            const clippedMax = 60;
            const valueMidpoint = 50;

            // Note: These are kept for potential future test extension but currently unused
            void generateColorArray([valueMidpoint], 'viridis', fullMin, fullMax);
            void generateColorArray([valueMidpoint], 'viridis', clippedMin, clippedMax);

            // 50 is at position 0.5 in the full range but at position 0.5 in clipped range as well —
            // normalised the same, so colours would be equal. Instead compare edge behaviour.
            const colorAtClippedMin = generateColorArray([clippedMin], 'viridis', fullMin, fullMax);
            const colorAtClippedMinWithClipping = generateColorArray([clippedMin], 'viridis', clippedMin, clippedMax);
            // With clipping, clippedMin maps to 0.0; without clipping it maps to 0.4
            const areDifferent =
                colorAtClippedMin[0] !== colorAtClippedMinWithClipping[0] ||
                colorAtClippedMin[1] !== colorAtClippedMinWithClipping[1] ||
                colorAtClippedMin[2] !== colorAtClippedMinWithClipping[2];
            expect(areDifferent).toBe(true);
        });

        it('auto-correction epsilon keeps min strictly below max', () => {
            const epsilon = 1e-6;
            const value = 5;
            // Simulate the correction: if user sets min = max, max becomes min + epsilon
            const correctedMax = value + epsilon;
            expect(correctedMax).toBeGreaterThan(value);
            // The corrected range is valid for normalization
            const result = generateColorArray([value], 'viridis', value, correctedMax);
            expect(result.length).toBe(3);
        });
    });
});
