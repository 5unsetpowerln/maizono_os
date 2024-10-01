use core::fmt::Display;

use crate::{address::AddressError, graphic::GraphicError};

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    GraphicError(GraphicError),
    AddressError(AddressError),
}

impl Error {
    pub fn msg(&self) -> &'static str {
        match &self {
            Self::GraphicError(graphic_err) => graphic_err.msg(),
            Self::AddressError(address_err) => address_err.msg(),
        }
    }
}

impl From<GraphicError> for Error {
    fn from(err: GraphicError) -> Self {
        Self::GraphicError(err)
    }
}

impl From<AddressError> for Error {
    fn from(err: AddressError) -> Self {
        Self::AddressError(err)
    }
}

pub type Result<T> = core::result::Result<T, Error>;
