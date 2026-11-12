//! [`VarName`], the name of an environment variable a config may point at.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::TypesError;

/// The longest accepted name: one leading character plus up to 127 more.
const MAX_LEN: usize = 128;

/// The name of an environment variable a config may point at (a local runner's `key_env`).
///
/// It matches `^[A-Z_][A-Z0-9_]{0,127}$`. This crate holds no credential variable name
/// (REQ-SAG-0401); sc-agent-secrets and sc-agent-config share this type (REQ-SAG-0013).
///
/// # Examples
///
/// ```
/// use sc_agent_types::VarName;
///
/// let name: VarName = "MY_RUNNER_TOKEN".parse().unwrap();
/// assert_eq!(name.as_str(), "MY_RUNNER_TOKEN");
/// assert!("lower".parse::<VarName>().is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VarName(String);

impl VarName {
    /// The variable name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for VarName {
    type Err = TypesError;

    /// Parses `^[A-Z_][A-Z0-9_]{0,127}$`.
    ///
    /// # Errors
    ///
    /// [`TypesError::InvalidVarName`] for any other text.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut bytes = text.bytes();
        let valid = text.len() <= MAX_LEN
            && bytes
                .next()
                .is_some_and(|first| first.is_ascii_uppercase() || first == b'_')
            && bytes.all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_');
        if valid {
            Ok(Self(text.to_owned()))
        } else {
            Err(TypesError::InvalidVarName {
                text: text.to_owned(),
            })
        }
    }
}

impl fmt::Display for VarName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl AsRef<str> for VarName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Serialize for VarName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for VarName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}
