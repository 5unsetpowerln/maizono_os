// use xhci::{registers::Capability, Registers};

// pub struct Controller {
//     mmio_base: u64,
//     capability_register: Registers<Capability<>>,
// }

// fn a() {}

use core::num::NonZeroUsize;

use common::array::AlignedArray64;
// use xhci::{accessor::Mapper, Registers};

use crate::{pci::usb::memory::alloc_array, printk};

use super::{device_manager::DeviceManager, error::UsbResult};

// const DEVICE_SIZE: usize = 8;
const NUMBER_OF_DEVICE: usize = 8; // 1 ~ 255
const MAX_SLOTS_EN: u8 = 3;
const DCBAA_LENGTH: usize = MAX_SLOTS_EN as usize + 1;
static mut DCBAA: AlignedArray64<u64, DCBAA_LENGTH> =
    AlignedArray64::from_array([0; DCBAA_LENGTH as usize]);

#[derive(Clone, Copy)]
pub struct MemoryMapper;

impl xhci::accessor::Mapper for MemoryMapper {
    unsafe fn map(&mut self, phys_start: usize, _bytes: usize) -> core::num::NonZeroUsize {
        NonZeroUsize::new_unchecked(phys_start)
    }

    fn unmap(&mut self, virt_start: usize, bytes: usize) {}
}

// pub fn init_host_controller(mmio_base: u64) {
//     let registers = unsafe { xhci::registers::Registers::new(mmio_base as usize, MemoryMapper) };

//     // reset contoroller
//     {
//         let usb_status_register = registers.operational.usbsts.read_volatile();
//         let mut usb_command_register = registers.operational.usbcmd.read_volatile();

//         // check if USBSTS.HCH is 1 (if xHC is stopped state)
//         if !usb_status_register.hc_halted() {
//             printk!("xHC is not stopped state!");
//             return;
//         }

//         // write 1 to USBCMD.HCRST (write 1 to reset xhc)
//         usb_command_register.set_host_controller_reset();

//         // wait until USUBSTS.CNR is 0
//         printk!("Waiting until USBSTS.CHR is 0.");
//         loop {
//             if !usb_status_register.controller_not_ready() {
//                 break;
//             }
//         }
//         printk!("Done.");
//     }

//     // configure device context
//     {
//         // read HCSPARAMS1.MaxSlots.
//         let max_slots = registers
//             .capability
//             .hcsparams1
//             .read_volatile()
//             .number_of_device_slots();

//         // write number of device ctx that it will actually be enabled to CONFIG.MaxSlotsEn (0 ~ MaxSlots)
//         let mut config_register = registers.operational.config.read_volatile();
//         if MAX_SLOTS_EN as u8 > max_slots {
//             printk!("HCSPARAMS1.MaxSlots is too few.");
//             return;
//         }
//         config_register.set_max_device_slots_enabled(3);

//         // set pointer of the DCBAA to DCBAAP.
//         let mut dcbaap = registers.operational.dcbaap.read_volatile();
//         dcbaap.set(unsafe { DCBAA.as_ptr() } as u64);
//     }

//     // generation and registeration of Command Ring
//     {}
// }

pub struct Controller {
    mmio_base: u64,
    registers: xhci::registers::Registers<MemoryMapper>,
    device_manager: DeviceManager,
}

impl Controller {
    pub unsafe fn new(mmio_base: u64) -> Self {
        Self {
            mmio_base,
            registers: xhci::registers::Registers::new(mmio_base as usize, MemoryMapper),
            device_manager: DeviceManager::new(NUMBER_OF_DEVICE),
        }
    }

    pub fn init(&mut self) -> UsbResult<()> {
        // if (auto err = devmgr_.Initialize(kDeviceSize))
        if let Err(err) = self.device_manager.init() {
            return Err(err); // return err;
        }

        // RequestHCOwnership(mmio_base_, cap_->HCCPARAMS1.Read());

        let mut usbcmd = self.registers.operational.usbcmd.read_volatile(); // auto usbcmd = op_->USBCMD.Read();

        // usbcmd.bits.interrupter_enable = false;
        // usbcmd.bits.host_system_error_enable = false;
        // usbcmd.bits.enable_wrap_event = false;

        // Host controller must be halted before resetting it.
        if !self // if (!op_->USBSTS.Read().bits.host_controller_halted)
            .registers
            .operational
            .usbsts
            .read_volatile()
            .hc_halted()
        {
            // usbcmd.bits.run_stop = false; // stop
            usbcmd.clear_run_stop();
        }

        self.registers.operational.usbcmd.write_volatile(usbcmd); // op_->USBCMD.Write(usbcmd);
        while !self // while (!op_->USBSTS.Read().bits.host_controller_halted);
            .registers
            .operational
            .usbsts
            .read_volatile()
            .hc_halted()
        {}

        // Reset controller
        let mut usbcmd = self.registers.operational.usbcmd.read_volatile(); // usbcmd = op_->USBCMD.Read();

        usbcmd.set_host_controller_reset(); // usbcmd.bits.host_controller_reset = true;

        self.registers.operational.usbcmd.write_volatile(usbcmd); // op_->USBCMD.Write(usbcmd);

        while self // while (op_->USBCMD.Read().bits.host_controller_reset);
            .registers
            .operational
            .usbcmd
            .read_volatile()
            .host_controller_reset()
        {}

        while self // while (op_->USBSTS.Read().bits.controller_not_ready);
            .registers
            .operational
            .usbsts
            .read_volatile()
            .controller_not_ready()
        {}

        // Log(kDebug, "MaxSlots: %u\n", cap_->HCSPARAMS1.Read().bits.max_device_slots);
        printk!(
            "MaxSlots: {}",
            self.registers
                .capability
                .hcsparams1
                .read_volatile()
                .number_of_device_slots()
        );
        // Set "Max Slots Enabled" field in CONFIG.
        let mut config = self.registers.operational.config.read_volatile(); // auto config = op_->CONFIG.Read();
        config.set_max_device_slots_enabled(NUMBER_OF_DEVICE as u8); // config.bits.max_device_slots_enabled = kDeviceSize;
        self.registers.operational.config.write_volatile(config); // op_->CONFIG.Write(config);

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

        let mut dcbaap = self.registers.operational.dcbaap.read_volatile(); // DCBAAP_Bitmap dcbaap{};
        dcbaap.set(self.device_manager.device_context_pointers_ptr().get()); // dcbaap.SetPointer(reinterpret_cast<uint64_t>(devmgr_.DeviceContexts()));
        self.registers.operational.dcbaap.write_volatile(dcbaap); // op_->DCBAAP.Write(dcbaap);

        printk!("done");

        // auto primary_interrupter = &InterrupterRegisterSets()[0];
        // if (auto err = cr_.Initialize(32)) {
        //     return err;
        // }
        // if (auto err = RegisterCommandRing(&cr_, &op_->CRCR)) {
        //     return err; }
        // if (a
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

        Ok(())
    }
}
