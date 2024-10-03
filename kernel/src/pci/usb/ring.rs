use common::address::AlignedAddress64;
use xhci::{
    accessor::{marker::ReadWrite, Mapper},
    registers::{runtime::Interrupter, InterrupterRegisterSet},
};

use super::{error::UsbResult, memory::alloc_array, xhci::TRB};

#[derive(Debug)]
pub struct CommandRing {
    buf_ptr: AlignedAddress64,
    buf_size: usize,
    cycle_bit: bool,
    write_index: usize,
}

impl CommandRing {
    pub fn new() -> Self {
        Self {
            buf_ptr: AlignedAddress64::new(0).unwrap(),
            buf_size: 0,
            cycle_bit: false,
            write_index: 0,
        }
    }

    pub fn init(&mut self, buf_size: usize) -> UsbResult<()> {
        self.cycle_bit = true;
        self.write_index = 0;
        self.buf_size = buf_size;
        self.buf_ptr = alloc_array::<TRB>(self.buf_size, 64, 64 * 1024)?;
        Ok(())
    }

    pub fn get_ptr(&self) -> AlignedAddress64 {
        self.buf_ptr
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct EventRingSegmentTableEntry {
    data: [u32; 4],
    bits: EventRingSegmentTableEntryBits,
}

#[derive(Debug)]
#[repr(packed, C)]
pub struct EventRingSegmentTableEntryBits {
    ring_segment_base_addr: u64,
    ring_segment_size: u32,
    _padding: u32,
    _reserved: u32,
}

#[derive(Debug)]
#[repr(C)]
pub struct EventRing<M: Mapper + Clone> {
    buf: *mut TRB,
    buf_size: u32,
    cycle_bit: bool,
    event_ring_segment_table: *mut EventRingSegmentTableEntry, // Actually, * const [EventRingSegmentTableEntry]
    interrupter_register_set: InterrupterRegisterSet<M>,
}

impl<M: Mapper + Clone> EventRing<M> {
    pub fn new(interrupter_register_set: InterrupterRegisterSet<M>) -> Self {
        Self {
            buf: 0 as *mut TRB,
            buf_size: 0,
            cycle_bit: false,
            event_ring_segment_table: 0 as *mut EventRingSegmentTableEntry,
            interrupter_register_set,
        }
    }

    // pub fn init(&mut self, buf_size: u32, interrupter_index: usize) -> UsbResult<()> {
    //     self.cycle_bit = true;
    //     self.buf_size = buf_size;

    //     self.buf = alloc_array::<TRB>(self.buf_size as usize, 64, 64 * 1024)?.get() as *mut TRB;
    //     self.event_ring_segment_table = alloc_array::<EventRingSegmentTableEntry>(1, 64, 64 * 1024)?
    //         .get() as *mut EventRingSegmentTableEntry;

    //     unsafe {
    //         (*self.event_ring_segment_table).bits.ring_segment_base_addr = self.buf as u64;
    //         (*self.event_ring_segment_table).bits.ring_segment_size = self.buf_size;
    //     }

    //     let mut interrupter = self
    //         .interrupter_register_set
    //         .interrupter_mut(interrupter_index);
    //     // self.interrupter_register_set;

    //     interrupter.erstsz.update_volatile(|erstsz| erstsz.set(1));

    //     self.write_dequeue_pointer(unsafe { &(*self.buf) });

    //     interrupter
    //         .erstba
    //         .update_volatile(|erstba| erstba.set(self.event_ring_segment_table as u64));

    //     Ok(())
    // }

    // fn write_dequeue_pointer(&mut self, ptr: &TRB) {
    //     let mut erdp = self.interrupter.erdp.read_volatile();
    //     erdp.set_event_ring_dequeue_pointer(ptr as *const TRB as u64);
    //     self.interrupter.erdp.write_volatile(erdp);
    // }
}
