use crate::model::{NameError, PortCategory, PortData, PortDirection, PortFullname};
use std::convert::TryFrom;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, TryLockError};
use std::time::Duration;
use thiserror::*;

#[derive(Debug, Error)]
pub enum GraphError {
    #[error(transparent)]
    Jack(#[from] jack::Error),
    #[error(transparent)]
    ItemName(#[from] NameError),
}

#[derive(Debug)]
pub struct JackGraph {
    update_flag: Notifier,
    client: jack::AsyncClient<Notifier, ()>,
    ports: Vec<PortData>,
    connections: Vec<(usize, usize)>,
}

impl JackGraph {
    pub fn new(client: jack::Client) -> Result<Self, GraphError> {
        let notifier = Notifier::new();
        let update_flag = notifier.handle();
        let client = client.activate_async(notifier, ())?;
        let mut retvl = JackGraph {
            update_flag,
            client,
            ports: Vec::new(),
            connections: Vec::new(),
        };
        retvl.update()?;
        Ok(retvl)
    }

    pub fn disconnect(
        &mut self,
        source: &PortFullname,
        dest: &PortFullname,
    ) -> Result<(), GraphError> {
        self.client
            .as_client()
            .disconnect_ports_by_name(source.as_ref(), dest.as_ref())?;
        let mut source_idx = None;
        let mut dest_idx = None;
        for (cur_idx, cur_data) in self.ports.iter().enumerate() {
            if source == &cur_data.name {
                source_idx = Some(cur_idx);
            } else if dest == &cur_data.name {
                dest_idx = Some(cur_idx);
            }
            if source_idx.is_some() && dest_idx.is_some() {
                break;
            }
        }
        let key = source_idx.zip(dest_idx).map(|(a, b)| (a.min(b), a.max(b)));
        let con_idx = key.and_then(|k| self.connections.binary_search(&k).ok());
        if let Some(con_idx) = con_idx {
            self.connections.remove(con_idx);
        } else {
            self.update()?;
        }
        Ok(())
    }

    pub fn connect(
        &mut self,
        source: &PortFullname,
        dest: &PortFullname,
    ) -> Result<(), GraphError> {
        self.client
            .as_client()
            .connect_ports_by_name(source.as_ref(), dest.as_ref())?;
        let mut source_idx = None;
        let mut dest_idx = None;
        for (cur_idx, cur_data) in self.ports.iter().enumerate() {
            if source == &cur_data.name {
                source_idx = Some(cur_idx);
            } else if dest == &cur_data.name {
                dest_idx = Some(cur_idx);
            }
            if source_idx.is_some() && dest_idx.is_some() {
                break;
            }
        }
        if let (Some(source_idx), Some(dest_idx)) = (source_idx, dest_idx) {
            let key = if source_idx < dest_idx {
                (source_idx, dest_idx)
            } else {
                (dest_idx, source_idx)
            };
            if let Err(con_idx) = self.connections.binary_search(&key) {
                self.connections.insert(con_idx, key);
            }
            Ok(())
        } else {
            self.update()?;
            Ok(())
        }
    }

    pub fn wait_for_update(&self) {
        self.update_flag.wait_timeout(None);
    }

    pub fn wait_for_update_timeout(&self, dur: Duration) {
        self.update_flag.wait_timeout(Some(dur));
    }

    pub fn needs_update(&self) -> bool {
        self.update_flag.check()
    }

    pub fn update(&mut self) -> Result<(), GraphError> {
        self.update_flag.reset();
        let port_names = self
            .client
            .as_client()
            .ports(None, None, jack::PortFlags::empty())
            .into_iter()
            .map(PortFullname::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let mut connections = Vec::new();
        let port_iter = port_names.iter().enumerate().filter_map(|(idx, name)| {
            Some((
                idx,
                name,
                self.client.as_client().port_by_name(name.as_ref())?,
            ))
        });
        let mut ports = Vec::new();
        for (port_a_idx, port_a_name, port_a) in port_iter {
            let direction = if port_a.flags().contains(jack::PortFlags::IS_INPUT) {
                PortDirection::In
            } else {
                PortDirection::Out
            };
            let kindstr = port_a.port_type()?.to_lowercase();
            let category = if kindstr.contains("midi") {
                PortCategory::Midi
            } else if kindstr.contains("audio") {
                PortCategory::Audio
            } else {
                PortCategory::Unknown
            };
            let data = PortData {
                name: port_a_name.clone(),
                direction,
                category,
            };
            ports.push(data);
            for (port_b_idx, port_b) in port_names.iter().enumerate().skip(port_a_idx + 1) {
                if port_a.is_connected_to(port_b.as_ref())? {
                    connections.push((port_a_idx, port_b_idx));
                }
            }
        }
        self.ports = ports;
        self.connections = connections;
        Ok(())
    }

    pub fn port_connections<'a, 'b>(
        &'a self,
        name: &'b PortFullname,
    ) -> impl Iterator<Item = &'a PortData> + 'a {
        let port_idx = self
            .ports
            .iter()
            .map(|data| &data.name)
            .enumerate()
            .find(|(_, cur)| cur == &name)
            .map(|(idx, _)| idx);
        let con_ref = &self.connections;
        port_idx
            .map(|idx| {
                con_ref.iter().filter_map(move |&(a, b)| {
                    if a == idx {
                        Some(b)
                    } else if b == idx {
                        Some(a)
                    } else {
                        None
                    }
                })
            })
            .into_iter()
            .flatten()
            .filter_map(move |con_idx| self.ports.get(con_idx))
    }

    pub fn all_connections<'a>(
        &'a self,
    ) -> impl Iterator<Item = (&'a PortData, &'a PortData)> + 'a {
        let conref = &self.connections;
        conref.iter().filter_map(move |&(a, b)| {
            let a_name = self.ports.get(a)?;
            let b_name = self.ports.get(b)?;
            Some((a_name, b_name))
        })
    }

    pub fn port_by_name<'a, 'b>(&'a self, name: &'b PortFullname) -> Option<&'a PortData> {
        self.ports.iter().find(|data| &data.name == name)
    }

    pub fn all_clients<'a>(&'a self) -> impl Iterator<Item = &'a str> + 'a {
        let first_client = self.ports.first().map(|data| data.name.client_name());
        let mut cur_client = first_client;
        let rest_iter = self
            .ports
            .iter()
            .map(|data| &data.name)
            .map(|fullname| fullname.client_name())
            .filter(move |&cur| {
                if Some(cur) == cur_client {
                    false
                } else {
                    cur_client = Some(cur);
                    true
                }
            });
        first_client.into_iter().chain(rest_iter)
    }

    pub fn client_ports<'a>(&'a self, client: &'a str) -> impl Iterator<Item = &PortData> + 'a {
        self.ports
            .iter()
            .skip_while(move |fullname| fullname.name.client_name() != client)
            .take_while(move |fullname| fullname.name.client_name() == client)
    }
}

#[derive(Debug)]
pub struct Notifier {
    rf: Arc<AtomicBool>,
    cvar: Arc<(Mutex<()>, Condvar)>,
}

impl Notifier {
    pub fn new() -> Self {
        Self {
            rf: Arc::new(AtomicBool::new(false)),
            cvar: Arc::new((Mutex::new(()), Condvar::new())),
        }
    }
    pub fn set(&self) {
        self.rf.store(true, Ordering::Release);
        self.cvar.1.notify_all();
    }
    pub fn reset(&self) {
        // Need SeqCst b/c we need to gurantee that the graph updating occurs
        // *after* the store, and that the actual jack client data is updated
        // *before* the store. Otherwise, if the notifier gets a change mid-update
        // call, that notification could be written over without the change being
        // loaded.
        self.rf.store(false, Ordering::SeqCst);
    }
    pub fn check(&self) -> bool {
        // Since we aren't actually touching the data yet,
        // we can load this Relaxed and worry about casuality later.
        self.rf.load(Ordering::Relaxed)
    }
    pub fn handle(&self) -> Self {
        Self {
            rf: Arc::clone(&self.rf),
            cvar: Arc::clone(&self.cvar),
        }
    }
    pub fn wait_timeout(&self, dur: Option<Duration>) {
        if self.check() {
            return;
        }
        let (mtx, cvar) = &*self.cvar;
        let lk = match mtx.try_lock() {
            Ok(lk) => lk,
            Err(TryLockError::Poisoned(e)) => e.into_inner(),
            Err(TryLockError::WouldBlock) => {
                return;
            }
        };
        let lk = if let Some(dur) = dur {
            cvar.wait_timeout_while(lk, dur, |_| !self.check())
                .unwrap_or_else(|e| e.into_inner())
                .0
        } else {
            cvar.wait_while(lk, |_| !self.check())
                .unwrap_or_else(|e| e.into_inner())
        };
        drop(lk);
    }
}

impl jack::NotificationHandler for Notifier {
    fn graph_reorder(&mut self, _: &jack::Client) -> jack::Control {
        self.set();
        jack::Control::Continue
    }
    fn ports_connected(
        &mut self,
        _: &jack::Client,
        _port_id_a: jack::PortId,
        _port_id_b: jack::PortId,
        _are_connected: bool,
    ) {
        self.set();
    }
    fn client_registration(&mut self, _: &jack::Client, _name: &str, _is_registered: bool) {
        self.set();
    }
    fn port_registration(
        &mut self,
        _: &jack::Client,
        _port_id: jack::PortId,
        _is_registered: bool,
    ) {
        self.set();
    }
    fn port_rename(
        &mut self,
        _: &jack::Client,
        _port_id: jack::PortId,
        _old_name: &str,
        _new_name: &str,
    ) -> jack::Control {
        self.set();
        jack::Control::Continue
    }
}
