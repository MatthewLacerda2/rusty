use glam::Vec2;

use super::super::rooms::{find_rooms, RoomParams};
use rusty_scene::lighting::floor::{FloorCell, FloorGrid};

fn room_with_boxes(boxes: &[(usize, usize, usize)]) -> FloorGrid {
    let n = 40;
    let mut cells = vec![
        Some(FloorCell {
            floor: 0.0,
            ceiling: f32::INFINITY
        });
        n * n
    ];
    for &(x0, z0, side) in boxes {
        for z in z0 - 1..z0 + side + 1 {
            for x in x0 - 1..x0 + side + 1 {
                let top = (x0..x0 + side).contains(&x) && (z0..z0 + side).contains(&z);
                cells[z * n + x] = top.then_some(FloorCell {
                    floor: 1.0,
                    ceiling: f32::INFINITY,
                });
            }
        }
    }
    FloorGrid {
        origin: Vec2::ZERO,
        cell: 0.5,
        width: n,
        depth: n,
        cells,
        upper: Vec::new(),
        inset: 0.25,
    }
}

#[test]
fn a_jump_box_top_joins_the_room_around_it() {
    let grid = room_with_boxes(&[(6, 6, 4), (20, 8, 4), (10, 28, 4)]);
    let (room, count) = find_rooms(&grid, &RoomParams::default());
    assert_eq!(count, 1, "one room, no probe per box");
    let open = grid.cells.iter().zip(&room).filter(|(c, _)| c.is_some());
    assert!(open.into_iter().all(|(_, r)| *r == Some(0)));
}

#[test]
fn a_platform_past_the_floor_keeps_its_own_room() {
    let grid = room_with_boxes(&[(6, 6, 4), (20, 8, 4)]);
    let mut grid = grid;
    for z in 20..30 {
        for x in 24..28 {
            grid.cells[z * 40 + x] = Some(FloorCell {
                floor: 2.0,
                ceiling: f32::INFINITY,
            });
        }
    }
    let (room, count) = find_rooms(&grid, &RoomParams::default());
    assert_eq!(count, 2, "the room and the ledge");
    assert_eq!(room[25 * 40 + 25], Some(1));
    assert_eq!(room[6 * 40 + 6], Some(0), "the box still joins the room");
}
