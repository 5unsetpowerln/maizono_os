use controller::Controller;
use keyboard::Keyboard;
use log::{debug, info};
use mouse::Mouse;
use pc_keyboard::{DecodedKey, HandleControl, ScancodeSet1, layouts};
use spin::{Lazy, Mutex, Once};
use static_assertions::const_assert;

pub mod controller;
pub mod keyboard;
pub mod mouse;

const_assert!(controller::LOOP_TIMEOUT > mouse::EVENT_BUFFER_LENGTH);

static MOUSE: Once<Mutex<Mouse>> = Once::new();
static KEYBOARD_CONTROLLER: Once<Mutex<Keyboard>> = Once::new();
static KEYBOARD: Lazy<Mutex<pc_keyboard::Keyboard<layouts::Us104Key, ScancodeSet1>>> =
    Lazy::new(|| {
        Mutex::new(pc_keyboard::Keyboard::new(
            ScancodeSet1::new(),
            layouts::Us104Key,
            HandleControl::Ignore,
        ))
    });

pub fn mouse() -> &'static Mutex<Mouse> {
    if MOUSE.is_completed() {
        MOUSE.wait()
    } else {
        panic!("ps/2 devices are not initialized.")
    }
}

pub fn read_key_event() -> Option<DecodedKey> {
    let mut kbd_controller = KEYBOARD_CONTROLLER.wait().lock();

    if let Ok(scancode) = unsafe { kbd_controller.read_data() } {
        let mut kbd = KEYBOARD.lock();
        if let Ok(Some(event)) = kbd.add_byte(scancode) {
            return kbd.process_keyevent(event);
        }
    }
    None
}

// pub fn keyboard() -> &'static Mutex<Keyboard> {
//     if KEYBOARD.is_completed() {
//         KEYBOARD.wait()
//     } else {
//         panic!("ps/2 devices are not initialized.")
//     }
// }

pub fn init(_keyboard_enabled: bool, mouse_enabled: bool) {
    // https://wiki.osdev.org/%228042%22_PS/2_Controller#Initialising%20the%20PS/2%20Controller

    let mut controller = Controller::new();

    // Step 3: Disable Devices
    controller.disable_first_port();
    controller.disable_second_port();

    // Step 4: Flush The Output Buffer
    unsafe { controller.flush_data_port() };

    // Step 5: Set the Controller Configuration Byte
    let mut config_byte = controller
        .read_config_byte()
        .expect("Failed to read the PS/2 controller config byte.");
    // disable IRQs and translation for port 1 by clearing bits 0 and 6.
    config_byte.set_first_port_interrupt(false);
    config_byte.set_first_port_translation(false);
    // check that the clock signal is enabled by clearing bit 4.
    config_byte.set_first_port_clock(false);
    controller
        .write_config_byte(config_byte)
        .expect("Failed to write to the PS/2 controller config byte.");

    // Step 6: Perform Controller Self Test
    controller
        .test_controller()
        .expect("Test for PS/2 Controller failed.");
    // This can reset the PS/2 controller on some hardware.
    // At the very least, the Controller Configuration Byte should be restored
    // // for compatibility with such hardware.
    // restore the value read before issuing 0xAA (self test).
    controller
        .write_config_byte(config_byte)
        .expect("Failed to write to the PS/2 controller config byte.");

    // Step 7: Determine If There Are 2 Channels
    let has_second_port = {
        // enable the second PS/2 port
        controller.enable_second_port();

        // read the Controller Configuration Byte
        let mut config_byte = controller
            .read_config_byte()
            .expect("Failed to read the PS/2 controller config byte.");

        let has_second_port = !config_byte.get_second_port_clock();
        if has_second_port {
            info!("second port is supported.");
            // if the controller has a dual channel.
            // disable the second PS/2 port again
            controller.disable_second_port();

            // clear bits 1 and 5 of the Controller Configuration Byte to disable IRQs and enable the clock for port 2
            config_byte.set_second_port_interrupt(false);
            config_byte.set_second_port_clock(false);

            controller
                .write_config_byte(config_byte)
                .expect("Failed to write to the PS/2 controller config byte.");
        }

        has_second_port
    };

    // Step 8: Perform Interface Tests
    // test the first PS/2 port
    let first_port_works = controller.test_first_port().is_ok();
    // test the second PS/2 port
    let second_port_works = has_second_port && controller.test_second_port().is_ok();

    // Step 9: Enable Devices
    // enable any usable PS/2 port that exists and interrupts for any usable PS/2 ports
    let mut config_byte = controller
        .read_config_byte()
        .expect("Failed to read the PS/2 controller config byte.");
    if first_port_works {
        controller.enable_first_port();
        config_byte.set_first_port_interrupt(true);
    }
    if second_port_works {
        controller.enable_second_port();
        config_byte.set_second_port_interrupt(true);
    }
    controller
        .write_config_byte(config_byte)
        .expect("Failed to write to the PS/2 controller config byte.");

    // Step 10: Reset Devices
    let mut keyboard = Keyboard::new();
    let mut mouse = Mouse::new();

    unsafe { keyboard.reset_and_self_test() }
        .unwrap_or_else(|err| panic!("failed to reset the keyboard: {:?}", err));
    unsafe { mouse.reset_and_self_test() }
        .unwrap_or_else(|err| panic!("failed to reset the mouse: {:?}", err));

    // enable mouse's data-reporting
    if second_port_works && mouse_enabled {
        unsafe { mouse.enable_data_reporting() }.unwrap_or_else(|err| {
            panic!("failed to enable data-reporting of the mouse: {:?}", err)
        });
    }

    KEYBOARD.call_once(|| Mutex::new(keyboard));
    MOUSE.call_once(|| Mutex::new(mouse));
}
