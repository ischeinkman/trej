use serde::de::Error as SerdeError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::convert::TryFrom;
use std::fmt;

use super::NameError;

#[derive(Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct PortFullname {
    buffer: String,
    split_idx: usize,
}

impl<'de> Deserialize<'de> for PortFullname {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let buffer = String::deserialize(deserializer)?;
        let res = Self::new(buffer).map_err(SerdeError::custom)?;
        Ok(res)
    }
}

impl Serialize for PortFullname {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.buffer.as_ref())
    }
}

impl fmt::Display for PortFullname {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "{}:{}",
            self.client_name(),
            self.port_shortname()
        ))
    }
}

impl<'a> TryFrom<&'a str> for PortFullname {
    type Error = NameError;
    fn try_from(value: &'a str) -> Result<Self, NameError> {
        PortFullname::new(value.to_owned())
    }
}

impl TryFrom<String> for PortFullname {
    type Error = NameError;
    fn try_from(value: String) -> Result<Self, NameError> {
        PortFullname::new(value)
    }
}

impl PortFullname {
    pub fn new(buffer: String) -> Result<Self, NameError> {
        if buffer.len() > *jack::PORT_NAME_SIZE {
            return Err(NameError::PortnameTooLong);
        }
        let split_idx = buffer.find(':').ok_or_else(|| NameError::InvalidFullname)?;
        if split_idx >= *jack::CLIENT_NAME_SIZE {
            return Err(NameError::ClientnameTooLong);
        }
        Ok(Self { buffer, split_idx })
    }

    pub fn client_name(&self) -> &str {
        self.buffer.split_at(self.split_idx).0
    }

    pub fn port_shortname(&self) -> &str {
        self.buffer.split_at(self.split_idx + 1).1
    }
}

impl AsRef<str> for PortFullname {
    fn as_ref(&self) -> &str {
        self.buffer.as_ref()
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum PortDirection {
    In,
    Out,
}

impl PortDirection {
    pub const fn is_input(self) -> bool {
        !self.is_output()
    }
    pub const fn is_output(self) -> bool {
        match self {
            PortDirection::In => false,
            PortDirection::Out => true,
        }
    }

    pub const fn flip(self) -> PortDirection {
        match self {
            PortDirection::In => PortDirection::Out,
            PortDirection::Out => PortDirection::In,
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum PortCategory {
    Midi,
    Audio,
    Unknown,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct PortData {
    pub name: PortFullname,
    pub category: PortCategory,
    pub direction: PortDirection,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    #[test]
    fn test_portname_serde() {
        let name1 = PortFullname::new("client1:port1:port3".to_owned()).unwrap();
        assert_eq!(name1.client_name(), "client1");
        assert_eq!(name1.port_shortname(), "port1:port3");
        let name2 = PortFullname::new("client2:port1:port3".to_owned()).unwrap();
        assert_eq!(name2.client_name(), "client2");
        assert_eq!(name2.port_shortname(), "port1:port3");
        let mut map1 = HashMap::new();
        map1.insert("root", vec![name1, name2]);
        let ser_mapped = toml::to_string_pretty(&map1).unwrap();
        let parsed: HashMap<&str, Vec<PortFullname>> = toml::de::from_str(&ser_mapped).unwrap();
        assert_eq!(map1, parsed);
    }
}
