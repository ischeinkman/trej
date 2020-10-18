use crate::config::LockConfig;
use crate::graph::JackGraph;
use crate::model::PortData;
use crate::ui::UiAction;

use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::style::{Modifier, Style};
use tui::text::Span;
use tui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, StatefulWidget, Widget,
};

use crossterm::event::{self, KeyCode};

use std::convert::{TryFrom, TryInto};
use std::time::Duration;

#[derive(Debug)]
pub struct DelConnectionState {
    port: PortData,
    selected_idx: ListState,
}

impl DelConnectionState {
    pub fn new(port: &PortData) -> Self {
        Self {
            port: port.clone(),
            selected_idx: ListState::default(),
        }
    }
    pub fn resolve_tree_state(&mut self, graph: &JackGraph, conf: &LockConfig) {
        let cur_idx = match self.selected_idx.selected() {
            Some(n) => n,
            None => {
                return;
            }
        };
        let cur_itr = connected_ports(&self.port, graph, conf);
        let cur_available: Vec<_> = cur_itr.collect();

        if cur_idx >= cur_available.len() {
            self.selected_idx.select(Some(cur_available.len() - 1));
        }
    }
    pub fn into_selection<'a>(
        self,
        graph: &'a JackGraph,
        locks: &LockConfig,
    ) -> (PortData, Option<&'a PortData>) {
        let idx = match self.selected_idx.selected() {
            Some(n) => n,
            None => {
                return (self.port, None);
            }
        };
        let con = connected_ports(&self.port, graph, locks).nth(idx);
        (self.port, con)
    }
    pub fn handle_pending_event(
        &mut self,
        timeout: Option<Duration>,
    ) -> Result<Option<UiAction>, crate::Error> {
        if !event::poll(timeout.unwrap_or_else(|| Duration::from_micros(0)))? {
            return Ok(None);
        }
        let raw = event::read()?;
        let parsed = match raw.try_into() {
            Ok(evt) => evt,
            Err(()) => {
                return Ok(None);
            }
        };
        match parsed {
            DelConnectionEvent::MoveUp => {
                let cur = self.selected_idx.selected();
                let nxt = match cur {
                    Some(n) => n.checked_sub(1),
                    None => Some(0),
                };
                self.selected_idx.select(nxt);
                Ok(Some(UiAction::Redraw))
            }
            DelConnectionEvent::MoveDown => {
                let cur = self.selected_idx.selected();
                let nxt = match cur {
                    Some(n) => n.checked_add(1),
                    None => Some(0),
                };
                self.selected_idx.select(nxt);
                Ok(Some(UiAction::Redraw))
            }
            DelConnectionEvent::Cancel => {
                self.selected_idx.select(None);
                Ok(Some(UiAction::Close))
            }
            DelConnectionEvent::Select => Ok(Some(UiAction::Close)),
        }
    }
}

pub struct DelConnectionWidget<'a> {
    graph: &'a JackGraph,
    conf: &'a LockConfig,
}

impl<'a> DelConnectionWidget<'a> {
    pub fn new(graph: &'a JackGraph, conf: &'a LockConfig) -> Self {
        Self { graph, conf }
    }
}

impl<'a> DelConnectionWidget<'a> {
    pub fn dims(&self, state: &DelConnectionState) -> (u16, u16) {
        let (max_item_size, count) = connected_ports(&state.port, self.graph, self.conf)
            .map(|data| data.name.as_ref().len())
            .fold((0, 0), |(w, h), cur_width| (w.max(cur_width), h + 1));

        const TITLE_LEN: usize = "Connected Ports".len() + 3;
        let item_width = max_item_size.max(TITLE_LEN);
        let item_width = item_width as u16;
        let w = item_width + 4; // Left border + left padding + right border + right padding
        let count = count.max(1);
        let h = (count as u16) + 2; // Top border + bottom border
        (w, h)
    }
}

impl<'a> StatefulWidget for DelConnectionWidget<'a> {
    type State = DelConnectionState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.resolve_tree_state(self.graph, self.conf);
        let graph: &JackGraph = self.graph;
        let conf: &LockConfig = self.conf;
        let selected: &mut ListState = &mut state.selected_idx;
        let port: &PortData = &state.port;

        let available_iter = connected_ports(port, graph, conf);

        let list_items: Vec<_> = available_iter
            .map(|itm| ListItem::new(itm.name.as_ref()))
            .collect();
        let list = List::new(list_items)
            .block(make_block())
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        Widget::render(Clear {}, area, buf);
        StatefulWidget::render(list, area, buf, selected);
    }
}

fn connected_ports<'a, 'b: 'a>(
    port: &'a PortData,
    graph: &'b JackGraph,
    conf: &'a LockConfig,
) -> impl Iterator<Item = &'b PortData> + 'a {
    graph.port_connections(&port.name).filter(move |other| {
        !conf
            .connection_status(&port.name, &other.name)
            .should_force()
    })
}

fn make_block<'a>() -> Block<'a> {
    let title_style = Style::default()
        .add_modifier(Modifier::BOLD)
        .add_modifier(Modifier::UNDERLINED);
    let title = Span::styled("Connected Ports", title_style);
    Block::default()
        .borders(Borders::all())
        .border_type(BorderType::Double)
        .border_style(Style::default().add_modifier(Modifier::BOLD))
        .title(title)
}

enum DelConnectionEvent {
    MoveUp,
    MoveDown,
    Cancel,
    Select,
}

impl TryFrom<event::KeyEvent> for DelConnectionEvent {
    type Error = ();
    fn try_from(value: event::KeyEvent) -> Result<Self, Self::Error> {
        const UP_CODES: &[KeyCode] = &[KeyCode::Up, KeyCode::Char('w'), KeyCode::Char('k')];
        const DOWN_CODES: &[KeyCode] = &[KeyCode::Down, KeyCode::Char('s'), KeyCode::Char('j')];

        let code = value.code;

        if UP_CODES.contains(&code) {
            Ok(DelConnectionEvent::MoveUp)
        } else if DOWN_CODES.contains(&code) {
            Ok(DelConnectionEvent::MoveDown)
        } else if code == KeyCode::Esc || code == KeyCode::Backspace {
            Ok(DelConnectionEvent::Cancel)
        } else if code == KeyCode::Enter {
            Ok(DelConnectionEvent::Select)
        } else {
            Err(())
        }
    }
}

impl TryFrom<event::Event> for DelConnectionEvent {
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
