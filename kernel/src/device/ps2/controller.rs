use x86_64::instructions::port::{Port, PortReadOnly, PortWriteOnly};

use crate::kprintln;

// references:
// https://wiki.osdev.org/%228042%22_PS/2_Controller

type Result<T> = core::result::Result<T, ControllerError>;

pub(crate) const LOOP_TIMEOUT: usize = 1000000;

#[derive(Debug)]
pub(crate) enum ControllerError {
    Timeout,
    TestFailed,
}

#[derive(Debug)]
pub(crate) struct Controller {
    data_port: Port<u8>,
    status_port: PortReadOnly<u8>,
    command_port: PortWriteOnly<u8>,
    pub(crate) loop_timeout: usize,
}

impl Controller {
    pub(crate) fn new() -> Self {
        let data_port = Port::new(0x60);
        let command_port = PortWriteOnly::new(0x64);
        let status_port = PortReadOnly::new(0x64);

        Self {
            data_port,
            status_port,
            command_port,
            loop_timeout: LOOP_TIMEOUT,
        }
    }

    unsafe fn write_command(&mut self, command: Command) {
        unsafe {
            self.command_port.write(command.as_u8());
        }
    }

    pub(crate) unsafe fn write_data(&mut self, data: u8) -> Result<()> {
        unsafe { self.wait_for_write()? };
        unsafe {
            self.data_port.write(data);
        }
        Ok(())
    }

    pub(crate) unsafe fn read_status(&mut self) -> ControllerStatus {
        ControllerStatus::from_u8(unsafe { self.status_port.read() })
    }

    pub(crate) unsafe fn read_data(&mut self) -> Result<u8> {
        unsafe { self.wait_for_read()? };
        Ok(unsafe { self.data_port.read() })
    }

    unsafe fn wait_for_read(&mut self) -> Result<()> {
        let mut count = 0;
        while count < self.loop_timeout {
            if unsafe { self.read_status().is_output_full() } {
                return Ok(());
            }
            count += 1;
        }
        Err(ControllerError::Timeout)
    }

    unsafe fn wait_for_write(&mut self) -> Result<()> {
        let mut count = 0;
        while count < self.loop_timeout {
            if unsafe { !self.read_status().is_input_full() } {
                return Ok(());
            }
            count += 1;
        }
        Err(ControllerError::Timeout)
    }

    pub(crate) unsafe fn flush_data_port(&mut self) {
        // Bit 0: Output buffer status (0 = empty, 1 = full)
        let mut count = 0;
        while unsafe { self.read_status() }.is_output_full() && count < self.loop_timeout {
            let _ = unsafe { self.read_data() };
            count += 1;
        }
    }

    pub(crate) unsafe fn flush_data_port_debug(&mut self, handler: fn(usize, u8)) {
        // Bit 0: Output buffer status (0 = empty, 1 = full)
        let mut count = 0;
        while unsafe { self.read_status() }.is_output_full() && count < self.loop_timeout {
            let data = unsafe { self.read_data() };
            handler(count, data.unwrap_or_else(|e| panic!("{:?}", e)));
            count += 1;
        }
    }

    pub(crate) unsafe fn write_to_second_port_ouput_buffer(&mut self, data: u8) -> Result<()> {
        unsafe {
            self.write_command(Command::WriteSecondPortOutputBuffer);
            self.write_data(data)?;
        }
        Ok(())
    }

    pub(crate) unsafe fn write_to_second_port_input_buffer(&mut self, data: u8) -> Result<()> {
        unsafe {
            self.write_command(Command::WriteSecondPortInputBuffer);
            self.write_data(data)?;
        }
        Ok(())
    }

    // Patial implementations for commands enumerated at https://wiki.osdev.org/%228042%22_PS/2_Controller#Command_Register.
    /// Disable first PS/2 port
    pub(crate) fn disable_first_port(&mut self) {
        unsafe {
            self.write_command(Command::DisableFirstPort);
        }
    }

    /// Disable second PS/2 port (only if 2 PS/2 ports supported)
    pub(crate) fn disable_second_port(&mut self) {
        unsafe {
            self.write_command(Command::DisableSecondPort);
        }
    }

    pub(crate) fn read_config_byte(&mut self) -> Result<ControllerConfigByte> {
        // Read "byte 0" from internal RAM
        unsafe { self.write_command(Command::ReadControllerConfigByte) };
        Ok(ControllerConfigByte::from_u8(unsafe { self.read_data() }?))
    }

    pub(crate) fn write_config_byte(&mut self, config_byte: ControllerConfigByte) -> Result<()> {
        // Write next byte to "byte 0" of internal RAM
        unsafe {
            self.write_command(Command::WriteControllerConfigByte);
            self.write_data(config_byte.get())?;
        };
        Ok(())
    }

    pub(crate) fn test_controller(&mut self) -> Result<()> {
        unsafe { self.write_command(Command::TestController) };
        let response = unsafe { self.read_data() }?;

        if response == 0x55 {
            Ok(())
        } else {
            Err(ControllerError::TestFailed)
        }
    }

    pub(crate) fn enable_second_port(&mut self) {
        unsafe { self.write_command(Command::EnableSecondPort) }
    }

    pub(crate) fn test_first_port(&mut self) -> Result<()> {
        unsafe { self.write_command(Command::TestFirstPort) };
        let response = unsafe { self.read_data() }?;

        if response == 0x00 {
            Ok(())
        } else {
            Err(ControllerError::TestFailed)
        }
    }

    pub(crate) fn test_second_port(&mut self) -> Result<()> {
        unsafe { self.write_command(Command::TestSecondPort) };
        let response = unsafe { self.read_data() }?;

        if response == 0x00 {
            Ok(())
        } else {
            Err(ControllerError::TestFailed)
        }
    }

    pub(crate) fn enable_first_port(&mut self) {
        unsafe { self.write_command(Command::EnableFirstPort) };
    }
}

enum Command {
    ReadControllerConfigByte = 0x20,
    WriteControllerConfigByte = 0x60,
    DisableSecondPort = 0xa7,
    EnableSecondPort = 0xa8,
    TestSecondPort = 0xa9,
    TestController = 0xaa,
    TestFirstPort = 0xab,
    DisableFirstPort = 0xad,
    EnableFirstPort = 0xae,
    WriteSecondPortOutputBuffer = 0xd3,
    WriteSecondPortInputBuffer = 0xd4,
}
impl Command {
    fn as_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ControllerConfigByte(u8);
impl ControllerConfigByte {
    pub(crate) fn get(self) -> u8 {
        self.0
    }

    pub(crate) fn from_u8(value: u8) -> Self {
        Self(value)
    }

    fn get_bit(&self, bit: u8) -> bool {
        self.0 & (1 << bit) != 0
    }

    fn set_bit(&mut self, bit: u8, value: bool) {
        if value {
            self.0 |= 1 << bit;
        } else {
            self.0 &= !(1 << bit)
        }
    }

    /// Bit 0: First PS/2 port interrupt (1 = enabled, 0 = disabled)
    pub(crate) fn get_first_port_interrupt(&self) -> bool {
        self.get_bit(0)
    }

    /// Bit 0: First PS/2 port interrupt (1 = enabled, 0 = disabled)
    pub(crate) fn set_first_port_interrupt(&mut self, value: bool) {
        self.set_bit(0, value);
    }

    /// Bit 1: Second PS/2 port interrupt (1 = enabled, 0 = disabled, only if 2 PS/2 ports supported)
    pub(crate) fn get_second_port_interrupt(&self) -> bool {
        self.get_bit(1)
    }

    /// Bit 1: Second PS/2 port interrupt (1 = enabled, 0 = disabled, only if 2 PS/2 ports supported)
    pub(crate) fn set_second_port_interrupt(&mut self, value: bool) {
        self.set_bit(1, value);
    }

    /// Bit 4: First PS/2 port clock (1 = disabled, 0 = enabled)
    pub(crate) fn get_first_port_clock(&self) -> bool {
        self.get_bit(4)
    }

    /// Bit 4: First PS/2 port clock (1 = disabled, 0 = enabled)
    pub(crate) fn set_first_port_clock(&mut self, value: bool) {
        self.set_bit(4, value);
    }

    /// Bit 5: Second PS/2 port clock (1 = disabled, 0 = enabled, only if 2 PS/2 ports supported)
    pub(crate) fn get_second_port_clock(&self) -> bool {
        self.get_bit(5)
    }

    /// Bit 5: Second PS/2 port clock (1 = disabled, 0 = enabled, only if 2 PS/2 ports supported)
    pub(crate) fn set_second_port_clock(&mut self, value: bool) {
        self.set_bit(5, value);
    }

    /// Bit 6: First PS/2 port translation (1 = enabled, 0 = disabled)
    pub(crate) fn get_first_port_translation(&self) -> bool {
        self.get_bit(6)
    }

    /// Bit 6: First PS/2 port translation (1 = enabled, 0 = disabled)
    pub(crate) fn set_first_port_translation(&mut self, value: bool) {
        self.set_bit(6, value);
    }
}

#[derive(Debug)]
pub(crate) struct ControllerStatus(u8);
impl ControllerStatus {
    pub(crate) fn get(self) -> u8 {
        self.0
    }

    pub(crate) fn from_u8(value: u8) -> Self {
        Self(value)
    }

    pub(crate) fn get_bit(&self, bit: u8) -> bool {
        self.0 & (1 << bit) != 0
    }

    pub(crate) fn is_output_full(&self) -> bool {
        self.get_bit(0)
    }

    pub(crate) fn is_input_full(&self) -> bool {
        self.get_bit(1)
    }
}
