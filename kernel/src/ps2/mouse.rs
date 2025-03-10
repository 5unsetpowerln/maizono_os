pub(crate) const EVENT_BUFFER_LENGTH: usize = 128;

use super::{
    controller::{Controller, ControllerError},
    keyboard::Response,
};

type Result<T> = core::result::Result<T, MouseError>;

#[derive(Debug)]
pub(crate) enum MouseError {
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
    EnableDataReporting = 0xf4,
    ResetAndSelfTest = 0xff,
}

impl Command {
    fn as_u8(self) -> u8 {
        self as u8
    }
}

pub(crate) struct MouseEvent {
    y_overflow: bool,
    x_overflow: bool,
    y_sign: bool,
    x_sign: bool,
    button_middle: bool,
    button_right: bool,
    button_left: bool,
    x_offset: u8,
    y_offset: u8,
}

impl MouseEvent {
    fn new(data0: u8, data1: u8, data2: u8) -> Self {
        let y_overflow = data0 & 1 == 1;
        let x_overflow = (data0 >> 1) & 1 == 1;
        let y_sign = (data0 >> 2) & 1 == 1;
        let x_sign = (data0 >> 3) & 1 == 1;
        let button_middle = (data0 >> 5) & 1 == 1;
        let button_right = (data0 >> 6) & 1 == 1;
        let button_left = (data0 >> 7) & 1 == 1;

        Self {
            y_overflow,
            x_overflow,
            y_sign,
            x_sign,
            button_middle,
            button_right,
            button_left,
            x_offset: data1,
            y_offset: data2,
        }
    }
}

pub(crate) struct Mouse {
    controller: Controller,
}

impl Mouse {
    pub(crate) fn new() -> Self {
        Self {
            controller: Controller::new(),
        }
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

    pub(crate) unsafe fn enable_data_reporting(&mut self) -> Result<()> {
        unsafe { self.write_command(Command::EnableDataReporting, None) }?;
        Ok(())
    }

    pub(crate) unsafe fn reset_and_self_test(&mut self) -> Result<u8> {
        unsafe { self.write_command(Command::ResetAndSelfTest, None)? };

        let test_result = unsafe { self.read_response() }?;
        match test_result {
            Response::SelfTestPassed => {
                let id = unsafe { self.read_data() }?;
                Ok(id)
            }
            Response::SelfTestFailed1 => Err(MouseError::SelfTestFailed),
            Response::SelfTestFailed2 => Err(MouseError::SelfTestFailed),
            _ => Err(MouseError::InvalidResponse),
        }
    }

    pub unsafe fn receive_events(&mut self, handler: fn(u8, u8, u8)) -> Result<()> {
        let mut count = 0;
        let mut buffer = [0; 3];
        while unsafe { self.controller.read_status() }.is_output_full()
            && count < self.controller.loop_timeout
        {
            buffer[count % 3] = unsafe { self.read_data() }?;
            if count % 3 == 1 {
                handler(buffer[0], buffer[1], buffer[2]);
            }
            count += 1;
        }

        Ok(())
    }
}
