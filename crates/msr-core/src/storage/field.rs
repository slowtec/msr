use std::fmt;

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

impl fmt::Display for ScalarField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { name, unit, r#type } = self;
        debug_assert!(
            r#type.is_none() || !r#type.unwrap().to_string().contains(FIELD_TYPE_SEPARATOR)
        );
        match (unit, r#type) {
            // "<name>"
            (None, None) => f.write_str(name),
            // "<name>[<unit>]"
            (Some(unit), None) => write!(f, "{name}{FIELD_UNIT_PREFIX}{unit}{FIELD_UNIT_SUFFIX}"),
            // "<name>.<type>"
            (None, Some(r#type)) => write!(f, "{name}{FIELD_TYPE_SEPARATOR}{type}"),
            // "<name>[<unit>].<type>"
            (Some(unit), Some(r#type)) => write!(
                f,
                "{name}{FIELD_UNIT_PREFIX}{unit}{FIELD_UNIT_SUFFIX}{FIELD_TYPE_SEPARATOR}{type}"
            ),
        }
    }
}
