// use xhci::{registers::Capability, Registers};

// pub struct Controller {
//     mmio_base: u64,
//     capability_register: Registers<Capability<>>,
// }

// fn a() {}

use core::num::NonZeroUsize;

use common::{address::AlignedAddress64, array::AlignedArray64};
use spin::Mutex;
use xhci::{accessor::Mapper, registers::InterrupterRegisterSet, Registers};
// use xhci::{accessor::Mapper, Registers};

use crate::{pci::usb::memory::alloc_array, printk};

use super::{
    device_manager::DeviceManager,
    error::UsbResult,
    ring::{CommandRing, EventRing},
};

#[repr(align(64))]
pub struct TRB(u128);

// const DEVICE_SIZE: usize = 8;
const NUMBER_OF_DEVICE: usize = 8; // 1 ~ 255
const MAX_SLOTS_EN: u8 = 3;
const DCBAA_LENGTH: usize = MAX_SLOTS_EN as usize + 1;
static mut DCBAA: AlignedArray64<u64, DCBAA_LENGTH> =
    AlignedArray64::from_array([0; DCBAA_LENGTH as usize]);

#[derive(Clone, Copy, Debug)]
pub struct MemoryMapper();

impl xhci::accessor::Mapper for MemoryMapper {
    unsafe fn map(&mut self, phys_start: usize, _bytes: usize) -> core::num::NonZeroUsize {
        NonZeroUsize::new_unchecked(phys_start)
    }

    fn unmap(&mut self, virt_start: usize, bytes: usize) {}
}

fn register_command_ring<M: Mapper + Clone>(
    command_ring: &CommandRing,
    operational_registers: &mut xhci::registers::Operational<M>,
) {
    let mut crcr = operational_registers.crcr.read_volatile();
    crcr.set_ring_cycle_state();
    crcr.set_command_ring_pointer(command_ring.get_ptr().get());
    operational_registers.crcr.write_volatile(crcr);
}

pub struct Controller<M: Mapper + Clone> {
    mmio_base: u64,
    registers: xhci::registers::Registers<M>,
    // interrupter_register_set: InterrupterRegisterSet<M>,
    device_manager: DeviceManager,
    command_ring: CommandRing,
    // event_ring: EventRing<'a, M>,
    mapper: M,
}

impl<M: Mapper + Clone> Controller<M> {
    pub unsafe fn new(mmio_base: u64, mapper: M) -> Self {
        let registers = xhci::registers::Registers::new(mmio_base as usize, mapper.clone());

        Self {
            mmio_base,
            registers,
            // interrupter_register_set,
            device_manager: DeviceManager::new(NUMBER_OF_DEVICE),
            command_ring: CommandRing::new(),
            // event_ring: EventRing::new(interrupter_register_set),
            mapper,
        }
    }

    // fn get_interrupter_register_set(&self) -> &'a InterrupterRegisterSet<M> {
    //     todo!()
    // }

    pub fn init(&mut self) -> UsbResult<()> {
        self.device_manager.init()?;

        // RequestHCOwnership(mmio_base_, cap_->HCCPARAMS1.Read());

        let mut usbcmd = self.registers.operational.usbcmd.read_volatile(); // auto usbcmd = op_->USBCMD.Read();

        // usbcmd.bits.interrupter_enable = false;
        // usbcmd.bits.host_system_error_enable = false;
        // usbcmd.bits.enable_wrap_event = false;

        // Host controller must be halted before resetting it.
        if !self
            .registers
            .operational
            .usbsts
            .read_volatile()
            .hc_halted()
        {
            usbcmd.clear_run_stop();
        }

        self.registers.operational.usbcmd.write_volatile(usbcmd);

        while !self
            .registers
            .operational
            .usbsts
            .read_volatile()
            .hc_halted()
        {}

        // Reset controller
        let mut usbcmd = self.registers.operational.usbcmd.read_volatile();

        usbcmd.set_host_controller_reset();

        self.registers.operational.usbcmd.write_volatile(usbcmd);

        while self
            .registers
            .operational
            .usbcmd
            .read_volatile()
            .host_controller_reset()
        {}

        while self
            .registers
            .operational
            .usbsts
            .read_volatile()
            .controller_not_ready()
        {}

        printk!(
            "MaxSlots: {}",
            self.registers
                .capability
                .hcsparams1
                .read_volatile()
                .number_of_device_slots()
        );

        // Set "Max Slots Enabled" field in CONFIG.
        let mut config = self.registers.operational.config.read_volatile();
        config.set_max_device_slots_enabled(NUMBER_OF_DEVICE as u8);
        self.registers.operational.config.write_volatile(config);

        // auto hcsparams2 = cap_->HCSPARAMS2.Read();
        // const uint16_t max_scratchpad_buffers =
        //   hcsparams2.bits.max_scratchpad_buffers_low
        //   | (hcsparams2.bits.max_scratchpad_buffers_high << 5);
        // if (max_scratchpad_buffers > 0) {
        //   auto scratchpad_buf_arr = AllocArray<void*>(max_scratchpad_buffers, 64, 4096);
        //   for (int i = 0; i < max_scratchpad_buffers; ++i) {
        //     scratchpad_buf_arr[i] = AllocMem(4096, 4096, 4096);
        //     Log(kDebug, "scratchpad buffer array %d = %p\n",
        //         i, scratchpad_buf_arr[i]);
        //   }
        //   devmgr_.DeviceContexts()[0] = reinterpret_cast<DeviceContext*>(scratchpad_buf_arr);
        //   Log(kInfo, "wrote scratchpad buffer array %p to dev ctx array 0\n",
        //       scratchpad_buf_arr);
        // }

        let mut dcbaap = self.registers.operational.dcbaap.read_volatile();
        dcbaap.set(self.device_manager.device_context_pointers_ptr().get());
        self.registers.operational.dcbaap.write_volatile(dcbaap);

        self.command_ring.init(32)?;
        register_command_ring(&self.command_ring, &mut self.registers.operational);

        // let mut primary_interrupter = unsafe {
        //     InterrupterRegisterSet::new(
        //         self.mmio_base as usize,
        //         self.registers.capability.rtsoff.read_volatile(),
        //         self.mapper.clone(),
        //     )
        // }
        // .interrupter_mut(0);

        // self.event_ring.init(32);
        // if (auto err = er_.Initialize(32, primary_interrupter)) {
        //     return err;
        // }

        // // Enable interrupt for the primary interrupter
        // auto iman = primary_interrupter->IMAN.Read();
        // iman.bits.interrupt_pending = true;
        // iman.bits.interrupt_enable = true;
        // primary_interrupter->IMAN.Write(iman);

        // // Enable interrupt for the controller
        // usbcmd = op_->USBCMD.Read();
        // usbcmd.bits.interrupter_enable = true;
        // op_->USBCMD.Write(usbcmd);

        // return MAKE_ERROR(Error::kSuccess);

        // todo!()
        Ok(())
    }
}
