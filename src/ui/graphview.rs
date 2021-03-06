use crate::config::LockConfig;
use crate::graph::JackGraph;
use crate::ui::UiAction;

use crate::model::ItemKey;

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
mod disconnect;
use disconnect::*;

#[derive(Debug, Default)]
pub struct GraphViewState {
    connect_popup: Option<AddConnectionState>,
    disconnect_popup: Option<DelConnectionState>,
    tree_state: JackTreeState,
}

impl GraphViewState {
    pub fn new() -> Self {
        Self::default()
    }
    fn resolve_tree_state(&mut self, graph: &JackGraph) {
        let current_selection = self.tree_state.selected();
        let next_selection = resolve_partial(graph, current_selection);
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
        if let Some(mut dispop) = self.disconnect_popup.take() {
            let rs = dispop.handle_pending_event(timeout);
            if let Ok(Some(UiAction::Close)) = rs {
                let (port_a, port_b_opt) = dispop.into_selection(graph, conf);
                if let Some(port_b) = port_b_opt {
                    let (src, dst) = if port_a.direction.is_input() {
                        (port_b.clone(), port_a)
                    } else {
                        (port_a, port_b.clone())
                    };
                    graph.disconnect(&src.name, &dst.name)?;
                }
                return Ok(Some(UiAction::Redraw));
            } else {
                self.disconnect_popup = Some(dispop);
                return rs;
            }
        }
        if !event::poll(timeout.unwrap_or_else(|| Duration::from_micros(0)))? {
            return Ok(None);
        }
        let raw = event::read()?;
        if let event::Event::Resize(_, _) = raw {
            return Ok(Some(UiAction::Redraw));
        }
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
                    .unwrap_or_else(ItemKey::root);
                if cur == ItemKey::root() && nxt == ItemKey::root() {
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
                    .unwrap_or_else(ItemKey::root);
                if cur == ItemKey::root() && nxt == ItemKey::root() {
                    nxt = nxt.nth_child(0);
                }
                self.tree_state.select(nxt);
                Ok(Some(UiAction::Redraw))
            }
            GraphUiEvent::MoveLeft => {
                let cur = self.tree_state.selected();
                let nxt = cur.parent().unwrap_or_else(ItemKey::root);
                self.tree_state.select(nxt);
                Ok(Some(UiAction::Redraw))
            }
            GraphUiEvent::MoveRight => {
                let cur = self.tree_state.selected();
                let nxt = cur.nth_child(0);
                self.tree_state.select(nxt);
                Ok(Some(UiAction::Redraw))
            }
            GraphUiEvent::DelConnection => {
                let cur_selected = self.tree_state.selected();
                let client_idx = cur_selected.client_idx();
                let port_idx = cur_selected.port_idx();
                let con_idx = cur_selected.connection_idx();
                let (client_idx, port_idx, _con_idx) = match (client_idx, port_idx, con_idx) {
                    (Some(c), Some(p), Some(con)) => (c, p, con),
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

                let state = DelConnectionState::new(port);
                self.disconnect_popup = Some(state);
                Ok(Some(UiAction::Redraw))
            }
            GraphUiEvent::AddConnection => {
                let cur_selected = self.tree_state.selected();
                let client_idx = cur_selected.client_idx();
                let port_idx = cur_selected.port_idx();
                let con_idx = cur_selected.connection_idx();
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

                let state = AddConnectionState::new(port);
                self.connect_popup = Some(state);
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
        if let Some(constate) = state.disconnect_popup.as_mut() {
            let widget = DelConnectionWidget::new(graph, conf);
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
    DelConnection,
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
        const DISCONNECT_CODES: &[KeyCode] = &[KeyCode::Char('d')];

        let code = value.code;
        let modifiers = value.modifiers;
        if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
            return Ok(GraphUiEvent::Quit);
        }

        if CONNECT_CODES.contains(&code) {
            Ok(GraphUiEvent::AddConnection)
        } else if DISCONNECT_CODES.contains(&code) {
            Ok(GraphUiEvent::DelConnection)
        } else if UP_CODES.contains(&code) {
            Ok(GraphUiEvent::MoveUp)
        } else if DOWN_CODES.contains(&code) {
            Ok(GraphUiEvent::MoveDown)
        } else if LEFT_CODES.contains(&code) {
            Ok(GraphUiEvent::MoveLeft)
        } else if RIGHT_CODES.contains(&code) {
            Ok(GraphUiEvent::MoveRight)
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

fn resolve_partial(graph: &JackGraph, path: ItemKey) -> ItemKey {
    macro_rules! do_layer {
        ($idx:expr, $itr:expr, $retvl:expr) => {{
            let (cur_idx, cur_key) = match $idx.and_then(|n| Some((n, $itr.nth(n)?))) {
                Some(vals) => vals,
                None => {
                    return $retvl;
                }
            };
            ($retvl.nth_child(cur_idx), cur_key)
        }};
    }

    let retvl = ItemKey::root();
    let (retvl, client_name) = do_layer!(path.client_idx(), graph.all_clients(), retvl);
    let (retvl, port) = do_layer!(path.port_idx(), graph.client_ports(client_name), retvl);
    let (retvl, _connection) = do_layer!(
        path.connection_idx(),
        graph.port_connections(&port.name),
        retvl
    );

    retvl
}
