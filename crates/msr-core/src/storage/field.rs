use std::{fmt, str::FromStr};

use thiserror::Error;

use crate::ScalarType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarField {
    pub name: String,
    pub unit: Option<String>,
    pub r#type: Option<ScalarType>,
}

const FIELD_UNIT_PREFIX: &str = "[";
const FIELD_UNIT_SUFFIX: &str = "]";
const FIELD_TYPE_SEPARATOR: &str = ".";

// Format: "<name>[<type>].<unit>"
impl fmt::Display for ScalarField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { name, unit, r#type } = self;
        let type_part = r#type
            .as_ref()
            .map(|t| format!("{}{}", FIELD_TYPE_SEPARATOR, t))
            .unwrap_or_default();
        let unit_part = unit
            .as_ref()
            .map(|u| format!("{}{}{}", FIELD_UNIT_PREFIX, u, FIELD_UNIT_SUFFIX))
            .unwrap_or_default();
        write!(f, "{}{}{}", name, unit_part, type_part)
    }
}

#[derive(Error, Debug)]
pub enum ScalarFieldParseError {
    //#[error("unknown error")]
//Unknown,
}

impl FromStr for ScalarField {
    type Err = ScalarFieldParseError;

    fn from_str(_s: &str) -> Result<Self, Self::Err> {
        unimplemented!("TODO: impl FromStr for ScalarField")
    }
}
