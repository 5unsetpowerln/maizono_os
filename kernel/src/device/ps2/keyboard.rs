use log::debug;
use x86_64::{instructions::interrupts::without_interrupts, structures::idt::InterruptStackFrame};

use crate::{
    device::ps2::{self, InterpretableResponse, KEYBOARD_CONTROLLER, Response, read_key_event},
    interrupts, kprint, message, task,
};

use super::controller::{Controller, ControllerError};

// use pc_keyboard::{DecodedKey, HandleControl, ScancodeSet1, layouts};

type Result<T> = core::result::Result<T, KeyboardError>;

#[derive(Debug, Clone, Copy)]
pub enum KeyboardError {
    ControllerError(ControllerError),
    CommandNotAcknowledged(Response),
    SelfTestFailed,
    InvalidResponse,
    ScanCodeNotSet,
}

impl From<ControllerError> for KeyboardError {
    fn from(err: ControllerError) -> Self {
        KeyboardError::ControllerError(err)
    }
}

#[derive(Debug)]
enum Command {
    GetOrSetCurrentScanCodeSet = 0xF0,
    ResetAndSelfTest = 0xff,
}

impl Command {
    fn as_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Debug)]
pub enum ScanCode {
    ScanCode1 = 1,
    ScanCode2 = 2,
    ScanCode3 = 3,
}

impl ScanCode {
    fn as_u8(self) -> u8 {
        self as u8
    }
}

pub struct Keyboard {
    controller: Controller,
}

impl Keyboard {
    pub fn new() -> Self {
        Self {
            controller: Controller::new(),
        }
    }

    unsafe fn read_response(&mut self) -> Result<Response> {
        let response = unsafe { self.controller.read_data() }?;
        Ok(Response::from_u8(response))
    }

    unsafe fn write_command(&mut self, command: Command, data: Option<u8>) -> Result<()> {
        // write command
        unsafe { self.controller.write_data(command.as_u8()) }?;

        // check response
        let response = unsafe { self.read_response() }?;

        if !matches!(
            response,
            Response::Interpretable(InterpretableResponse::Acknowledged)
        ) {
            return Err(KeyboardError::CommandNotAcknowledged(response));
        }

        if let Some(data) = data {
            // write data
            unsafe { self.controller.write_data(data) }?;

            // check response
            if !matches!(
                response,
                Response::Interpretable(InterpretableResponse::Acknowledged)
            ) {
                return Err(KeyboardError::CommandNotAcknowledged(response));
            }
        }
        return Ok(());
    }

    pub unsafe fn get_current_scan_code(&mut self) -> Result<ScanCode> {
        unsafe { self.write_command(Command::GetOrSetCurrentScanCodeSet, Some(0)) }?;

        let response = unsafe { self.read_response() }?;

        if matches!(
            response,
            Response::Interpretable(InterpretableResponse::Acknowledged)
        ) {
            let response = unsafe { self.read_response() }?;

            if let Response::Other(other_resp) = response {
                match other_resp {
                    1 => return Ok(ScanCode::ScanCode1),
                    2 => return Ok(ScanCode::ScanCode2),
                    3 => return Ok(ScanCode::ScanCode3),
                    _ => {}
                }
            }

            panic!("Invalid scan code number.");
        } else {
            panic!("ACK is not returned.")
        }
    }

    pub unsafe fn set_scan_code(&mut self, scan_code_type: ScanCode) -> Result<()> {
        unsafe {
            self.write_command(
                Command::GetOrSetCurrentScanCodeSet,
                Some(scan_code_type.as_u8()),
            )
        }?;

        let response = unsafe { self.read_response() }?;

        if matches!(
            response,
            Response::Interpretable(InterpretableResponse::Acknowledged)
        ) {
            return Ok(());
        }

        panic!("ACK is not returned.")
    }

    pub unsafe fn reset_and_self_test(&mut self) -> Result<()> {
        unsafe { self.write_command(Command::ResetAndSelfTest, None) }?;
        let response = unsafe { self.read_response() }?;

        if let Response::Interpretable(i_resp) = response {
            match i_resp {
                InterpretableResponse::SelfTestPassed => Ok(()),
                InterpretableResponse::SelfTestFailed1 => Err(KeyboardError::SelfTestFailed),
                InterpretableResponse::SelfTestFailed2 => Err(KeyboardError::SelfTestFailed),
                _ => Err(KeyboardError::InvalidResponse),
            }
        } else {
            Err(KeyboardError::InvalidResponse)
        }
    }

    pub unsafe fn read_data(&mut self) -> Result<u8> {
        Ok(unsafe { self.controller.read_data()? })
    }
}

pub extern "x86-interrupt" fn interrupt_handler(_stack_frame: InterruptStackFrame) {
    let result = unsafe { ps2::KEYBOARD_CONTROLLER.wait().lock().read_data() };

    without_interrupts(|| {
        task::TASK_MANAGER
            .wait()
            .lock()
            .send_message_to_task(1, &message::Message::PS2KeyboardInterrupt(result))
            .expect("Failed to send a message to main task.");
    }); // lock is dropped here.

    interrupts::notify_end_of_interrupt();
}
