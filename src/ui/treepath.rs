use std::cmp::Ordering;

macro_rules! const_try_opt {
    ($e:expr) => {{
        match $e {
            Some(v) => v,
            None => {
                return None;
            }
        }
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
