import { mapValueToColor, type ColorScheme } from '../utils/colorMapping';
import { formatValue } from '../utils/solutionData';
import './ColorLegend.css';

interface ColorLegendProps {
    min: number;
    max: number;
    colorScheme: ColorScheme;
    orientation?: 'horizontal' | 'vertical';
    numTicks?: number;
    numGradientStops?: number;
    label?: string;
    /** Resolved absolute contour levels to overlay on the gradient bar. */
    contourLevels?: number[];
    /** Field minimum corresponding to the contour levels (defaults to `min`). */
    fieldMin?: number;
    /** Field maximum corresponding to the contour levels (defaults to `max`). */
    fieldMax?: number;
}

/**
 * ColorLegend component displays a color scale bar with value labels
 * for visualizing the mapping between scalar values and colors.
 */
export function ColorLegend({
    min,
    max,
    colorScheme,
    orientation = 'horizontal',
    numTicks = 5,
    numGradientStops = 64,
    label,
    contourLevels,
    fieldMin,
    fieldMax,
}: ColorLegendProps) {
    // Generate gradient stops
    const gradientStops = Array.from({ length: numGradientStops }, (_, i) => {
        const value = i / (numGradientStops - 1);
        const color = mapValueToColor(value, colorScheme);
        const hex = `rgb(${Math.round(color.r * 255)}, ${Math.round(color.g * 255)}, ${Math.round(color.b * 255)})`;
        const percentage = (value * 100).toFixed(2);
        return `${hex} ${percentage}%`;
    });

    // Generate tick positions and labels
    const ticks = Array.from({ length: numTicks }, (_, i) => {
        const fraction = i / (numTicks - 1);
        const value = min + fraction * (max - min);
        return {
            position: fraction * 100,
            value,
            label: formatValue(value),
        };
    });

    // Compute contour level tick positions as percentages along the bar.
    const fMin = fieldMin ?? min;
    const fMax = fieldMax ?? max;
    const fSpan = fMax - fMin;
    const contourTicks = (contourLevels && contourLevels.length > 0 && fSpan > 0)
        ? contourLevels
            .map(level => ({ level, position: ((level - fMin) / fSpan) * 100 }))
            .filter(({ position }) => position >= 0 && position <= 100)
        : [];

    const isVertical = orientation === 'vertical';
    const gradientDirection = isVertical ? 'to top' : 'to right';

    return (
        <div className={`color-legend ${isVertical ? 'vertical' : 'horizontal'}`}>
            {label && (
                <div className="color-legend-label">
                    {label}
                </div>
            )}

            <div className="color-legend-container">
                {/* Gradient bar */}
                <div
                    className="color-legend-gradient"
                    style={{
                        background: `linear-gradient(${gradientDirection}, ${gradientStops.join(', ')})`,
                    }}
                >
                    {/* Contour level tick marks overlaid on the gradient bar */}
                    {contourTicks.map(({ level, position }) => (
                        <div
                            key={level}
                            className="color-legend-contour-tick"
                            style={{
                                [isVertical ? 'bottom' : 'left']: `${position}%`,
                            }}
                            title={formatValue(level)}
                        />
                    ))}
                </div>

                {/* Tick marks and labels */}
                <div className="color-legend-ticks">
                    {ticks.map((tick, idx) => (
                        <div
                            key={idx}
                            className="color-legend-tick"
                            style={{
                                [isVertical ? 'bottom' : 'left']: `${tick.position}%`,
                            }}
                        >
                            <div className="color-legend-tick-mark" />
                            <div className="color-legend-tick-label">
                                {tick.label}
                            </div>
                        </div>
                    ))}
                </div>
            </div>
        </div>
    );
}
