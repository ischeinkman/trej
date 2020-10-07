use super::ScreenWrapper;
use crate::config::{LockConfig, LockStatus};
use crate::graph::JackGraph;
use crate::model::{PortData, PortDirection};
use crossterm::{event, style, terminal};
use std::cmp::{Ord, Ordering, PartialOrd};
use std::collections::HashSet;
use std::time::Duration;

pub type ShouldShutdown = bool;

#[derive(Debug)]
pub struct GraphUi {
    graph: JackGraph,
    config: LockConfig,
    output: ScreenWrapper,
    collapsed: HashSet<TreePath>,
    current_screen: Vec<TreePath>,
    selected: TreePath,
    scroll_offset: usize,
    needs_display: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum RowPayload {
    Client {
        client: String,
    },
    Port {
        data: PortData,
    },
    Connection {
        data: PortData,
        connection: PortData,
    },
}

struct DisplayRowArgs {
    path: TreePath,
    is_collapsed: bool,
    payload: RowPayload,
}

impl DisplayRowArgs {
    pub fn path(&self) -> TreePath {
        self.path
    }
    pub fn payload(&self) -> &RowPayload {
        &self.payload
    }
    pub fn connection(
        path: TreePath,
        is_collapsed: bool,
        data: &PortData,
        connection: &PortData,
    ) -> Self {
        let data = data.to_owned();
        let connection = connection.to_owned();
        let payload = RowPayload::Connection { data, connection };
        Self {
            path,
            is_collapsed,
            payload,
        }
    }
    pub fn port(path: TreePath, is_collapsed: bool, data: &PortData) -> Self {
        let data = data.to_owned();
        let payload = RowPayload::Port { data };
        Self {
            path,
            is_collapsed,
            payload,
        }
    }
    pub fn client(path: TreePath, is_collapsed: bool, client: &str) -> Self {
        let client = client.to_owned();
        let payload = RowPayload::Client { client };
        Self {
            path,
            is_collapsed,
            payload,
        }
    }
}

impl GraphUi {
    pub fn new(graph: JackGraph, config: LockConfig, output: ScreenWrapper) -> Self {
        Self {
            graph,
            config,
            output,
            collapsed: HashSet::new(),
            current_screen: Vec::with_capacity(64),
            selected: TreePath::Root,
            scroll_offset: 0,
            needs_display: true,
        }
    }
    fn connection_subrows<'a>(
        &'a self,
        root: TreePath,
        port: &'a PortData,
    ) -> impl Iterator<Item = DisplayRowArgs> + 'a {
        let port_itr = self.graph.port_connections(&port.name).enumerate();
        port_itr.map(move |(conidx, con)| {
            let path = root.nth_child(conidx);
            let top_port = port;
            let bottom_port = con;
            let is_collapsed = self.collapsed.contains(&path);
            DisplayRowArgs::connection(path, is_collapsed, top_port, bottom_port)
        })
    }
    fn port_subrows<'a>(
        &'a self,
        root: TreePath,
        client: &'a str,
    ) -> impl Iterator<Item = DisplayRowArgs> + 'a {
        let client_itr = self.graph.client_ports(client).enumerate();
        client_itr.flat_map(move |(portidx, data)| {
            let path = root.nth_child(portidx);
            let is_collapsed = self.collapsed.contains(&path);
            let header = DisplayRowArgs::port(path, is_collapsed, data);
            let connection_rows = if !is_collapsed {
                Some(self.connection_subrows(path, data))
            } else {
                None
            };
            let connection_rows = connection_rows.into_iter().flatten();
            std::iter::once(header).chain(connection_rows)
        })
    }
    fn all_rows<'a>(&'a self) -> impl Iterator<Item = DisplayRowArgs> + 'a {
        let root_itr = self.graph.all_clients().enumerate();
        root_itr.flat_map(move |(clientidx, client)| {
            let path = TreePath::client_root(clientidx);
            let is_collapsed = self.collapsed.contains(&path);
            let header = DisplayRowArgs::client(path, is_collapsed, client);
            let port_rows = if !is_collapsed {
                Some(self.port_subrows(path, client)).into_iter().flatten()
            } else {
                None.into_iter().flatten()
            };
            std::iter::once(header).chain(port_rows)
        })
    }
    fn display_row(&mut self, args: DisplayRowArgs) -> crossterm::Result<()> {
        let is_selected = self.selected == args.path();
        let color_attr = if is_selected {
            style::Attribute::Reverse
        } else {
            style::Attribute::Reset
        };
        let is_collapsed = args.is_collapsed && args.path().layer() < 3;
        let collapse_marker = if is_collapsed { ">" } else { "v" };
        let idx = args.path().offset_in_layer();
        match args.payload() {
            RowPayload::Client { client } => {
                let lock = self.config.client_status(&client);
                let lockstr = match lock {
                    LockStatus::None => " ",
                    LockStatus::Force => "\\",
                    LockStatus::Block => "/",
                    LockStatus::Full => "X",
                };
                self.output.writeln(format_args!(
                    "{}[{:02}] {} {} [{}]{}",
                    color_attr,
                    idx,
                    collapse_marker,
                    client,
                    lockstr,
                    style::Attribute::Reset
                ))?;
            }
            RowPayload::Port { data, .. } => {
                let lock = self.config.port_status(&data.name);
                let lockstr = match lock {
                    LockStatus::None => " ",
                    LockStatus::Force => "\\",
                    LockStatus::Block => "/",
                    LockStatus::Full => "X",
                };
                let direction = data.direction;
                let arrow = match direction {
                    PortDirection::In => "<=",
                    PortDirection::Out => "=>",
                };
                self.output.writeln(format_args!(
                    "{}     | {} [{:02}] {} {} [{}]{}",
                    color_attr,
                    arrow,
                    idx,
                    collapse_marker,
                    data.name.port_shortname(),
                    lockstr,
                    style::Attribute::Reset
                ))?;
            }
            RowPayload::Connection {
                data, connection, ..
            } => {
                let lock = self.config.connection_status(&data.name, &connection.name);
                let lockstr = match lock {
                    LockStatus::None => " ",
                    LockStatus::Force => "\\",
                    LockStatus::Block => "/",
                    LockStatus::Full => "X",
                };
                self.output.writeln(format_args!(
                    "{}               | -> [{:02}] {} [{}]{}",
                    color_attr,
                    idx,
                    connection.name.port_shortname(),
                    lockstr,
                    style::Attribute::Reset
                ))?;
            }
        }
        Ok(())
    }
    fn displayed_rows<'a>(
        &'a self,
    ) -> crossterm::Result<impl Iterator<Item = DisplayRowArgs> + 'a> {
        let (_, rows) = terminal::size()?;
        let res = self.all_rows().skip(self.scroll_offset).take(rows as usize);
        Ok(res)
    }

    fn row_to_path(&self, row_idx: usize) -> crossterm::Result<TreePath> {
        self.displayed_rows()?
            .nth(row_idx)
            .map(|r| Ok(r.path()))
            .unwrap_or(Ok(TreePath::Root))
    }
    pub fn display(&mut self) -> crossterm::Result<()> {
        if !self.needs_display {
            return Ok(());
        }
        let (_, rows) = terminal::size()?;
        self.current_screen.clear();
        self.output.clear()?;
        let relevant = self
            .all_rows()
            .skip(self.scroll_offset)
            .take(rows as usize)
            .collect::<Vec<_>>();
        for row in relevant {
            self.current_screen.push(row.path());
            self.display_row(row)?;
        }
        self.needs_display = false;
        Ok(())
    }

    pub fn step(&mut self) -> Result<ShouldShutdown, crate::Error> {
        if self.needs_display {
            self.display()?;
        }
        if self.graph.needs_update() {
            self.graph.update()?;
            self.on_event(GraphUiEvent::Refresh)?;
        }
        let evt = if event::poll(Duration::from_millis(0))? {
            Some(event::read()?)
        } else {
            None
        };
        match evt {
            Some(event::Event::Key(event::KeyEvent {
                code: event::KeyCode::Up,
                ..
            })) => {
                eprintln!("Up.");
                self.on_event(GraphUiEvent::MoveUp).unwrap();
            }
            Some(event::Event::Key(event::KeyEvent {
                code: event::KeyCode::Down,
                ..
            })) => {
                eprintln!("Down.");
                self.on_event(GraphUiEvent::MoveDown).unwrap();
            }
            Some(event::Event::Key(event::KeyEvent {
                code: event::KeyCode::Enter,
                ..
            }))
            | Some(event::Event::Key(event::KeyEvent {
                code: event::KeyCode::Char(' '),
                ..
            })) => {
                eprintln!("Select.");
                self.on_event(GraphUiEvent::ToggleCollapse).unwrap();
            }
            Some(event::Event::Key(event::KeyEvent {
                code: event::KeyCode::Char('c'),
                modifiers: event::KeyModifiers::CONTROL,
            })) => return Ok(true),
            Some(other) => {
                eprintln!("Other: {:?}", other);
            }
            None => {}
        }
        Ok(false)
    }

    pub fn on_event(&mut self, event: GraphUiEvent) -> Result<(), crate::Error> {
        let cur_idx = self.current_screen.binary_search(&self.selected);
        let (_cols, rows) = terminal::size()?;
        self.needs_display = true;
        match event {
            GraphUiEvent::MoveUp => match cur_idx {
                Ok(0) if self.scroll_offset == 0 => {
                    self.selected = TreePath::Root;
                }
                Ok(0) => {
                    self.scroll_offset -= 1;
                    let new_selected = self.row_to_path(0)?;
                    self.selected = new_selected;
                }
                Ok(n) => {
                    self.selected = self.row_to_path(n - 1)?;
                }
                Err(_) => {
                    self.selected = self.row_to_path(0)?;
                }
            },
            GraphUiEvent::MoveDown => match cur_idx {
                Ok(n)
                    if (n + 1 >= self.current_screen.len()
                        && self.current_screen.len() + 4 <= usize::from(rows)) =>
                {
                    self.needs_display = false;
                }
                Ok(n) if n + 1 >= self.current_screen.len() => {
                    self.scroll_offset += 1;
                    let new_selected = self
                        .displayed_rows()?
                        .last()
                        .map_or(TreePath::Root, |args| args.path());
                    self.selected = new_selected;
                }
                Ok(n) => {
                    self.selected = self.row_to_path(n + 1)?;
                }
                Err(_) => {
                    self.selected = self.row_to_path(0)?;
                }
            },
            GraphUiEvent::Refresh => {
                self.graph.update()?;
            }
            GraphUiEvent::ToggleCollapse => {
                if self.selected != TreePath::Root {
                    if self.collapsed.contains(&self.selected) {
                        self.collapsed.remove(&self.selected);
                    } else {
                        self.collapsed.insert(self.selected);
                    }
                } else {
                    self.needs_display = false;
                }
            }
            _ => todo!(),
        }
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone)]
pub enum GraphUiEvent {
    MoveUp,
    MoveDown,
    ToggleCollapse,
    Click { row: u16 },
    Refresh,
}

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
    pub const fn root() -> Self {
        TreePath::Root
    }

    pub const fn client_root(client: usize) -> Self {
        TreePath::Client { client }
    }

    pub const fn parent(&self) -> Option<TreePath> {
        match *self {
            TreePath::Root => None,
            TreePath::Client { .. } => Some(TreePath::Root),
            TreePath::Port { client, .. } => Some(TreePath::Client { client }),
            TreePath::Connection { client, port, .. } => Some(TreePath::Port { client, port }),
        }
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

    pub const fn is_child_of(&self, other: &TreePath) -> bool {
        if self.client_offset() == 0 {
            return false;
        }
        if self.client_offset() != other.client_offset() {
            return other.client_offset() == 0;
        }
        if self.port_offset() != other.port_offset() {
            return other.port_offset() == 0;
        }
        if self.connection_offset() != other.connection_offset() {
            return other.connection_offset() == 0;
        }
        false
    }

    pub const fn is_parent_of(&self, other: &TreePath) -> bool {
        match (*self, *other) {
            (TreePath::Root, TreePath::Root) => false,
            (TreePath::Root, _) => true,
            (TreePath::Client { client: c }, TreePath::Port { client, .. }) => c == client,
            (TreePath::Client { client: c }, TreePath::Connection { client, .. }) => c == client,
            (TreePath::Port { client: c, port: p }, TreePath::Connection { client, port, .. }) => {
                c == client && p == port
            }
            _ => false,
        }
    }

    pub fn ancestors(&self) -> impl Iterator<Item = TreePath> {
        std::iter::successors(self.parent(), |cur| cur.parent())
    }

    pub const fn next_sibling(self) -> Self {
        match self {
            TreePath::Root => TreePath::Root,
            TreePath::Client { client } => TreePath::Client { client: client + 1 },
            TreePath::Port { client, port } => TreePath::Port {
                client,
                port: port + 1,
            },
            TreePath::Connection {
                client,
                port,
                connection,
            } => TreePath::Connection {
                client,
                port,
                connection: connection + 1,
            },
        }
    }

    /// Resolves this `TreePath` into a tuple of `(client_offset, port_offset, connection_offset)`.
    ///
    /// These offsets use 0 to indicate the "header" of that particular element; for example,
    /// a path of `(1, 0, 0)` translates to the root node of the first client, while `(1, 1, 0)`
    /// translates to the first port under that client.
    /// `(0, 0, 0)` is the root-level node of the tree.
    pub const fn path_offsets(&self) -> (usize, usize, usize) {
        (
            self.client_offset(),
            self.port_offset(),
            self.connection_offset(),
        )
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
            TreePath::Client { .. } => self.path_offsets().0,
            TreePath::Port { .. } => self.path_offsets().1,
            TreePath::Connection { .. } => self.path_offsets().2,
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
        self.path_offsets().cmp(&other.path_offsets())
    }
}
