use core::mem::MaybeUninit;

use alloc::{format, sync::Arc, vec::Vec};
use common::matrix::Vec2;
use slotmap::SlotMap;
use spin::{Lazy, Mutex};

use crate::{allocator::Locked, graphic::PixelWriter, window::Window};

pub struct Layer {
    id: usize,
    origin_position: Vec2<usize>,
    window: Arc<Mutex<Window>>,
}

impl Layer {
    pub fn new(window: Arc<Mutex<Window>>) -> Self {
        Self {
            id: 0,
            origin_position: Vec2::new(0, 0),
            window,
        }
    }

    fn move_absolute(&mut self, origin_position: Vec2<usize>) {
        self.origin_position = origin_position;
    }

    fn move_relative(&mut self, origin_position_offset: Vec2<usize>) {
        self.origin_position += origin_position_offset;
    }

    fn draw_to<'a>(&mut self, writer: &'a mut dyn PixelWriter) {
        self.window.lock().draw_to(writer, self.origin_position)
    }
}

// type StaticSharedPixelWriter = &'static Mutex<(dyn PixelWriter + Send)>;
type ThreadSafeSharedPixelWriter = Arc<Mutex<(dyn PixelWriter + Send)>>;

pub struct LayerManager {
    layers: SlotMap<slotmap::DefaultKey, Layer>,
    layer_stack: Vec<slotmap::DefaultKey>,
    latest_id: usize,
    writer: MaybeUninit<ThreadSafeSharedPixelWriter>,
}

impl LayerManager {
    fn new() -> Self {
        Self {
            layers: SlotMap::new(),
            layer_stack: Vec::new(),
            latest_id: 0,
            writer: MaybeUninit::uninit(),
        }
    }

    pub fn init(&mut self, writer: ThreadSafeSharedPixelWriter) {
        self.set_writer(writer);
    }

    pub fn set_writer(&mut self, writer: ThreadSafeSharedPixelWriter) {
        self.writer = MaybeUninit::new(writer);
    }

    pub fn add_layer(&mut self, layer: Layer) {
        let mut layer = layer;

        self.latest_id += 1;
        layer.id = self.latest_id;

        self.layers.insert(layer);
    }

    fn find_layer(&mut self, id: usize) -> Option<(slotmap::DefaultKey, &'_ Layer)> {
        self.layers.iter().find(|(_, layer)| layer.id == id)
    }

    fn find_layer_mut(&mut self, id: usize) -> Option<(slotmap::DefaultKey, &'_ mut Layer)> {
        self.layers.iter_mut().find(|(_, layer)| layer.id == id)
    }

    fn move_absolute(&mut self, id: usize, position: Vec2<usize>) {
        self.find_layer_mut(id)
            .expect(&format!("No such a layer with id {}", id))
            .1
            .move_absolute(position);
    }

    fn move_relative(&mut self, id: usize, position: Vec2<usize>) {
        self.find_layer_mut(id)
            .expect(&format!("No such a layer with id {}", id))
            .1
            .move_relative(position);
    }

    fn draw(&mut self) {
        let mut writer = unsafe { &*self.writer.as_ptr() }.lock();

        for layer in self.layer_stack.iter() {
            self.layers[*layer].draw_to(&mut *writer);
        }
    }

    unsafe fn hide(&mut self, id: usize) {
        self.layer_stack.retain(|key| self.layers[*key].id == id);
    }

    unsafe fn up_or_down(&mut self, id: usize, new_height: usize) {
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
