use super::*;
use crate::context::create_dead_context;
use rio_backend::event::{TerminalDamage, VoidListener, WindowId};

// This file tests compute function on different layouts.
// I've added some real scenarios so I can make sure it doesn't go off again.

/// note: Computes the renderer's actual per-line height in physical pixels.
///
/// The renderer gets metrics from Metrics::for_rich_text() which packs
/// cell_height as (ascent, descent, 0.0). cell_height is computed by
/// Metrics::calc at physical font_size scale, with ceil applied.
///
/// basically renderer line_height = ceil((ascent + descent + leading) * scale) * line_height_mod
fn renderer_line_height(
    ascent: f32,
    descent: f32,
    leading: f32,
    line_height_mod: f32,
    scale: f32,
) -> f32 {
    // Matches the Metrics::calc path: scale to physical, then ceil
    let cell_height = ((ascent + descent + leading) * scale).ceil();
    cell_height * line_height_mod
}

fn sugar_height(
    ascent: f32,
    descent: f32,
    leading: f32,
    line_height_mod: f32,
    scale: f32,
) -> f32 {
    ((ascent + descent + leading) * line_height_mod * scale).ceil()
}

/// Verifies that compute() row count fits when rendered.
#[allow(clippy::too_many_arguments)]
fn assert_rows_fit(
    panel_width: f32,
    panel_height: f32,
    sugar_width: f32,
    scale: f32,
    line_height_mod: f32,
    ascent: f32,
    descent: f32,
    leading: f32,
) {
    let sh = sugar_height(ascent, descent, leading, line_height_mod, scale);
    let dimensions = TextDimensions {
        width: sugar_width,
        height: sh,
        scale,
    };

    let (cols, rows) = compute(
        panel_width,
        panel_height,
        dimensions,
        line_height_mod,
        Margin::all(0.0),
    );

    let actual_line_height =
        renderer_line_height(ascent, descent, leading, line_height_mod, scale);
    let rendered_height = rows as f32 * actual_line_height;

    assert!(
        rendered_height <= panel_height,
        "Rows overflow! {} rows * {:.2}px = {:.2}px rendered, but panel is only {:.2}px tall \
         (cols={}, sugar={:.2}x{:.2}, scale={:.1}, lh_mod={:.1})",
        rows,
        actual_line_height,
        rendered_height,
        panel_height,
        cols,
        sugar_width,
        sh,
        scale,
        line_height_mod,
    );
}

#[test]
fn test_user_case_1834x1436() {
    assert_rows_fit(1834.0, 1436.0, 16.41, 2.0, 1.0, 13.0, 3.5, 0.0);
}

#[test]
fn test_user_case_3766x1996() {
    assert_rows_fit(3766.0, 1996.0, 16.41, 2.0, 1.0, 13.0, 3.5, 0.0);
}

#[test]
fn test_user_case_5104x2736() {
    assert_rows_fit(5104.0, 2736.0, 16.41, 2.0, 1.0, 13.0, 3.5, 0.0);
}

#[test]
fn test_rows_fit_various_sizes() {
    for height in (500..=3000).step_by(50) {
        for width in [800.0, 1600.0, 2400.0, 3200.0] {
            assert_rows_fit(width, height as f32, 16.41, 2.0, 1.0, 13.0, 3.5, 0.0);
        }
    }
}

#[test]
fn test_rows_fit_with_nonzero_leading() {
    let test_cases: Vec<(f32, f32, f32)> = vec![
        (12.0, 3.0, 0.5),
        (12.0, 3.0, 1.0),
        (14.0, 4.0, 0.25),
        (10.0, 3.0, 2.0),
    ];

    for (ascent, descent, leading) in test_cases {
        for height in (500..=2000).step_by(100) {
            assert_rows_fit(
                1600.0,
                height as f32,
                16.0,
                2.0,
                1.0,
                ascent,
                descent,
                leading,
            );
        }
    }
}

#[test]
fn test_rows_fit_with_line_height_modifier() {
    for lh_mod in [1.1, 1.2, 1.5, 2.0] {
        for height in (500..=2000).step_by(100) {
            assert_rows_fit(1600.0, height as f32, 16.0, 2.0, lh_mod, 12.0, 3.0, 0.5);
        }
    }
}

#[test]
fn test_rows_fit_scale_1() {
    for height in (300..=1200).step_by(50) {
        assert_rows_fit(800.0, height as f32, 8.0, 1.0, 1.0, 13.0, 3.5, 0.0);
    }
}

#[test]
fn test_rows_fit_zero_leading() {
    for height in (500..=2000).step_by(100) {
        assert_rows_fit(1600.0, height as f32, 16.0, 2.0, 1.0, 12.77, 3.50, 0.0);
    }
}

#[test]
fn test_rows_fit_fractional_metrics() {
    // Fractional ascent+descent that would produce different results
    // with and without ceil
    assert_rows_fit(1600.0, 1000.0, 16.0, 2.0, 1.0, 12.3, 3.4, 0.1);
    assert_rows_fit(1600.0, 1000.0, 16.0, 2.0, 1.0, 11.9, 4.6, 0.3);
    assert_rows_fit(1600.0, 1000.0, 16.0, 1.5, 1.0, 12.0, 3.0, 0.5);
}

#[test]
fn test_compute_returns_min_for_zero_dimensions() {
    let dims = TextDimensions {
        width: 16.0,
        height: 32.0,
        scale: 2.0,
    };
    let (cols, rows) = compute(0.0, 0.0, dims, 1.0, Margin::all(0.0));
    assert_eq!(cols, MIN_COLS);
    assert_eq!(rows, MIN_LINES);
}

#[test]
fn test_compute_returns_min_for_negative_dimensions() {
    let dims = TextDimensions {
        width: 16.0,
        height: 32.0,
        scale: 2.0,
    };
    let (cols, rows) = compute(-100.0, -100.0, dims, 1.0, Margin::all(0.0));
    assert_eq!(cols, MIN_COLS);
    assert_eq!(rows, MIN_LINES);
}

#[test]
fn test_compute_returns_min_for_zero_scale() {
    let dims = TextDimensions {
        width: 16.0,
        height: 32.0,
        scale: 0.0,
    };
    let (cols, rows) = compute(1600.0, 900.0, dims, 1.0, Margin::all(0.0));
    assert_eq!(cols, MIN_COLS);
    assert_eq!(rows, MIN_LINES);
}

#[test]
fn test_compute_basic_grid() {
    let dims = TextDimensions {
        width: 16.0,
        height: 33.0,
        scale: 2.0,
    };
    let (cols, rows) = compute(1600.0, 825.0, dims, 1.0, Margin::all(0.0));
    assert_eq!(cols, 100);
    assert_eq!(rows, 25);
}

#[test]
fn test_compute_floors_fractional_rows() {
    // 840px / 33px = 25.45 → floor → 25
    let dims = TextDimensions {
        width: 16.0,
        height: 33.0,
        scale: 1.0,
    };
    let (_, rows) = compute(1600.0, 840.0, dims, 1.0, Margin::all(0.0));
    assert_eq!(rows, 25);
}

#[test]
fn test_compute_respects_margins() {
    let dims = TextDimensions {
        width: 16.0,
        height: 32.0,
        scale: 2.0,
    };
    let margin = Margin::new(0.0, 10.0, 0.0, 10.0);
    let (cols, _) = compute(1600.0, 800.0, dims, 1.0, margin);
    // available = 1600 - 10*2 - 10*2 = 1560, cols = 1560/16 = 97
    assert_eq!(cols, 97);
}

#[test]
fn test_compute_margin_exceeds_size() {
    let dims = TextDimensions {
        width: 16.0,
        height: 32.0,
        scale: 2.0,
    };
    let margin = Margin::new(0.0, 0.0, 0.0, 1000.0);
    let (cols, rows) = compute(100.0, 800.0, dims, 1.0, margin);
    assert_eq!(cols, MIN_COLS);
    assert_eq!(rows, MIN_LINES);
}

#[test]
fn test_context_dimension_build() {
    let dims = TextDimensions {
        width: 16.0,
        height: 33.0,
        scale: 2.0,
    };
    let cd = ContextDimension::build(1650.0, 825.0, dims, 1.0, Margin::all(0.0));
    assert_eq!(cd.columns, 103);
    assert_eq!(cd.lines, 25);
}

#[test]
fn test_context_dimension_update_width() {
    let dims = TextDimensions {
        width: 16.0,
        height: 33.0,
        scale: 2.0,
    };
    let mut cd = ContextDimension::build(1600.0, 825.0, dims, 1.0, Margin::all(0.0));
    assert_eq!(cd.columns, 100);

    cd.update_width(800.0);
    assert_eq!(cd.columns, 50);
    assert_eq!(cd.lines, 25);
}

#[test]
fn test_context_dimension_update_height() {
    let dims = TextDimensions {
        width: 16.0,
        height: 33.0,
        scale: 2.0,
    };
    let mut cd = ContextDimension::build(1600.0, 825.0, dims, 1.0, Margin::all(0.0));
    assert_eq!(cd.lines, 25);

    cd.update_height(660.0);
    assert_eq!(cd.lines, 20);
    assert_eq!(cd.columns, 100);
}

#[test]
fn test_context_dimension_update_dimensions() {
    let dims = TextDimensions {
        width: 16.0,
        height: 33.0,
        scale: 1.0,
    };
    let mut cd = ContextDimension::build(1600.0, 825.0, dims, 1.0, Margin::all(0.0));
    assert_eq!(cd.lines, 25);

    let new_dims = TextDimensions {
        width: 16.0,
        height: 66.0,
        scale: 1.0,
    };
    cd.update_dimensions(new_dims);
    assert_eq!(cd.lines, 12); // 825/66 = 12.5 → 12
}

/// Reproduces the bug: after resizing a panel to 80%/20% and then
/// resizing the window, the panel proportions should be preserved
/// but they are not because set_panel_size uses flex_shrink: 0.0.
#[test]
fn test_panel_resize_preserves_proportions_on_window_resize() {
    use taffy::{FlexDirection, TaffyTree};

    let mut tree: TaffyTree<()> = TaffyTree::new();

    let initial_width = 1000.0;

    // Root container (simulates the grid root after margin subtraction)
    let root = tree
        .new_leaf(Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            size: geometry::Size {
                width: length(initial_width),
                height: length(800.0),
            },
            ..Default::default()
        })
        .unwrap();

    // Two panels, initially equal (flex_grow: 1.0)
    let left = tree
        .new_leaf(Style {
            display: Display::Flex,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        })
        .unwrap();
    let right = tree
        .new_leaf(Style {
            display: Display::Flex,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        })
        .unwrap();

    tree.add_child(root, left).unwrap();
    tree.add_child(root, right).unwrap();

    // Compute initial layout — should be 500/500
    tree.compute_layout(
        root,
        geometry::Size {
            width: AvailableSpace::MaxContent,
            height: AvailableSpace::MaxContent,
        },
    )
    .unwrap();
    let left_w = tree.layout(left).unwrap().size.width;
    let right_w = tree.layout(right).unwrap().size.width;
    assert!(
        (left_w - 500.0).abs() < 1.0,
        "left should be ~500, got {left_w}"
    );
    assert!(
        (right_w - 500.0).abs() < 1.0,
        "right should be ~500, got {right_w}"
    );

    // Simulate move_divider: set left to 80%, right to 20%
    // Uses flex_grow proportional to the size so panels scale on resize
    let mut left_style = tree.style(left).unwrap().clone();
    left_style.flex_basis = length(0.0);
    left_style.flex_grow = 800.0;
    left_style.flex_shrink = 1.0;
    tree.set_style(left, left_style).unwrap();

    let mut right_style = tree.style(right).unwrap().clone();
    right_style.flex_basis = length(0.0);
    right_style.flex_grow = 200.0;
    right_style.flex_shrink = 1.0;
    tree.set_style(right, right_style).unwrap();

    // Verify 80/20 split
    tree.compute_layout(
        root,
        geometry::Size {
            width: AvailableSpace::MaxContent,
            height: AvailableSpace::MaxContent,
        },
    )
    .unwrap();
    let left_w = tree.layout(left).unwrap().size.width;
    let right_w = tree.layout(right).unwrap().size.width;
    assert!(
        (left_w - 800.0).abs() < 1.0,
        "left should be 800, got {left_w}"
    );
    assert!(
        (right_w - 200.0).abs() < 1.0,
        "right should be 200, got {right_w}"
    );

    // Now resize the window to 1200px (simulates try_update_size)
    let new_width = 1200.0;
    let mut root_style = tree.style(root).unwrap().clone();
    root_style.size.width = length(new_width);
    tree.set_style(root, root_style).unwrap();

    tree.compute_layout(
        root,
        geometry::Size {
            width: AvailableSpace::MaxContent,
            height: AvailableSpace::MaxContent,
        },
    )
    .unwrap();

    let left_w = tree.layout(left).unwrap().size.width;
    let right_w = tree.layout(right).unwrap().size.width;

    // The 80/20 proportion should be preserved: 960/240
    let expected_left = new_width * 0.8;
    let expected_right = new_width * 0.2;

    assert!(
        (left_w - expected_left).abs() < 1.0,
        "After resize, left should be ~{expected_left} (80%), got {left_w}"
    );
    assert!(
        (right_w - expected_right).abs() < 1.0,
        "After resize, right should be ~{expected_right} (20%), got {right_w}"
    );
}

/// Reproduces bug: two panels with 20/80 split, then splitting the 80%
/// panel horizontally should keep the 20/80 proportion in the parent.
#[test]
fn test_split_inside_resized_panel_preserves_proportions() {
    use taffy::{FlexDirection, TaffyTree};

    let mut tree: TaffyTree<()> = TaffyTree::new();

    // Root container
    let root = tree
        .new_leaf(Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            size: geometry::Size {
                width: length(1000.0),
                height: length(800.0),
            },
            ..Default::default()
        })
        .unwrap();

    // Two panels
    let left = tree
        .new_leaf(Style {
            display: Display::Flex,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        })
        .unwrap();
    let right = tree
        .new_leaf(Style {
            display: Display::Flex,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        })
        .unwrap();

    tree.add_child(root, left).unwrap();
    tree.add_child(root, right).unwrap();

    // Resize: left=20%, right=80% (using flex_grow proportional)
    let mut left_style = tree.style(left).unwrap().clone();
    left_style.flex_basis = length(0.0);
    left_style.flex_grow = 200.0;
    left_style.flex_shrink = 1.0;
    tree.set_style(left, left_style).unwrap();

    let mut right_style = tree.style(right).unwrap().clone();
    right_style.flex_basis = length(0.0);
    right_style.flex_grow = 800.0;
    right_style.flex_shrink = 1.0;
    tree.set_style(right, right_style).unwrap();

    // Verify 20/80 split
    let available = geometry::Size {
        width: AvailableSpace::MaxContent,
        height: AvailableSpace::MaxContent,
    };
    tree.compute_layout(root, available).unwrap();
    let left_w = tree.layout(left).unwrap().size.width;
    let right_w = tree.layout(right).unwrap().size.width;
    assert!(
        (left_w - 200.0).abs() < 1.0,
        "left should be 200, got {left_w}"
    );
    assert!(
        (right_w - 800.0).abs() < 1.0,
        "right should be 800, got {right_w}"
    );

    // Now split the right panel horizontally (Column direction).
    // This simulates what split_panel does:
    // 1. Create container inheriting right's flex properties
    // 2. Reset right to flex_grow: 1.0
    // 3. Create new panel with flex_grow: 1.0
    // 4. Move right into container, add new panel

    let right_inherited = tree.style(right).unwrap().clone();
    let container = tree
        .new_leaf(Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            flex_basis: right_inherited.flex_basis,
            flex_grow: right_inherited.flex_grow,
            flex_shrink: right_inherited.flex_shrink,
            ..Default::default()
        })
        .unwrap();

    // Reset right panel to flexible inside container
    let mut reset_right = right_inherited;
    reset_right.flex_basis = taffy::Dimension::auto();
    reset_right.flex_grow = 1.0;
    reset_right.flex_shrink = 1.0;
    tree.set_style(right, reset_right).unwrap();

    let bottom = tree
        .new_leaf(Style {
            display: Display::Flex,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        })
        .unwrap();

    tree.remove_child(root, right).unwrap();
    tree.add_child(container, right).unwrap();
    tree.add_child(container, bottom).unwrap();
    tree.add_child(root, container).unwrap();

    tree.compute_layout(root, available).unwrap();

    // The container (replacing right) should still be ~800px wide (80%)
    let container_w = tree.layout(container).unwrap().size.width;
    assert!(
        (container_w - 800.0).abs() < 1.0,
        "Container should keep 80% (800px), got {container_w}"
    );

    // Left should still be ~200px (20%)
    let left_w = tree.layout(left).unwrap().size.width;
    assert!(
        (left_w - 200.0).abs() < 1.0,
        "Left should keep 20% (200px), got {left_w}"
    );

    // The two children inside the container should each be ~400px tall (50/50)
    let right_h = tree.layout(right).unwrap().size.height;
    let bottom_h = tree.layout(bottom).unwrap().size.height;
    assert!(
        (right_h - 400.0).abs() < 1.0,
        "Right (top half) should be ~400px tall, got {right_h}"
    );
    assert!(
        (bottom_h - 400.0).abs() < 1.0,
        "Bottom (bottom half) should be ~400px tall, got {bottom_h}"
    );
}

#[test]
fn test_apply_zoomed_panel_styles_hides_non_selected_branch_and_restores() {
    use taffy::{FlexDirection, TaffyTree};

    let mut tree: TaffyTree<()> = TaffyTree::new();

    let root = tree
        .new_leaf(Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            size: geometry::Size {
                width: length(1200.0),
                height: length(800.0),
            },
            ..Default::default()
        })
        .unwrap();

    let left = tree
        .new_leaf(Style {
            display: Display::Flex,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        })
        .unwrap();

    let right_container = tree
        .new_leaf(Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        })
        .unwrap();

    let right_top = tree
        .new_leaf(Style {
            display: Display::Flex,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        })
        .unwrap();

    let right_bottom = tree
        .new_leaf(Style {
            display: Display::Flex,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        })
        .unwrap();

    tree.add_child(root, left).unwrap();
    tree.add_child(root, right_container).unwrap();
    tree.add_child(right_container, right_top).unwrap();
    tree.add_child(right_container, right_bottom).unwrap();

    let available = geometry::Size {
        width: AvailableSpace::MaxContent,
        height: AvailableSpace::MaxContent,
    };

    tree.compute_layout(root, available).unwrap();
    let original_left_width = tree.layout(left).unwrap().size.width;
    let original_right_top_width = tree.layout(right_top).unwrap().size.width;

    let snapshot = capture_tree_styles(&tree, root).unwrap();
    apply_zoomed_panel_styles(&mut tree, root, right_bottom, &snapshot).unwrap();
    tree.compute_layout(root, available).unwrap();

    assert_eq!(tree.style(left).unwrap().display, Display::None);
    assert_eq!(tree.style(right_top).unwrap().display, Display::None);
    assert_eq!(tree.style(right_container).unwrap().display, Display::Flex);

    let zoomed_bottom = tree.layout(right_bottom).unwrap();
    assert!(
        (zoomed_bottom.size.width - 1200.0).abs() < 1.0,
        "zoomed width should fill the root, got {}",
        zoomed_bottom.size.width
    );
    assert!(
        (zoomed_bottom.size.height - 800.0).abs() < 1.0,
        "zoomed height should fill the root, got {}",
        zoomed_bottom.size.height
    );

    for node in collect_tree_nodes(&tree, root) {
        if let Some(style) = snapshot.get(&node) {
            tree.set_style(node, style.clone()).unwrap();
        }
    }
    tree.compute_layout(root, available).unwrap();

    assert_eq!(tree.style(left).unwrap().display, Display::Flex);
    assert_eq!(tree.style(right_top).unwrap().display, Display::Flex);

    let restored_left_width = tree.layout(left).unwrap().size.width;
    let restored_right_top_width = tree.layout(right_top).unwrap().size.width;

    assert!(
        (restored_left_width - original_left_width).abs() < 1.0,
        "left width should restore to {original_left_width}, got {restored_left_width}"
    );
    assert!(
        (restored_right_top_width - original_right_top_width).abs() < 1.0,
        "right-top width should restore to {original_right_top_width}, got {restored_right_top_width}"
    );
}

#[test]
fn test_directional_focus_prefers_overlapping_adjacent_panel() {
    use taffy::{FlexDirection, TaffyTree};

    let mut tree: TaffyTree<()> = TaffyTree::new();
    let available = geometry::Size {
        width: AvailableSpace::MaxContent,
        height: AvailableSpace::MaxContent,
    };

    let root = tree
        .new_leaf(Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            gap: geometry::Size {
                width: length(10.0),
                height: length(0.0),
            },
            size: geometry::Size {
                width: length(1210.0),
                height: length(900.0),
            },
            ..Default::default()
        })
        .unwrap();

    let left_container = tree
        .new_leaf(Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            gap: geometry::Size {
                width: length(0.0),
                height: length(10.0),
            },
            ..Default::default()
        })
        .unwrap();

    let right = tree
        .new_leaf(Style {
            display: Display::Flex,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        })
        .unwrap();

    let top_left = tree
        .new_leaf(Style {
            display: Display::Flex,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        })
        .unwrap();

    let bottom_left = tree
        .new_leaf(Style {
            display: Display::Flex,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        })
        .unwrap();

    tree.add_child(root, left_container).unwrap();
    tree.add_child(root, right).unwrap();
    tree.add_child(left_container, top_left).unwrap();
    tree.add_child(left_container, bottom_left).unwrap();

    tree.compute_layout(root, available).unwrap();

    let selected = {
        let current_layout = tree.layout(right).unwrap();
        let current_bottom = current_layout.location.y + current_layout.size.height;
        let current_center_y =
            current_layout.location.y + current_layout.size.height / 2.0;
        let mut best: Option<(NodeId, f32, f32, f32, f32)> = None;

        for other in [top_left, bottom_left] {
            let other_layout = tree.layout(other).unwrap();
            let other_bottom = other_layout.location.y + other_layout.size.height;
            let overlap = (current_bottom.min(other_bottom)
                - current_layout.location.y.max(other_layout.location.y))
            .max(0.0);
            let distance = current_layout.location.x
                - (other_layout.location.x + other_layout.size.width);
            let center_delta = (current_center_y
                - (other_layout.location.y + other_layout.size.height / 2.0))
                .abs();
            let tie_breaker = other_layout.location.y;

            if overlap <= 0.0 || distance < 0.0 || distance > 11.0 {
                continue;
            }

            let should_replace = match best {
                None => true,
                Some((_, best_overlap, best_distance, best_center_delta, best_tie)) => {
                    overlap > best_overlap + 0.5
                        || ((overlap - best_overlap).abs() <= 0.5
                            && distance < best_distance - 0.5)
                        || ((overlap - best_overlap).abs() <= 0.5
                            && (distance - best_distance).abs() <= 0.5
                            && center_delta < best_center_delta - 0.5)
                        || ((overlap - best_overlap).abs() <= 0.5
                            && (distance - best_distance).abs() <= 0.5
                            && (center_delta - best_center_delta).abs() <= 0.5
                            && tie_breaker < best_tie)
                }
            };

            if should_replace {
                best = Some((other, overlap, distance, center_delta, tie_breaker));
            }
        }

        best.map(|(node, _, _, _, _)| node)
    };

    assert_eq!(
        selected,
        Some(top_left),
        "focusing left from a full-height panel should choose the upper overlapping neighbor first"
    );
}

#[test]
fn test_rich_text_visibility_by_layout_respects_zoomed_panel() {
    let text_dimensions = TextDimensions {
        width: 10.0,
        height: 20.0,
        scale: 1.0,
    };
    let dimension =
        ContextDimension::build(1210.0, 900.0, text_dimensions, 1.0, Margin::all(0.0));
    let panel_config = rio_backend::config::layout::Panel {
        margin: Margin::all(0.0),
        padding: Margin::all(0.0),
        row_gap: 10.0,
        column_gap: 10.0,
        border_width: 2.0,
        border_radius: 0.0,
    };
    let mut grid = ContextGrid::new(
        create_dead_context(VoidListener, WindowId::from(0), 1, 101, dimension),
        Margin::all(0.0),
        [0.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 0.0, 1.0],
        panel_config,
    );

    let left = grid.current;
    let right = grid.try_split_right().unwrap();
    grid.inner.insert(
        right,
        ContextGridItem::new(create_dead_context(
            VoidListener,
            WindowId::from(0),
            2,
            202,
            dimension,
        )),
    );
    grid.calculate_positions();

    let snapshot = capture_tree_styles(&grid.tree, grid.root_node).unwrap();
    apply_zoomed_panel_styles(&mut grid.tree, grid.root_node, right, &snapshot).unwrap();

    let visibility = grid.rich_text_visibility_by_layout();
    assert_eq!(visibility.get(&101), Some(&false));
    assert_eq!(visibility.get(&202), Some(&true));
    assert!(grid.is_node_visible(right));
    assert!(!grid.is_node_visible(left));
}

#[test]
fn test_select_split_right_uses_absolute_panel_positions_for_nested_splits() {
    let text_dimensions = TextDimensions {
        width: 10.0,
        height: 20.0,
        scale: 1.0,
    };
    let dimension =
        ContextDimension::build(1210.0, 900.0, text_dimensions, 1.0, Margin::all(0.0));
    let panel_config = rio_backend::config::layout::Panel {
        margin: Margin::all(0.0),
        padding: Margin::all(0.0),
        row_gap: 10.0,
        column_gap: 10.0,
        border_width: 2.0,
        border_radius: 0.0,
    };
    let mut grid = ContextGrid::new(
        create_dead_context(VoidListener, WindowId::from(0), 1, 1, dimension),
        Margin::all(0.0),
        [0.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 0.0, 1.0],
        panel_config,
    );

    let left = grid.current;

    let right = grid.try_split_right().unwrap();
    grid.inner.insert(
        right,
        ContextGridItem::new(create_dead_context(
            VoidListener,
            WindowId::from(0),
            2,
            2,
            dimension,
        )),
    );
    grid.calculate_positions();

    grid.current = right;
    let bottom_right = grid.try_split_down().unwrap();
    grid.inner.insert(
        bottom_right,
        ContextGridItem::new(create_dead_context(
            VoidListener,
            WindowId::from(0),
            3,
            3,
            dimension,
        )),
    );
    grid.calculate_positions();

    grid.current = left;
    assert!(
        grid.select_split_right(),
        "moving right from the left panel should find the nested right branch"
    );
    assert_eq!(
        grid.current, right,
        "directional focus should choose the upper overlapping panel on the right"
    );
}

#[test]
fn test_select_split_right_respects_panel_margins_without_explicit_gap() {
    let text_dimensions = TextDimensions {
        width: 10.0,
        height: 20.0,
        scale: 1.0,
    };
    let dimension =
        ContextDimension::build(1210.0, 900.0, text_dimensions, 1.0, Margin::all(0.0));
    let panel_config = rio_backend::config::layout::Panel {
        padding: Margin::all(0.0),
        ..rio_backend::config::layout::Panel::default()
    };
    let mut grid = ContextGrid::new(
        create_dead_context(VoidListener, WindowId::from(0), 1, 1, dimension),
        Margin::all(0.0),
        [0.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 0.0, 1.0],
        panel_config,
    );

    let left = grid.current;
    let right = grid.try_split_right().unwrap();
    grid.inner.insert(
        right,
        ContextGridItem::new(create_dead_context(
            VoidListener,
            WindowId::from(0),
            2,
            2,
            dimension,
        )),
    );
    grid.calculate_positions();

    grid.current = left;
    assert!(
        grid.select_split_right(),
        "panel margins should still allow focusing the panel on the right"
    );
    assert_eq!(grid.current, right);
}

#[test]
fn test_select_split_down_respects_panel_margins_without_explicit_gap() {
    let text_dimensions = TextDimensions {
        width: 10.0,
        height: 20.0,
        scale: 1.0,
    };
    let dimension =
        ContextDimension::build(1210.0, 900.0, text_dimensions, 1.0, Margin::all(0.0));
    let panel_config = rio_backend::config::layout::Panel {
        padding: Margin::all(0.0),
        ..rio_backend::config::layout::Panel::default()
    };
    let mut grid = ContextGrid::new(
        create_dead_context(VoidListener, WindowId::from(0), 1, 1, dimension),
        Margin::all(0.0),
        [0.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 0.0, 1.0],
        panel_config,
    );

    let top = grid.current;
    let bottom = grid.try_split_down().unwrap();
    grid.inner.insert(
        bottom,
        ContextGridItem::new(create_dead_context(
            VoidListener,
            WindowId::from(0),
            2,
            2,
            dimension,
        )),
    );
    grid.calculate_positions();

    grid.current = top;
    assert!(
        grid.select_split_down(),
        "panel margins should still allow focusing the panel below"
    );
    assert_eq!(grid.current, bottom);
}

#[test]
fn test_split_right_terminal_columns_use_panel_content_width() {
    let text_dimensions = TextDimensions {
        width: 10.0,
        height: 20.0,
        scale: 1.0,
    };
    let dimension =
        ContextDimension::build(120.0, 100.0, text_dimensions, 1.0, Margin::all(0.0));
    let panel_config = rio_backend::config::layout::Panel {
        margin: Margin::all(0.0),
        padding: Margin::all(5.0),
        row_gap: 0.0,
        column_gap: 0.0,
        border_width: 2.0,
        border_radius: 0.0,
    };
    let mut grid = ContextGrid::new(
        create_dead_context(VoidListener, WindowId::from(0), 1, 1, dimension),
        Margin::all(0.0),
        [0.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 0.0, 1.0],
        panel_config,
    );

    let left = grid.current;
    let right = grid.try_split_right().unwrap();
    grid.inner.insert(
        right,
        ContextGridItem::new(create_dead_context(
            VoidListener,
            WindowId::from(0),
            2,
            2,
            dimension,
        )),
    );
    grid.calculate_positions();

    let left_item = grid.inner.get(&left).unwrap();
    assert_eq!(left_item.layout_rect, [0.0, 0.0, 60.0, 100.0]);
    assert_eq!(left_item.terminal_rect, [5.0, 5.0, 50.0, 90.0]);

    grid.apply_taffy_layout_for_tests();

    let left_item = grid.inner.get(&left).unwrap();
    let right_item = grid.inner.get(&right).unwrap();
    assert_eq!(left_item.val.dimension.columns, 5);
    assert_eq!(right_item.val.dimension.columns, 5);
    assert_eq!(left_item.val.dimension.width, left_item.terminal_rect[2]);
    assert_eq!(right_item.val.dimension.width, right_item.terminal_rect[2]);
}

#[test]
fn test_split_down_terminal_rows_use_panel_content_height() {
    let text_dimensions = TextDimensions {
        width: 10.0,
        height: 20.0,
        scale: 1.0,
    };
    let dimension =
        ContextDimension::build(120.0, 100.0, text_dimensions, 1.0, Margin::all(0.0));
    let panel_config = rio_backend::config::layout::Panel {
        margin: Margin::all(0.0),
        padding: Margin::all(5.0),
        row_gap: 0.0,
        column_gap: 0.0,
        border_width: 2.0,
        border_radius: 0.0,
    };
    let mut grid = ContextGrid::new(
        create_dead_context(VoidListener, WindowId::from(0), 1, 1, dimension),
        Margin::all(0.0),
        [0.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 0.0, 1.0],
        panel_config,
    );

    let top = grid.current;
    let bottom = grid.try_split_down().unwrap();
    grid.inner.insert(
        bottom,
        ContextGridItem::new(create_dead_context(
            VoidListener,
            WindowId::from(0),
            2,
            2,
            dimension,
        )),
    );
    grid.calculate_positions();

    let top_item = grid.inner.get(&top).unwrap();
    assert_eq!(top_item.layout_rect, [0.0, 0.0, 120.0, 50.0]);
    assert_eq!(top_item.terminal_rect, [5.0, 5.0, 110.0, 40.0]);

    grid.apply_taffy_layout_for_tests();

    let top_item = grid.inner.get(&top).unwrap();
    let bottom_item = grid.inner.get(&bottom).unwrap();
    assert_eq!(top_item.val.dimension.lines, 2);
    assert_eq!(bottom_item.val.dimension.lines, 2);
    assert_eq!(top_item.val.dimension.height, top_item.terminal_rect[3]);
    assert_eq!(
        bottom_item.val.dimension.height,
        bottom_item.terminal_rect[3]
    );
}

#[test]
fn test_update_panel_config_refreshes_existing_split_layout() {
    let text_dimensions = TextDimensions {
        width: 10.0,
        height: 20.0,
        scale: 1.0,
    };
    let dimension =
        ContextDimension::build(120.0, 100.0, text_dimensions, 1.0, Margin::all(0.0));
    let mut grid = ContextGrid::new(
        create_dead_context(VoidListener, WindowId::from(0), 1, 1, dimension),
        Margin::all(0.0),
        [0.1, 0.1, 0.1, 1.0],
        [0.9, 0.9, 0.9, 1.0],
        rio_backend::config::layout::Panel {
            margin: Margin::all(0.0),
            padding: Margin::all(0.0),
            row_gap: 0.0,
            column_gap: 0.0,
            border_width: 2.0,
            border_radius: 0.0,
        },
    );

    let left = grid.current;
    let right = grid.try_split_right().unwrap();
    grid.inner.insert(
        right,
        ContextGridItem::new(create_dead_context(
            VoidListener,
            WindowId::from(0),
            2,
            2,
            dimension,
        )),
    );

    assert!(grid.apply_taffy_layout_for_tests());
    let left_item = grid.inner.get(&left).unwrap();
    assert_eq!(left_item.terminal_rect, [0.0, 0.0, 60.0, 100.0]);

    grid.update_panel_config(
        rio_backend::config::layout::Panel {
            margin: Margin::all(0.0),
            padding: Margin::all(5.0),
            row_gap: 0.0,
            column_gap: 0.0,
            border_width: 3.0,
            border_radius: 0.0,
        },
        [0.2, 0.3, 0.4, 1.0],
    );
    assert!(grid.apply_taffy_layout_for_tests());

    let left_item = grid.inner.get(&left).unwrap();
    let right_item = grid.inner.get(&right).unwrap();
    assert_eq!(left_item.terminal_rect, [5.0, 5.0, 50.0, 90.0]);
    assert_eq!(right_item.terminal_rect, [65.0, 5.0, 50.0, 90.0]);
    assert_eq!(left_item.val.dimension.columns, 5);
    assert_eq!(right_item.val.dimension.columns, 5);
    assert_eq!(grid.border_config.width, 3.0);
    assert_eq!(grid.border_config.color, [0.2, 0.3, 0.4, 1.0]);
}

#[test]
fn test_update_scale_refreshes_existing_split_spacing() {
    let text_dimensions = TextDimensions {
        width: 10.0,
        height: 20.0,
        scale: 1.0,
    };
    let dimension =
        ContextDimension::build(120.0, 100.0, text_dimensions, 1.0, Margin::all(0.0));
    let panel_config = rio_backend::config::layout::Panel {
        margin: Margin::all(1.0),
        padding: Margin::all(5.0),
        row_gap: 0.0,
        column_gap: 0.0,
        border_width: 2.0,
        border_radius: 0.0,
    };
    let mut grid = ContextGrid::new(
        create_dead_context(VoidListener, WindowId::from(0), 1, 1, dimension),
        Margin::all(0.0),
        [0.1, 0.1, 0.1, 1.0],
        [0.9, 0.9, 0.9, 1.0],
        panel_config,
    );

    let left = grid.current;
    let right = grid.try_split_right().unwrap();
    grid.inner.insert(
        right,
        ContextGridItem::new(create_dead_context(
            VoidListener,
            WindowId::from(0),
            2,
            2,
            dimension,
        )),
    );

    assert!(grid.apply_taffy_layout_for_tests());
    let left_item = grid.inner.get(&left).unwrap();
    assert_eq!(left_item.terminal_rect, [6.0, 6.0, 48.0, 88.0]);

    grid.update_scale(2.0);
    assert!(grid.apply_taffy_layout_for_tests());

    let left_item = grid.inner.get(&left).unwrap();
    let right_item = grid.inner.get(&right).unwrap();
    assert_eq!(left_item.terminal_rect, [12.0, 12.0, 36.0, 76.0]);
    assert_eq!(right_item.terminal_rect, [72.0, 12.0, 36.0, 76.0]);
    assert_eq!(left_item.val.dimension.width, left_item.terminal_rect[2]);
    assert_eq!(right_item.val.dimension.height, right_item.terminal_rect[3]);
}

#[test]
fn test_invalidate_visible_panels_for_full_redraw_marks_all_split_panels_dirty() {
    let text_dimensions = TextDimensions {
        width: 10.0,
        height: 20.0,
        scale: 1.0,
    };
    let dimension =
        ContextDimension::build(1210.0, 900.0, text_dimensions, 1.0, Margin::all(0.0));
    let mut grid = ContextGrid::new(
        create_dead_context(VoidListener, WindowId::from(0), 1, 1, dimension),
        Margin::all(0.0),
        [0.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 0.0, 1.0],
        rio_backend::config::layout::Panel::default(),
    );

    let right = grid.try_split_right().unwrap();
    grid.inner.insert(
        right,
        ContextGridItem::new(create_dead_context(
            VoidListener,
            WindowId::from(0),
            2,
            2,
            dimension,
        )),
    );
    grid.calculate_positions();

    for item in grid.contexts_mut().values_mut() {
        item.val.renderable_content.pending_update.reset();
        assert!(!item.val.renderable_content.pending_update.is_dirty());
    }

    grid.invalidate_visible_panels_for_full_redraw();

    for item in grid.contexts_mut().values_mut() {
        let pending = &mut item.val.renderable_content.pending_update;
        assert!(pending.is_dirty());
        assert_eq!(pending.take_terminal_damage(), Some(TerminalDamage::Full));
    }
}
