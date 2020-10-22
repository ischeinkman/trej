use crate::model::PortData;

pub enum ItemDataRef<'a> {
    Root,
    Client(&'a str),
    Port(&'a PortData),
    Connection(&'a PortData, &'a PortData),
}

impl<'a> ItemDataRef<'a> {
    pub const fn root() -> Self {
        Self::Root
    }
    pub const fn from_client(client: &'a str) -> Self {
        ItemDataRef::Client(client)
    }
    pub const fn from_port(port: &'a PortData) -> Self {
        ItemDataRef::Port(port)
    }
    pub const fn from_connection(port: &'a PortData, con: &'a PortData) -> Self {
        ItemDataRef::Connection(port, con)
    }
    pub fn client(&self) -> Option<&'a str> {
        match self {
            ItemDataRef::Root => None,
            ItemDataRef::Client(cl) => Some(cl),
            ItemDataRef::Port(prt) | ItemDataRef::Connection(prt, _) => {
                Some(prt.name.client_name())
            }
        }
    }
    pub fn port(&self) -> Option<&'a PortData> {
        match self {
            ItemDataRef::Root | ItemDataRef::Client(_) => None,
            ItemDataRef::Port(prt) | ItemDataRef::Connection(prt, _) => Some(prt),
        }
    }
    pub fn connection(&self) -> Option<&'a PortData> {
        if let ItemDataRef::Connection(_, con) = self {
            Some(con)
        } else {
            None
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct ItemKey {
    client_offset: usize,
    port_offset: usize,
    connection_offset: usize,
}

impl ItemKey {
    pub const fn new(
        client_idx: Option<usize>,
        port_idx: Option<usize>,
        connection_idx: Option<usize>,
    ) -> Self {
        let client_offset = match client_idx {
            Some(n) => n + 1,
            None => 0,
        };
        let port_offset = match port_idx {
            Some(n) => n + 1,
            None => 0,
        };
        let connection_offset = match connection_idx {
            Some(n) => n + 1,
            None => 0,
        };
        Self {
            client_offset,
            port_offset,
            connection_offset,
        }
    }
    pub const fn root() -> Self {
        ItemKey::new(None, None, None)
    }
    pub const fn parent(&self) -> Option<ItemKey> {
        match (self.client_idx(), self.port_idx(), self.connection_idx()) {
            (Some(cl), Some(pt), Some(_)) => Some(ItemKey::new(Some(cl), Some(pt), None)),
            (Some(cl), Some(_), None) => Some(ItemKey::new(Some(cl), None, None)),
            (Some(_), None, _) => Some(ItemKey::new(None, None, None)),
            (None, _, _) => None,
        }
    }
    pub const fn nth_child(&self, child_idx: usize) -> ItemKey {
        match (self.client_idx(), self.port_idx(), self.connection_idx()) {
            (Some(cl), Some(pt), Some(cn)) => ItemKey::new(Some(cl), Some(pt), Some(cn)),
            (Some(cl), Some(pt), None) => ItemKey::new(Some(cl), Some(pt), Some(child_idx)),
            (Some(cl), None, _) => ItemKey::new(Some(cl), Some(child_idx), None),
            (None, _, _) => ItemKey::new(Some(child_idx), None, None),
        }
    }
    pub const fn next_sibling(&self) -> Option<ItemKey> {
        match self.layer() {
            ItemLayer::Connection => match self.connection_offset.checked_add(1) {
                Some(c) => Some(ItemKey {
                    connection_offset: c,
                    ..*self
                }),
                None => None,
            },
            ItemLayer::Port => match self.port_offset.checked_add(1) {
                Some(p) => Some(ItemKey {
                    port_offset: p,
                    ..*self
                }),
                None => None,
            },
            ItemLayer::Client => match self.client_offset.checked_add(1) {
                Some(c) => Some(ItemKey {
                    client_offset: c,
                    ..*self
                }),
                None => None,
            },
            ItemLayer::Root => Some(ItemKey::root()),
        }
    }
    pub const fn prev_sibling(&self) -> Option<ItemKey> {
        match self.layer() {
            ItemLayer::Connection => {
                if self.connection_offset <= 1 {
                    None
                } else {
                    Some(ItemKey {
                        connection_offset: self.connection_offset - 1,
                        ..*self
                    })
                }
            }
            ItemLayer::Port => {
                if self.port_offset <= 1 {
                    None
                } else {
                    Some(ItemKey {
                        port_offset: self.port_offset - 1,
                        ..*self
                    })
                }
            }
            ItemLayer::Client => {
                if self.client_offset <= 1 {
                    None
                } else {
                    Some(ItemKey {
                        client_offset: self.client_offset - 1,
                        ..*self
                    })
                }
            }
            ItemLayer::Root => Some(*self),
        }
    }
    pub const fn layer(&self) -> ItemLayer {
        match (self.client_idx(), self.port_idx(), self.connection_idx()) {
            (Some(_), Some(_), Some(_)) => ItemLayer::Connection,
            (Some(_), Some(_), None) => ItemLayer::Port,
            (Some(_), None, _) => ItemLayer::Client,
            (None, _, _) => ItemLayer::Root,
        }
    }
    pub const fn client_idx(&self) -> Option<usize> {
        self.client_offset.checked_sub(1)
    }
    pub const fn port_idx(&self) -> Option<usize> {
        self.port_offset.checked_sub(1)
    }
    pub const fn connection_idx(&self) -> Option<usize> {
        self.connection_offset.checked_sub(1)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum ItemLayer {
    Root = 0,
    Client = 1,
    Port = 2,
    Connection = 3,
}

impl From<ItemLayer> for u8 {
    fn from(wrapped: ItemLayer) -> u8 {
        wrapped as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_layering() {
        let root = ItemKey::root();
        assert_eq!(root.layer(), ItemLayer::Root);
        assert_eq!(root.client_idx(), None);
        assert_eq!(root.port_idx(), None);
        assert_eq!(root.connection_idx(), None);

        let cli = ItemKey::new(Some(0), None, None);
        assert_eq!(cli.layer(), ItemLayer::Client);
        assert_eq!(cli.client_idx(), Some(0));
        assert_eq!(cli.port_idx(), None);
        assert_eq!(cli.connection_idx(), None);

        let prt = ItemKey::new(Some(3), Some(0), None);
        assert_eq!(prt.layer(), ItemLayer::Port);
        assert_eq!(prt.client_idx(), Some(3));
        assert_eq!(prt.port_idx(), Some(0));
        assert_eq!(prt.connection_idx(), None);

        let con = ItemKey::new(Some(6), Some(3), Some(0));
        assert_eq!(con.layer(), ItemLayer::Connection);
        assert_eq!(con.client_idx(), Some(6));
        assert_eq!(con.port_idx(), Some(3));
        assert_eq!(con.connection_idx(), Some(0));
    }

    #[test]
    fn test_key_child() {
        let cur = ItemKey::root();
        let cli = cur.nth_child(0);
        assert_eq!(cli, ItemKey::new(Some(0), None, None));

        let prt = cli.nth_child(0);
        assert_eq!(prt, ItemKey::new(Some(0), Some(0), None));

        let con = prt.nth_child(0);
        assert_eq!(con, ItemKey::new(Some(0), Some(0), Some(0)));
        assert_eq!(con, con.nth_child(0));
    }
}
