pub(crate) const EVENT_BUFFER_LENGTH: usize = 128;

use glam::I64Vec2;
use x86_64::structures::idt::InterruptStackFrame;

use crate::device::ps2;
use crate::{interrupts, kprintln, message, mouse};

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

pub struct MouseEvent {
    pub y_overflow: bool,
    pub x_overflow: bool,
    pub y_sign: bool,
    pub x_sign: bool,
    pub button_middle: bool,
    pub button_right: bool,
    pub button_left: bool,
    pub x_offset: i8,
    pub y_offset: i8,
}

impl MouseEvent {
    fn new(data0: u8, data1: u8, data2: u8) -> Self {
        let button_left = data0 & 1 == 1;
        let button_right = (data0 >> 1) & 1 == 1;
        let button_middle = (data0 >> 2) & 1 == 1;
        let x_sign = (data0 >> 4) & 1 == 1;
        let y_sign = (data0 >> 5) & 1 == 1;
        let x_overflow = (data0 >> 6) & 1 == 1;
        let y_overflow = (data0 >> 7) & 1 == 1;

        Self {
            y_overflow,
            x_overflow,
            y_sign,
            x_sign,
            button_middle,
            button_right,
            button_left,
            x_offset: data1 as i8,
            y_offset: data2 as i8,
        }
    }
}

impl From<MouseEvent> for mouse::MouseEvent {
    fn from(event: MouseEvent) -> mouse::MouseEvent {
        let dx = if !event.x_sign {
            (event.x_offset + if event.x_overflow { 1 } else { 0 } * 127) as i64
        } else {
            (event.x_offset + if event.x_overflow { 1 } else { 0 } * -127) as i64
        };

        let mut dy = if !event.y_sign {
            (event.y_offset + if event.y_overflow { 1 } else { 0 } * 127) as i64
        } else {
            (event.y_offset + if event.y_overflow { 1 } else { 0 } * -127) as i64
        };
        dy = -dy;

        if event.button_left {
            return mouse::MouseEvent::LeftClick;
        }
        if event.button_middle {
            return mouse::MouseEvent::MiddleClick;
        }
        if event.button_right {
            return mouse::MouseEvent::RightClick;
        }

        mouse::MouseEvent::Move {
            displacement: I64Vec2::new(dx, dy),
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

    // pub unsafe fn receive_events(&mut self) -> Result<mouse::MouseEvent> {
    //     let mut buffer = [0; 3];
    //     for i in 0..3 {
    //         buffer[i] = unsafe { self.read_data() }?;
    //         kprintln!("receive_events: received data[{}]", i);
    //     }
    //     let event = MouseEvent::new(buffer[0], buffer[1], buffer[2]);

    //     Ok(event.into())
    // }

    pub unsafe fn receive_events(&mut self) -> Result<mouse::MouseEvent> {
        let mut buffer = [0; 3];
        for i in 0..3 {
            buffer[i] = unsafe { self.read_data() }?;
        }
        let event = MouseEvent::new(buffer[0], buffer[1], buffer[2]);

        Ok(mouse::MouseEvent::from(event))
    }
}

pub extern "x86-interrupt" fn interrupt_handler(_stack_frame: InterruptStackFrame) {
    message::enqueue(message::Message::PS2MouseInterrupt);
    interrupts::notify_end_of_interrupt();
}
