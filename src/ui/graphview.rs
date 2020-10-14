use super::TreePath;
use crate::graph::JackGraph;
use crate::TrejState;
use crossterm::event;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;
use tui::backend::Backend;
use tui::layout::{Constraint, Layout};
use tui::Terminal;

mod datapanel;
use datapanel::*;

mod jacktree;
use jacktree::*;

pub struct GraphView {
    state: TrejState,
    tree_state: JackTreeState,
}

impl GraphView {
    pub fn new(state: TrejState) -> Self {
        let tree_state = JackTreeState::default();
        Self { state, tree_state }
    }
    fn set_selected_path(&mut self, path: TreePath) {
        self.tree_state.select(path);
    }
    fn selected_path(&self) -> TreePath {
        self.tree_state.selected()
    }
    pub fn display<B: Backend>(&mut self, output: &mut Terminal<B>) -> Result<(), crate::Error> {
        output.draw(|f| {
            let selected = self.selected_path();
            let graph = self.state.graph();
            let conf = self.state.config();
            let tree_state = &mut self.tree_state;

            let mut height_resolver = Layout::default()
                .constraints([Constraint::Ratio(2, 3), Constraint::Ratio(1, 3)])
                .split(f.size());

            let info_rect = height_resolver.pop().unwrap();
            let list_rect = height_resolver.pop().unwrap();
            f.render_stateful_widget(JackTree::new(graph), list_rect, tree_state);

            let dataview = make_dataview(selected, graph, conf);
            f.render_widget(dataview, info_rect);
        })?;
        Ok(())
    }

    pub fn handle_event(&mut self, evt: GraphUiEvent) -> Result<ShouldShutdown, crate::Error> {
        let graph = self.state.graph();
        match evt {
            GraphUiEvent::Quit => Ok(true),
            GraphUiEvent::MoveUp => {
                let cur = self.selected_path();
                let mut nxt = cur
                    .prev_sibling()
                    .filter(|path| path_is_valid(graph, *path))
                    .or_else(|| cur.parent())
                    .unwrap_or_else(TreePath::root);
                if cur == TreePath::root() && nxt == TreePath::root() {
                    nxt = nxt.nth_child(0);
                }
                self.set_selected_path(nxt);
                Ok(false)
            }
            GraphUiEvent::MoveDown => {
                let cur = self.selected_path();
                let mut nxt = cur
                    .next_sibling()
                    .filter(|path| path_is_valid(graph, *path))
                    .or_else(|| cur.parent())
                    .unwrap_or_else(TreePath::root);
                if cur == TreePath::root() && nxt == TreePath::root() {
                    nxt = nxt.nth_child(0);
                }
                self.set_selected_path(nxt);
                Ok(false)
            }
            GraphUiEvent::MoveLeft => {
                let cur = self.selected_path();
                let nxt = cur
                    .parent()
                    .filter(|path| path_is_valid(graph, *path))
                    .unwrap_or_else(TreePath::root);
                self.set_selected_path(nxt);
                Ok(false)
            }
            GraphUiEvent::MoveRight => {
                let cur = self.selected_path();
                let nxt = cur.nth_child(0);
                let nxt = if path_is_valid(graph, nxt) { nxt } else { cur };
                self.set_selected_path(nxt);
                Ok(false)
            }
            GraphUiEvent::Refresh => {
                self.state.reload()?;
                Ok(false)
            }
        }
    }

    pub fn poll_event(
        &mut self,
        timeout: Option<Duration>,
    ) -> Result<Option<GraphUiEvent>, crate::Error> {
        let graph = self.state.graph();
        if graph.needs_update() {
            return Ok(Some(GraphUiEvent::Refresh));
        }
        let ui_poll_res = event::poll(timeout.unwrap_or_else(|| Duration::from_micros(0)));
        let ui_evt_res = ui_poll_res.and_then(|val| match val {
            true => event::read().map(Some),
            false => Ok(None),
        });
        match ui_evt_res {
            Ok(Some(raw_evt)) => Ok(resolve_crossterm_event(raw_evt)),
            Ok(None) if graph.needs_update() => Ok(Some(GraphUiEvent::Refresh)),
            Ok(None) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn step<B: Backend>(
        &mut self,
        timeout: Option<Duration>,
        output: &mut Terminal<B>,
    ) -> Result<ShouldShutdown, crate::Error> {
        if let Some(evt) = self.poll_event(timeout)? {
            let should_shutdown = self.handle_event(evt)?;
            self.display(output)?;
            Ok(should_shutdown)
        } else {
            Ok(false)
        }
    }
}

fn path_is_valid(graph: &JackGraph, path: TreePath) -> bool {
    macro_rules! resolve {
        ($offset:expr, $iter:expr) => {{
            let n = match $offset {
                Some(n) => n,
                None => {
                    return true;
                }
            };
            match $iter.nth(n) {
                Some(val) => val,
                None => {
                    return false;
                }
            }
        }};
    };
    let client = resolve!(path.client_idx(), graph.all_clients());
    let port = resolve!(path.port_idx(), graph.client_ports(client));
    let _ = resolve!(path.connection_idx(), graph.port_connections(&port.name));
    true
}

fn resolve_crossterm_event(raw: event::Event) -> Option<GraphUiEvent> {
    match raw {
        event::Event::Key(KeyEvent { code, modifiers }) => {
            resolve_crossterm_keyevent(code, modifiers)
        }
        event::Event::Mouse(_mouseevent) => {
            //TODO: handle mouse event
            None
        }
        event::Event::Resize(_cols, _rows) => {
            //TODO: handle resize event
            None
        }
    }
}

fn resolve_crossterm_keyevent(code: KeyCode, modifiers: KeyModifiers) -> Option<GraphUiEvent> {
    if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
        return Some(GraphUiEvent::Quit);
    }
    const UP_CODES: &[KeyCode] = &[KeyCode::Up, KeyCode::Char('w'), KeyCode::Char('k')];
    const LEFT_CODES: &[KeyCode] = &[KeyCode::Left, KeyCode::Char('a'), KeyCode::Char('h')];
    const DOWN_CODES: &[KeyCode] = &[KeyCode::Down, KeyCode::Char('s'), KeyCode::Char('j')];
    const RIGHT_CODES: &[KeyCode] = &[KeyCode::Right, KeyCode::Char('d'), KeyCode::Char('l')];

    if UP_CODES.contains(&code) {
        Some(GraphUiEvent::MoveUp)
    } else if DOWN_CODES.contains(&code) {
        Some(GraphUiEvent::MoveDown)
    } else if LEFT_CODES.contains(&code) {
        Some(GraphUiEvent::MoveLeft)
    } else if RIGHT_CODES.contains(&code) {
        Some(GraphUiEvent::MoveRight)
    } else {
        None
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum GraphUiEvent {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    Refresh,
    Quit,
}

pub type ShouldShutdown = bool;
