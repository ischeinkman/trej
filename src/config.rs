use crate::model::PortFullname;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::hash::Hash;

mod file;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(from = "file::ConfigFile", into = "file::ConfigFile")]
pub struct LockConfig {
    client_locks: HashMap<String, LockStatus>,
    port_locks: HashMap<PortFullname, LockStatus>,
    connections_list: Vec<(PortFullname, PortFullname)>,
}

impl From<LockConfig> for file::ConfigFile {
    fn from(conf: LockConfig) -> Self {
        let mut client_map: HashMap<String, file::ClientInfo> = HashMap::new();
        let mut port_map: HashMap<PortFullname, file::PortInfo> = HashMap::new();
        for (client_name, lock) in conf.client_locks {
            client_map.insert(client_name, file::ClientInfo::new().with_lock(lock));
        }
        for (port_name, lock) in conf.port_locks {
            let ent = client_map
                .get_mut(port_name.client_name())
                .map(|client_info| &mut client_info.ports)
                .map(|port_map| {
                    port_map
                        .entry(port_name.port_shortname().to_owned())
                        .or_default()
                })
                .unwrap_or_else(|| port_map.entry(port_name).or_default());
            ent.set_lock(lock);
        }
        for (first, second) in conf.connections_list {
            let first_ent = client_map
                .get_mut(first.client_name())
                .map(|client_info| &mut client_info.ports)
                .and_then(|port_map| port_map.get_mut(first.port_shortname()))
                .or_else(|| port_map.get_mut(&first));
            if let Some(first_ent) = first_ent {
                first_ent.connections.push(second.clone());
            }

            let second_ent = client_map
                .get_mut(second.client_name())
                .map(|client_info| &mut client_info.ports)
                .and_then(|port_map| port_map.get_mut(second.port_shortname()))
                .or_else(|| port_map.get_mut(&second));
            if let Some(second_ent) = second_ent {
                second_ent.connections.push(first.clone());
            }
        }
        let client_ents = client_map
            .into_iter()
            .map(|(name, info)| file::LockEntry::Client { name, info });
        let port_ents = port_map
            .into_iter()
            .map(|(name, info)| file::LockEntry::Port { name, info });
        let entries = client_ents.chain(port_ents).collect();
        file::ConfigFile { entries }
    }
}

impl From<file::ConfigFile> for LockConfig {
    fn from(fl: file::ConfigFile) -> Self {
        let mut retvl = LockConfig::new();
        for ent in fl.entries {
            match ent {
                file::LockEntry::Client { name, info } => {
                    if let Some(lock) = info.lock {
                        retvl.client_locks.insert(name.clone(), lock);
                    }
                    for (shortname, port_info) in info.ports.into_iter() {
                        let raw_fullname = format!("{}:{}", name, shortname);
                        let fullname = PortFullname::try_from(raw_fullname).unwrap();
                        if let Some(lock) = port_info.lock {
                            retvl.port_locks.insert(fullname.clone(), lock);
                        }
                        for other in port_info.connections {
                            let connection = if fullname > other {
                                (other, fullname.clone())
                            } else {
                                (fullname.clone(), other)
                            };
                            if let Err(idx) = retvl.connections_list.binary_search(&connection) {
                                retvl.connections_list.insert(idx, connection);
                            }
                        }
                    }
                }
                file::LockEntry::Port { name, info } => {
                    if let Some(lock) = info.lock {
                        retvl.port_locks.insert(name.clone(), lock);
                    }
                    for other in info.connections {
                        let connection = if name > other {
                            (other, name.clone())
                        } else {
                            (name.clone(), other)
                        };
                        if let Err(idx) = retvl.connections_list.binary_search(&connection) {
                            retvl.connections_list.insert(idx, connection);
                        }
                    }
                }
            }
        }
        retvl
    }
}

impl LockConfig {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn client_status(&self, client: &str) -> LockStatus {
        self.client_locks.get(client).copied().unwrap_or_default()
    }
    pub fn port_status(&self, port: &PortFullname) -> LockStatus {
        self.port_locks
            .get(port)
            .copied()
            .unwrap_or_else(|| self.client_status(port.client_name()))
    }
    pub fn forced_connections<'a>(
        &'a self,
    ) -> impl Iterator<Item = (&'a PortFullname, &'a PortFullname)> + 'a {
        self.connections_list
            .iter()
            .filter(move |(a, b)| self.connection_status(a, b).should_force())
            .map(|(a, b)| (a, b))
    }
    pub fn connection_status(&self, a: &PortFullname, b: &PortFullname) -> LockStatus {
        let con_key = (a.min(b), a.max(b));
        let con_preexists = self
            .connections_list
            .binary_search_by_key(&con_key, |(a, b)| (&a, &b))
            .is_ok();
        let a_lock = self.port_status(a);
        let b_lock = self.port_status(b);
        if con_preexists && (a_lock.should_force() || b_lock.should_force()) {
            LockStatus::Force
        } else if !con_preexists && (a_lock.should_block() || b_lock.should_block()) {
            LockStatus::Block
        } else {
            LockStatus::None
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
pub enum LockStatus {
    None = 0b00,
    Force = 0b01,
    Block = 0b10,
    Full = 0b11,
}

impl LockStatus {
    const fn as_bits(self) -> u8 {
        self as u8
    }
    const fn from_bits(bits: u8) -> LockStatus {
        let mut retvl = LockStatus::None;
        if bits & LockStatus::Force.as_bits() != 0 {
            retvl = retvl.with_force();
        }
        if bits & LockStatus::Block.as_bits() != 0 {
            retvl = retvl.with_block();
        }
        retvl
    }
    pub const fn with_block(self) -> LockStatus {
        match self {
            LockStatus::None | LockStatus::Block => LockStatus::Block,
            LockStatus::Force | LockStatus::Full => LockStatus::Full,
        }
    }
    pub const fn with_force(self) -> LockStatus {
        match self {
            LockStatus::None | LockStatus::Force => LockStatus::Block,
            LockStatus::Block | LockStatus::Full => LockStatus::Full,
        }
    }
    pub const fn should_force(self) -> bool {
        match self {
            LockStatus::None | LockStatus::Block => false,
            LockStatus::Force | LockStatus::Full => true,
        }
    }
    pub const fn should_block(self) -> bool {
        match self {
            LockStatus::None | LockStatus::Force => false,
            LockStatus::Block | LockStatus::Full => true,
        }
    }
}

impl Default for LockStatus {
    fn default() -> Self {
        LockStatus::None
    }
}
