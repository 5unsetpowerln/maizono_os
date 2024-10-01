use crate::error::Result;

#[derive(Debug, Clone, PartialEq)]
pub enum AddressError {
    AddressNotAlignedError,
}
impl AddressError {
    pub fn msg(&self) -> &'static str {
        match *self {
            Self::AddressNotAlignedError => "The address is not 64 byte aligned.",
        }
    }
}

pub struct AlignedAddress<const SIZE: usize>(u64);
impl<const SIZE: usize> AlignedAddress<SIZE> {
    pub fn new(addr: u64) -> Result<Self> {
        if addr % 64 == 0 {
            Ok(Self(addr))
        } else {
            Err(AddressError::AddressNotAlignedError.into())
        }
    }

    pub fn get(&self) -> u64 {
        self.0
    }
}
