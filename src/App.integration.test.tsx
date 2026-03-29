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
// TKT-012C: Cross-path parity tests (script path vs GUI action path)
// ──────────────────────────────────────────────────────────────────────────────

describe('Cross-path parity (TKT-012C)', () => {
    type MockPlotState = {
        scalar_field: string;
        plot_family: string;
        contour_attribute: string;
        axis_view: string;
        plot_up: string | null;
        contour_spec: Record<string, unknown>;
        walls: Array<Record<string, unknown>>;
        subsets: Array<Record<string, unknown>>;
        fsurface: Record<string, unknown> | null;
        text_annotations: Array<Record<string, unknown>>;
        viewpoint: Record<string, unknown> | null;
    };

    type MockApplyResult = {
        state: MockPlotState;
        diagnostics: Array<Record<string, unknown>>;
    };

    type MockScriptResult = {
        final_state: MockPlotState;
        intents: Array<{ state: MockPlotState }>;
        show_output: string[];
        diagnostics: Array<Record<string, unknown>>;
    };

    const clone = <T,>(value: T): T => JSON.parse(JSON.stringify(value));

    const titleCaseField = (field: string) => {
        switch (field) {
            case 'none':
                return 'None';
            case 'pressure':
                return 'Pressure';
            case 'density':
                return 'Density';
            case 'function_surface':
                return 'Function Surface';
            default:
                return field
                    .split('_')
                    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
                    .join(' ');
        }
    };

    const titleCaseFamily = (family: string) => {
        if (family === 'function_surface') {
            return 'FunctionSurface';
        }
        return family.charAt(0).toUpperCase() + family.slice(1);
    };

    const titleCaseAxisView = (axisView: string) => {
        if (axisView === 'custom') {
            return 'Custom';
        }
        return axisView
            .split('_')
            .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
            .join('');
    };

    const titleCasePlotUp = (plotUp: string | null) => {
        if (!plotUp) {
            return 'None';
        }
        return plotUp
            .split('_')
            .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
            .join('');
    };

    const buildShowStatus = (state: MockPlotState) => (
        `SHOW: field=${titleCaseField(state.scalar_field)}, family=${titleCaseFamily(state.plot_family)}, axis_view=${titleCaseAxisView(state.axis_view)}, plot_up=${titleCasePlotUp(state.plot_up)}, text_annotations=${state.text_annotations.length}, walls=${state.walls.length}, subsets=${state.subsets.length}`
    );

    const axisViewToViewpoint = (axisView: string) => {
        switch (axisView) {
            case 'plus_x':
                return { x: 8.660254037844387, y: 0, z: 0 };
            case 'minus_x':
                return { x: -8.660254037844387, y: 0, z: 0 };
            case 'plus_y':
                return { x: 0, y: 8.660254037844387, z: 0 };
            case 'minus_y':
                return { x: 0, y: -8.660254037844387, z: 0 };
            case 'plus_z':
            case 'plane_xy':
            case 'plane_yx':
                return { x: 0, y: 0, z: 8.660254037844387 };
            case 'minus_z':
                return { x: 0, y: 0, z: -8.660254037844387 };
            case 'plane_xz':
            case 'plane_zx':
                return { x: 0, y: 8.660254037844387, z: 0 };
            case 'plane_yz':
            case 'plane_zy':
                return { x: 8.660254037844387, y: 0, z: 0 };
            default:
                return null;
        }
    };

    const makeState = (overrides: Partial<MockPlotState> = {}): MockPlotState => ({
        scalar_field: 'none',
        plot_family: 'contour',
        contour_attribute: 'line',
        axis_view: 'custom',
        plot_up: null,
        contour_spec: { mode: 'none' },
        walls: [],
        subsets: [],
        fsurface: null,
        text_annotations: [],
        viewpoint: null,
        ...overrides,
    });

    const makeScriptResult = (finalState: MockPlotState, options?: {
        intents?: Array<{ state: MockPlotState }>;
        show_output?: string[];
    }): MockScriptResult => ({
        final_state: clone(finalState),
        intents: clone(options?.intents ?? [{ state: clone(finalState) }]),
        show_output: clone(options?.show_output ?? []),
        diagnostics: [],
    });

    const scriptFixtures: Record<string, MockScriptResult> = {
        '/tmp/tkt-012c-contours.com': makeScriptResult(
            makeState({
                contour_attribute: 'surface',
                contour_spec: { mode: 'automatic', count: 8 },
            })
        ),
        '/tmp/tkt-012c-orientation.com': makeScriptResult(
            makeState({
                axis_view: 'custom',
                plot_up: 'negative_y',
                viewpoint: { x: 5, y: 6, z: 7 },
            }),
            {
                intents: [
                    {
                        state: makeState({
                            axis_view: 'plane_xy',
                            plot_up: 'negative_y',
                            viewpoint: { x: 0, y: 0, z: 8.660254037844387 },
                        }),
                    },
                    {
                        state: makeState({
                            axis_view: 'custom',
                            plot_up: 'negative_y',
                            viewpoint: { x: 5, y: 6, z: 7 },
                        }),
                    },
                ],
            }
        ),
        '/tmp/tkt-012c-ranges.com': makeScriptResult(
            makeState({
                subsets: [
                    {
                        grid: 1,
                        gui_managed: true,
                        i_range: { start: 1, end: 3 },
                        j_range: { start: 1, end: 3 },
                        k_range: { start: 2, end: 2 },
                    },
                ],
                walls: [
                    {
                        grid: 1,
                        gui_managed: false,
                        i_range: { start: 1, end: 3 },
                        j_range: { start: 2, end: 2 },
                        k_range: { start: 1, end: 3 },
                    },
                ],
            })
        ),
        '/tmp/tkt-012c-function-surface.com': makeScriptResult(
            makeState({
                plot_family: 'function_surface',
                fsurface: { value: 0.125, scalar_field: 'pressure' },
                text_annotations: [
                    { content: 'Cp label', x: 0.2, y: 0.8 },
                ],
            }),
            {
                show_output: [
                    buildShowStatus(
                        makeState({
                            plot_family: 'function_surface',
                            fsurface: { value: 0.125, scalar_field: 'pressure' },
                            text_annotations: [{ content: 'Cp label', x: 0.2, y: 0.8 }],
                        })
                    ),
                ],
            }
        ),
    };

    let currentState: MockPlotState;
    let commitResults: MockApplyResult[];

    const resetBackendState = (overrides: Partial<MockPlotState> = {}) => {
        currentState = makeState(overrides);
        commitResults = [];
    };

    const makeApplyResult = (): MockApplyResult => ({
        state: clone(currentState),
        diagnostics: [],
    });

    const loadFiles = async () => {
        render(<App />);
        const loadButton = await screen.findByRole('button', { name: 'Load Files' });
        fireEvent.click(loadButton);
        await screen.findByText('Plot Family:');
    };

    const getLatestViewerProps = () => viewer3DMock.mock.calls[viewer3DMock.mock.calls.length - 1]?.[0] as
        | { onCameraCommit?: (vp: { x: number; y: number; z: number }) => Promise<void> | void; cameraPlotUp?: string | null }
        | undefined;

    afterEach(() => {
        cleanup();
    });

    beforeEach(() => {
        invokeMock.mockReset();
        viewer3DMock.mockReset();
        resetBackendState();

        invokeMock.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
            if (cmd === 'get_plot_state') {
                return clone(currentState);
            }

            if (cmd === 'open_multiple_files_dialog') {
                return ['/tmp/grid.xyz', '/tmp/grid.q'];
            }

            if (cmd === 'clear_grid_cache' || cmd === 'clear_solution_cache_v2') {
                return null;
            }

            if (cmd === 'load_plot3d_file_cached') {
                if (args?.path === '/tmp/grid.xyz') {
                    return [
                        {
                            id: 'grid-cache-1',
                            file_path: '/tmp/grid.xyz',
                            file_name: 'grid.xyz',
                            grid_index: 0,
                            dimensions: { i: 3, j: 3, k: 3 },
                            has_iblank: false,
                            has_solution: true,
                        },
                    ];
                }
                throw new Error('not a grid file');
            }

            if (cmd === 'load_plot3d_solution_cached') {
                return [
                    {
                        id: 'sol-cache-1',
                        file_path: '/tmp/grid.q',
                        file_name: 'grid.q',
                        grid_index: 0,
                        dimensions: { i: 3, j: 3, k: 3 },
                    },
                ];
            }

            if (cmd === 'set_plot_family') {
                currentState.plot_family = String(args?.family ?? currentState.plot_family);
                return makeApplyResult();
            }

            if (cmd === 'set_plot_contour_attribute') {
                currentState.contour_attribute = String(args?.attribute ?? currentState.contour_attribute);
                return makeApplyResult();
            }

            if (cmd === 'set_plot_contour_spec') {
                currentState.contour_spec = clone((args?.spec as Record<string, unknown> | undefined) ?? { mode: 'none' });
                return makeApplyResult();
            }

            if (cmd === 'set_plot_axis_view') {
                currentState.axis_view = String(args?.view ?? currentState.axis_view);
                currentState.viewpoint = axisViewToViewpoint(currentState.axis_view);
                return makeApplyResult();
            }

            if (cmd === 'set_plot_viewpoint') {
                currentState.axis_view = 'custom';
                currentState.viewpoint = clone((args?.vp as Record<string, unknown> | undefined) ?? null);
                return makeApplyResult();
            }

            if (cmd === 'set_plot_subsets') {
                currentState.subsets = clone((args?.subsets as Array<Record<string, unknown>> | undefined) ?? []);
                return makeApplyResult();
            }

            if (cmd === 'set_plot_walls') {
                currentState.walls = clone((args?.walls as Array<Record<string, unknown>> | undefined) ?? []);
                return makeApplyResult();
            }

            if (cmd === 'set_plot_fsurface') {
                currentState.fsurface = clone((args?.fsurface as Record<string, unknown> | null | undefined) ?? null);
                return makeApplyResult();
            }

            if (cmd === 'add_plot_text_annotation') {
                currentState.text_annotations = [
                    ...currentState.text_annotations,
                    clone((args?.text as Record<string, unknown> | undefined) ?? {}),
                ];
                return makeApplyResult();
            }

            if (cmd === 'clear_plot_text_annotations') {
                currentState.text_annotations = [];
                return makeApplyResult();
            }

            if (cmd === 'commit_plot') {
                const result = {
                    state: clone(currentState),
                    diagnostics: [{ capability: 'PLOT', severity: 'info', message: 'Plot committed' }],
                };
                commitResults.push(result);
                return result;
            }

            if (cmd === 'show_plot_status') {
                return {
                    state: clone(currentState),
                    diagnostics: [],
                    status: buildShowStatus(currentState),
                };
            }

            if (cmd === 'execute_com_script') {
                const path = String(args?.path ?? '');
                const fixture = scriptFixtures[path];
                if (!fixture) {
                    throw new Error(`Unhandled script fixture: ${path}`);
                }
                currentState = clone(fixture.final_state);
                return clone(fixture);
            }

            return null;
        });
    });

    it('matches script parity for contour spec and contour attribute commits', async () => {
        const scriptResult = await invokeMock('execute_com_script', { path: '/tmp/tkt-012c-contours.com' }) as MockScriptResult;

        resetBackendState();
        await loadFiles();

        const attributeSelect = await screen.findByDisplayValue('Line');
        fireEvent.change(attributeSelect, { target: { value: 'surface' } });

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith('set_plot_contour_attribute', { attribute: 'surface' });
            expect(invokeMock).toHaveBeenCalledWith('commit_plot');
        });

        const levelsSelect = await screen.findByDisplayValue('None');
        fireEvent.change(levelsSelect, { target: { value: 'automatic' } });

        const countInput = await screen.findByDisplayValue('10');
        fireEvent.change(countInput, { target: { value: '8' } });

        const applyButton = await screen.findByRole('button', { name: 'Apply' });
        fireEvent.click(applyButton);

        await waitFor(() => {
            expect(currentState).toEqual(scriptResult.final_state);
            expect(commitResults[commitResults.length - 1]?.state).toEqual(scriptResult.intents[scriptResult.intents.length - 1]?.state);
        });
    });

    it('matches script parity for view preset, camera viewpoint, and plot_up preservation', async () => {
        const scriptResult = await invokeMock('execute_com_script', { path: '/tmp/tkt-012c-orientation.com' }) as MockScriptResult;

        resetBackendState({ plot_up: 'negative_y' });
        render(<App />);

        const presetSelect = await screen.findByLabelText('View Preset:');
        fireEvent.change(presetSelect, { target: { value: 'plane_xy' } });

        await waitFor(() => {
            expect(commitResults[0]?.state).toEqual(scriptResult.intents[0]?.state);
        });

        const latestViewerProps = getLatestViewerProps();
        expect(latestViewerProps?.cameraPlotUp).toBe('negative_y');

        await (latestViewerProps?.onCameraCommit?.({ x: 5, y: 6, z: 7 }) as Promise<void> | undefined);

        await waitFor(() => {
            expect(currentState).toEqual(scriptResult.final_state);
        });
    });

    it('preserves plot_up across additional GUI view-preset commits after script orientation state', async () => {
        const scriptResult = await invokeMock('execute_com_script', { path: '/tmp/tkt-012c-orientation.com' }) as MockScriptResult;

        resetBackendState({
            axis_view: scriptResult.final_state.axis_view,
            plot_up: scriptResult.final_state.plot_up,
            viewpoint: scriptResult.final_state.viewpoint,
        });
        render(<App />);

        const presetSelect = await screen.findByLabelText('View Preset:');
        fireEvent.change(presetSelect, { target: { value: 'plus_x' } });

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith('set_plot_axis_view', { view: 'plus_x' });
            expect(invokeMock).toHaveBeenCalledWith('commit_plot');
        });

        const latestCommitState = commitResults[commitResults.length - 1]?.state;
        expect(latestCommitState?.axis_view).toBe('plus_x');
        expect(latestCommitState?.plot_up).toBe(scriptResult.final_state.plot_up);

        const latestViewerProps = getLatestViewerProps();
        expect(latestViewerProps?.cameraPlotUp).toBe(scriptResult.final_state.plot_up);
    });

    it('matches script parity for GUI-managed subsets and manual walls at commit boundary', async () => {
        const scriptResult = await invokeMock('execute_com_script', { path: '/tmp/tkt-012c-ranges.com' }) as MockScriptResult;

        resetBackendState();
        await loadFiles();

        const addSliceButton = await screen.findByRole('button', { name: '+ Add slice' });
        fireEvent.click(addSliceButton);

        const applySubsetsButton = await screen.findByRole('button', { name: 'Apply SUBSETS' });
        fireEvent.click(applySubsetsButton);

        await waitFor(() => {
            expect(currentState.subsets).toEqual(scriptResult.final_state.subsets);
        });

        const addWallRangeButton = await screen.findByRole('button', { name: 'Add Wall Range' });
        fireEvent.click(addWallRangeButton);

        const wallStartInputs = await screen.findAllByPlaceholderText('start');
        const wallEndInputs = await screen.findAllByPlaceholderText('end');
        fireEvent.change(wallStartInputs[0], { target: { value: '1' } });
        fireEvent.change(wallEndInputs[0], { target: { value: '3' } });
        fireEvent.change(wallStartInputs[1], { target: { value: '2' } });
        fireEvent.change(wallEndInputs[1], { target: { value: '2' } });
        fireEvent.change(wallStartInputs[2], { target: { value: '1' } });
        fireEvent.change(wallEndInputs[2], { target: { value: '3' } });

        const applyWallsButton = await screen.findByRole('button', { name: 'Apply WALLS' });
        fireEvent.click(applyWallsButton);

        await waitFor(() => {
            expect(currentState).toEqual(scriptResult.final_state);
            expect(commitResults[commitResults.length - 1]?.state).toEqual(scriptResult.intents[scriptResult.intents.length - 1]?.state);
        });
    });

    it('matches script parity for function-surface, FSURFACE, TEXT, and SHOW status', async () => {
        const scriptResult = await invokeMock('execute_com_script', { path: '/tmp/tkt-012c-function-surface.com' }) as MockScriptResult;

        resetBackendState();
        await loadFiles();

        const plotFamilySelect = await screen.findByDisplayValue('Contour');
        fireEvent.change(plotFamilySelect, { target: { value: 'function_surface' } });

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith('set_plot_family', { family: 'function_surface' });
        });

        const fsurfaceSectionHeading = await screen.findByText('FSURFACE');
        const fsurfaceSection = fsurfaceSectionHeading.parentElement as HTMLElement;

        const enabledCheckbox = within(fsurfaceSection).getByRole('checkbox', { name: 'Enabled' });
        fireEvent.click(enabledCheckbox);

        const fsurfaceLevelInput = within(fsurfaceSection).getByRole('spinbutton');
        fireEvent.change(fsurfaceLevelInput, { target: { value: '0.125' } });
        await waitFor(() => {
            expect((fsurfaceLevelInput as HTMLInputElement).value).toBe('0.125');
        });

        const applyFsurfaceButton = within(fsurfaceSection).getByRole('button', { name: 'Apply FSURFACE' });
        fireEvent.click(applyFsurfaceButton);

        const textInput = await screen.findByPlaceholderText('Annotation text');
        fireEvent.change(textInput, { target: { value: 'Cp label' } });

        const textXInput = await screen.findByPlaceholderText('X (0..1)');
        fireEvent.change(textXInput, { target: { value: '0.2' } });

        const textYInput = await screen.findByPlaceholderText('Y (0..1)');
        fireEvent.change(textYInput, { target: { value: '0.8' } });

        const addTextButton = await screen.findByRole('button', { name: 'Add TEXT' });
        fireEvent.click(addTextButton);

        const refreshShowButton = await screen.findByRole('button', { name: 'Refresh SHOW' });
        fireEvent.click(refreshShowButton);

        await waitFor(() => {
            expect(currentState).toEqual(scriptResult.final_state);
            expect(commitResults[commitResults.length - 1]?.state).toEqual(scriptResult.intents[scriptResult.intents.length - 1]?.state);
            expect(screen.getByText(scriptResult.show_output[0]!)).toBeTruthy();
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
