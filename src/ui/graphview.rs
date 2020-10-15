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

#[derive(Debug, Default)]
pub struct GraphViewState {
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
        timeout: Option<Duration>,
    ) -> Result<Option<UiAction>, crate::Error> {
        if !event::poll(timeout.unwrap_or_else(|| Duration::from_micros(0)))? {
            return Ok(None);
        }
        let raw = event::read()?;
        let parsed: Option<GraphUiEvent> = raw.try_into().ok();
        parsed.map_or(Ok(None), |evt| self.handle_event(evt))
    }
    fn handle_event(&mut self, evt: GraphUiEvent) -> Result<Option<UiAction>, crate::Error> {
        match evt {
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
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum GraphUiEvent {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    Quit,
}

impl TryFrom<event::KeyEvent> for GraphUiEvent {
    type Error = ();
    fn try_from(value: event::KeyEvent) -> Result<Self, Self::Error> {
        const UP_CODES: &[KeyCode] = &[KeyCode::Up, KeyCode::Char('w'), KeyCode::Char('k')];
        const LEFT_CODES: &[KeyCode] = &[KeyCode::Left, KeyCode::Char('a'), KeyCode::Char('h')];
        const DOWN_CODES: &[KeyCode] = &[KeyCode::Down, KeyCode::Char('s'), KeyCode::Char('j')];
        const RIGHT_CODES: &[KeyCode] = &[KeyCode::Right, KeyCode::Char('d'), KeyCode::Char('l')];

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
