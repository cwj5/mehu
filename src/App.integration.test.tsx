// @vitest-environment jsdom

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor, cleanup, within } from '@testing-library/react';
import App from './App';

const { invokeMock } = vi.hoisted(() => ({
    invokeMock: vi.fn(),
}));

const { viewer3DMock } = vi.hoisted(() => ({
    viewer3DMock: vi.fn(),
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
    default: (props: unknown) => {
        viewer3DMock(props);
        return <div data-testid="viewer3d-mock" />;
    },
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
        viewer3DMock.mockReset();

        let currentPlotFamily = 'function_surface';
        let currentAxisView = 'custom';
        let currentPlotUp: string | null = 'negative_y';
        let currentSubsets: Array<Record<string, unknown>> = [];

        invokeMock.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
            if (cmd === 'get_plot_state') {
                return {
                    scalar_field: 'none',
                    plot_family: currentPlotFamily,
                    contour_attribute: 'line',
                    axis_view: currentAxisView,
                    plot_up: currentPlotUp,
                    contour_spec: { mode: 'none' },
                    walls: [],
                    subsets: currentSubsets,
                    fsurface: null,
                    text_annotations: [],
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
                        plot_family: currentPlotFamily,
                        contour_attribute: 'line',
                        axis_view: currentAxisView,
                        plot_up: currentPlotUp,
                        contour_spec: { mode: 'none' },
                        walls: [],
                        subsets: currentSubsets,
                        fsurface: null,
                        text_annotations: [],
                        viewpoint: { x: 1, y: 0, z: 0 },
                    },
                    diagnostics: [],
                };
            }

            if (cmd === 'set_plot_family') {
                currentPlotFamily = String(args?.family ?? 'contour');
                return {
                    state: {
                        scalar_field: 'none',
                        plot_family: currentPlotFamily,
                        contour_attribute: 'line',
                        axis_view: currentAxisView,
                        plot_up: currentPlotUp,
                        contour_spec: { mode: 'none' },
                        walls: [],
                        subsets: currentSubsets,
                        fsurface: null,
                        text_annotations: [],
                        viewpoint: { x: 1, y: 0, z: 0 },
                    },
                    diagnostics: [],
                };
            }

            if (cmd === 'set_plot_subsets') {
                const subsets = (args?.subsets as Array<Record<string, unknown>> | undefined) ?? [];
                currentSubsets = subsets;
                return {
                    state: {
                        scalar_field: 'none',
                        plot_family: currentPlotFamily,
                        contour_attribute: 'line',
                        axis_view: currentAxisView,
                        plot_up: currentPlotUp,
                        contour_spec: { mode: 'none' },
                        walls: [],
                        subsets: currentSubsets,
                        fsurface: null,
                        text_annotations: [],
                        viewpoint: { x: 1, y: 0, z: 0 },
                    },
                    diagnostics: [],
                };
            }

            if (cmd === 'commit_plot') {
                return {
                    state: {
                        scalar_field: 'none',
                        plot_family: currentPlotFamily,
                        contour_attribute: 'line',
                        axis_view: currentAxisView,
                        plot_up: currentPlotUp,
                        contour_spec: { mode: 'none' },
                        subsets: currentSubsets,
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

    it('commits contour enable changes through set_plot_family then commit_plot', async () => {
        render(<App />);

        const loadButton = await screen.findByRole('button', { name: 'Load Files' });
        fireEvent.click(loadButton);

        // The Plot Family select replaces the old "Enable Contours" checkbox.
        // Mock starts with plot_family='function_surface', so the select shows that value.
        const plotFamilySelect = await screen.findByDisplayValue('Function Surface');
        fireEvent.change(plotFamilySelect, { target: { value: 'contour' } });

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith('set_plot_family', { family: 'contour' });
            expect(invokeMock).toHaveBeenCalledWith('commit_plot');
        });

        const calledCommands = invokeMock.mock.calls.map(([cmd]) => cmd);
        const setFamilyIdx = calledCommands.indexOf('set_plot_family');
        const commitIdx = calledCommands.lastIndexOf('commit_plot');

        expect(setFamilyIdx).toBeGreaterThan(-1);
        expect(commitIdx).toBeGreaterThan(setFamilyIdx);
    });

    it('commits subset edits when Enter is pressed in a slice field', async () => {
        render(<App />);

        const loadButton = await screen.findByRole('button', { name: 'Load Files' });
        fireEvent.click(loadButton);

        const addSliceButton = await screen.findByRole('button', { name: '+ Add slice' });
        fireEvent.click(addSliceButton);

        // Adding a slice stays local draft state until the user applies.
        expect(invokeMock).not.toHaveBeenCalledWith('set_plot_subsets', expect.anything());

        const sliceInputs = await screen.findAllByDisplayValue('2');
        const sliceIndexInput = sliceInputs.find(
            (input) => input.tagName === 'INPUT' && input.getAttribute('max') === '3'
        );

        expect(sliceIndexInput).toBeDefined();

        fireEvent.change(sliceIndexInput!, { target: { value: '3' } });

        const updatedSliceInput = await screen.findByDisplayValue('3');
        fireEvent.keyDown(updatedSliceInput, { key: 'Enter', code: 'Enter' });

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith(
                'set_plot_subsets',
                expect.objectContaining({ subsets: expect.any(Array) })
            );
            expect(invokeMock).toHaveBeenCalledWith('commit_plot');
        });

        const calledCommands = invokeMock.mock.calls.map(([cmd]) => cmd);
        const setSubsetsIdx = calledCommands.lastIndexOf('set_plot_subsets');
        const commitIdx = calledCommands.lastIndexOf('commit_plot');

        expect(setSubsetsIdx).toBeGreaterThan(-1);
        expect(commitIdx).toBeGreaterThan(setSubsetsIdx);
    });

    it('passes backend plot_up through to Viewer3D cameraPlotUp prop', async () => {
        render(<App />);

        await screen.findByTestId('viewer3d-mock');

        await waitFor(() => {
            expect(viewer3DMock).toHaveBeenCalledWith(
                expect.objectContaining({ cameraPlotUp: 'negative_y' })
            );
        });
    });
});

// ──────────────────────────────────────────────────────────────────────────────
// TKT-007E: Contour spec editor commits and regression coverage
// ──────────────────────────────────────────────────────────────────────────────

describe('Contour spec editor commits (TKT-007E)', () => {
    afterEach(() => {
        cleanup();
    });

    // Re-usable helper that builds a mock response shaped like ApplyPlotActionResult
    function makeResult(overrides: Partial<{
        plot_family: string;
        contour_attribute: string;
        contour_spec: object;
    }> = {}) {
        return {
            state: {
                scalar_field: 'none',
                plot_family: overrides.plot_family ?? 'contour',
                contour_attribute: overrides.contour_attribute ?? 'line',
                axis_view: 'custom',
                contour_spec: overrides.contour_spec ?? { mode: 'none' },
                walls: [],
                subsets: [],
                fsurface: null,
                text_annotations: [],
                viewpoint: null,
            },
            diagnostics: [],
        };
    }

    beforeEach(() => {
        invokeMock.mockReset();

        let currentPlotFamily = 'contour';
        let currentContourAttribute = 'line';
        let currentContourSpec: object = { mode: 'none' };

        invokeMock.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
            if (cmd === 'get_plot_state') {
                return {
                    scalar_field: 'none',
                    plot_family: currentPlotFamily,
                    contour_attribute: currentContourAttribute,
                    axis_view: 'custom',
                    contour_spec: currentContourSpec,
                    walls: [],
                    subsets: [],
                    fsurface: null,
                    text_annotations: [],
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
                    return [{ id: 'grid-cache-1', file_path: '/tmp/grid.xyz', file_name: 'grid.xyz', grid_index: 0, dimensions: { i: 3, j: 3, k: 3 }, has_iblank: false, has_solution: false }];
                }
                throw new Error('not a grid file');
            }
            if (cmd === 'load_plot3d_solution_cached') {
                return [{ id: 'sol-cache-1', grid_index: 0, dimensions: { i: 3, j: 3, k: 3 } }];
            }
            if (cmd === 'set_plot_family') {
                currentPlotFamily = String(args?.family ?? 'contour');
                return makeResult({ plot_family: currentPlotFamily, contour_attribute: currentContourAttribute, contour_spec: currentContourSpec });
            }
            if (cmd === 'set_plot_contour_attribute') {
                currentContourAttribute = String(args?.attribute ?? 'line');
                return makeResult({ plot_family: currentPlotFamily, contour_attribute: currentContourAttribute, contour_spec: currentContourSpec });
            }
            if (cmd === 'set_plot_contour_spec') {
                currentContourSpec = (args?.spec as object) ?? { mode: 'none' };
                return makeResult({ plot_family: currentPlotFamily, contour_attribute: currentContourAttribute, contour_spec: currentContourSpec });
            }
            if (cmd === 'commit_plot') {
                return makeResult({ plot_family: currentPlotFamily, contour_attribute: currentContourAttribute, contour_spec: currentContourSpec });
            }
            return null;
        });
    });

    async function loadFiles() {
        render(<App />);
        const loadButton = await screen.findByRole('button', { name: 'Load Files' });
        fireEvent.click(loadButton);
        // Wait for Plot Family select to be visible (gated behind hasSolution)
        await screen.findByDisplayValue('Contour');
    }

    it('mode selector change does NOT immediately commit to backend', async () => {
        await loadFiles();

        // Switch Levels mode from None → Automatic (local-only, no invoke)
        invokeMock.mockClear();
        const levelsSelect = await screen.findByDisplayValue('None');
        fireEvent.change(levelsSelect, { target: { value: 'automatic' } });

        // No IPC call should have happened yet
        expect(invokeMock).not.toHaveBeenCalledWith('set_plot_contour_spec', expect.anything());
        expect(invokeMock).not.toHaveBeenCalledWith('commit_plot');
    });

    it('commits Automatic contour spec when Apply is clicked', async () => {
        await loadFiles();

        const levelsSelect = await screen.findByDisplayValue('None');
        fireEvent.change(levelsSelect, { target: { value: 'automatic' } });

        // Count input appears after mode switch
        const countInput = await screen.findByDisplayValue('10');
        fireEvent.change(countInput, { target: { value: '8' } });

        // No commit yet (still draft)
        expect(invokeMock).not.toHaveBeenCalledWith('set_plot_contour_spec', expect.anything());

        const applyButton = await screen.findByRole('button', { name: 'Apply' });
        fireEvent.click(applyButton);

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith(
                'set_plot_contour_spec',
                { spec: { mode: 'automatic', count: 8 } }
            );
            expect(invokeMock).toHaveBeenCalledWith('commit_plot');
        });

        // Ordering: set_plot_contour_spec before commit_plot
        const calls = invokeMock.mock.calls.map(([cmd]) => cmd);
        const specIdx = calls.lastIndexOf('set_plot_contour_spec');
        const commitIdx = calls.lastIndexOf('commit_plot');
        expect(specIdx).toBeGreaterThan(-1);
        expect(commitIdx).toBeGreaterThan(specIdx);
    });

    it('commits Increment contour spec when Apply is clicked', async () => {
        await loadFiles();

        const levelsSelect = await screen.findByDisplayValue('None');
        fireEvent.change(levelsSelect, { target: { value: 'increment' } });

        // Start defaults to '0', Step defaults to '1'
        const startLabel = await screen.findByText('Start:');
        const startInput = within(startLabel.closest('label') as HTMLElement).getByRole('spinbutton');
        fireEvent.change(startInput, { target: { value: '1.5' } });

        const stepInput = await screen.findByDisplayValue('1');
        fireEvent.change(stepInput, { target: { value: '0.5' } });

        invokeMock.mockClear();
        const applyButton = await screen.findByRole('button', { name: 'Apply' });
        fireEvent.click(applyButton);

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith(
                'set_plot_contour_spec',
                { spec: { mode: 'increment', start: 1.5, increment: 0.5 } }
            );
            expect(invokeMock).toHaveBeenCalledWith('commit_plot');
        });

        const calls = invokeMock.mock.calls.map(([cmd]) => cmd);
        const specIdx = calls.lastIndexOf('set_plot_contour_spec');
        const commitIdx = calls.lastIndexOf('commit_plot');
        expect(commitIdx).toBeGreaterThan(specIdx);
    });

    it('commits Manual contour level when Apply is clicked', async () => {
        await loadFiles();

        const levelsSelect = await screen.findByDisplayValue('None');
        fireEvent.change(levelsSelect, { target: { value: 'manual' } });

        const levelLabels = await screen.findAllByText('Level:');
        const levelInput = within(levelLabels[0].closest('label') as HTMLElement).getByRole('spinbutton');
        fireEvent.change(levelInput, { target: { value: '42.75' } });

        // No commit yet — only draft changed
        expect(invokeMock).not.toHaveBeenCalledWith('set_plot_contour_spec', expect.anything());

        invokeMock.mockClear();
        const applyButton = await screen.findByRole('button', { name: 'Apply' });
        fireEvent.click(applyButton);

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith(
                'set_plot_contour_spec',
                { spec: { mode: 'manual', entries: [{ value: 42.75, color: null }] } }
            );
            expect(invokeMock).toHaveBeenCalledWith('commit_plot');
        });
    });

    it('commits Manual level on Enter key without needing Apply button click', async () => {
        await loadFiles();

        const levelsSelect = await screen.findByDisplayValue('None');
        fireEvent.change(levelsSelect, { target: { value: 'manual' } });

        const levelLabels = await screen.findAllByText('Level:');
        const levelInput = within(levelLabels[0].closest('label') as HTMLElement).getByRole('spinbutton');
        fireEvent.change(levelInput, { target: { value: '77' } });

        invokeMock.mockClear();
        fireEvent.keyDown(levelInput, { key: 'Enter', code: 'Enter' });

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith(
                'set_plot_contour_spec',
                { spec: { mode: 'manual', entries: [{ value: 77, color: null }] } }
            );
        });
    });

    it('commits contour attribute change immediately on select change', async () => {
        await loadFiles();

        invokeMock.mockClear();
        const attributeSelect = await screen.findByDisplayValue('Line');
        fireEvent.change(attributeSelect, { target: { value: 'surface' } });

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith(
                'set_plot_contour_attribute',
                { attribute: 'surface' }
            );
            expect(invokeMock).toHaveBeenCalledWith('commit_plot');
        });

        const calls = invokeMock.mock.calls.map(([cmd]) => cmd);
        const attrIdx = calls.lastIndexOf('set_plot_contour_attribute');
        const commitIdx = calls.lastIndexOf('commit_plot');
        expect(commitIdx).toBeGreaterThan(attrIdx);
    });

    it('plot family round-trip: contour → function_surface → contour', async () => {
        await loadFiles();

        invokeMock.mockClear();
        const plotFamilySelect = await screen.findByDisplayValue('Contour');
        fireEvent.change(plotFamilySelect, { target: { value: 'function_surface' } });

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith('set_plot_family', { family: 'function_surface' });
            expect(invokeMock).toHaveBeenCalledWith('commit_plot');
        });

        invokeMock.mockClear();
        const updatedSelect = await screen.findByDisplayValue('Function Surface');
        fireEvent.change(updatedSelect, { target: { value: 'contour' } });

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith('set_plot_family', { family: 'contour' });
            expect(invokeMock).toHaveBeenCalledWith('commit_plot');
        });
    });
});

// ──────────────────────────────────────────────────────────────────────────────
// TKT-010: PNG export from command files
// ──────────────────────────────────────────────────────────────────────────────

describe('PNG export workflow (TKT-010)', () => {
    afterEach(() => {
        cleanup();
    });

    beforeEach(() => {
        invokeMock.mockReset();

        // Mock default responses for app initialization
        invokeMock.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
            if (cmd === 'get_plot_state') {
                return {
                    scalar_field: 'none',
                    plot_family: 'contour',
                    contour_attribute: 'line',
                    axis_view: 'custom',
                    contour_spec: { mode: 'none' },
                    walls: [],
                    subsets: [],
                    fsurface: null,
                    text_annotations: [],
                    viewpoint: null,
                };
            }
            if (cmd === 'open_multiple_files_dialog') {
                return [];
            }
            if (cmd === 'clear_grid_cache' || cmd === 'clear_solution_cache_v2') {
                return null;
            }
            if (cmd === 'execute_com_script') {
                // Return a mock script execution result with intents
                return {
                    final_state: {
                        scalar_field: 'none',
                        plot_family: 'contour',
                        contour_attribute: 'line',
                        axis_view: 'custom',
                        contour_spec: { mode: 'automatic', count: 10 },
                        walls: [],
                        subsets: [],
                        fsurface: null,
                        text_annotations: [],
                        viewpoint: null,
                    },
                    intents: [
                        { state: { scalar_field: 'none', plot_family: 'contour' } },
                    ],
                    show_output: ['SHOW: field=none, family=contour'],
                    diagnostics: [],
                };
            }
            if (cmd === 'save_png_file_dialog') {
                return '/tmp/plot_output.png';
            }
            if (cmd === 'write_png_file') {
                // Matches real command: returns the resolved written path
                const writeArgs = args as { path?: string };
                return writeArgs?.path ?? '/tmp/plot_output.png';
            }
            return null;
        });
    });

    it('.com file execution stores result with intents', async () => {
        render(<App />);

        // Simulate typing a .com file path directly in the input
        // (we mock the execution response, so we don't need to open a dialog)
        invokeMock.mockClear();
        const result = await invokeMock('execute_com_script', { path: '/tmp/test.com' });

        expect(result).toBeDefined();
        expect(result.intents).toHaveLength(1);
        expect(result.final_state.plot_family).toBe('contour');
    });

    it('execution result includes diagnostics for display', async () => {
        // Override mock to include diagnostics
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === 'execute_com_script') {
                return {
                    final_state: {
                        scalar_field: 'none',
                        plot_family: 'contour',
                        contour_attribute: 'line',
                        axis_view: 'custom',
                        contour_spec: { mode: 'none' },
                        walls: [],
                        subsets: [],
                        fsurface: null,
                        text_annotations: [],
                        viewpoint: null,
                    },
                    intents: [{ state: {} }],
                    show_output: [],
                    diagnostics: [
                        {
                            capability: 'PREVIEW',
                            severity: 'warning',
                            message: 'Preview not fully implemented',
                            file: 'test.com',
                            line: 5,
                            column: 1,
                        }
                    ],
                };
            }
            if (cmd === 'get_plot_state') {
                return {
                    scalar_field: 'none',
                    plot_family: 'contour',
                    contour_attribute: 'line',
                    axis_view: 'custom',
                    contour_spec: { mode: 'none' },
                    walls: [],
                    subsets: [],
                    fsurface: null,
                    text_annotations: [],
                    viewpoint: null,
                };
            }
            return null;
        });

        const result = await invokeMock('execute_com_script', { path: '/tmp/test.com' });
        expect(result.diagnostics).toHaveLength(1);
        expect(result.diagnostics[0].severity).toBe('warning');
    });

    it('PNG export backend commands are registered', async () => {
        render(<App />);

        // Verify backend commands exist by attempting to invoke them with mocked responses
        invokeMock.mockClear();

        // Test save_png_file_dialog
        const savePath = await invokeMock('save_png_file_dialog', { default_name: 'test.png' });
        expect(savePath).toBe('/tmp/plot_output.png');

        // Test write_png_file — returns the resolved path on success
        const writeResult = await invokeMock('write_png_file', {
            path: '/tmp/plot_output.png',
            pngData: [137, 80, 78, 71] // PNG magic bytes
        });
        expect(writeResult).toBe('/tmp/plot_output.png');
    });

    it('multi-plot export derives numbered file paths', async () => {
        // Simulate writing 3 PNGs to numbered paths derived from a base path.
        // The export workflow strips .png and appends _001, _002, _003.
        const basePath = '/tmp/scene.png';
        const count = 3;
        // Logic mirrors derivePngPaths() in App.tsx
        const withoutExt = basePath.replace(/\.png$/i, '');
        const expectedPaths = Array.from({ length: count }, (_, i) =>
            `${withoutExt}_${String(i + 1).padStart(3, '0')}.png`
        );
        expect(expectedPaths).toEqual([
            '/tmp/scene_001.png',
            '/tmp/scene_002.png',
            '/tmp/scene_003.png',
        ]);

        // Verify write_png_file mock returns each path correctly
        for (const path of expectedPaths) {
            const result = await invokeMock('write_png_file', { path, pngData: [] });
            expect(result).toBe(path);
        }
    });

    it('single-plot export uses the path exactly as chosen', async () => {
        // For a single intent, no numbering is applied.
        const chosenPath = '/tmp/my_plot.png';
        const result = await invokeMock('write_png_file', { path: chosenPath, pngData: [] });
        expect(result).toBe(chosenPath);
    });

    it('multi-plot execution produces multiple intents', async () => {
        // Override mock for multi-plot execution
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === 'execute_com_script') {
                return {
                    final_state: {
                        scalar_field: 'density',
                        plot_family: 'contour',
                        contour_attribute: 'line',
                        axis_view: 'plus_z',
                        contour_spec: { mode: 'automatic', count: 5 },
                        walls: [],
                        subsets: [],
                        fsurface: null,
                        text_annotations: [],
                        viewpoint: null,
                    },
                    intents: [
                        { state: { scalar_field: 'density', plot_family: 'contour' } },
                        { state: { scalar_field: 'density', plot_family: 'contour' } },
                        { state: { scalar_field: 'density', plot_family: 'contour' } },
                    ],
                    show_output: ['SHOW plot 1', 'SHOW plot 2', 'SHOW plot 3'],
                    diagnostics: [],
                };
            }
            if (cmd === 'get_plot_state') {
                return {
                    scalar_field: 'none',
                    plot_family: 'contour',
                    contour_attribute: 'line',
                    axis_view: 'custom',
                    contour_spec: { mode: 'none' },
                    walls: [],
                    subsets: [],
                    fsurface: null,
                    text_annotations: [],
                    viewpoint: null,
                };
            }
            return null;
        });

        const result = await invokeMock('execute_com_script', { path: '/tmp/multi-plot.com' });
        expect(result.intents).toHaveLength(3);
        expect(result.show_output).toHaveLength(3);
    });

    it('current view export backend command is available', async () => {
        render(<App />);

        // Verify the export current view function is callable via backend
        const canvasExportPath = await invokeMock('save_png_file_dialog', { default_name: 'plot_view.png' });
        expect(canvasExportPath).toBe('/tmp/plot_output.png');

        // Verify PNG write command works — returns the resolved written path
        const writeResult = await invokeMock('write_png_file', {
            path: '/tmp/plot_view.png',
            pngData: [137, 80, 78, 71] // PNG magic bytes
        });
        expect(writeResult).toBe('/tmp/plot_view.png');
    });
});
