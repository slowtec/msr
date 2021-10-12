use crate::time::SystemTimeInstantError;
use std::result::Result as StdResult;
use thiserror::Error;

pub mod policy;

#[cfg(feature = "csv-storage")]
pub mod csv;

#[derive(Error, Debug)]
pub enum Error {
    #[error("timing error")]
    Timing(SystemTimeInstantError),

    #[cfg(feature = "csv-storage")]
    #[error("CSV format error")]
    Csv(::csv::Error),
}

impl From<SystemTimeInstantError> for Error {
    fn from(from: SystemTimeInstantError) -> Self {
        Self::Timing(from)
    }
}

#[cfg(feature = "csv-storage")]
impl From<::csv::Error> for Error {
    fn from(from: ::csv::Error) -> Self {
        Self::Csv(from)
    }
}

pub type Result<T> = StdResult<T, Error>;

#[derive(Error, Debug, Eq, PartialEq)]
pub enum WriteError {
    #[error("no file available for writing")]
    NoFile,

    #[error("writing repeatedly failed with OS error code {code:}")]
    RepeatedOsError { code: i32 },
}

pub type WriteResult = StdResult<(), WriteError>;
