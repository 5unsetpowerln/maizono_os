use core::mem::MaybeUninit;

use alloc::{format, sync::Arc, vec::Vec};
use common::graphic::GraphicInfo;
use glam::{I64Vec2, U64Vec2, u64vec2};
use log::debug;
use slotmap::SlotMap;
use spin::{Lazy, Mutex};

use crate::{
    graphic::{
        PixelWriter, PixelWriterCopyable,
        canvas::Canvas,
        frame_buffer::{self, FRAME_BUFFER_HEIGHT, FRAME_BUFFER_WIDTH, FrameBuffer},
    },
    serial, serial_println,
};

pub struct Layer {
    pub(crate) id: usize,
    origin_position: U64Vec2,
    max_position: U64Vec2,
    canvas: Arc<Mutex<Canvas>>,
}

impl Layer {
    pub fn new(canvas: Arc<Mutex<Canvas>>) -> Self {
        let max_position = {
            let locked = canvas.lock();

            let frame_buffer_width = frame_buffer::FRAME_BUFFER_WIDTH.wait().clone() as u64;
            let frame_buffer_height = frame_buffer::FRAME_BUFFER_HEIGHT.wait().clone() as u64;
            let canvas_width = locked.width();
            let canvas_height = locked.height();

            debug!(
                "frame_buffer_width: {}, canvas_width: {}",
                frame_buffer_width, canvas_width
            );
            debug!(
                "frame_buffer_height: {}, canvas_height: {}",
                frame_buffer_height, canvas_height
            );

            let max_x = *frame_buffer::FRAME_BUFFER_WIDTH.wait() as u64 - locked.width();
            let max_y = *frame_buffer::FRAME_BUFFER_HEIGHT.wait() as u64 - locked.height();
            u64vec2(max_x, max_y)
        };

        Self {
            id: 0,
            origin_position: U64Vec2::new(0, 0),
            max_position,
            canvas,
        }
    }

    fn move_absolute(&mut self, origin_position: U64Vec2) {
        self.origin_position = origin_position.min(self.max_position);
    }

    fn move_relative(&mut self, origin_position_offset: I64Vec2) {
        self.origin_position = self
            .origin_position
            .saturating_add_signed(origin_position_offset)
            .min(self.max_position);
        debug!("({}, {})", self.origin_position.x, self.origin_position.y);
    }

    fn draw_to<'a>(&mut self, writer: &mut FrameBuffer) {
        self.canvas.lock().draw_to(writer, self.origin_position)
    }
}

pub struct LayerManager {
    layers: SlotMap<slotmap::DefaultKey, Layer>,
    layer_stack: Vec<slotmap::DefaultKey>,
    latest_id: usize,
    frame_buffer: MaybeUninit<Arc<Mutex<FrameBuffer>>>,
    back_buffer: FrameBuffer,
}

impl LayerManager {
    fn new() -> Self {
        Self {
            layers: SlotMap::new(),
            layer_stack: Vec::new(),
            latest_id: 0,
            frame_buffer: MaybeUninit::uninit(),
            back_buffer: FrameBuffer::new_empty(),
        }
    }

    pub fn init(&mut self, writer: Arc<Mutex<FrameBuffer>>) {
        let graphic_info = GraphicInfo {
            width: FRAME_BUFFER_WIDTH.wait().clone() as u64,
            height: FRAME_BUFFER_HEIGHT.wait().clone() as u64,
            stride: FRAME_BUFFER_WIDTH.wait().clone(),
            pixel_format: frame_buffer::PIXEL_FORMAT.wait().clone(),
            bytes_per_pixel: frame_buffer::BYTES_PER_PIXEL.wait().clone(),
            frame_buffer_addr: None,
            frame_buffer_size: 0,
        };
        self.back_buffer.init(&graphic_info);

        self.set_writer(writer);
    }

    pub fn set_writer(&mut self, writer: Arc<Mutex<FrameBuffer>>) {
        self.frame_buffer = MaybeUninit::new(writer);
    }

    pub fn add_layer(&mut self, layer: Layer) -> usize {
        let mut layer = layer;

        self.latest_id += 1;
        layer.id = self.latest_id;

        let id = layer.id;
        self.layers.insert(layer);
        id
    }

    fn find_layer(&mut self, id: usize) -> Option<(slotmap::DefaultKey, &'_ Layer)> {
        self.layers.iter().find(|(_, layer)| layer.id == id)
    }

    fn find_layer_mut(&mut self, id: usize) -> Option<(slotmap::DefaultKey, &'_ mut Layer)> {
        self.layers.iter_mut().find(|(_, layer)| layer.id == id)
    }

    pub fn move_absolute(&mut self, id: usize, position: U64Vec2) {
        self.find_layer_mut(id)
            .expect(&format!("No such a layer with id {}", id))
            .1
            .move_absolute(position);
    }

    pub fn move_relative(&mut self, id: usize, offset: I64Vec2) {
        self.find_layer_mut(id)
            .expect(&format!("No such a layer with id {}", id))
            .1
            .move_relative(offset);
    }

    pub fn draw(&mut self) {
        for key in self.layer_stack.iter() {
            self.layers[*key].draw_to(&mut self.back_buffer);
        }

        let frame_buffer = unsafe { &*self.frame_buffer.as_ptr() };
        unsafe { frame_buffer.lock().copy(u64vec2(0, 0), &self.back_buffer) };
    }

    pub fn draw_from(&mut self, id: usize) {
        let specified_layer = if let Some((layer, _)) = self.find_layer(id) {
            layer
        } else {
            return;
        };

        let mut found = false;

        for layer in self.layer_stack.iter() {
            if specified_layer == *layer {
                found = true;
            }
            if found {
                self.layers[*layer].draw_to(&mut self.back_buffer);
            }
        }

        let frame_buffer = unsafe { &*self.frame_buffer.as_ptr() };
        unsafe { frame_buffer.lock().copy(u64vec2(0, 0), &self.back_buffer) };
    }

    // fn hide(&mut self, id: usize) {
    //     self.layer_stack.retain(|key| self.layers[*key].id == id);
    // }

    pub fn up_or_down(&mut self, id: usize, new_height: usize) {
        let mut local_new_height = new_height;

        if new_height > self.layer_stack.len() {
            local_new_height = self.layer_stack.len();
        }

        let mut new_position = local_new_height;

        let old_position_opt = self
            .layer_stack
            .iter()
            .position(|key| self.layers[*key].id == id);

        if let Some(old_position) = old_position_opt {
            if new_position == self.layer_stack.len() {
                new_position -= 1;
            }

            let layer = self.layer_stack.remove(old_position);
            self.layer_stack.insert(new_position, layer)
        } else {
            let key = self
                .find_layer(id)
                .expect(&format!("No such a layer with id {}.", id))
                .clone()
                .0;
            self.layer_stack.insert(new_position, key);
            return;
        }
    }
}

pub static LAYER_MANAGER: Lazy<Mutex<LayerManager>> = Lazy::new(|| Mutex::new(LayerManager::new()));
