use crate::model::{NameError, PortFullname};
use thiserror::*;

#[derive(Debug, Error)]
pub enum GraphError {
    #[error(transparent)]
    Jack(#[from] jack::Error),
    #[error(transparent)]
    ItemName(#[from] NameError),
}

#[derive(Debug, Eq, PartialEq)]
struct PortEntry {
    name: PortFullname,
    connections: Vec<usize>,
}

impl PortEntry {
    pub fn new(name: PortFullname) -> Self {
        Self {
            name,
            connections: Vec::new(),
        }
    }
}

#[derive(Default, Debug, Eq, PartialEq)]
pub struct JackGraph {
    data: Vec<PortEntry>,
}

impl JackGraph {
    pub fn parse_client(client: &jack::Client) -> Result<Self, GraphError> {
        let mut graph = JackGraph::new();
        let port_names = client.ports(None, None, jack::PortFlags::empty());
        for port in &port_names {
            let parsed = PortFullname::new(port.to_owned())?;
            graph.add_port(parsed);
        }
        let all_ports = port_names
            .iter()
            .flat_map(|n| client.port_by_name(n).into_iter());
        for a_data in all_ports {
            let port_a = a_data.name()?;
            for port_b in port_names.iter() {
                if a_data.is_connected_to(&port_b)? {
                    graph.add_connection(
                        PortFullname::new(port_a.to_owned())?,
                        PortFullname::new(port_b.to_owned())?,
                    );
                }
            }
        }
        Ok(graph)
    }
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }
    pub fn add_port(&mut self, name: PortFullname) -> Option<PortFullname> {
        match self.port_idx(&name) {
            Ok(_) => Some(name),
            Err(idx) => {
                self.data.insert(idx, PortEntry::new(name));
                None
            }
        }
    }
    fn port_idx(&self, port: &PortFullname) -> Result<usize, usize> {
        self.data.binary_search_by_key(&port, |elm| &elm.name)
    }
    fn port_idx_or_insert(&mut self, port: PortFullname) -> usize {
        match self.port_idx(&port) {
            Ok(idx) => idx,
            Err(idx) => {
                self.data.insert(idx, PortEntry::new(port));
                idx
            }
        }
    }
    pub fn add_connection(&mut self, port_a: PortFullname, port_b: PortFullname) {
        let port_a_idx = self.port_idx_or_insert(port_a);
        let port_b_idx = self.port_idx_or_insert(port_b);
        if let Err(idx) = self.data[port_a_idx].connections.binary_search(&port_b_idx) {
            self.data[port_a_idx].connections.insert(idx, port_b_idx);
        }
        if let Err(idx) = self.data[port_b_idx].connections.binary_search(&port_a_idx) {
            self.data[port_b_idx].connections.insert(idx, port_a_idx);
        }
    }

    pub fn port_connections<'a, 'b>(
        &'a self,
        name: &'b PortFullname,
    ) -> impl Iterator<Item = &'a PortFullname> + 'a {
        let entry_idx = self.port_idx(name).ok();
        let entry = entry_idx.and_then(|idx| self.data.get(idx));
        let con_idx_iter = entry.into_iter().flat_map(|ent| ent.connections.iter());
        con_idx_iter.flat_map(move |idx| self.entry_name(*idx))
    }

    pub fn all_connections<'a>(
        &'a self,
    ) -> impl Iterator<Item = (&'a PortFullname, &'a PortFullname)> + 'a {
        let data_ref = &self.data;
        self.data.iter().flat_map(move |ent| {
            let a_name = &ent.name;
            ent.connections
                .iter()
                .filter_map(move |idx| data_ref.get(*idx))
                .map(move |b_ent| (a_name, &b_ent.name))
        })
    }

    fn entry_name(&self, idx: usize) -> Option<&PortFullname> {
        self.data.get(idx).map(|ent| &ent.name)
    }

    pub fn client_connections<'a>(
        &'a self,
        client: &'a str,
    ) -> impl Iterator<Item = (&PortFullname, impl Iterator<Item = &PortFullname>)> + 'a {
        let client_ports = self
            .data
            .iter()
            .filter(move |ent| ent.name.client_name() == client);

        let connection_callback = move |idx: &usize| self.entry_name(*idx).into_iter();

        client_ports.map(move |ent| {
            let ret = ent.connections.iter().flat_map(connection_callback);
            (&ent.name, ret)
        })
    }

    pub fn all_ports(&self) -> impl Iterator<Item = &PortFullname> {
        self.data.iter().map(|ent| &ent.name)
    }

    pub fn all_clients<'a>(&'a self) -> impl Iterator<Item = &'a str> + 'a {
        let first = self.data.get(0).map(|ent| ent.name.client_name());
        let mut prev = first;
        let rest = self
            .data
            .iter()
            .map(|ent| ent.name.client_name())
            .filter(move |cur| {
                if Some(*cur) == prev {
                    false
                } else {
                    prev = Some(cur);
                    true
                }
            });
        first.into_iter().chain(rest)
    }

    pub fn client_ports<'a>(&'a self, client: &'a str) -> impl Iterator<Item = &PortFullname> + 'a {
        self.data
            .iter()
            .map(|ent| &ent.name)
            .skip_while(move |name| name.client_name() != client)
            .take_while(move |name| name.client_name() == client)
    }
}
