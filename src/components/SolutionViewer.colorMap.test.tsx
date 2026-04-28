// @vitest-environment jsdom

import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor, cleanup } from '@testing-library/react';
import { SolutionViewer } from './SolutionViewer';
import type { GridItem } from '../types/grids';

const mockGrid: GridItem = {
    id: 'test-grid',
    gridCacheId: 'cache-1',
    filePath: '/tmp/grid.xyz',
    fileName: 'grid.xyz',
    gridIndex: 0,
    dimensions: { i: 3, j: 3, k: 3 },
    hasIblank: false,
    color: '#ffffff',
    visible: true,
    hasSolution: true,
    solutionCacheId: 'sol-cache-1',
};

describe('SolutionViewer color map range controls', () => {
    afterEach(() => {
        cleanup();
    });
    it('shows color range inputs when actualMin/actualMax are provided and a field is selected', () => {
        render(
            <SolutionViewer
                selectedGrid={mockGrid}
                selectedField="density"
                actualMin={0.5}
                actualMax={2.0}
            />
        );

        const minInput = screen.getByLabelText('Min') as HTMLInputElement;
        const maxInput = screen.getByLabelText('Max') as HTMLInputElement;
        expect(minInput).toBeTruthy();
        expect(maxInput).toBeTruthy();
    });

    it('does not show color range inputs when no field is selected', () => {
        render(
            <SolutionViewer
                selectedGrid={mockGrid}
                selectedField="none"
                actualMin={0.5}
                actualMax={2.0}
            />
        );

        expect(screen.queryByLabelText('Min')).toBeNull();
        expect(screen.queryByLabelText('Max')).toBeNull();
    });

    it('does not show color range inputs when actualMin/actualMax are absent', () => {
        render(
            <SolutionViewer
                selectedGrid={mockGrid}
                selectedField="density"
            />
        );

        expect(screen.queryByLabelText('Min')).toBeNull();
        expect(screen.queryByLabelText('Max')).toBeNull();
    });

    it('shows actual dataset values as reference text', () => {
        render(
            <SolutionViewer
                selectedGrid={mockGrid}
                selectedField="density"
                actualMin={0.5}
                actualMax={2.0}
            />
        );

        expect(screen.getByText(/actual:.*0\.5/)).toBeTruthy();
        expect(screen.getByText(/actual:.*2/)).toBeTruthy();
    });

    it('calls onColorMapMinChange with parsed value on blur', async () => {
        const onMinChange = vi.fn();
        render(
            <SolutionViewer
                selectedGrid={mockGrid}
                selectedField="density"
                actualMin={0.5}
                actualMax={2.0}
                colorMapMax={2.0}
                onColorMapMinChange={onMinChange}
            />
        );

        const minInput = screen.getByLabelText('Min') as HTMLInputElement;
        fireEvent.change(minInput, { target: { value: '1.0' } });
        fireEvent.blur(minInput);

        await waitFor(() => expect(onMinChange).toHaveBeenCalledWith(1.0));
    });

    it('calls onColorMapMaxChange with parsed value on blur', async () => {
        const onMaxChange = vi.fn();
        render(
            <SolutionViewer
                selectedGrid={mockGrid}
                selectedField="density"
                actualMin={0.5}
                actualMax={2.0}
                colorMapMin={0.5}
                onColorMapMaxChange={onMaxChange}
            />
        );

        const maxInput = screen.getByLabelText('Max') as HTMLInputElement;
        fireEvent.change(maxInput, { target: { value: '1.5' } });
        fireEvent.blur(maxInput);

        await waitFor(() => expect(onMaxChange).toHaveBeenCalledWith(1.5));
    });

    it('auto-corrects: when new min >= active max, increases max to min + epsilon', async () => {
        const onMinChange = vi.fn();
        const onMaxChange = vi.fn();
        render(
            <SolutionViewer
                selectedGrid={mockGrid}
                selectedField="density"
                actualMin={0.5}
                actualMax={2.0}
                colorMapMax={1.0}
                onColorMapMinChange={onMinChange}
                onColorMapMaxChange={onMaxChange}
            />
        );

        const minInput = screen.getByLabelText('Min') as HTMLInputElement;
        // Set min to equal the current max
        fireEvent.change(minInput, { target: { value: '1.0' } });
        fireEvent.blur(minInput);

        await waitFor(() => {
            expect(onMinChange).toHaveBeenCalledWith(1.0);
            // max should be corrected to 1.0 + 1e-6
            expect(onMaxChange).toHaveBeenCalledWith(1.0 + 1e-6);
        });
    });

    it('auto-corrects: when new max <= active min, decreases min to max - epsilon', async () => {
        const onMinChange = vi.fn();
        const onMaxChange = vi.fn();
        render(
            <SolutionViewer
                selectedGrid={mockGrid}
                selectedField="density"
                actualMin={0.5}
                actualMax={2.0}
                colorMapMin={1.0}
                onColorMapMinChange={onMinChange}
                onColorMapMaxChange={onMaxChange}
            />
        );

        const maxInput = screen.getByLabelText('Max') as HTMLInputElement;
        // Set max to equal the current min
        fireEvent.change(maxInput, { target: { value: '1.0' } });
        fireEvent.blur(maxInput);

        await waitFor(() => {
            expect(onMaxChange).toHaveBeenCalledWith(1.0);
            // min should be corrected to 1.0 - 1e-6
            expect(onMinChange).toHaveBeenCalledWith(1.0 - 1e-6);
        });
    });

    it('commits on Enter key for min input', async () => {
        const onMinChange = vi.fn();
        render(
            <SolutionViewer
                selectedGrid={mockGrid}
                selectedField="density"
                actualMin={0.5}
                actualMax={2.0}
                colorMapMax={2.0}
                onColorMapMinChange={onMinChange}
            />
        );

        const minInput = screen.getByLabelText('Min') as HTMLInputElement;
        fireEvent.change(minInput, { target: { value: '0.8' } });
        fireEvent.keyDown(minInput, { key: 'Enter' });

        await waitFor(() => expect(onMinChange).toHaveBeenCalledWith(0.8));
    });

    it('resets to null when non-numeric input is committed', async () => {
        const onMinChange = vi.fn();
        render(
            <SolutionViewer
                selectedGrid={mockGrid}
                selectedField="density"
                actualMin={0.5}
                actualMax={2.0}
                colorMapMin={1.0}
                colorMapMax={2.0}
                onColorMapMinChange={onMinChange}
            />
        );

        const minInput = screen.getByLabelText('Min') as HTMLInputElement;
        fireEvent.change(minInput, { target: { value: 'abc' } });
        fireEvent.blur(minInput);

        await waitFor(() => expect(onMinChange).toHaveBeenCalledWith(null));
    });

    it('shows Reset button when colorMapMin is active', () => {
        render(
            <SolutionViewer
                selectedGrid={mockGrid}
                selectedField="density"
                actualMin={0.5}
                actualMax={2.0}
                colorMapMin={0.8}
            />
        );

        expect(screen.getByRole('button', { name: /reset/i })).toBeTruthy();
    });

    it('does not show Reset button when both clipping values are null', () => {
        render(
            <SolutionViewer
                selectedGrid={mockGrid}
                selectedField="density"
                actualMin={0.5}
                actualMax={2.0}
                colorMapMin={null}
                colorMapMax={null}
            />
        );

        expect(screen.queryByRole('button', { name: /reset/i })).toBeNull();
    });

    it('Reset button calls both onColorMapMinChange and onColorMapMaxChange with null', async () => {
        const onMinChange = vi.fn();
        const onMaxChange = vi.fn();
        render(
            <SolutionViewer
                selectedGrid={mockGrid}
                selectedField="density"
                actualMin={0.5}
                actualMax={2.0}
                colorMapMin={0.8}
                colorMapMax={1.8}
                onColorMapMinChange={onMinChange}
                onColorMapMaxChange={onMaxChange}
            />
        );

        fireEvent.click(screen.getByRole('button', { name: /reset/i }));

        await waitFor(() => {
            expect(onMinChange).toHaveBeenCalledWith(null);
            expect(onMaxChange).toHaveBeenCalledWith(null);
        });
    });
});
