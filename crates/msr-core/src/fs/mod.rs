use std::result::Result as StdResult;

use thiserror::Error;

pub mod policy;

#[cfg(feature = "with-csv-storage")]
pub mod csv;

#[derive(Error, Debug)]
pub enum Error {
    #[error("timing error")]
    Timing,

    #[cfg(feature = "with-csv-storage")]
    #[error("CSV format error")]
    Csv(::csv::Error),
}

#[cfg(feature = "with-csv-storage")]
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
