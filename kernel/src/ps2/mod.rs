use controller::{Controller, ControllerConfigByte};
use x86_64::instructions::port::Port;

use crate::printk;

pub mod controller;
pub mod keyboard;
pub mod mouse;

pub fn controller() -> Controller {
    Controller::new()
}

pub fn init() {
    // https://wiki.osdev.org/%228042%22_PS/2_Controller#Initialising%20the%20PS/2%20Controller

    // fn do_steps_before_self_test() -> Controller {
    //     // Step 2: Determine if the PS/2 Controller Exists
    //     let mut controller = Controller::new();

    //     // Step 3: Disable Devices
    //     controller.disable_first_port();
    //     controller.disable_second_port();

    //     // Step 4: Flush The Output Buffer
    //     unsafe { controller.flush_data_port() };

    //     // Step 5: Set the Controller Configuration Byte
    //     {
    //         let mut config_byte = controller
    //             .read_config_byte()
    //             .expect("Failed to read the PS/2 controller config byte.");

    //         // disable IRQs and translation for port 1 by clearing bits 0 and 6.
    //         config_byte.set_first_port_interrupt(false);
    //         config_byte.set_first_port_translation(false);

    //         // check that the clock signal is enabled by clearing bit 4.
    //         config_byte.set_first_port_clock(false);

    //         controller
    //             .write_config_byte(config_byte)
    //             .expect("Failed to write to the PS/2 controller config byte.");
    //     }
    //     controller
    // }

    // let mut controller = do_steps_before_self_test();

    // // Step 6: Perform Controller Self Test
    // {
    //     controller
    //         .test_controller()
    //         .expect("Test for PS/2 Controller failed.");
    //     // This can reset the PS/2 controller on some hardware.
    //     // At the very least, the Controller Configuration Byte should be restored
    //     // // for compatibility with such hardware.
    //     // restore the value read before issuing 0xAA (self test).
    //     controller = do_steps_before_self_test();
    // }

    // // Step 7: Determine If There Are 2 Channels
    // let has_second_port = {
    //     // enable the second PS/2 port
    //     controller.enable_second_port();

    //     // read the Controller Configuration Byte
    //     let mut config_byte = controller
    //         .read_config_byte()
    //         .expect("Failed to read the PS/2 controller config byte.");

    //     let has_second_port = !config_byte.get_second_port_clock();
    //     if has_second_port {
    //         // if the controller has a dual channel.
    //         // disable the second PS/2 port again
    //         controller.disable_second_port();

    //         // clear bits 1 and 5 of the Controller Configuration Byte to disable IRQs and enable the clock for port 2
    //         config_byte.set_second_port_interrupt(false);
    //         config_byte.set_second_port_clock(false);

    //         controller
    //             .write_config_byte(config_byte)
    //             .expect("Failed to write to the PS/2 controller config byte.");
    //     }

    //     has_second_port
    // };

    // // Step 8: Perform Interface Tests
    // // test the first PS/2 port
    // let first_port_works = controller.test_first_port().is_ok();
    // // test the second PS/2 port
    // let second_port_works = has_second_port && controller.test_second_port().is_ok();

    // // Step 9: Enable Devices
    // // enable any usable PS/2 port that exists and interrupts for any usable PS/2 ports
    // let mut config_byte = controller
    //     .read_config_byte()
    //     .expect("Failed to read the PS/2 controller config byte.");
    // if first_port_works {
    //     controller.enable_first_port();
    //     config_byte.set_first_port_interrupt(true);

    //     // Step 10: Reset Devices
    //     unsafe { controller.keyboard().reset_and_self_test() }
    //         .unwrap_or_else(|err| panic!("failed to reset the keyboard: {:?}", err));
    // }
    // if second_port_works {
    //     controller.enable_second_port();
    //     config_byte.set_second_port_interrupt(true);

    //     // Step 10: Reset Devices
    //     unsafe { controller.mouse().reset_and_self_test() }
    //         .unwrap_or_else(|err| panic!("failed to reset the mouse: {:?}", err));
    // }

    let mut controller = Controller::new();
    let config_byte = ControllerConfigByte::from_u8(0x47);
    printk!(
        "ps/2 controller config byte: 0x{:X}",
        config_byte.get().clone()
    );

    controller
        .write_config_byte(config_byte.clone())
        .expect("Failed to write to the PS/2 controller config byte.");

    printk!(
        "ps/2 controller config byte: 0x{:X}",
        controller.read_config_byte().unwrap().get() // config_byte
    );
}
