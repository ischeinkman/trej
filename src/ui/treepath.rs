use std::cmp::Ordering;

use crate::graph::JackGraph;
use crate::model::PortData;

macro_rules! unwrap_or_ret {
    ($itm:expr, $ret:expr) => {{
        match $itm {
            Some(val) => val,
            None => {
                return $ret;
            }
        }
    }};
}

macro_rules! const_try_opt {
    ($e:expr) => {{
        unwrap_or_ret!($e, None)
    }};
}

#[derive(Debug, Hash, Eq, PartialEq, Copy, Clone)]
pub struct TreePath {
    client_offset: usize,
    port_offset: usize,
    connection_offset: usize,
}

impl TreePath {
    pub const fn new(
        client_idx: Option<usize>,
        port_idx: Option<usize>,
        connection_idx: Option<usize>,
    ) -> Self {
        let (client_offset, port_offset, connection_offset) =
            match (client_idx, port_idx, connection_idx) {
                (Some(cli), Some(port), Some(con)) => (cli + 1, port + 1, con + 1),
                (Some(cli), Some(port), None) => (cli + 1, port + 1, 0),
                (Some(cli), None, _) => (cli + 1, 0, 0),
                (None, _, _) => (0, 0, 0),
            };
        Self {
            client_offset,
            port_offset,
            connection_offset,
        }
    }
    pub const fn root() -> Self {
        TreePath {
            client_offset: 0,
            port_offset: 0,
            connection_offset: 0,
        }
    }

    pub const fn nth_child(&self, n: usize) -> TreePath {
        if self.client_offset == 0 {
            TreePath {
                client_offset: n + 1,
                ..*self
            }
        } else if self.port_offset == 0 {
            TreePath {
                port_offset: n + 1,
                ..*self
            }
        } else if self.connection_offset == 0 {
            TreePath {
                connection_offset: n + 1,
                ..*self
            }
        } else {
            *self
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

    pub const fn next_sibling(&self) -> Option<TreePath> {
        let mut retvl = *self;
        if retvl.connection_offset != 0 {
            retvl.connection_offset = const_try_opt!(retvl.connection_offset.checked_add(1));
            Some(retvl)
        } else if retvl.port_offset != 0 {
            retvl.port_offset = const_try_opt!(retvl.port_offset.checked_add(1));
            Some(retvl)
        } else if retvl.client_offset != 0 {
            retvl.client_offset = const_try_opt!(retvl.client_offset.checked_add(1));
            Some(retvl)
        } else {
            None
        }
    }
    pub const fn prev_sibling(&self) -> Option<TreePath> {
        let mut retvl = *self;
        if retvl.connection_offset > 1 {
            retvl.connection_offset = const_try_opt!(retvl.connection_offset.checked_sub(1));
            Some(retvl)
        } else if retvl.port_offset > 1 {
            retvl.port_offset = const_try_opt!(retvl.port_offset.checked_sub(1));
            Some(retvl)
        } else if retvl.client_offset > 1 {
            retvl.client_offset = const_try_opt!(retvl.client_offset.checked_sub(1));
            Some(retvl)
        } else {
            None
        }
    }

    pub const fn parent(&self) -> Option<TreePath> {
        if self.client_offset == 0 {
            None
        } else if self.port_offset == 0 {
            Some(TreePath::root())
        } else if self.connection_offset == 0 {
            Some(TreePath {
                client_offset: self.client_offset,
                ..TreePath::root()
            })
        } else {
            Some(TreePath {
                connection_offset: 0,
                ..*self
            })
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
        self.client_offset
            .cmp(&other.client_offset)
            .then(self.port_offset.cmp(&other.port_offset))
            .then(self.connection_offset.cmp(&other.connection_offset))
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct ResolvedTreepath<'a> {
    path: TreePath,
    client: Option<&'a str>,
    port: Option<&'a PortData>,
    connection: Option<&'a PortData>,
}

impl<'a> ResolvedTreepath<'a> {
    pub const fn root() -> Self {
        Self {
            path: TreePath::root(),
            client: None,
            port: None,
            connection: None,
        }
    }
    #[allow(dead_code)]
    pub fn path(&self) -> TreePath {
        self.path
    }
    pub fn client(&self) -> Option<&'a str> {
        self.client
    }
    pub fn port(&self) -> Option<&'a PortData> {
        self.port
    }
    pub fn connection(&self) -> Option<&'a PortData> {
        self.connection
    }
    pub fn resolve(graph: &'a JackGraph, path: TreePath) -> Option<ResolvedTreepath<'a>> {
        let mut retvl = ResolvedTreepath::root();
        retvl.path = path;
        let client = match path.client_idx() {
            Some(n) => graph.all_clients().nth(n)?,
            None => {
                return Some(retvl);
            }
        };
        retvl.client = Some(client);
        let port = match path.port_idx() {
            Some(n) => graph.client_ports(client).nth(n)?,
            None => {
                return Some(retvl);
            }
        };
        retvl.port = Some(port);
        let connection = match path.connection_idx() {
            Some(n) => graph.port_connections(&port.name).nth(n)?,
            None => {
                return Some(retvl);
            }
        };
        retvl.connection = Some(connection);
        Some(retvl)
    }
    pub fn resolve_partial(graph: &'a JackGraph, path: TreePath) -> ResolvedTreepath<'a> {
        let mut retvl = ResolvedTreepath::root();
        let client_idx = unwrap_or_ret!(path.client_idx(), retvl);
        let client_name = unwrap_or_ret!(graph.all_clients().nth(client_idx), retvl);

        retvl.path = retvl.path.nth_child(client_idx);
        retvl.client = Some(client_name);

        let port_idx = unwrap_or_ret!(path.port_idx(), retvl);
        let port = unwrap_or_ret!(graph.client_ports(client_name).nth(port_idx), retvl);

        retvl.path = retvl.path.nth_child(port_idx);
        retvl.port = Some(port);

        let connection_idx = unwrap_or_ret!(path.connection_idx(), retvl);
        let connection = unwrap_or_ret!(
            graph.port_connections(&port.name).nth(connection_idx),
            retvl
        );

        retvl.path = retvl.path.nth_child(connection_idx);
        retvl.connection = Some(connection);
        retvl
    }
}
