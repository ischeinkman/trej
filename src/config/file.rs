use super::LockStatus;
use crate::model::PortFullname;
use serde::{
    de::{Deserializer, MapAccess, Visitor},
    ser::{SerializeMap, Serializer},
    Deserialize, Serialize,
};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Default)]
pub struct ConfigFile {
    pub entries: Vec<LockEntry>,
}

#[derive(Debug)]
pub enum LockEntry {
    Client { name: String, info: ClientInfo },
    Port { name: PortFullname, info: PortInfo },
}

impl Serialize for ConfigFile {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map_serializer = serializer.serialize_map(Some(self.entries.len()))?;
        for ent in self.entries.iter() {
            match ent {
                LockEntry::Client { name, info } => {
                    map_serializer.serialize_entry(name, info)?;
                }
                LockEntry::Port { name, info } => {
                    map_serializer.serialize_entry(name, info)?;
                }
            }
        }
        map_serializer.end()
    }
}

impl<'de> Deserialize<'de> for ConfigFile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(LockEntryVisitor {})
    }
}

struct LockEntryVisitor {}
impl<'de> Visitor<'de> for LockEntryVisitor {
    type Value = ConfigFile;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "either a client with a list of port configurations or a port fullname and configuration")
    }
    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut entries = Vec::new();
        while let Some(rawkey) = map.next_key::<String>()? {
            match PortFullname::new(rawkey.clone()) {
                Ok(name) => {
                    let info = map.next_value()?;
                    entries.push(LockEntry::Port { name, info });
                }
                Err(_) => {
                    let name = rawkey;
                    let info = map.next_value()?;
                    entries.push(LockEntry::Client { name, info });
                }
            }
        }
        Ok(ConfigFile { entries })
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct ClientInfo {
    #[serde(default)]
    pub lock: Option<LockStatus>,
    #[serde(flatten, default)]
    pub ports: HashMap<String, PortInfo>,
}

impl ClientInfo {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_lock(self, lock: LockStatus) -> Self {
        Self {
            lock: Some(lock),
            ..self
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize, Default)]
pub struct PortInfo {
    #[serde(default)]
    pub lock: Option<LockStatus>,
    #[serde(default)]
    pub connections: Vec<PortFullname>,
}

impl PortInfo {
    pub fn set_lock(&mut self, lock: LockStatus) {
        self.lock = Some(lock);
    }
}
