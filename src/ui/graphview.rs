use super::{ResolvedTreepath, TreePath};
use crate::config::LockConfig;
use crate::graph::JackGraph;
use crate::ui::UiAction;

use crossterm::event;
use crossterm::event::{KeyCode, KeyModifiers};
use std::time::Duration;
use tui::buffer::Buffer;
use tui::layout::{Constraint, Layout, Rect};
use tui::widgets::{StatefulWidget, Widget};

use std::convert::{TryFrom, TryInto};

mod datapanel;
use datapanel::*;

mod jacktree;
use jacktree::*;

mod connect;
use connect::*;

#[derive(Debug, Default)]
pub struct GraphViewState {
    connect_popup: Option<AddConnectionState>,
    tree_state: JackTreeState,
}

impl GraphViewState {
    pub fn new() -> Self {
        Self::default()
    }
    fn resolve_tree_state(&mut self, graph: &JackGraph) {
        let current_selection = self.tree_state.selected();
        let next_selection = ResolvedTreepath::resolve_partial(graph, current_selection).path();
        self.tree_state.select(next_selection);
    }
    pub fn handle_pending_event(
        &mut self,
        graph: &mut JackGraph,
        conf: &mut LockConfig,
        timeout: Option<Duration>,
    ) -> Result<Option<UiAction>, crate::Error> {
        if let Some(mut conpop) = self.connect_popup.take() {
            let conres = conpop.handle_pending_event(timeout);
            if let Ok(Some(UiAction::Close)) = conres {
                let (port_a, port_b_opt) = conpop.into_selection(graph, conf);
                if let Some(port_b) = port_b_opt {
                    let (src, dst) = if port_a.direction.is_input() {
                        (port_b.clone(), port_a)
                    } else {
                        (port_a, port_b.clone())
                    };
                    graph.connect(&src.name, &dst.name)?;
                }
                return Ok(Some(UiAction::Redraw));
            } else {
                self.connect_popup = Some(conpop);
                return conres;
            }
        }
        if !event::poll(timeout.unwrap_or_else(|| Duration::from_micros(0)))? {
            return Ok(None);
        }
        let raw = event::read()?;
        let parsed = match raw.try_into() {
            Ok(p) => p,
            Err(()) => {
                return Ok(None);
            }
        };
        match parsed {
            GraphUiEvent::Quit => Ok(Some(UiAction::Close)),
            GraphUiEvent::MoveUp => {
                let cur = self.tree_state.selected();
                let mut nxt = cur
                    .prev_sibling()
                    .or_else(|| cur.parent())
                    .unwrap_or_else(TreePath::root);
                if cur == TreePath::root() && nxt == TreePath::root() {
                    nxt = nxt.nth_child(0);
                }
                self.tree_state.select(nxt);
                Ok(Some(UiAction::Redraw))
            }
            GraphUiEvent::MoveDown => {
                let cur = self.tree_state.selected();
                let mut nxt = cur
                    .next_sibling()
                    .or_else(|| cur.parent())
                    .unwrap_or_else(TreePath::root);
                if cur == TreePath::root() && nxt == TreePath::root() {
                    nxt = nxt.nth_child(0);
                }
                self.tree_state.select(nxt);
                Ok(Some(UiAction::Redraw))
            }
            GraphUiEvent::MoveLeft => {
                let cur = self.tree_state.selected();
                let nxt = cur.parent().unwrap_or_else(TreePath::root);
                self.tree_state.select(nxt);
                Ok(Some(UiAction::Redraw))
            }
            GraphUiEvent::MoveRight => {
                let cur = self.tree_state.selected();
                let nxt = cur.nth_child(0);
                self.tree_state.select(nxt);
                Ok(Some(UiAction::Redraw))
            }
            GraphUiEvent::AddConnection => {
                eprintln!("A");
                let cur_selected = self.tree_state.selected();
                let client_idx = cur_selected.client_idx();
                let port_idx = cur_selected.port_idx();
                let con_idx = cur_selected.connection_idx();
                eprintln!("B");
                let (client_idx, port_idx) = match (client_idx, port_idx, con_idx) {
                    (Some(c), Some(p), None) => (c, p),
                    _ => {
                        return Ok(None);
                    }
                };
                let client = match graph.all_clients().nth(client_idx) {
                    Some(c) => c,
                    None => {
                        return Ok(None);
                    }
                };
                let port = match graph.client_ports(client).nth(port_idx) {
                    Some(p) => p,
                    None => {
                        return Ok(None);
                    }
                };

                eprintln!("C");
                let state = AddConnectionState::new(port);
                self.connect_popup = Some(state);
                eprintln!("D");
                Ok(Some(UiAction::Redraw))
            }
        }
    }
}

pub struct GraphViewWidget<'a> {
    graph: &'a JackGraph,
    config: &'a LockConfig,
}

impl<'a> GraphViewWidget<'a> {
    pub fn new(graph: &'a JackGraph, config: &'a LockConfig) -> Self {
        Self { graph, config }
    }
}

impl<'a> StatefulWidget for GraphViewWidget<'a> {
    type State = GraphViewState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.resolve_tree_state(self.graph);
        let selected = state.tree_state.selected();
        let graph = self.graph;
        let conf = self.config;

        let mut height_resolver = Layout::default()
            .constraints([Constraint::Ratio(2, 3), Constraint::Ratio(1, 3)])
            .split(area);

        let info_rect = height_resolver.pop().unwrap();
        let list_rect = height_resolver.pop().unwrap();
        JackTree::new(graph).render(list_rect, buf, &mut state.tree_state);

        let dataview = make_dataview(selected, graph, conf);
        dataview.render(info_rect, buf);

        if let Some(constate) = state.connect_popup.as_mut() {
            let widget = AddConnectionWidget::new(graph, conf);
            let (width, height) = widget.dims(constate);

            // Center the list horizontally.
            let extra_space = area.width.saturating_sub(width);
            let left_pad = extra_space / 2;
            let mut list_area = area;
            list_area.x += left_pad;
            list_area.width -= extra_space;

            list_area.y += 4; // 4 spaces of vertical padding.
            list_area.height -= 8; // 4 to offset the y padding + 4 to pad the bottom
            let extra_height = list_area.height.saturating_sub(height);
            list_area.height -= extra_height; // Align as far up as possible.

            widget.render(list_area, buf, constate);
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum GraphUiEvent {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    AddConnection,
    Quit,
}

impl TryFrom<event::KeyEvent> for GraphUiEvent {
    type Error = ();
    fn try_from(value: event::KeyEvent) -> Result<Self, Self::Error> {
        const UP_CODES: &[KeyCode] = &[KeyCode::Up, KeyCode::Char('w'), KeyCode::Char('k')];
        const LEFT_CODES: &[KeyCode] = &[KeyCode::Left, KeyCode::Char('a'), KeyCode::Char('h')];
        const DOWN_CODES: &[KeyCode] = &[KeyCode::Down, KeyCode::Char('s'), KeyCode::Char('j')];
        const RIGHT_CODES: &[KeyCode] = &[KeyCode::Right, KeyCode::Char('d'), KeyCode::Char('l')];
        const CONNECT_CODES: &[KeyCode] = &[KeyCode::Char('c')];

        let code = value.code;
        let modifiers = value.modifiers;
        if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
            return Ok(GraphUiEvent::Quit);
        }

        if UP_CODES.contains(&code) {
            Ok(GraphUiEvent::MoveUp)
        } else if DOWN_CODES.contains(&code) {
            Ok(GraphUiEvent::MoveDown)
        } else if LEFT_CODES.contains(&code) {
            Ok(GraphUiEvent::MoveLeft)
        } else if RIGHT_CODES.contains(&code) {
            Ok(GraphUiEvent::MoveRight)
        } else if CONNECT_CODES.contains(&code) {
            Ok(GraphUiEvent::AddConnection)
        } else {
            Err(())
        }
    }
}

impl TryFrom<event::Event> for GraphUiEvent {
    type Error = ();
    fn try_from(value: event::Event) -> Result<Self, Self::Error> {
        match value {
            event::Event::Key(keyevent) => keyevent.try_into(),
            event::Event::Mouse(_mouseevent) => {
                //TODO: handle mouse event
                Err(())
            }
            event::Event::Resize(_cols, _rows) => {
                //TODO: handle resize event
                Err(())
            }
        }
    }
}
