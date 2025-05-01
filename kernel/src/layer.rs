use alloc::{format, vec::Vec};
use common::matrix::Vec2;
use ouroboros::self_referencing;

use crate::{allocator::Locked, window::Window};

struct Layer {
    id: usize,
    origin_position: Vec2<usize>,
    window: Window,
}

impl Layer {
    fn move_absolute(&mut self, origin_position: Vec2<usize>) {
        self.origin_position = origin_position;
    }

    fn move_relative(&mut self, origin_position_offset: Vec2<usize>) {
        self.origin_position += origin_position_offset;
    }

    fn draw_to_frame_buffer(&mut self) {
        self.window.draw_to_frame_buffer(self.origin_position);
    }
}

struct LayerManager {
    layers: Vec<Layer>,
    layer_stack: Vec<*mut Layer>,
    latest_id: usize,
}

impl LayerManager {
    // fn new_layer(&mut self) -> &'a mut Layer {
    //     self.latest_id += 1;
    //     self.layers.push(Layer {
    //         id: self.latest_id,
    //         origin_position,
    //     });
    //     todo!()
    // }

    fn find_layer_mut(&mut self, id: usize) -> Option<&mut Layer> {
        let layer = self.layers.iter_mut().find(|layer| layer.id == id);
        return layer;
    }

    fn find_layer(&mut self, id: usize) -> Option<&Layer> {
        let layer = self.layers.iter().find(|layer| layer.id == id);
        return layer;
    }

    fn move_absolute(&mut self, id: usize, position: Vec2<usize>) {
        self.find_layer_mut(id)
            .expect(&format!("No such a layer with id {}", id))
            .move_absolute(position);
    }

    fn move_relative(&mut self, id: usize, position: Vec2<usize>) {
        self.find_layer_mut(id)
            .expect(&format!("No such a layer with id {}", id))
            .move_relative(position);
    }

    unsafe fn draw(&mut self) {
        for layer in self.layer_stack.iter() {
            unsafe { &mut *(*layer) }.draw_to_frame_buffer();
        }
    }

    unsafe fn hide(&mut self, id: usize) {
        self.layer_stack
            .retain(|layer| unsafe { &*(*layer) }.id == id);
    }

    unsafe fn up_or_down(&mut self, id: usize, new_height: usize) {
        let mut local_new_height = new_height;

        if new_height > self.layer_stack.len() {
            local_new_height = self.layer_stack.len();
        }

        let optional_old_position = self
            .layer_stack
            .iter()
            .position(|&layer| unsafe { &*layer }.id == id);
        let mut new_position = local_new_height;

        if let Some(old_position) = optional_old_position {
            if new_position == self.layer_stack.len() {
                new_position -= 1;
            }

            let layer = self.layer_stack.remove(old_position);
            self.layer_stack.insert(new_position, layer)
        } else {
            let layer = self
                .find_layer_mut(id)
                .expect(&format!("No such a layer with id {}.", id));
            let mut_ptr = layer as *mut Layer;
            self.layer_stack.insert(new_position, mut_ptr);
            return;
        }
    }
}
