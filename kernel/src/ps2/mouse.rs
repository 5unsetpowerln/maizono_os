use spin::Mutex;
use x86_64::instructions::port::{Port, PortWriteOnly};

use super::{
    controller::{Controller, ControllerError},
    keyboard::Response,
};

type Result<T> = core::result::Result<T, MouseError>;

#[derive(Debug)]
pub enum MouseError {
    ControllerError(ControllerError),
    CommandNotAcknowledged(Response),
    InvalidResponse,
    SelfTestFailed,
}

impl From<ControllerError> for MouseError {
    fn from(err: ControllerError) -> Self {
        MouseError::ControllerError(err)
    }
}

#[derive(Debug)]
enum Command {
    ResetAndSelfTest = 0xff,
}

impl Command {
    fn as_u8(self) -> u8 {
        self as u8
    }
}

pub struct Mouse<'a> {
    controller: &'a mut Controller,
}

impl<'a> Mouse<'a> {
    pub fn new(controller: &'a mut Controller) -> Self {
        Self { controller }
    }

    unsafe fn read_response(&mut self) -> Result<Response> {
        let response = unsafe { self.controller.read_data() }?;
        Ok(Response::from_u8(response))
    }

    unsafe fn read_data(&mut self) -> Result<u8> {
        let data = unsafe { self.controller.read_data() }?;
        Ok(data)
    }

    unsafe fn write_command(&mut self, command: Command, data: Option<u8>) -> Result<()> {
        // write command
        unsafe {
            self.controller
                .write_to_second_port_input_buffer(command.as_u8())
        }?;

        // check response
        let response = unsafe { self.read_response() }?;
        if !matches!(response, Response::Acknowledged) {
            return Err(MouseError::CommandNotAcknowledged(response));
        }

        if let Some(data) = data {
            // write data
            unsafe { self.controller.write_to_second_port_input_buffer(data) }?;

            // check response
            let response = unsafe { self.read_response() }?;
            if !matches!(response, Response::Acknowledged) {
                return Err(MouseError::CommandNotAcknowledged(response));
            }
        }

        Ok(())
    }

    pub unsafe fn reset_and_self_test(&mut self) -> Result<u8> {
        unsafe { self.write_command(Command::ResetAndSelfTest, None)? };

        let test_result = unsafe { self.read_response() }?;
        return match test_result {
            Response::SelfTestPassed => {
                let id = unsafe { self.read_data() }?;
                Ok(id)
            }
            Response::SelfTestFailed1 => Err(MouseError::SelfTestFailed),
            Response::SelfTestFailed2 => Err(MouseError::SelfTestFailed),
            _ => Err(MouseError::InvalidResponse),
        };
    }
}
