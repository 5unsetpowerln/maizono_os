use core::fmt::Display;

use crate::graphic::GraphicError;

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    GraphicError(GraphicError),
}

impl Error {
    pub fn msg(&self) -> &'static str {
        match &self {
            Self::GraphicError(graphic_err) => graphic_err.msg(),
        }
    }
}

impl From<GraphicError> for Error {
    fn from(err: GraphicError) -> Self {
        Self::GraphicError(err)
    }
}

pub type Result<T> = core::result::Result<T, Error>;
