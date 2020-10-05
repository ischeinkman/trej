use crate::model::{NameError, PortCategory, PortDirection, PortFullname};
use std::convert::TryFrom;
use thiserror::*;

#[derive(Debug, Error)]
pub enum GraphError {
    #[error(transparent)]
    Jack(#[from] jack::Error),
    #[error(transparent)]
    ItemName(#[from] NameError),
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct PortData {
    pub name: PortFullname,
    pub category: PortCategory,
    pub direction: PortDirection,
}

#[derive(Debug)]
pub struct JackGraph {
    client: jack::Client,
    ports: Vec<PortData>,
    connections: Vec<(usize, usize)>,
}

impl JackGraph {
    pub fn new(client: jack::Client) -> Result<Self, GraphError> {
        let mut retvl = JackGraph {
            client,
            ports: Vec::new(),
            connections: Vec::new(),
        };
        retvl.update()?;
        Ok(retvl)
    }

    pub fn update(&mut self) -> Result<(), GraphError> {
        let port_names = self
            .client
            .ports(None, None, jack::PortFlags::empty())
            .into_iter()
            .map(PortFullname::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let mut connections = Vec::new();
        let port_iter = port_names
            .iter()
            .enumerate()
            .filter_map(|(idx, name)| Some((idx, name, self.client.port_by_name(name.as_ref())?)));
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
    ) -> impl Iterator<Item = (&'a PortFullname, &'a PortFullname)> + 'a {
        let conref = &self.connections;
        conref.iter().filter_map(move |&(a, b)| {
            let a_name = &self.ports.get(a)?.name;
            let b_name = &self.ports.get(b)?.name;
            Some((a_name, b_name))
        })
    }

    pub fn client_connections<'a>(
        &'a self,
        client: &'a str,
    ) -> impl Iterator<Item = (&PortData, impl Iterator<Item = &PortData>)> + 'a {
        self.client_ports(client)
            .map(move |port| (port, self.port_connections(&port.name)))
    }

    pub fn all_ports(&self) -> impl Iterator<Item = &PortData> {
        self.ports.iter()
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
