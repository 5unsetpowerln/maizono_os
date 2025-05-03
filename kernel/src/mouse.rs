use alloc::sync::Arc;
use common::{graphic::RgbColor, matrix::Vec2};
use spin::{Lazy, Mutex};

use crate::{device::ps2, error::Error, graphic::PixelWriter, kprintln};

use super::frame_buffer;

pub const MOUSE_CURSOR_WIDTH: usize = 15;
pub const MOUSE_CURSOR_HEIGHT: usize = 24;

#[derive(Debug, Copy, Clone)]
enum MousePixel {
    Null,
    Border(RgbColor),
    Inner(RgbColor),
}

pub const MOUSE_TRANSPARENT_COLOR: RgbColor = RgbColor::rgb(0, 0, 1);

const MOUSE_CURSOR_DATA: [[MousePixel; MOUSE_CURSOR_WIDTH]; MOUSE_CURSOR_HEIGHT] = {
    const MOUSE_CURSOR_SHAPE: [[u8; MOUSE_CURSOR_WIDTH]; MOUSE_CURSOR_HEIGHT] = [
        [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [1, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [1, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [1, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [1, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [1, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0],
        [1, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0],
        [1, 2, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0],
        [1, 2, 2, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0],
        [1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0],
        [1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0],
        [1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 0, 0],
        [1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 0],
        [1, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1],
        [1, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0],
        [1, 2, 2, 2, 2, 1, 1, 2, 1, 0, 0, 0, 0, 0, 0],
        [1, 2, 2, 2, 1, 0, 1, 2, 1, 0, 0, 0, 0, 0, 0],
        [1, 2, 2, 1, 0, 0, 0, 1, 2, 1, 0, 0, 0, 0, 0],
        [1, 2, 1, 0, 0, 0, 0, 1, 2, 1, 0, 0, 0, 0, 0],
        [1, 1, 0, 0, 0, 0, 0, 0, 1, 2, 1, 0, 0, 0, 0],
        [1, 0, 0, 0, 0, 0, 0, 0, 1, 2, 1, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 1, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0],
    ];

    let mut mouse_cursor = [[MousePixel::Null; MOUSE_CURSOR_WIDTH]; MOUSE_CURSOR_HEIGHT];

    let mut dy = 0;
    while dy < MOUSE_CURSOR_HEIGHT {
        let mut dx = 0;
        while dx < MOUSE_CURSOR_WIDTH {
            mouse_cursor[dy][dx] = match MOUSE_CURSOR_SHAPE[dy][dx] {
                0 => MousePixel::Null,
                1 => MousePixel::Border(RgbColor::from(0xffffff00)),
                2 => MousePixel::Inner(RgbColor::from(0x0)),
                _ => panic!("unexpected mouse pixel."),
            };
            dx += 1;
        }
        dy += 1;
    }

    mouse_cursor
};

pub enum MouseEvent {
    Move { displacement: Vec2<isize> },
    LeftClick,
    MiddleClick,
    RightClick,
}

// struct MouseCursor {
//     erase_color: RgbColor,
//     position: Vec2<isize>,
// }

// impl MouseCursor {
//     const fn new() -> Self {
//         Self {
//             erase_color: RgbColor::from(0x0),
//             position: Vec2::new(0, 0, 0, 0, 0, 0), // the cordinate shouldn't be negative values
//         }
//     }

//     fn init(&mut self, initial_x: isize, initial_y: isize, erase_color: RgbColor) {
//         self.position = Vec2::new(
//             initial_x,
//             initial_y,
//             0,
//             frame_buffer::width() as isize,
//             0,
//             frame_buffer::height() as isize,
//         );
//         self.position.clip();
//         self.erase_color = erase_color;
//         draw_mouse_cursor(self.position);
//     }

//     // fn move_relative(&mut self, displacement: Vec2<isize>) {
//     //     erase_mouse_cursor(self.position, self.erase_color);
//     //     self.position += displacement;
//     //     draw_mouse_cursor(self.position);
// }

// fn write_pixel_ignore_outside_buffer_error(x: usize, y: usize, pixel: RgbColor) {
//     if let Err(e) = unsafe {
//         frame_buffer::get_frame_buffer_reference()
//             .lock()
//             .write_pixel(x, y, pixel)
//     } {
//         if let Error::FrameBufferError(frame_buffer::FrameBufferError::OutsideBufferError) = e {
//         } else {
//             panic!("{:?}", e);
//         }
//     }
// }

// type PixelWriterRef<'a> = &'a (dyn PixelWriter);
type ThreadSafeSharedPixelWriter = Arc<Mutex<dyn PixelWriter>>;

// fn draw_mouse_cursor<'a>(writer: PixelWriterRef<'a>, position: Vec2<usize>) {
//     for dy in 0..MOUSE_CURSOR_HEIGHT {
//         for dx in 0..MOUSE_CURSOR_WIDTH {
//             writer.write_pixel(position.x + dx, position.y + dy, RgbColor);
//         }
//     }
// }

pub fn draw_mouse_cursor<'a>(writer: ThreadSafeSharedPixelWriter, position: Vec2<isize>) {
    let mut writer = writer.lock();
    for dy in 0..MOUSE_CURSOR_HEIGHT {
        for dx in 0..MOUSE_CURSOR_WIDTH {
            match MOUSE_CURSOR_DATA[dy][dx] {
                MousePixel::Border(color) => writer
                    .write_pixel(position.x as usize + dx, position.y as usize + dy, color)
                    .expect("Failed to write a pixel to the writer."),
                MousePixel::Inner(color) => writer
                    .write_pixel(position.x as usize + dx, position.y as usize + dy, color)
                    .expect("Failed to write a pixel to the writer"),
                MousePixel::Null => {}
            }
        }
    }
}

pub fn mouse_observer(dx: isize, dy: isize) {}

// fn erase_mouse_cursor(position: Vec2<isize>, erase_color: RgbColor) {
//     for dy in 0..MOUSE_CURSOR_HEIGHT {
//         for dx in 0..MOUSE_CURSOR_WIDTH {
//             if let MousePixel::Null = MOUSE_CURSOR_DATA[dy][dx] {
//             } else {
//                 write_pixel_ignore_outside_buffer_error(
//                     position.x as usize + dx,
//                     position.y as usize + dy,
//                     erase_color.into(),
//                 )
//             }
//         }
//     }
// }

// static MOUSE_CURSOR: Mutex<MouseCursor> = Mutex::new(MouseCursor::new());

// pub fn init(initial_x: isize, initial_y: isize, erase_color: RgbColor) {
//     // MOUSE_CURSOR.lock().init(initial_x, initial_y, erase_color);
// }

// pub fn move_relative(displacement: Vec2<isize>) {
//     MOUSE_CURSOR.lock().move_relative(displacement);
// }
