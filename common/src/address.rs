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

#[derive(Clone, Copy, Debug)]
pub struct AlignedAddress<const SIZE: usize>(u64);
impl<const SIZE: usize> AlignedAddress<SIZE> {
    pub fn new(addr: u64) -> Result<Self> {
        if addr as usize % SIZE == 0 {
            Ok(Self(addr))
        } else {
            Err(AddressError::AddressNotAlignedError.into())
        }
    }

    pub fn get(&self) -> u64 {
        self.0
    }
}

pub type AlignedAddress64 = AlignedAddress<64>;

// #[derive(Clone, Copy, Debug)]
// pub struct AlignedAddress64(u64);
// impl AlignedAddress64 {
//     pub fn new(addr: u64) -> Result<Self> {
//         if addr % 64 == 0 {
//             Ok(Self(addr))
//         } else {
//             Err(AddressError::AddressNotAlignedError.into())
//         }
//     }

//     pub fn get()
// }
