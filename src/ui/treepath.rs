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
