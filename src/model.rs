use serde::de::Error as SerdeError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::convert::TryFrom;
use std::fmt;
use thiserror::*;

#[derive(Debug, Error)]
pub enum NameError {
    #[error("Invalid port full name.")]
    InvalidFullname,

    #[error("Port name too long.")]
    PortnameTooLong,

    #[error("Client name too long.")]
    ClientnameTooLong,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct PortFullname<T: AsRef<str> = String> {
    buffer: T,
    split_idx: usize,
}

impl<'de, T: AsRef<str> + Deserialize<'de>> Deserialize<'de> for PortFullname<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let buffer = T::deserialize(deserializer)?;
        let res = Self::new(buffer).map_err(SerdeError::custom)?;
        Ok(res)
    }
}

impl<T: AsRef<str>> Serialize for PortFullname<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.buffer.as_ref())
    }
}

impl<T: AsRef<str>> fmt::Display for PortFullname<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "{}:{}",
            self.client_name(),
            self.port_shortname()
        ))
    }
}

impl<'a> TryFrom<&'a str> for PortFullname<&'a str> {
    type Error = NameError;
    fn try_from(value: &'a str) -> Result<PortFullname<&'a str>, NameError> {
        PortFullname::new(value)
    }
}

impl TryFrom<String> for PortFullname<String> {
    type Error = NameError;
    fn try_from(value: String) -> Result<PortFullname<String>, NameError> {
        PortFullname::new(value)
    }
}

impl<T: AsRef<str>> PortFullname<T> {
    pub fn new(buffer: T) -> Result<PortFullname<T>, NameError> {
        if buffer.as_ref().len() > *jack::PORT_NAME_SIZE {
            return Err(NameError::PortnameTooLong);
        }
        let split_idx = buffer
            .as_ref()
            .find(':')
            .ok_or_else(|| NameError::InvalidFullname)?;
        if split_idx >= *jack::CLIENT_NAME_SIZE {
            return Err(NameError::ClientnameTooLong);
        }
        Ok(Self { buffer, split_idx })
    }

    pub fn client_name(&self) -> &str {
        self.buffer.as_ref().split_at(self.split_idx).0
    }

    pub fn port_shortname(&self) -> &str {
        self.buffer.as_ref().split_at(self.split_idx + 1).1
    }

    pub fn borrow<'a>(&'a self) -> PortFullname<&'a str> {
        PortFullname {
            buffer: self.buffer.as_ref(),
            split_idx: self.split_idx,
        }
    }

    pub fn to_string(&self) -> PortFullname<String> {
        PortFullname {
            buffer: self.buffer.as_ref().to_owned(),
            split_idx: self.split_idx,
        }
    }
}

impl<T> AsRef<str> for PortFullname<T>
where
    T: AsRef<str>,
{
    fn as_ref(&self) -> &str {
        self.buffer.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    #[test]
    fn test_serde() {
        let name1 = PortFullname::new("client1:port1:port3").unwrap();
        assert_eq!(name1.client_name(), "client1");
        assert_eq!(name1.port_shortname(), "port1:port3");
        let name2 = PortFullname::new("client2:port1:port3").unwrap();
        assert_eq!(name2.client_name(), "client2");
        assert_eq!(name2.port_shortname(), "port1:port3");
        let mut map1 = HashMap::new();
        map1.insert("root", vec![name1, name2]);
        let ser_mapped = toml::to_string_pretty(&map1).unwrap();
        let parsed: HashMap<&str, Vec<PortFullname<&str>>> =
            toml::de::from_str(&ser_mapped).unwrap();
        assert_eq!(map1, parsed);
    }
}
