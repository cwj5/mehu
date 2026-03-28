/// Software rasterizer for the headless CLI.
///
/// Renders a [`SolutionSnapshot`] to an RGBA image using:
///  1. A bilinear-interpolated colormap heatmap of the scalar field.
///  2. Marching-squares iso-contour lines / attributes.
///  3. A thin frame border.
///
/// The output is intentionally close to the in-app Three.js renderer for
/// axis-aligned views.  Documented deviations: no anti-aliasing, no lighting
/// model, orthographic (no perspective), and function-surface mode is a
/// bounded filled MVP rather than a full Three.js-equivalent mesh/material path.
use image::{Rgba, RgbaImage};

use crate::colormap;
use crate::plot_state::{
    AxisView, ContourAttribute, ContourSpec, PlotFamily, PlotState, PlotUpAxis, ViewPoint,
};
use crate::script_executor::SolutionSnapshot;

// ─── Public entry point ──────────────────────────────────────────────────────

/// Render `snapshot` into `img` according to the visualization settings in
/// `state`.  Any renderer warnings are appended to `render_warnings`.
pub fn render_snapshot(
    img: &mut RgbaImage,
    snapshot: &SolutionSnapshot,
    state: &PlotState,
    render_warnings: &mut Vec<String>,
) {
    // Clear to black
    for pixel in img.pixels_mut() {
        *pixel = Rgba([0, 0, 0, 255]);
    }

    let margin = 20u32;

    // Scalar range: data-derived (MINMAX override is a spatial-axis command,
    // not a colormap range override, so we always use the snapshot's range).
    let field_min = snapshot.field_min;
    let field_max = snapshot.field_max;

    if matches!(state.plot_family, PlotFamily::FunctionSurface) {
        render_function_surface(
            img,
            snapshot,
            state,
            field_min,
            field_max,
            margin,
            render_warnings,
        );
        if !matches!(state.contour_spec, ContourSpec::None) {
            render_warnings.push(
                "Renderer: contour spec is ignored in Function Surface mode (filled MVP)"
                    .to_string(),
            );
        }
    } else {
        let (uvs, scalars, slab_w, slab_h) = extract_face_slab(snapshot, state, render_warnings);

        if uvs.is_empty() || slab_w == 0 || slab_h == 0 {
            render_warnings.push("Renderer: slab extraction produced no data".to_string());
            return;
        }

        rasterize_heatmap(
            img, &uvs, &scalars, slab_w, slab_h, field_min, field_max, margin,
        );

        let levels = resolve_contour_levels(&state.contour_spec, field_min, field_max);
        if !levels.is_empty() {
            draw_iso_features(
                img,
                &uvs,
                &scalars,
                slab_w,
                slab_h,
                &levels,
                &state.contour_attribute,
                field_min,
                field_max,
                margin,
            );
        }
    }

    draw_frame_border(img);
}

// ─── Function-surface filled MVP (depth-buffer) ─────────────────────────────

fn render_function_surface(
    img: &mut RgbaImage,
    snap: &SolutionSnapshot,
    state: &PlotState,
    field_min: f32,
    field_max: f32,
    margin: u32,
    warnings: &mut Vec<String>,
) {
    let ni = snap.ni as usize;
    let nj = snap.nj as usize;
    let nk = snap.nk as usize;
    let orientation = function_surface_orientation(snap);
    let (u_dim, v_dim) = function_surface_dims(orientation, ni, nj, nk);
    if u_dim < 2 || v_dim < 2 {
        warnings
            .push("Renderer: Function Surface requires at least a 2x2 projected slab".to_string());
        return;
    }

    let domain_scale = function_surface_extent(snap, orientation).max(1e-3) * 0.75;
    let field_span = (field_max - field_min).max(1e-20);

    let oblique_fallback = state.viewpoint.is_none()
        && (matches!(state.axis_view, AxisView::Custom)
            || axis_view_aligns_with_function_height(
                state.axis_view,
                function_surface_height_axis(orientation),
            ));

    if oblique_fallback {
        warnings.push(
            "Renderer: using oblique fallback camera for Function Surface height-aligned view"
                .to_string(),
        );
    }

    // Narrow parity gap: explicit custom VPOINT now uses a bounded perspective
    // projection for Function Surface renders.
    let use_perspective = matches!(state.axis_view, AxisView::Custom)
        && state.viewpoint.is_some()
        && !oblique_fallback;
    if use_perspective {
        warnings.push(
            "Renderer: using bounded perspective projection for custom Function Surface viewpoint"
                .to_string(),
        );
    }

    let camera = if oblique_fallback {
        if let Some(plot_up) = state.plot_up {
            camera_basis_from_viewpoint(
                &ViewPoint {
                    x: 2.2,
                    y: 1.8,
                    z: 1.4,
                },
                plot_up,
                true,
                warnings,
            )
        } else {
            camera_basis_from_viewpoint_default(&ViewPoint {
                x: 2.2,
                y: 1.8,
                z: 1.4,
            })
        }
    } else {
        camera_basis_for_state(state, warnings)
    };

    let mut world_points = Vec::with_capacity(u_dim * v_dim);
    let mut projected = Vec::with_capacity(u_dim * v_dim);
    let mut scalars = Vec::with_capacity(u_dim * v_dim);
    let camera_origin = state
        .viewpoint
        .as_ref()
        .map(|vp| (vp.x as f32, vp.y as f32, vp.z as f32));

    for v in 0..v_dim {
        for u in 0..u_dim {
            let idx = function_surface_index(orientation, u, v, ni, nj, nk);
            let t = ((snap.scalar[idx] - field_min) / field_span).clamp(0.0, 1.0);
            let height = (t - 0.5) * 2.0 * domain_scale;
            let point = function_surface_world_point(snap, orientation, idx, height);
            world_points.push(point);
            let mut p = if use_perspective {
                project_point_perspective(
                    point,
                    camera,
                    camera_origin.expect("camera origin required for perspective mode"),
                    55.0,
                )
            } else {
                project_point(point, camera)
            };
            if is_swapped_plane_view(state.axis_view) {
                p = (p.1, p.0, p.2);
            }
            projected.push(p);
            scalars.push(snap.scalar[idx]);
        }
    }

    let img_w = img.width();
    let img_h = img.height();
    let projected_uv: Vec<(f32, f32)> = projected.iter().map(|p| (p.0, p.1)).collect();
    let (min_u, max_u, min_v, max_v) = bbox(&projected_uv);
    let range_u = (max_u - min_u).max(1e-20);
    let range_v = (max_v - min_v).max(1e-20);
    let draw_w = img_w.saturating_sub(2 * margin) as f32;
    let draw_h = img_h.saturating_sub(2 * margin) as f32;
    if draw_w <= 0.0 || draw_h <= 0.0 {
        return;
    }

    let uv_to_screen = |u: f32, v: f32| -> (f32, f32) {
        let sx = margin as f32 + (u - min_u) / range_u * draw_w;
        let sy = margin as f32 + (v - min_v) / range_v * draw_h;
        (sx, sy)
    };

    let mut screen_verts = Vec::with_capacity(u_dim * v_dim);
    for idx in 0..(u_dim * v_dim) {
        let (sx, sy) = uv_to_screen(projected[idx].0, projected[idx].1);
        screen_verts.push(SurfaceVertex {
            x: sx,
            y: sy,
            depth: projected[idx].2,
            scalar: scalars[idx],
        });
    }

    // Filled rasterization with hidden-surface handling via z-buffer.
    let mut zbuf = vec![f32::INFINITY; (img.width() as usize) * (img.height() as usize)];

    for v in 0..(v_dim - 1) {
        for u in 0..(u_dim - 1) {
            let a = u + v * u_dim;
            let b = (u + 1) + v * u_dim;
            let c = u + (v + 1) * u_dim;
            let d = (u + 1) + (v + 1) * u_dim;

            let i0 = face_intensity(world_points[a], world_points[b], world_points[d], camera.2);
            rasterize_triangle_z(
                img,
                &mut zbuf,
                screen_verts[a],
                screen_verts[b],
                screen_verts[d],
                &state.contour_attribute,
                field_min,
                field_max,
                i0,
            );

            let i1 = face_intensity(world_points[a], world_points[d], world_points[c], camera.2);
            rasterize_triangle_z(
                img,
                &mut zbuf,
                screen_verts[a],
                screen_verts[d],
                screen_verts[c],
                &state.contour_attribute,
                field_min,
                field_max,
                i1,
            );
        }
    }

    // Attribute-specific overlays.
    if matches!(
        state.contour_attribute,
        ContourAttribute::Line | ContourAttribute::Grid
    ) {
        for v in 0..v_dim {
            for u in 0..(u_dim - 1) {
                let a = u + v * u_dim;
                let b = (u + 1) + v * u_dim;
                let color = surface_segment_color(
                    &state.contour_attribute,
                    0.5 * (scalars[a] + scalars[b]),
                    field_min,
                    field_max,
                );
                draw_line(
                    img,
                    screen_verts[a].x.round() as i32,
                    screen_verts[a].y.round() as i32,
                    screen_verts[b].x.round() as i32,
                    screen_verts[b].y.round() as i32,
                    color,
                );
            }
        }
        for v in 0..(v_dim - 1) {
            for u in 0..u_dim {
                let a = u + v * u_dim;
                let b = u + (v + 1) * u_dim;
                let color = surface_segment_color(
                    &state.contour_attribute,
                    0.5 * (scalars[a] + scalars[b]),
                    field_min,
                    field_max,
                );
                draw_line(
                    img,
                    screen_verts[a].x.round() as i32,
                    screen_verts[a].y.round() as i32,
                    screen_verts[b].x.round() as i32,
                    screen_verts[b].y.round() as i32,
                    color,
                );
            }
        }
    }

    if matches!(state.contour_attribute, ContourAttribute::Dots) {
        for (idx, vtx) in screen_verts.iter().enumerate() {
            let color =
                surface_segment_color(&state.contour_attribute, scalars[idx], field_min, field_max);
            paint_dot(img, vtx.x.round() as i32, vtx.y.round() as i32, color);
        }
    }
}

fn xy_extent(snap: &SolutionSnapshot) -> f32 {
    let min_x = snap.x.iter().copied().fold(f32::INFINITY, f32::min);
    let max_x = snap.x.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let min_y = snap.y.iter().copied().fold(f32::INFINITY, f32::min);
    let max_y = snap.y.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    (max_x - min_x).abs().max((max_y - min_y).abs())
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum FunctionSurfaceOrientation {
    KPlane,
    IPlane,
    JPlane,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum SpatialAxis {
    X,
    Y,
    Z,
}

fn function_surface_orientation(snap: &SolutionSnapshot) -> FunctionSurfaceOrientation {
    let ni = snap.ni as usize;
    let nj = snap.nj as usize;
    let nk = snap.nk as usize;

    if nk == 1 {
        FunctionSurfaceOrientation::KPlane
    } else if ni == 1 {
        FunctionSurfaceOrientation::IPlane
    } else if nj == 1 {
        FunctionSurfaceOrientation::JPlane
    } else {
        FunctionSurfaceOrientation::KPlane
    }
}

fn function_surface_dims(
    orientation: FunctionSurfaceOrientation,
    ni: usize,
    nj: usize,
    nk: usize,
) -> (usize, usize) {
    match orientation {
        FunctionSurfaceOrientation::KPlane => (ni, nj),
        FunctionSurfaceOrientation::IPlane => (nj, nk),
        FunctionSurfaceOrientation::JPlane => (ni, nk),
    }
}

fn function_surface_index(
    orientation: FunctionSurfaceOrientation,
    u: usize,
    v: usize,
    ni: usize,
    nj: usize,
    nk: usize,
) -> usize {
    match orientation {
        FunctionSurfaceOrientation::KPlane => {
            let k = nk.saturating_sub(1);
            u + v * ni + k * ni * nj
        }
        FunctionSurfaceOrientation::IPlane => {
            let i = 0;
            let j = u;
            let k = v;
            i + j * ni + k * ni * nj
        }
        FunctionSurfaceOrientation::JPlane => {
            let i = u;
            let j = 0;
            let k = v;
            i + j * ni + k * ni * nj
        }
    }
}

fn function_surface_world_point(
    snap: &SolutionSnapshot,
    orientation: FunctionSurfaceOrientation,
    idx: usize,
    height: f32,
) -> (f32, f32, f32) {
    match orientation {
        FunctionSurfaceOrientation::KPlane => (snap.x[idx], snap.y[idx], height),
        FunctionSurfaceOrientation::IPlane => (height, snap.y[idx], snap.z[idx]),
        FunctionSurfaceOrientation::JPlane => (snap.x[idx], height, snap.z[idx]),
    }
}

fn function_surface_height_axis(orientation: FunctionSurfaceOrientation) -> SpatialAxis {
    match orientation {
        FunctionSurfaceOrientation::KPlane => SpatialAxis::Z,
        FunctionSurfaceOrientation::IPlane => SpatialAxis::X,
        FunctionSurfaceOrientation::JPlane => SpatialAxis::Y,
    }
}

fn axis_view_aligns_with_function_height(view: AxisView, axis: SpatialAxis) -> bool {
    match axis {
        SpatialAxis::X => matches!(
            view,
            AxisView::PlusX | AxisView::MinusX | AxisView::PlaneYZ | AxisView::PlaneZY
        ),
        SpatialAxis::Y => matches!(
            view,
            AxisView::PlusY | AxisView::MinusY | AxisView::PlaneXZ | AxisView::PlaneZX
        ),
        SpatialAxis::Z => matches!(
            view,
            AxisView::PlusZ | AxisView::MinusZ | AxisView::PlaneXY | AxisView::PlaneYX
        ),
    }
}

#[derive(Copy, Clone)]
struct AxisDirection {
    axis: SpatialAxis,
    sign: f32,
}

fn plot_up_axis_direction(axis: PlotUpAxis) -> AxisDirection {
    match axis {
        PlotUpAxis::PositiveX => AxisDirection {
            axis: SpatialAxis::X,
            sign: 1.0,
        },
        PlotUpAxis::PositiveY => AxisDirection {
            axis: SpatialAxis::Y,
            sign: 1.0,
        },
        PlotUpAxis::PositiveZ => AxisDirection {
            axis: SpatialAxis::Z,
            sign: 1.0,
        },
        PlotUpAxis::NegativeX => AxisDirection {
            axis: SpatialAxis::X,
            sign: -1.0,
        },
        PlotUpAxis::NegativeY => AxisDirection {
            axis: SpatialAxis::Y,
            sign: -1.0,
        },
        PlotUpAxis::NegativeZ => AxisDirection {
            axis: SpatialAxis::Z,
            sign: -1.0,
        },
    }
}

fn default_contour_axes_for_view(view: AxisView) -> (AxisDirection, AxisDirection) {
    match view {
        AxisView::PlusZ | AxisView::PlaneXY => (
            AxisDirection {
                axis: SpatialAxis::X,
                sign: 1.0,
            },
            AxisDirection {
                axis: SpatialAxis::Y,
                sign: 1.0,
            },
        ),
        AxisView::PlaneYX => (
            AxisDirection {
                axis: SpatialAxis::Y,
                sign: 1.0,
            },
            AxisDirection {
                axis: SpatialAxis::X,
                sign: 1.0,
            },
        ),
        AxisView::MinusZ => (
            AxisDirection {
                axis: SpatialAxis::X,
                sign: -1.0,
            },
            AxisDirection {
                axis: SpatialAxis::Y,
                sign: 1.0,
            },
        ),
        AxisView::PlusX | AxisView::PlaneYZ => (
            AxisDirection {
                axis: SpatialAxis::Y,
                sign: 1.0,
            },
            AxisDirection {
                axis: SpatialAxis::Z,
                sign: 1.0,
            },
        ),
        AxisView::PlaneZY => (
            AxisDirection {
                axis: SpatialAxis::Z,
                sign: 1.0,
            },
            AxisDirection {
                axis: SpatialAxis::Y,
                sign: 1.0,
            },
        ),
        AxisView::MinusX => (
            AxisDirection {
                axis: SpatialAxis::Y,
                sign: -1.0,
            },
            AxisDirection {
                axis: SpatialAxis::Z,
                sign: 1.0,
            },
        ),
        AxisView::PlusY | AxisView::PlaneXZ => (
            AxisDirection {
                axis: SpatialAxis::X,
                sign: 1.0,
            },
            AxisDirection {
                axis: SpatialAxis::Z,
                sign: 1.0,
            },
        ),
        AxisView::PlaneZX => (
            AxisDirection {
                axis: SpatialAxis::Z,
                sign: 1.0,
            },
            AxisDirection {
                axis: SpatialAxis::X,
                sign: 1.0,
            },
        ),
        AxisView::MinusY => (
            AxisDirection {
                axis: SpatialAxis::X,
                sign: -1.0,
            },
            AxisDirection {
                axis: SpatialAxis::Z,
                sign: 1.0,
            },
        ),
        AxisView::Custom => (
            AxisDirection {
                axis: SpatialAxis::X,
                sign: 1.0,
            },
            AxisDirection {
                axis: SpatialAxis::Y,
                sign: 1.0,
            },
        ),
    }
}

fn resolve_contour_axes_for_view(
    view: AxisView,
    plot_up: Option<PlotUpAxis>,
    warnings: &mut Vec<String>,
) -> (AxisDirection, AxisDirection) {
    let (default_horizontal, default_vertical) = default_contour_axes_for_view(view);
    let Some(desired_vertical) = plot_up.map(plot_up_axis_direction) else {
        return (default_horizontal, default_vertical);
    };

    if desired_vertical.axis == default_vertical.axis {
        return (
            default_horizontal,
            AxisDirection {
                axis: desired_vertical.axis,
                sign: desired_vertical.sign,
            },
        );
    }

    if desired_vertical.axis == default_horizontal.axis {
        return (
            AxisDirection {
                axis: default_vertical.axis,
                sign: 1.0,
            },
            AxisDirection {
                axis: desired_vertical.axis,
                sign: desired_vertical.sign,
            },
        );
    }

    warnings.push(
        "Renderer: PLOT/UP axis is not visible for the selected contour view; using default view orientation"
            .to_string(),
    );
    (default_horizontal, default_vertical)
}

fn axis_value(axis: SpatialAxis, x: f32, y: f32, z: f32) -> f32 {
    match axis {
        SpatialAxis::X => x,
        SpatialAxis::Y => y,
        SpatialAxis::Z => z,
    }
}

fn axis_extent(values: &[f32]) -> f32 {
    let min_value = values.iter().copied().fold(f32::INFINITY, f32::min);
    let max_value = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    (max_value - min_value).abs()
}

fn function_surface_extent(
    snap: &SolutionSnapshot,
    orientation: FunctionSurfaceOrientation,
) -> f32 {
    match orientation {
        FunctionSurfaceOrientation::KPlane => xy_extent(snap),
        FunctionSurfaceOrientation::IPlane => axis_extent(&snap.y).max(axis_extent(&snap.z)),
        FunctionSurfaceOrientation::JPlane => axis_extent(&snap.x).max(axis_extent(&snap.z)),
    }
}

type CameraBasis = ((f32, f32, f32), (f32, f32, f32), (f32, f32, f32));

fn is_swapped_plane_view(view: AxisView) -> bool {
    matches!(
        view,
        AxisView::PlaneYX | AxisView::PlaneZX | AxisView::PlaneZY
    )
}

fn camera_basis_for_state(state: &PlotState, warnings: &mut Vec<String>) -> CameraBasis {
    if let Some(vp) = &state.viewpoint {
        return if let Some(plot_up) = state.plot_up {
            camera_basis_from_viewpoint(vp, plot_up, true, warnings)
        } else {
            camera_basis_from_viewpoint_default(vp)
        };
    }

    let viewpoint = match state.axis_view {
        AxisView::PlusX => ViewPoint {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        },
        AxisView::MinusX => ViewPoint {
            x: -1.0,
            y: 0.0,
            z: 0.0,
        },
        AxisView::PlusY => ViewPoint {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        },
        AxisView::MinusY => ViewPoint {
            x: 0.0,
            y: -1.0,
            z: 0.0,
        },
        AxisView::PlusZ | AxisView::PlaneXY | AxisView::PlaneYX => ViewPoint {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        },
        AxisView::MinusZ => ViewPoint {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        },
        AxisView::PlaneXZ | AxisView::PlaneZX => ViewPoint {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        },
        AxisView::PlaneYZ | AxisView::PlaneZY => ViewPoint {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        },
        AxisView::Custom => ViewPoint {
            x: 1.6,
            y: 1.2,
            z: 1.1,
        },
    };

    if let Some(plot_up) = state.plot_up {
        camera_basis_from_viewpoint(&viewpoint, plot_up, true, warnings)
    } else {
        camera_basis_from_viewpoint_default(&viewpoint)
    }
}

fn camera_basis_from_viewpoint_default(vp: &ViewPoint) -> CameraBasis {
    let (lx, ly, lz) = normalize((-vp.x as f32, -vp.y as f32, -vp.z as f32));
    let (wx, wy, wz) = if ly.abs() > 0.9 {
        (0.0f32, 0.0, 1.0)
    } else {
        (0.0, 1.0, 0.0)
    };
    let right = normalize(cross((lx, ly, lz), (wx, wy, wz)));
    let up = normalize(cross(right, (lx, ly, lz)));
    (right, up, (lx, ly, lz))
}

fn camera_basis_from_viewpoint(
    vp: &ViewPoint,
    preferred_up: PlotUpAxis,
    warn_on_fallback: bool,
    warnings: &mut Vec<String>,
) -> CameraBasis {
    let (lx, ly, lz) = normalize((-vp.x as f32, -vp.y as f32, -vp.z as f32));
    let preferred = plot_up_axis_direction(preferred_up);
    let up_hint = match preferred.axis {
        SpatialAxis::X => (preferred.sign, 0.0, 0.0),
        SpatialAxis::Y => (0.0, preferred.sign, 0.0),
        SpatialAxis::Z => (0.0, 0.0, preferred.sign),
    };
    let projected_up = (
        up_hint.0 - dot(up_hint, (lx, ly, lz)) * lx,
        up_hint.1 - dot(up_hint, (lx, ly, lz)) * ly,
        up_hint.2 - dot(up_hint, (lx, ly, lz)) * lz,
    );
    let projected_len = (projected_up.0 * projected_up.0
        + projected_up.1 * projected_up.1
        + projected_up.2 * projected_up.2)
        .sqrt();
    let (wx, wy, wz) = if projected_len > 1e-6 {
        normalize(projected_up)
    } else if ly.abs() > 0.9 {
        if warn_on_fallback {
            warnings.push(
                "Renderer: PLOT/UP is parallel to the camera look direction; using fallback camera up vector"
                    .to_string(),
            );
        }
        (0.0f32, 0.0, 1.0)
    } else {
        if warn_on_fallback {
            warnings.push(
                "Renderer: PLOT/UP is parallel to the camera look direction; using fallback camera up vector"
                    .to_string(),
            );
        }
        (0.0, 1.0, 0.0)
    };
    let right = normalize(cross((lx, ly, lz), (wx, wy, wz)));
    let up = normalize(cross(right, (lx, ly, lz)));
    (right, up, (lx, ly, lz))
}

fn project_point(point: (f32, f32, f32), camera: CameraBasis) -> (f32, f32, f32) {
    let u = point.0 * (camera.0).0 + point.1 * (camera.0).1 + point.2 * (camera.0).2;
    let v = point.0 * (camera.1).0 + point.1 * (camera.1).1 + point.2 * (camera.1).2;
    let depth = point.0 * (camera.2).0 + point.1 * (camera.2).1 + point.2 * (camera.2).2;
    (u, v, depth)
}

fn project_point_perspective(
    point: (f32, f32, f32),
    camera: CameraBasis,
    camera_origin: (f32, f32, f32),
    fov_y_degrees: f32,
) -> (f32, f32, f32) {
    let rel = (
        point.0 - camera_origin.0,
        point.1 - camera_origin.1,
        point.2 - camera_origin.2,
    );
    let u = rel.0 * (camera.0).0 + rel.1 * (camera.0).1 + rel.2 * (camera.0).2;
    let v = rel.0 * (camera.1).0 + rel.1 * (camera.1).1 + rel.2 * (camera.1).2;
    let depth = rel.0 * (camera.2).0 + rel.1 * (camera.2).1 + rel.2 * (camera.2).2;

    let z = depth.max(1e-3);
    let tan_half_fov = (0.5 * fov_y_degrees.to_radians()).tan().max(1e-6);
    let scale = 1.0 / (z * tan_half_fov);
    (u * scale, v * scale, z)
}

#[derive(Copy, Clone)]
struct SurfaceVertex {
    x: f32,
    y: f32,
    depth: f32,
    scalar: f32,
}

fn face_intensity(
    p0: (f32, f32, f32),
    p1: (f32, f32, f32),
    p2: (f32, f32, f32),
    look: (f32, f32, f32),
) -> f32 {
    let e1 = (p1.0 - p0.0, p1.1 - p0.1, p1.2 - p0.2);
    let e2 = (p2.0 - p0.0, p2.1 - p0.1, p2.2 - p0.2);
    let n = normalize(cross(e1, e2));
    let view = (-look.0, -look.1, -look.2);
    let lambert = dot(n, view).abs().clamp(0.0, 1.0);
    0.35 + 0.65 * lambert
}

fn surface_fill_color(
    attr: &ContourAttribute,
    scalar: f32,
    field_min: f32,
    field_max: f32,
    intensity: f32,
) -> Rgba<u8> {
    let t = ((scalar - field_min) / (field_max - field_min).max(1e-20)).clamp(0.0, 1.0);
    let base = match attr {
        ContourAttribute::Surface | ContourAttribute::ColorContours => colormap::apply(t),
        ContourAttribute::Line => colormap::grayscale(0.45 + 0.4 * t),
        ContourAttribute::Grid => colormap::grayscale(0.35 + 0.35 * t),
        ContourAttribute::Dots => colormap::grayscale(0.4 + 0.35 * t),
    };
    let k = intensity.clamp(0.0, 1.0);
    Rgba([
        (base[0] as f32 * k).clamp(0.0, 255.0) as u8,
        (base[1] as f32 * k).clamp(0.0, 255.0) as u8,
        (base[2] as f32 * k).clamp(0.0, 255.0) as u8,
        255,
    ])
}

fn rasterize_triangle_z(
    img: &mut RgbaImage,
    zbuf: &mut [f32],
    v0: SurfaceVertex,
    v1: SurfaceVertex,
    v2: SurfaceVertex,
    attr: &ContourAttribute,
    field_min: f32,
    field_max: f32,
    intensity: f32,
) {
    let w = img.width() as i32;
    let h = img.height() as i32;

    let min_x = v0.x.min(v1.x).min(v2.x).floor().max(0.0) as i32;
    let max_x = v0.x.max(v1.x).max(v2.x).ceil().min((w - 1) as f32) as i32;
    let min_y = v0.y.min(v1.y).min(v2.y).floor().max(0.0) as i32;
    let max_y = v0.y.max(v1.y).max(v2.y).ceil().min((h - 1) as f32) as i32;

    if min_x > max_x || min_y > max_y {
        return;
    }

    let area = edge_fn(v0.x, v0.y, v1.x, v1.y, v2.x, v2.y);
    if area.abs() < 1e-12 {
        return;
    }

    for py in min_y..=max_y {
        for px in min_x..=max_x {
            let x = px as f32 + 0.5;
            let y = py as f32 + 0.5;
            let w0 = edge_fn(v1.x, v1.y, v2.x, v2.y, x, y) / area;
            let w1 = edge_fn(v2.x, v2.y, v0.x, v0.y, x, y) / area;
            let w2 = edge_fn(v0.x, v0.y, v1.x, v1.y, x, y) / area;

            if w0 >= -1e-6 && w1 >= -1e-6 && w2 >= -1e-6 {
                let depth = w0 * v0.depth + w1 * v1.depth + w2 * v2.depth;
                let idx = py as usize * img.width() as usize + px as usize;
                if depth < zbuf[idx] {
                    zbuf[idx] = depth;
                    let scalar = w0 * v0.scalar + w1 * v1.scalar + w2 * v2.scalar;
                    let color = surface_fill_color(attr, scalar, field_min, field_max, intensity);
                    img.put_pixel(px as u32, py as u32, color);
                }
            }
        }
    }
}

fn edge_fn(x0: f32, y0: f32, x1: f32, y1: f32, x: f32, y: f32) -> f32 {
    (x - x0) * (y1 - y0) - (y - y0) * (x1 - x0)
}

fn dot(a: (f32, f32, f32), b: (f32, f32, f32)) -> f32 {
    a.0 * b.0 + a.1 * b.1 + a.2 * b.2
}

fn surface_segment_color(
    attr: &ContourAttribute,
    scalar: f32,
    field_min: f32,
    field_max: f32,
) -> Rgba<u8> {
    let t = ((scalar - field_min) / (field_max - field_min).max(1e-20)).clamp(0.0, 1.0);
    match attr {
        ContourAttribute::Line => Rgba([245, 245, 245, 255]),
        ContourAttribute::Grid => Rgba([190, 255, 190, 255]),
        ContourAttribute::Dots => Rgba([255, 245, 230, 255]),
        ContourAttribute::Surface | ContourAttribute::ColorContours => {
            let [r, g, b] = colormap::turbo(t);
            Rgba([r, g, b, 255])
        }
    }
}

// ─── Slab extraction ─────────────────────────────────────────────────────────

/// Extract a 2D projected slab from the snapshot.
///
/// Returns `(uvs, scalars, slab_width, slab_height)` where `uvs` contains
/// physical 2D projected coordinates and the slab is addressed as
/// `idx = u_idx + v_idx * slab_width`.
fn extract_face_slab(
    snap: &SolutionSnapshot,
    state: &PlotState,
    warnings: &mut Vec<String>,
) -> (Vec<(f32, f32)>, Vec<f32>, usize, usize) {
    let ni = snap.ni as usize;
    let nj = snap.nj as usize;
    let nk = snap.nk as usize;

    // For a Custom view (explicit VPOINT), use orthographic projection.
    if matches!(state.axis_view, AxisView::Custom) {
        if state.viewpoint.is_some() {
            return project_orthographic(snap, state, warnings);
        } else {
            // No viewpoint set — fall back to PlusZ.
            warnings
                .push("Renderer: Custom axis view with no viewpoint; defaulting to +Z".to_string());
        }
    }

    let (horizontal_axis, vertical_axis) =
        resolve_contour_axes_for_view(state.axis_view, state.plot_up, warnings);
    let point_uv = |idx: usize| {
        (
            horizontal_axis.sign
                * axis_value(horizontal_axis.axis, snap.x[idx], snap.y[idx], snap.z[idx]),
            vertical_axis.sign
                * axis_value(vertical_axis.axis, snap.x[idx], snap.y[idx], snap.z[idx]),
        )
    };

    match &state.axis_view {
        // ── Looking down Z, seeing X-Y ──────────────────────────────────────
        AxisView::PlusZ | AxisView::PlaneXY | AxisView::PlaneYX => {
            let k = nk - 1;
            face_slab(snap, ni, nj, |i, j| {
                let idx = i + j * ni + k * ni * nj;
                (point_uv(idx), snap.scalar[idx])
            })
        }
        AxisView::MinusZ => {
            let k = 0;
            face_slab(snap, ni, nj, |i, j| {
                let idx = i + j * ni + k * ni * nj;
                (point_uv(idx), snap.scalar[idx])
            })
        }

        // ── Looking from +X, seeing Y-Z ─────────────────────────────────────
        AxisView::PlusX | AxisView::PlaneYZ | AxisView::PlaneZY => {
            let i = ni - 1;
            face_slab(snap, nj, nk, |j, k| {
                let idx = i + j * ni + k * ni * nj;
                (point_uv(idx), snap.scalar[idx])
            })
        }
        AxisView::MinusX => {
            let i = 0;
            face_slab(snap, nj, nk, |j, k| {
                let idx = i + j * ni + k * ni * nj;
                (point_uv(idx), snap.scalar[idx])
            })
        }

        // ── Looking from +Y, seeing X-Z ─────────────────────────────────────
        AxisView::PlusY | AxisView::PlaneXZ | AxisView::PlaneZX => {
            let j = nj - 1;
            face_slab(snap, ni, nk, |i, k| {
                let idx = i + j * ni + k * ni * nj;
                (point_uv(idx), snap.scalar[idx])
            })
        }
        AxisView::MinusY => {
            let j = 0;
            face_slab(snap, ni, nk, |i, k| {
                let idx = i + j * ni + k * ni * nj;
                (point_uv(idx), snap.scalar[idx])
            })
        }

        // AxisView::Custom is handled above; this branch is unreachable but
        // the fallback is PlusZ to keep the match total.
        AxisView::Custom => {
            let k = nk - 1;
            face_slab(snap, ni, nj, |i, j| {
                let idx = i + j * ni + k * ni * nj;
                (point_uv(idx), snap.scalar[idx])
            })
        }
    }
}

/// Build a contiguous slab by calling `point_fn(u_idx, v_idx)` for each cell.
fn face_slab<F>(
    snap: &SolutionSnapshot,
    slab_w: usize,
    slab_h: usize,
    point_fn: F,
) -> (Vec<(f32, f32)>, Vec<f32>, usize, usize)
where
    F: Fn(usize, usize) -> ((f32, f32), f32),
{
    let _ = snap; // not used directly but keeps the signature consistent
    let n = slab_w * slab_h;
    let mut uvs = Vec::with_capacity(n);
    let mut scalars = Vec::with_capacity(n);

    for v in 0..slab_h {
        for u in 0..slab_w {
            let (uv, s) = point_fn(u, v);
            uvs.push(uv);
            scalars.push(s);
        }
    }

    (uvs, scalars, slab_w, slab_h)
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum ProjectionFace {
    IMin,
    IMax,
    JMin,
    JMax,
    KMin,
    KMax,
}

/// Orthographic projection for arbitrary VPOINT views.
///
/// To stay aligned with axis-view semantics, we project the outer slab from the
/// dominant camera look axis (X/Y/Z) and pick min/max side from the look sign.
/// For oblique viewpoints this is a bounded approximation and emits a warning.
fn project_orthographic(
    snap: &SolutionSnapshot,
    state: &PlotState,
    warnings: &mut Vec<String>,
) -> (Vec<(f32, f32)>, Vec<f32>, usize, usize) {
    let camera = camera_basis_for_state(state, warnings);
    let look = camera.2;
    let abs_x = look.0.abs();
    let abs_y = look.1.abs();
    let abs_z = look.2.abs();

    let ni = snap.ni as usize;
    let nj = snap.nj as usize;
    let nk = snap.nk as usize;

    let face_dims = |face: ProjectionFace| -> (usize, usize) {
        match face {
            ProjectionFace::IMin | ProjectionFace::IMax => (nj, nk),
            ProjectionFace::JMin | ProjectionFace::JMax => (ni, nk),
            ProjectionFace::KMin | ProjectionFace::KMax => (ni, nj),
        }
    };

    let mut candidates = vec![
        (
            abs_x,
            if look.0 < 0.0 {
                ProjectionFace::IMax
            } else {
                ProjectionFace::IMin
            },
        ),
        (
            abs_y,
            if look.1 < 0.0 {
                ProjectionFace::JMax
            } else {
                ProjectionFace::JMin
            },
        ),
        (
            abs_z,
            if look.2 < 0.0 {
                ProjectionFace::KMax
            } else {
                ProjectionFace::KMin
            },
        ),
    ];
    candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let (dominant, primary_face) = candidates[0];
    let mut face = primary_face;
    for (_, candidate_face) in &candidates {
        let (w, h) = face_dims(*candidate_face);
        if w >= 2 && h >= 2 {
            face = *candidate_face;
            break;
        }
    }

    if dominant < 0.85 {
        warnings.push(
            "Renderer: custom VPOINT is oblique; using dominant-axis outer-face projection"
                .to_string(),
        );
    }
    if face != primary_face {
        warnings.push(
            "Renderer: custom VPOINT dominant face is degenerate; using nearest non-degenerate outer face"
                .to_string(),
        );
    }

    let (slab_w, slab_h) = face_dims(face);

    let mut uvs = Vec::with_capacity(slab_w * slab_h);
    let mut scalars = Vec::with_capacity(slab_w * slab_h);

    for v in 0..slab_h {
        for u in 0..slab_w {
            let idx = match face {
                ProjectionFace::IMin => {
                    let i = 0;
                    let j = u;
                    let k = v;
                    i + j * ni + k * ni * nj
                }
                ProjectionFace::IMax => {
                    let i = ni - 1;
                    let j = u;
                    let k = v;
                    i + j * ni + k * ni * nj
                }
                ProjectionFace::JMin => {
                    let i = u;
                    let j = 0;
                    let k = v;
                    i + j * ni + k * ni * nj
                }
                ProjectionFace::JMax => {
                    let i = u;
                    let j = nj - 1;
                    let k = v;
                    i + j * ni + k * ni * nj
                }
                ProjectionFace::KMin => {
                    let i = u;
                    let j = v;
                    let k = 0;
                    i + j * ni + k * ni * nj
                }
                ProjectionFace::KMax => {
                    let i = u;
                    let j = v;
                    let k = nk - 1;
                    i + j * ni + k * ni * nj
                }
            };

            let p = (snap.x[idx], snap.y[idx], snap.z[idx]);
            let proj = project_point(p, camera);
            uvs.push((proj.0, proj.1));
            scalars.push(snap.scalar[idx]);
        }
    }

    (uvs, scalars, slab_w, slab_h)
}

fn normalize(v: (f32, f32, f32)) -> (f32, f32, f32) {
    let len = (v.0 * v.0 + v.1 * v.1 + v.2 * v.2).sqrt().max(1e-30);
    (v.0 / len, v.1 / len, v.2 / len)
}

fn cross(a: (f32, f32, f32), b: (f32, f32, f32)) -> (f32, f32, f32) {
    (
        a.1 * b.2 - a.2 * b.1,
        a.2 * b.0 - a.0 * b.2,
        a.0 * b.1 - a.1 * b.0,
    )
}

// ─── Heatmap rasterizer ──────────────────────────────────────────────────────

fn rasterize_heatmap(
    img: &mut RgbaImage,
    uvs: &[(f32, f32)],
    scalars: &[f32],
    slab_w: usize,
    slab_h: usize,
    field_min: f32,
    field_max: f32,
    margin: u32,
) {
    let img_w = img.width();
    let img_h = img.height();
    let (min_u, max_u, min_v, max_v) = bbox(uvs);
    let range_u = (max_u - min_u).max(1e-20);
    let range_v = (max_v - min_v).max(1e-20);
    let field_range = (field_max - field_min).max(1e-20);

    let draw_w = img_w.saturating_sub(2 * margin) as f32;
    let draw_h = img_h.saturating_sub(2 * margin) as f32;
    if draw_w <= 0.0 || draw_h <= 0.0 {
        return;
    }

    let sw = slab_w as f32;
    let sh = slab_h as f32;

    for py in margin..(img_h.saturating_sub(margin)) {
        let v = min_v + (py - margin) as f32 / draw_h * range_v;
        let fj = ((v - min_v) / range_v * (sh - 1.0)).clamp(0.0, sh - 1.001);
        let j0 = fj.floor() as usize;
        let j1 = (j0 + 1).min(slab_h - 1);
        let jt = fj - j0 as f32;

        for px in margin..(img_w.saturating_sub(margin)) {
            let u = min_u + (px - margin) as f32 / draw_w * range_u;
            let fi = ((u - min_u) / range_u * (sw - 1.0)).clamp(0.0, sw - 1.001);
            let i0 = fi.floor() as usize;
            let i1 = (i0 + 1).min(slab_w - 1);
            let it = fi - i0 as f32;

            // Bilinear interpolation
            let s00 = scalars[i0 + j0 * slab_w];
            let s10 = scalars[i1 + j0 * slab_w];
            let s01 = scalars[i0 + j1 * slab_w];
            let s11 = scalars[i1 + j1 * slab_w];
            let scalar = s00 * (1.0 - it) * (1.0 - jt)
                + s10 * it * (1.0 - jt)
                + s01 * (1.0 - it) * jt
                + s11 * it * jt;

            let t = ((scalar - field_min) / field_range).clamp(0.0, 1.0);
            let [r, g, b] = colormap::apply(t);
            img.put_pixel(px, py, Rgba([r, g, b, 255]));
        }
    }
}

// ─── Contour level resolution ────────────────────────────────────────────────

/// Resolve `ContourSpec` to a flat list of absolute physical values.
///
/// Delegates to the canonical [`ContourSpec::resolve`] method already present
/// in `plot_state.rs`, stripping diagnostics.
pub fn resolve_contour_levels(spec: &ContourSpec, field_min: f32, field_max: f32) -> Vec<f32> {
    let (levels_f64, _diags) = spec.resolve(field_min as f64, field_max as f64);
    levels_f64.into_iter().map(|v| v as f32).collect()
}

// ─── Marching squares / iso-feature drawer ───────────────────────────────────

fn draw_iso_features(
    img: &mut RgbaImage,
    uvs: &[(f32, f32)],
    scalars: &[f32],
    slab_w: usize,
    slab_h: usize,
    levels: &[f32],
    attr: &ContourAttribute,
    field_min: f32,
    field_max: f32,
    margin: u32,
) {
    let img_w = img.width();
    let img_h = img.height();
    let (min_u, max_u, min_v, max_v) = bbox(uvs);
    let range_u = (max_u - min_u).max(1e-20);
    let range_v = (max_v - min_v).max(1e-20);
    let field_range = (field_max - field_min).max(1e-20);

    let draw_w = img_w.saturating_sub(2 * margin) as f32;
    let draw_h = img_h.saturating_sub(2 * margin) as f32;
    if draw_w <= 0.0 || draw_h <= 0.0 {
        return;
    }

    let uv_to_px = |u: f32, v: f32| -> (i32, i32) {
        let px = margin as f32 + (u - min_u) / range_u * draw_w;
        let py = margin as f32 + (v - min_v) / range_v * draw_h;
        (px.round() as i32, py.round() as i32)
    };

    let field_range_inv = 1.0 / field_range;

    for &level in levels {
        let level_t = ((level - field_min) * field_range_inv).clamp(0.0, 1.0);
        let line_color = iso_line_color(attr, level_t);

        for jc in 0..(slab_h - 1) {
            for ic in 0..(slab_w - 1) {
                let idx00 = ic + jc * slab_w;
                let idx10 = (ic + 1) + jc * slab_w;
                let idx01 = ic + (jc + 1) * slab_w;
                let idx11 = (ic + 1) + (jc + 1) * slab_w;

                let d00 = scalars[idx00] - level;
                let d10 = scalars[idx10] - level;
                let d01 = scalars[idx01] - level;
                let d11 = scalars[idx11] - level;

                let (u00, v00) = uvs[idx00];
                let (u10, v10) = uvs[idx10];
                let (u01, v01) = uvs[idx01];
                let (u11, v11) = uvs[idx11];

                // Classify which edges are crossed (marching squares)
                let mut pts: [Option<(f32, f32)>; 4] = [None; 4];

                // Bottom edge: 00 → 10
                if d00 * d10 < 0.0 {
                    let t = d00 / (d00 - d10);
                    pts[0] = Some((u00 + t * (u10 - u00), v00 + t * (v10 - v00)));
                }
                // Right edge: 10 → 11
                if d10 * d11 < 0.0 {
                    let t = d10 / (d10 - d11);
                    pts[1] = Some((u10 + t * (u11 - u10), v10 + t * (v11 - v10)));
                }
                // Top edge: 01 → 11
                if d01 * d11 < 0.0 {
                    let t = d01 / (d01 - d11);
                    pts[2] = Some((u01 + t * (u11 - u01), v01 + t * (v11 - v01)));
                }
                // Left edge: 00 → 01
                if d00 * d01 < 0.0 {
                    let t = d00 / (d00 - d01);
                    pts[3] = Some((u00 + t * (u01 - u00), v00 + t * (v01 - v00)));
                }

                // Connect crossing pairs
                let crossings: Vec<(f32, f32)> = pts.iter().flatten().copied().collect();

                if matches!(attr, ContourAttribute::Dots) {
                    // Draw a small dot at each crossing point
                    for (cu, cv) in &crossings {
                        let (px, py) = uv_to_px(*cu, *cv);
                        paint_dot(img, px, py, line_color);
                    }
                } else if crossings.len() >= 2 {
                    // Draw a line segment between the first two crossings
                    let (px0, py0) = uv_to_px(crossings[0].0, crossings[0].1);
                    let (px1, py1) = uv_to_px(crossings[1].0, crossings[1].1);
                    draw_line(img, px0, py0, px1, py1, line_color);

                    // If a saddle case produced 4 crossings, connect the second pair too
                    if crossings.len() == 4 {
                        let (px2, py2) = uv_to_px(crossings[2].0, crossings[2].1);
                        let (px3, py3) = uv_to_px(crossings[3].0, crossings[3].1);
                        draw_line(img, px2, py2, px3, py3, line_color);
                    }
                }
            }
        }
    }
}

/// Color for an iso-line given its normalized position within the scalar range.
fn iso_line_color(attr: &ContourAttribute, level_t: f32) -> Rgba<u8> {
    match attr {
        ContourAttribute::Line => Rgba([255, 255, 255, 255]),
        ContourAttribute::Surface => {
            let [r, g, b] = colormap::turbo(level_t);
            Rgba([r, g, b, 255])
        }
        ContourAttribute::Grid => Rgba([200, 255, 200, 200]),
        ContourAttribute::ColorContours => {
            let [r, g, b] = colormap::turbo(level_t);
            Rgba([r, g, b, 255])
        }
        ContourAttribute::Dots => Rgba([255, 255, 255, 200]),
    }
}

// ─── Drawing primitives ──────────────────────────────────────────────────────

/// Bresenham line.
fn draw_line(img: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgba<u8>) {
    let w = img.width() as i32;
    let h = img.height() as i32;

    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;
    let mut x = x0;
    let mut y = y0;

    loop {
        if x >= 0 && x < w && y >= 0 && y < h {
            img.put_pixel(x as u32, y as u32, color);
        }
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }
}

/// 3×3 dot.
fn paint_dot(img: &mut RgbaImage, x: i32, y: i32, color: Rgba<u8>) {
    let w = img.width() as i32;
    let h = img.height() as i32;
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            let px = x + dx;
            let py = y + dy;
            if px >= 0 && px < w && py >= 0 && py < h {
                img.put_pixel(px as u32, py as u32, color);
            }
        }
    }
}

pub fn draw_frame_border(img: &mut RgbaImage) {
    let w = img.width();
    let h = img.height();
    if w < 2 || h < 2 {
        return;
    }
    let c = Rgba([216u8, 226, 255, 255]);
    for x in 0..w {
        img.put_pixel(x, 0, c);
        img.put_pixel(x, h - 1, c);
    }
    for y in 0..h {
        img.put_pixel(0, y, c);
        img.put_pixel(w - 1, y, c);
    }
}

// ─── Geometry helpers ─────────────────────────────────────────────────────────

fn bbox(uvs: &[(f32, f32)]) -> (f32, f32, f32, f32) {
    let mut min_u = f32::INFINITY;
    let mut max_u = f32::NEG_INFINITY;
    let mut min_v = f32::INFINITY;
    let mut max_v = f32::NEG_INFINITY;
    for &(u, v) in uvs {
        min_u = min_u.min(u);
        max_u = max_u.max(u);
        min_v = min_v.min(v);
        max_v = max_v.max(v);
    }
    (min_u, max_u, min_v, max_v)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plot_state::{ContourEntry, ContourSpec, PlotFamily, PlotUpAxis};
    use crate::script_executor::SolutionSnapshot;

    /// Build a synthetic 4×4×1 snapshot with a simple gradient scalar field.
    fn synthetic_snapshot() -> SolutionSnapshot {
        synthetic_snapshot_with_dims(4, 4, 1)
    }

    fn synthetic_snapshot_with_dims(ni: u32, nj: u32, nk: u32) -> SolutionSnapshot {
        let n = (ni * nj * nk) as usize;
        let mut x = Vec::with_capacity(n);
        let mut y = Vec::with_capacity(n);
        let mut z = Vec::with_capacity(n);
        let mut scalar = Vec::with_capacity(n);
        let mut mn = f32::INFINITY;
        let mut mx = f32::NEG_INFINITY;

        for k in 0..nk as usize {
            for j in 0..nj as usize {
                for i in 0..ni as usize {
                    x.push(i as f32);
                    y.push(j as f32);
                    z.push(k as f32);
                    let s = (i + j * ni as usize + k * (ni as usize + nj as usize + 1)) as f32;
                    scalar.push(s);
                    mn = mn.min(s);
                    mx = mx.max(s);
                }
            }
        }

        SolutionSnapshot {
            ni,
            nj,
            nk,
            x,
            y,
            z,
            scalar,
            field_min: mn,
            field_max: mx,
        }
    }

    #[test]
    fn snapshot_render_produces_output() {
        let snap = synthetic_snapshot();
        let state = PlotState::default();
        let mut img = RgbaImage::new(120, 80);
        let mut warnings = Vec::new();
        render_snapshot(&mut img, &snap, &state, &mut warnings);

        // Should have non-black pixels inside the margin area
        let inner_pixels: Vec<_> = (20..100u32)
            .flat_map(|x| (20..60u32).map(move |y| (x, y)))
            .map(|(x, y)| img.get_pixel(x, y))
            .collect();
        let any_colored = inner_pixels
            .iter()
            .any(|p| p[0] > 0 || p[1] > 0 || p[2] > 0);
        assert!(any_colored, "render produced no colored pixels");
    }

    #[test]
    fn snapshot_render_different_field_produces_different_image() {
        let snap = synthetic_snapshot();

        let state_default = PlotState::default();
        let mut state_a = state_default.clone();
        state_a.contour_spec = ContourSpec::Automatic { count: 3 };
        let mut state_b = state_default.clone();
        state_b.contour_spec = ContourSpec::Manual {
            entries: vec![
                ContourEntry {
                    value: 5.0,
                    color: None,
                },
                ContourEntry {
                    value: 10.0,
                    color: None,
                },
            ],
        };

        let mut img_a = RgbaImage::new(120, 80);
        let mut img_b = RgbaImage::new(120, 80);
        let mut w = Vec::new();
        render_snapshot(&mut img_a, &snap, &state_a, &mut w);
        render_snapshot(&mut img_b, &snap, &state_b, &mut w);
        assert_ne!(img_a.as_raw(), img_b.as_raw());
    }

    #[test]
    fn resolve_contour_levels_automatic() {
        let levels = resolve_contour_levels(&ContourSpec::Automatic { count: 4 }, 0.0, 1.0);
        assert_eq!(levels.len(), 4);
        // Levels should be within (0, 1) exclusive
        for &l in &levels {
            assert!(l > 0.0 && l < 1.0, "level out of range: {l}");
        }
    }

    #[test]
    fn resolve_contour_levels_manual() {
        let levels = resolve_contour_levels(
            &ContourSpec::Manual {
                entries: vec![
                    ContourEntry {
                        value: 0.25,
                        color: None,
                    },
                    ContourEntry {
                        value: 0.75,
                        color: None,
                    },
                ],
            },
            0.0,
            1.0,
        );
        assert_eq!(levels.len(), 2);
        assert!((levels[0] - 0.25).abs() < 1e-5);
        assert!((levels[1] - 0.75).abs() < 1e-5);
    }

    #[test]
    fn resolve_contour_levels_none_returns_empty() {
        let levels = resolve_contour_levels(&ContourSpec::None, 0.0, 1.0);
        assert!(levels.is_empty());
    }

    #[test]
    fn marching_squares_detects_crossing() {
        // 2×2 slab: bottom-left=0, bottom-right=0, top-left=2, top-right=2
        // Level=1.0 should cross the left and right edges (horizontal band).
        let uvs = vec![(0.0f32, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)];
        let scalars = vec![0.0f32, 0.0, 2.0, 2.0];
        let mut img = RgbaImage::new(100, 100);
        // Fill background black; contour lines should paint non-black pixels.
        for p in img.pixels_mut() {
            *p = Rgba([0, 0, 0, 255]);
        }
        draw_iso_features(
            &mut img,
            &uvs,
            &scalars,
            2,
            2,
            &[1.0],
            &ContourAttribute::Line,
            0.0,
            2.0,
            0,
        );
        // The rendered line must have at least one non-black pixel painted.
        let line_pixels: u32 = img
            .pixels()
            .map(|p| {
                if p[0] == 0 && p[1] == 0 && p[2] == 0 {
                    0
                } else {
                    1
                }
            })
            .sum();
        assert!(line_pixels > 0, "no contour line pixels painted");
    }

    #[test]
    fn face_slab_plus_z_selects_correct_face() {
        // 2×2×2 grid: nk-1 = 1 face should have z=1 values
        let snap = SolutionSnapshot {
            ni: 2,
            nj: 2,
            nk: 2,
            x: vec![0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0],
            y: vec![0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0],
            z: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            scalar: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0],
            field_min: 0.0,
            field_max: 7.0,
        };
        let state = PlotState {
            axis_view: AxisView::PlusZ,
            ..PlotState::default()
        };
        let mut w = Vec::new();
        let (_uvs, scalars, sw, sh) = extract_face_slab(&snap, &state, &mut w);
        assert_eq!(sw, 2);
        assert_eq!(sh, 2);
        // K=1 face has scalar values 4,5,6,7
        assert_eq!(scalars, vec![4.0, 5.0, 6.0, 7.0]);
    }

    #[test]
    fn function_surface_render_produces_non_black_pixels() {
        let snap = synthetic_snapshot();
        let state = PlotState {
            plot_family: PlotFamily::FunctionSurface,
            ..PlotState::default()
        };
        let mut img = RgbaImage::new(120, 80);
        let mut warnings = Vec::new();
        render_snapshot(&mut img, &snap, &state, &mut warnings);

        let any_colored = img.pixels().any(|p| p[0] > 0 || p[1] > 0 || p[2] > 0);
        assert!(
            any_colored,
            "function-surface render produced no visible pixels"
        );
    }

    #[test]
    fn function_surface_differs_from_contour_render() {
        let snap = synthetic_snapshot();
        let contour_state = PlotState::default();
        let surface_state = PlotState {
            plot_family: PlotFamily::FunctionSurface,
            ..PlotState::default()
        };

        let mut contour_img = RgbaImage::new(140, 100);
        let mut surface_img = RgbaImage::new(140, 100);
        let mut warnings = Vec::new();
        render_snapshot(&mut contour_img, &snap, &contour_state, &mut warnings);
        render_snapshot(&mut surface_img, &snap, &surface_state, &mut warnings);

        assert_ne!(contour_img.as_raw(), surface_img.as_raw());
    }

    #[test]
    fn function_surface_top_view_emits_oblique_warning() {
        let snap = synthetic_snapshot();
        let state = PlotState {
            plot_family: PlotFamily::FunctionSurface,
            axis_view: AxisView::PlaneXY,
            ..PlotState::default()
        };
        let mut img = RgbaImage::new(120, 80);
        let mut warnings = Vec::new();
        render_snapshot(&mut img, &snap, &state, &mut warnings);

        assert!(warnings
            .iter()
            .any(|w| w.contains("oblique fallback camera")));
    }

    #[test]
    fn function_surface_i_plane_supports_thin_yz_slab() {
        let snap = synthetic_snapshot_with_dims(1, 4, 4);
        let state = PlotState {
            plot_family: PlotFamily::FunctionSurface,
            axis_view: AxisView::PlaneYZ,
            ..PlotState::default()
        };
        let mut img = RgbaImage::new(120, 80);
        let mut warnings = Vec::new();
        render_snapshot(&mut img, &snap, &state, &mut warnings);

        let any_colored = img.pixels().any(|p| p[0] > 0 || p[1] > 0 || p[2] > 0);
        assert!(
            any_colored,
            "thin i-plane function-surface render produced no pixels"
        );
        assert!(warnings
            .iter()
            .any(|w| w.contains("oblique fallback camera")));
    }

    #[test]
    fn function_surface_j_plane_supports_thin_xz_slab() {
        let snap = synthetic_snapshot_with_dims(4, 1, 4);
        let state = PlotState {
            plot_family: PlotFamily::FunctionSurface,
            axis_view: AxisView::PlaneXZ,
            ..PlotState::default()
        };
        let mut img = RgbaImage::new(120, 80);
        let mut warnings = Vec::new();
        render_snapshot(&mut img, &snap, &state, &mut warnings);

        let any_colored = img.pixels().any(|p| p[0] > 0 || p[1] > 0 || p[2] > 0);
        assert!(
            any_colored,
            "thin j-plane function-surface render produced no pixels"
        );
        assert!(warnings
            .iter()
            .any(|w| w.contains("oblique fallback camera")));
    }

    #[test]
    fn face_slab_plane_yx_swaps_axes() {
        let snap = SolutionSnapshot {
            ni: 2,
            nj: 2,
            nk: 1,
            x: vec![10.0, 20.0, 30.0, 40.0],
            y: vec![1.0, 2.0, 3.0, 4.0],
            z: vec![0.0, 0.0, 0.0, 0.0],
            scalar: vec![0.0, 1.0, 2.0, 3.0],
            field_min: 0.0,
            field_max: 3.0,
        };
        let state = PlotState {
            axis_view: AxisView::PlaneYX,
            ..PlotState::default()
        };
        let mut w = Vec::new();
        let (uvs, _scalars, sw, sh) = extract_face_slab(&snap, &state, &mut w);

        assert_eq!((sw, sh), (2, 2));
        assert_eq!(uvs[0], (1.0, 10.0));
        assert_eq!(uvs[1], (2.0, 20.0));
        assert_eq!(uvs[2], (3.0, 30.0));
        assert_eq!(uvs[3], (4.0, 40.0));
    }

    #[test]
    fn face_slab_plane_zx_swaps_axes() {
        let snap = SolutionSnapshot {
            ni: 2,
            nj: 2,
            nk: 2,
            x: vec![0.0, 1.0, 0.0, 1.0, 10.0, 20.0, 30.0, 40.0],
            y: vec![0.0, 0.0, 1.0, 1.0, 5.0, 5.0, 5.0, 5.0],
            z: vec![7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0],
            scalar: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0],
            field_min: 0.0,
            field_max: 7.0,
        };
        let state = PlotState {
            axis_view: AxisView::PlaneZX,
            ..PlotState::default()
        };
        let mut w = Vec::new();
        let (uvs, _scalars, sw, sh) = extract_face_slab(&snap, &state, &mut w);

        assert_eq!((sw, sh), (2, 2));
        assert_eq!(uvs[0], (9.0, 0.0));
        assert_eq!(uvs[1], (10.0, 1.0));
        assert_eq!(uvs[2], (13.0, 30.0));
        assert_eq!(uvs[3], (14.0, 40.0));
    }

    #[test]
    fn face_slab_plane_zy_swaps_axes() {
        let snap = SolutionSnapshot {
            ni: 2,
            nj: 2,
            nk: 2,
            x: vec![0.0, 99.0, 0.0, 99.0, 10.0, 20.0, 30.0, 40.0],
            y: vec![1.0, 2.0, 3.0, 4.0, 11.0, 12.0, 13.0, 14.0],
            z: vec![5.0, 6.0, 7.0, 8.0, 15.0, 16.0, 17.0, 18.0],
            scalar: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0],
            field_min: 0.0,
            field_max: 7.0,
        };
        let state = PlotState {
            axis_view: AxisView::PlaneZY,
            ..PlotState::default()
        };
        let mut w = Vec::new();
        let (uvs, _scalars, sw, sh) = extract_face_slab(&snap, &state, &mut w);

        assert_eq!((sw, sh), (2, 2));
        assert_eq!(uvs[0], (6.0, 2.0));
        assert_eq!(uvs[1], (8.0, 4.0));
        assert_eq!(uvs[2], (16.0, 12.0));
        assert_eq!(uvs[3], (18.0, 14.0));
    }

    #[test]
    fn face_slab_plot_up_negative_y_flips_vertical_axis() {
        let snap = SolutionSnapshot {
            ni: 2,
            nj: 2,
            nk: 1,
            x: vec![10.0, 20.0, 30.0, 40.0],
            y: vec![1.0, 2.0, 3.0, 4.0],
            z: vec![0.0, 0.0, 0.0, 0.0],
            scalar: vec![0.0, 1.0, 2.0, 3.0],
            field_min: 0.0,
            field_max: 3.0,
        };
        let state = PlotState {
            axis_view: AxisView::PlaneXY,
            plot_up: Some(PlotUpAxis::NegativeY),
            ..PlotState::default()
        };
        let mut w = Vec::new();
        let (uvs, _scalars, sw, sh) = extract_face_slab(&snap, &state, &mut w);

        assert_eq!((sw, sh), (2, 2));
        assert_eq!(uvs[0], (10.0, -1.0));
        assert_eq!(uvs[3], (40.0, -4.0));
    }

    #[test]
    fn face_slab_plot_up_positive_x_swaps_xy_without_plane_token() {
        let snap = SolutionSnapshot {
            ni: 2,
            nj: 2,
            nk: 1,
            x: vec![10.0, 20.0, 30.0, 40.0],
            y: vec![1.0, 2.0, 3.0, 4.0],
            z: vec![0.0, 0.0, 0.0, 0.0],
            scalar: vec![0.0, 1.0, 2.0, 3.0],
            field_min: 0.0,
            field_max: 3.0,
        };
        let state = PlotState {
            axis_view: AxisView::PlaneXY,
            plot_up: Some(PlotUpAxis::PositiveX),
            ..PlotState::default()
        };
        let mut w = Vec::new();
        let (uvs, _scalars, sw, sh) = extract_face_slab(&snap, &state, &mut w);

        assert_eq!((sw, sh), (2, 2));
        assert_eq!(uvs[0], (1.0, 10.0));
        assert_eq!(uvs[3], (4.0, 40.0));
    }

    #[test]
    fn camera_basis_honors_explicit_plot_up_axis() {
        let state = PlotState {
            axis_view: AxisView::Custom,
            viewpoint: Some(ViewPoint {
                x: 3.0,
                y: 2.0,
                z: 1.0,
            }),
            plot_up: Some(PlotUpAxis::PositiveX),
            ..PlotState::default()
        };
        let mut warnings = Vec::new();
        let camera = camera_basis_for_state(&state, &mut warnings);

        assert!(camera.1 .0 > camera.1 .1);
        assert!(camera.1 .0 > camera.1 .2);
        assert!(warnings.is_empty());
    }

    #[test]
    fn custom_vpoint_projection_uses_matching_outer_face_for_plus_x() {
        let snap = SolutionSnapshot {
            ni: 2,
            nj: 2,
            nk: 2,
            x: vec![0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0],
            y: vec![0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0],
            z: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            scalar: vec![10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0],
            field_min: 10.0,
            field_max: 17.0,
        };
        let state = PlotState {
            axis_view: AxisView::Custom,
            viewpoint: Some(ViewPoint {
                x: 3.0,
                y: 0.0,
                z: 0.0,
            }),
            ..PlotState::default()
        };
        let mut warnings = Vec::new();
        let (_uvs, scalars, sw, sh) = extract_face_slab(&snap, &state, &mut warnings);

        assert_eq!((sw, sh), (2, 2));
        // +X viewpoint should match the i=ni-1 face (same as AxisView::PlusX).
        assert_eq!(scalars, vec![11.0, 13.0, 15.0, 17.0]);
        assert!(warnings.is_empty());
    }

    #[test]
    fn custom_vpoint_projection_warns_for_oblique_viewpoint() {
        let snap = SolutionSnapshot {
            ni: 2,
            nj: 2,
            nk: 2,
            x: vec![0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0],
            y: vec![0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0],
            z: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            scalar: vec![10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0],
            field_min: 10.0,
            field_max: 17.0,
        };
        let state = PlotState {
            axis_view: AxisView::Custom,
            viewpoint: Some(ViewPoint {
                x: 1.7,
                y: 1.3,
                z: 1.1,
            }),
            ..PlotState::default()
        };
        let mut warnings = Vec::new();
        let (_uvs, _scalars, _sw, _sh) = extract_face_slab(&snap, &state, &mut warnings);

        assert!(warnings
            .iter()
            .any(|w| w.contains("oblique") && w.contains("dominant-axis")));
    }

    #[test]
    fn custom_vpoint_projection_falls_back_from_degenerate_face() {
        // nk=1 means I/J outer faces are degenerate (width x 1). A +X-like
        // VPOINT should fall back to K face for contour slab extraction.
        let snap = SolutionSnapshot {
            ni: 2,
            nj: 2,
            nk: 1,
            x: vec![0.0, 1.0, 0.0, 1.0],
            y: vec![0.0, 0.0, 1.0, 1.0],
            z: vec![0.0, 0.0, 0.0, 0.0],
            scalar: vec![20.0, 21.0, 22.0, 23.0],
            field_min: 20.0,
            field_max: 23.0,
        };
        let state = PlotState {
            axis_view: AxisView::Custom,
            viewpoint: Some(ViewPoint {
                x: 3.0,
                y: 0.0,
                z: 0.0,
            }),
            ..PlotState::default()
        };
        let mut warnings = Vec::new();
        let (_uvs, scalars, sw, sh) = extract_face_slab(&snap, &state, &mut warnings);

        assert_eq!((sw, sh), (2, 2));
        assert_eq!(scalars, vec![20.0, 21.0, 22.0, 23.0]);
        assert!(warnings
            .iter()
            .any(|w| w.contains("dominant face is degenerate")));
    }

    #[test]
    fn perspective_projection_has_foreshortening() {
        let camera = camera_basis_from_viewpoint_default(&ViewPoint {
            x: 3.0,
            y: 0.0,
            z: 0.0,
        });
        let origin = (3.0f32, 0.0, 0.0);

        // Same lateral offset with different depths: near point should project larger.
        let near = project_point_perspective((2.0, 0.0, 1.0), camera, origin, 55.0);
        let far = project_point_perspective((0.0, 0.0, 1.0), camera, origin, 55.0);

        assert!(near.0.abs() > far.0.abs());
        assert!(near.2 < far.2);
    }

    #[test]
    fn function_surface_custom_vpoint_enables_perspective_warning() {
        let snap = synthetic_snapshot();
        let state = PlotState {
            plot_family: PlotFamily::FunctionSurface,
            axis_view: AxisView::Custom,
            viewpoint: Some(ViewPoint {
                x: 2.4,
                y: 1.6,
                z: 1.1,
            }),
            ..PlotState::default()
        };

        let mut img = RgbaImage::new(120, 80);
        let mut warnings = Vec::new();
        render_snapshot(&mut img, &snap, &state, &mut warnings);

        assert!(warnings
            .iter()
            .any(|w| w.contains("bounded perspective projection")));
    }
}
