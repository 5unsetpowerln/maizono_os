use super::controller::{Controller, ControllerError};

type Result<T> = core::result::Result<T, KeyboardError>;

#[derive(Debug)]
pub enum KeyboardError {
    ControllerError(ControllerError),
    CommandNotAcknowledged(Response),
    SelfTestFailed,
    InvalidResponse,
}

impl From<ControllerError> for KeyboardError {
    fn from(err: ControllerError) -> Self {
        KeyboardError::ControllerError(err)
    }
}

#[derive(Debug)]
pub enum Response {
    // Key detection error or internal buffer overrun
    InternalBufferOverrun = 0x00,

    // Self test passed (sent after "0xFF (reset)" command or keyboard power up)
    SelfTestPassed = 0xaa,

    // Response to "0xEE (echo)" command
    ResponseToEcho = 0xee,

    // Command acknowledged (ACK)
    Acknowledged = 0xfa,

    // Self test failed (sent after "0xFF (reset)" command or keyboard power up)
    SelfTestFailed1 = 0xfc,
    SelfTestFailed2 = 0xfd,

    // Resend (keyboard wants controller to repeat last command it sent)
    Resend = 0xfe,

    // Key detection error or internal buffer overrun
    KeyDetectionErrorOrInteralBufferOverrun = 0xff,
}

impl Response {
    pub fn from_u8(code: u8) -> Self {
        if code == Self::InternalBufferOverrun.as_u8() {
            return Self::InternalBufferOverrun;
        } else if code == Self::SelfTestPassed.as_u8() {
            return Self::SelfTestPassed;
        } else if code == Self::ResponseToEcho.as_u8() {
            return Self::ResponseToEcho;
        } else if code == Self::Acknowledged.as_u8() {
            return Self::Acknowledged;
        } else if code == Self::SelfTestFailed1.as_u8() {
            return Self::SelfTestFailed1;
        } else if code == Self::SelfTestFailed2.as_u8() {
            return Self::SelfTestFailed2;
        } else if code == Self::Resend.as_u8() {
            return Self::Resend;
        } else if code == Self::KeyDetectionErrorOrInteralBufferOverrun.as_u8() {
            return Self::KeyDetectionErrorOrInteralBufferOverrun;
        } else {
            panic!("ps/2 keyboard responded invalid response: 0x{:X}", code);
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
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
        return Ok(Response::from_u8(response));
    }

    unsafe fn write_command(&mut self, command: Command, data: Option<u8>) -> Result<()> {
        // write command
        unsafe { self.controller.write_data(command.as_u8()) }?;

        // check response
        let response = unsafe { self.read_response() }?;
        if !matches!(response, Response::Acknowledged) {
            return Err(KeyboardError::CommandNotAcknowledged(response));
        }

        if let Some(data) = data {
            // write data
            unsafe { self.controller.write_data(data) }?;
            // check response
            let response = unsafe { self.read_response() }?;
            if !matches!(response, Response::Acknowledged) {
                return Err(KeyboardError::CommandNotAcknowledged(response));
            }
        }
        return Ok(());
    }

    pub unsafe fn reset_and_self_test(&mut self) -> Result<()> {
        unsafe { self.write_command(Command::ResetAndSelfTest, None) }?;
        let response = unsafe { self.read_response() }?;

        return match response {
            Response::SelfTestPassed => Ok(()),
            Response::SelfTestFailed1 => Err(KeyboardError::SelfTestFailed),
            Response::SelfTestFailed2 => Err(KeyboardError::SelfTestFailed),
            _ => Err(KeyboardError::InvalidResponse),
        };
    }

    pub unsafe fn read_data(&mut self) -> Result<u8> {
        return Ok(unsafe { self.controller.read_data()? });
    }
}
