use super::{ScreenWrapper, TreePath};
use crate::config::{LockConfig, LockStatus};
use crate::graph::JackGraph;
use crate::model::{PortData, PortDirection};
use crossterm::{event, style, terminal};
use std::collections::HashSet;
use std::time::Duration;

pub type ShouldShutdown = bool;

#[derive(Debug)]
pub struct GraphUi {
    pub graph: JackGraph,
    pub config: LockConfig,
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
                    connection.name.as_ref(),
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
        }
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone)]
pub enum GraphUiEvent {
    MoveUp,
    MoveDown,
    ToggleCollapse,
    Refresh,
}
