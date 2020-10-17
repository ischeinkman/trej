use crate::model::{NameError, PortCategory, PortData, PortDirection, PortFullname};
use std::convert::TryFrom;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, TryLockError};
use std::time::Duration;
use thiserror::*;

use jack::Client as JackClient;
use jack::Error as JackError;

/// Errors that can occur when interacting with the JACK port graph.
#[derive(Debug, Error)]
pub enum GraphError {
    #[error(transparent)]
    Jack(#[from] JackError),
    #[error(transparent)]
    ItemName(#[from] NameError),
}

/// A wrapper around the graph of JACK clients and ports.
/// Note that this struct also caches information, and can therefore get stale.
/// It is therefore wise to periodically poll for graph changes via the `needs_update()`
/// method and reloading the graph via `update()`.
#[derive(Debug)]
pub struct JackGraph {
    /// The underlying `jack::Client` that will be used for synchronizing state.
    client: jack::AsyncClient<Notifier, ()>,

    /// All ports currently in the JACK graph.
    ports: Vec<PortData>,

    /// Connections between ports, stored as indices into `self.ports`.
    /// Currently stored as sorted.
    connections: Vec<(usize, usize)>,

    /// Set by the backing `jack::Client` whenever the graph changes.
    update_flag: Notifier,
}

impl JackGraph {
    /// Constructs a new `JackGraph` wrapping the given `jack::Client`.
    pub fn new(client: JackClient) -> Result<Self, GraphError> {
        let notifier = Notifier::new();
        let update_flag = notifier.handle();
        let client = client.activate_async(notifier, ())?;
        let mut retvl = JackGraph {
            client,
            update_flag,
            ports: Vec::new(),
            connections: Vec::new(),
        };
        retvl.update()?;
        Ok(retvl)
    }

    /// Removes a connection between two ports in the graph.
    /// Note that `source` must be an input port, `dest` must be an output port,
    /// and there must be an existing connection between them; otherwise, this
    /// function will return an `Err`.
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

    /// Connects two ports in the graph.
    /// Note that both `source` and `dest` must transfer the same data type,
    /// `source` must be an input port, `dest` must be an output port,
    /// and there must not be an existing connection between them; otherwise, this
    /// function will return an `Err`.
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

    /// Checks to see if the underlying `jack::Client` has unsynced updates that
    /// should be pulled in.
    pub fn needs_update(&self) -> bool {
        self.update_flag.check()
    }

    /// Refreshes the data in the interal graph cache with data from the underlying `jack::Client`.
    pub fn update(&mut self) -> Result<(), GraphError> {
        self.update_flag.reset();

        let raw_names = self
            .client
            .as_client()
            .ports(None, None, jack::PortFlags::empty());
        let port_names = raw_names
            .into_iter()
            .map(PortFullname::try_from)
            .collect::<Result<Vec<_>, _>>()?;

        let client = &self.client;
        let port_iter = port_names.iter().enumerate().filter_map(|(idx, name)| {
            let data = client.as_client().port_by_name(name.as_ref())?;
            Some((idx, name, data))
        });

        self.ports.clear();
        self.connections.clear();
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
            self.ports.push(data);
            for (port_b_idx, port_b) in port_names.iter().enumerate().skip(port_a_idx + 1) {
                if port_a.is_connected_to(port_b.as_ref())? {
                    self.connections.push((port_a_idx, port_b_idx));
                }
            }
        }
        Ok(())
    }

    /// Gets an iterator over all ports connected a provided port.
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

    /// Gets an iterator over all connections between all ports in the graph.
    /// Each `(&port_a, &port_b)` tuple represents a connection between
    /// `port_a` and `port_b`; the relative order within the tuple, while stable
    /// between calls, does not convey meaningful information.
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

    /// Gets the full metadata of a port with the given `name`.
    pub fn port_by_name<'a, 'b>(&'a self, name: &'b PortFullname) -> Option<&'a PortData> {
        self.ports.iter().find(|data| &data.name == name)
    }

    /// Gets an iterator over the names of all clients in the graph.
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

    /// Gets an iterator over all ports available for a given client name.
    pub fn client_ports<'a>(&'a self, client: &'a str) -> impl Iterator<Item = &PortData> + 'a {
        self.ports
            .iter()
            .skip_while(move |fullname| fullname.name.client_name() != client)
            .take_while(move |fullname| fullname.name.client_name() == client)
    }

    pub fn all_ports(&self) -> impl Iterator<Item = &PortData> {
        self.ports.iter()
    }

    pub fn is_connected(&self, a: &PortFullname, b: &PortFullname) -> bool {
        let mut aidx = None;
        let mut bidx = None;
        for (idx, cur) in self.ports.iter().enumerate() {
            if &cur.name == a {
                aidx = Some(idx);
                if bidx.is_some() {
                    break;
                }
            }
            if &cur.name == b {
                bidx = Some(idx);
                if aidx.is_some() {
                    break;
                }
            }
        }
        let (aidx, bidx) = match aidx.zip(bidx) {
            Some(v) => v,
            None => {
                return false;
            }
        };
        let key = if aidx < bidx {
            (aidx, bidx)
        } else {
            (bidx, aidx)
        };
        self.connections.binary_search(&key).is_ok()
    }
}

/// Internal flag used to signal to the parent `JackGraph` that its data is stale.
/// This is done by registering this struct as a `NotificationHandler` on the backing `Client`
/// and setting an internal flag.
#[derive(Debug)]
struct Notifier {
    /// The backing notification flag.
    rf: Arc<AtomicBool>,
    /// Used to wait for updates.
    /// The `Mutex` is only used due to the fact that `Condvar`s must be associated
    /// with exactly 1 `Mutex`.
    cvar: Arc<(Mutex<()>, Condvar)>,
}

impl Notifier {
    /// Constructs a new `Notifier`.
    pub fn new() -> Self {
        Self {
            rf: Arc::new(AtomicBool::new(false)),
            cvar: Arc::new((Mutex::new(()), Condvar::new())),
        }
    }

    /// Sets the internal event flag to indicate that the backing `jack::Client` has been updated.
    pub fn set(&self) {
        self.rf.store(true, Ordering::Release);
        self.cvar.1.notify_all();
    }
    /// Resets the internal event flag to indicate that all new changes in the backing `jack::Client`
    /// have been processed.
    pub fn reset(&self) {
        // Need SeqCst b/c we need to gurantee that the graph updating occurs
        // *after* the store, and that the actual jack client data is updated
        // *before* the store. Otherwise, if the notifier gets a change mid-update
        // call, that notification could be written over without the change being
        // loaded.
        self.rf.store(false, Ordering::SeqCst);
    }

    /// Returns whether or not there are unprocessed changes to the backing `jack::Client`.
    pub fn check(&self) -> bool {
        // Since we aren't actually touching the data yet,
        // we can load this Relaxed and worry about casuality later.
        self.rf.load(Ordering::Relaxed)
    }

    /// Creates a new watcher for the same backing client.
    /// Any calls to `set`, `reset`, or `check` will be reflected between `self` and the returned value.
    pub fn handle(&self) -> Self {
        Self {
            rf: Arc::clone(&self.rf),
            cvar: Arc::clone(&self.cvar),
        }
    }

    /// Blocks the calling thread until a new event appears on the backing client
    /// with an optional timeout.
    #[allow(dead_code)]
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
    fn graph_reorder(&mut self, _: &JackClient) -> jack::Control {
        self.set();
        jack::Control::Continue
    }
    fn ports_connected(&mut self, _: &JackClient, _: jack::PortId, _: jack::PortId, _: bool) {
        self.set();
    }
    fn client_registration(&mut self, _: &JackClient, _name: &str, _is_registered: bool) {
        self.set();
    }
    fn port_registration(&mut self, _: &JackClient, _port_id: jack::PortId, _is_registered: bool) {
        self.set();
    }
    fn port_rename(&mut self, _: &JackClient, _: jack::PortId, _: &str, _: &str) -> jack::Control {
        self.set();
        jack::Control::Continue
    }
}
