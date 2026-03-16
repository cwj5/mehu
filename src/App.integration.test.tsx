// @vitest-environment jsdom

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor, cleanup } from '@testing-library/react';
import App from './App';

const { invokeMock } = vi.hoisted(() => ({
    invokeMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
    invoke: invokeMock,
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn(async () => () => { }),
}));

vi.mock('@tauri-apps/api/menu', () => ({
    Menu: {
        new: vi.fn(async () => ({
            setAsAppMenu: vi.fn(async () => { }),
        })),
    },
    MenuItem: {
        new: vi.fn(async (opts: unknown) => opts),
    },
    Submenu: {
        new: vi.fn(async (opts: unknown) => opts),
    },
    CheckMenuItem: {
        new: vi.fn(async (opts: unknown) => opts),
    },
    PredefinedMenuItem: {
        new: vi.fn(async (opts: unknown) => opts),
    },
}));

vi.mock('./components/Viewer3D', () => ({
    default: () => <div data-testid="viewer3d-mock" />,
}));

vi.mock('./components/LogViewer', () => ({
    LogViewer: () => <div data-testid="log-viewer-mock" />,
}));

vi.mock('./components/SolutionViewer', () => ({
    SolutionViewer: () => <div data-testid="solution-viewer-mock" />,
}));

vi.mock('./components/LoadingIndicator', () => ({
    LoadingIndicator: () => <div data-testid="loading-indicator-mock" />,
}));

describe('App frontend integration', () => {
    afterEach(() => {
        cleanup();
    });

    beforeEach(() => {
        invokeMock.mockReset();

        let currentPlotMode = 'surface3d';
        let currentAxisView = 'custom';

        invokeMock.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
            if (cmd === 'get_plot_state') {
                return {
                    scalar_field: 'none',
                    plot_mode: currentPlotMode,
                    axis_view: currentAxisView,
                    contour_spec: { mode: 'none' },
                    subsets: [],
                    viewpoint: null,
                };
            }

            if (cmd === 'open_multiple_files_dialog') {
                return ['/tmp/grid.xyz', '/tmp/grid.q'];
            }

            if (cmd === 'clear_grid_cache' || cmd === 'clear_solution_cache_v2') {
                return null;
            }

            if (cmd === 'load_plot3d_file_cached') {
                const path = args?.path;
                if (path === '/tmp/grid.xyz') {
                    return [
                        {
                            id: 'grid-cache-1',
                            file_path: '/tmp/grid.xyz',
                            file_name: 'grid.xyz',
                            grid_index: 0,
                            dimensions: { i: 3, j: 3, k: 3 },
                            has_iblank: false,
                            has_solution: false,
                        },
                    ];
                }
                throw new Error('not a grid file');
            }

            if (cmd === 'load_plot3d_solution_cached') {
                return [
                    {
                        id: 'sol-cache-1',
                        grid_index: 0,
                        dimensions: { i: 3, j: 3, k: 3 },
                    },
                ];
            }

            if (cmd === 'set_plot_axis_view') {
                currentAxisView = String(args?.view ?? 'custom');
                return {
                    state: {
                        scalar_field: 'none',
                        plot_mode: currentPlotMode,
                        axis_view: currentAxisView,
                        contour_spec: { mode: 'none' },
                        subsets: [],
                        viewpoint: { x: 1, y: 0, z: 0 },
                    },
                    diagnostics: [],
                };
            }

            if (cmd === 'set_plot_mode') {
                currentPlotMode = String(args?.mode ?? 'surface3d');
                return {
                    state: {
                        scalar_field: 'none',
                        plot_mode: currentPlotMode,
                        axis_view: currentAxisView,
                        contour_spec: { mode: 'none' },
                        subsets: [],
                        viewpoint: { x: 1, y: 0, z: 0 },
                    },
                    diagnostics: [],
                };
            }

            if (cmd === 'commit_plot') {
                return {
                    state: {
                        scalar_field: 'none',
                        plot_mode: currentPlotMode,
                        axis_view: currentAxisView,
                        contour_spec: { mode: 'none' },
                        subsets: [],
                        viewpoint: { x: 1, y: 0, z: 0 },
                    },
                    diagnostics: [{ capability: 'PLOT', severity: 'info', message: 'Plot committed' }],
                };
            }

            return null;
        });
    });

    it('commits axis-view preset changes through backend actions', async () => {
        render(<App />);

        const presetSelect = await screen.findByLabelText('View Preset:');
        fireEvent.change(presetSelect, { target: { value: 'plus_x' } });

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith('set_plot_axis_view', { view: 'plus_x' });
            expect(invokeMock).toHaveBeenCalledWith('commit_plot');
        });

        const calledCommands = invokeMock.mock.calls.map(([cmd]) => cmd);
        const setIdx = calledCommands.indexOf('set_plot_axis_view');
        const commitIdx = calledCommands.indexOf('commit_plot');

        expect(setIdx).toBeGreaterThan(-1);
        expect(commitIdx).toBeGreaterThan(setIdx);
    });

    it('commits plane_xy view preset using the correct backend key', async () => {
        // Regression test: serde(rename_all="snake_case") serialises PlaneXY
        // as "plane_x_y" instead of "plane_xy", which silently fails the Tauri
        // invoke.  The explicit #[serde(rename = "plane_xy")] fix ensures the
        // key matches AXIS_VIEW_OPTIONS values in the frontend.
        render(<App />);

        const presetSelect = await screen.findByLabelText('View Preset:');
        fireEvent.change(presetSelect, { target: { value: 'plane_xy' } });

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith('set_plot_axis_view', { view: 'plane_xy' });
            expect(invokeMock).toHaveBeenCalledWith('commit_plot');
        });

        const calledCommands = invokeMock.mock.calls.map(([cmd]) => cmd);
        const setIdx = calledCommands.lastIndexOf('set_plot_axis_view');
        const commitIdx = calledCommands.lastIndexOf('commit_plot');

        expect(setIdx).toBeGreaterThan(-1);
        expect(commitIdx).toBeGreaterThan(setIdx);

        // The view arg must use the frontend-matching key, NOT "plane_x_y".
        const setCall = invokeMock.mock.calls[setIdx];
        expect(setCall[1]).toEqual({ view: 'plane_xy' });
    });

    it('commits contour enable changes through set_plot_mode then commit_plot', async () => {
        render(<App />);

        const loadButton = await screen.findByRole('button', { name: 'Load Files' });
        fireEvent.click(loadButton);

        const contourCheckbox = await screen.findByLabelText('Enable Contours');
        fireEvent.click(contourCheckbox);

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith('set_plot_mode', { mode: 'contours' });
            expect(invokeMock).toHaveBeenCalledWith('commit_plot');
        });

        const calledCommands = invokeMock.mock.calls.map(([cmd]) => cmd);
        const setModeIdx = calledCommands.indexOf('set_plot_mode');
        const commitIdx = calledCommands.lastIndexOf('commit_plot');

        expect(setModeIdx).toBeGreaterThan(-1);
        expect(commitIdx).toBeGreaterThan(setModeIdx);
    });
});
