//! Grid layout-group tests (#421): fixed columns / rows, start corner and axis,
//! the flexible column count, block alignment, and the grid's preferred size.

use glam::Vec2;

use super::super::fixture::{assert_rect, Fixture, HD};
use super::super::UiLayout;
use crate::components::{
    LayoutAxisFit, LayoutConstraint, LayoutCorner, LayoutElementComponent, LayoutGroupComponent,
    LayoutKind, TextAlignment,
};

fn grid(constraint: LayoutConstraint) -> LayoutGroupComponent {
    LayoutGroupComponent {
        kind: LayoutKind::Grid,
        cell_size: Vec2::new(100.0, 50.0),
        spacing: Vec2::splat(10.0),
        constraint,
        constraint_count: 2,
        ..LayoutGroupComponent::default()
    }
}

fn five(g: LayoutGroupComponent, size: Vec2) -> (Fixture, Vec<u32>) {
    let mut f = Fixture::new(size, g);
    let kids = (0..5).map(|_| f.child(f.panel, None, Vec2::ZERO)).collect();
    (f, kids)
}

#[test]
fn fixed_columns_fill_rows_from_the_upper_left() {
    let (f, k) = five(
        grid(LayoutConstraint::FixedColumnCount),
        Vec2::new(300.0, 200.0),
    );
    assert_rect(f.rect(k[0]), [0.0, 150.0], [100.0, 50.0]);
    assert_rect(f.rect(k[1]), [110.0, 150.0], [100.0, 50.0]);
    assert_rect(f.rect(k[2]), [0.0, 90.0], [100.0, 50.0]);
    assert_rect(f.rect(k[4]), [0.0, 30.0], [100.0, 50.0]);
}

#[test]
fn the_lower_right_corner_mirrors_both_axes() {
    let g = LayoutGroupComponent {
        start_corner: LayoutCorner::LowerRight,
        ..grid(LayoutConstraint::FixedColumnCount)
    };
    let (f, k) = five(g, Vec2::new(300.0, 200.0));
    // First cell: last column, last row of the 2×3 block.
    assert_rect(f.rect(k[0]), [110.0, 30.0], [100.0, 50.0]);
    assert_rect(f.rect(k[1]), [0.0, 30.0], [100.0, 50.0]);
}

#[test]
fn a_flexible_grid_fits_columns_and_aligns_the_block() {
    let g = LayoutGroupComponent {
        child_alignment: TextAlignment::MiddleCenter,
        ..grid(LayoutConstraint::Flexible)
    };
    let (f, k) = five(g, Vec2::new(330.0, 200.0));
    // Three 100-wide columns fit 330; the 320×110 block centres at (5, 45) from
    // the top-left.
    assert_rect(f.rect(k[0]), [5.0, 105.0], [100.0, 50.0]);
    assert_rect(f.rect(k[3]), [5.0, 45.0], [100.0, 50.0]);
}

#[test]
fn start_vertical_fills_columns_first() {
    let g = LayoutGroupComponent {
        start_vertical: true,
        ..grid(LayoutConstraint::FixedRowCount)
    };
    let (f, k) = five(g, Vec2::new(330.0, 200.0));
    assert_rect(f.rect(k[1]), [0.0, 90.0], [100.0, 50.0]);
    assert_rect(f.rect(k[2]), [110.0, 150.0], [100.0, 50.0]);
}

#[test]
fn a_fitted_grid_takes_its_blocks_preferred_size() {
    let (mut f, _) = five(grid(LayoutConstraint::FixedColumnCount), Vec2::ZERO);
    let fit = LayoutElementComponent {
        horizontal_fit: LayoutAxisFit::PreferredSize,
        vertical_fit: LayoutAxisFit::PreferredSize,
        ..LayoutElementComponent::default()
    };
    f.scene.world.set_layout_element(f.panel, Some(fit));
    let layout = UiLayout::compute(&f.scene.world, HD);
    let r = layout.get(f.panel).expect("laid out").rect;
    // Two columns, three rows: 2·100 + 10 by 3·50 + 2·10, centred on the pivot.
    assert_rect(r, [855.0, 455.0], [210.0, 170.0]);
}
