use std::cmp::Ordering;

#[derive(Debug, Hash, Eq, PartialEq, Copy, Clone)]
pub enum TreePath {
    Root,
    Client {
        client: usize,
    },

    Port {
        client: usize,
        port: usize,
    },

    Connection {
        client: usize,
        port: usize,
        connection: usize,
    },
}

impl TreePath {
    pub const fn from_offsets(
        client_offset: usize,
        port_offset: usize,
        connection_offset: usize,
    ) -> TreePath {
        let mut cur = TreePath::Root;
        if client_offset != 0 {
            cur = cur.nth_child(client_offset - 1);
        } else {
            return cur;
        }
        if port_offset != 0 {
            cur = cur.nth_child(port_offset - 1);
        } else {
            return cur;
        }
        if connection_offset != 0 {
            cur = cur.nth_child(port_offset - 1);
        } else {
            return cur;
        }
        cur
    }

    pub const fn client_root(client: usize) -> Self {
        TreePath::Client { client }
    }

    pub const fn nth_child(&self, n: usize) -> TreePath {
        match *self {
            TreePath::Root => TreePath::Client { client: n },
            TreePath::Client { client } => TreePath::Port { client, port: n },
            TreePath::Port { client, port } => TreePath::Connection {
                client,
                port,
                connection: n,
            },
            TreePath::Connection { .. } => *self,
        }
    }

    pub const fn layer(self) -> usize {
        match self {
            TreePath::Root => 0,
            TreePath::Client { .. } => 1,
            TreePath::Port { .. } => 2,
            TreePath::Connection { .. } => 3,
        }
    }

    pub const fn client_offset(&self) -> usize {
        match *self {
            TreePath::Root => 0,
            TreePath::Client { client }
            | TreePath::Port { client, .. }
            | TreePath::Connection { client, .. } => client + 1,
        }
    }
    pub const fn port_offset(&self) -> usize {
        match *self {
            TreePath::Root | TreePath::Client { .. } => 0,
            TreePath::Port { port, .. } | TreePath::Connection { port, .. } => port + 1,
        }
    }
    pub const fn connection_offset(&self) -> usize {
        match *self {
            TreePath::Root | TreePath::Client { .. } | TreePath::Port { .. } => 0,
            TreePath::Connection { connection, .. } => connection + 1,
        }
    }

    pub const fn offset_in_layer(&self) -> usize {
        match *self {
            TreePath::Root => 0,
            TreePath::Client { .. } => self.client_offset(),
            TreePath::Port { .. } => self.port_offset(),
            TreePath::Connection { .. } => self.connection_offset(),
        }
    }

    pub fn next_sibling(&self) -> Option<TreePath> {
        match *self {
            TreePath::Root => None,
            TreePath::Client { client } => {
                let client = client.checked_add(1)?;
                Some(TreePath::Client { client })
            }
            TreePath::Port { client, port } => {
                let port = port.checked_add(1)?;
                Some(TreePath::Port { client, port })
            }

            TreePath::Connection {
                client,
                port,
                connection,
            } => {
                let connection = connection.checked_add(1)?;
                Some(TreePath::Connection {
                    client,
                    port,
                    connection,
                })
            }
        }
    }
    pub fn prev_sibling(&self) -> Option<TreePath> {
        match *self {
            TreePath::Root => None,
            TreePath::Client { client } => {
                let client = client.checked_sub(1)?;
                Some(TreePath::Client { client })
            }
            TreePath::Port { client, port } => {
                let port = port.checked_sub(1)?;
                Some(TreePath::Port { client, port })
            }

            TreePath::Connection {
                client,
                port,
                connection,
            } => {
                let connection = connection.checked_sub(1)?;
                Some(TreePath::Connection {
                    client,
                    port,
                    connection,
                })
            }
        }
    }

    pub const fn parent(&self) -> Option<TreePath> {
        match *self {
            TreePath::Root => None,
            TreePath::Client { .. } => Some(TreePath::Root),
            TreePath::Port { client, .. } => Some(TreePath::Client { client }),
            TreePath::Connection { client, port, .. } => Some(TreePath::Port { client, port }),
        }
    }
}

impl PartialOrd for TreePath {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for TreePath {
    fn cmp(&self, other: &Self) -> Ordering {
        self.client_offset()
            .cmp(&other.client_offset())
            .then(self.port_offset().cmp(&other.port_offset()))
            .then(self.connection_offset().cmp(&other.connection_offset()))
    }
}
