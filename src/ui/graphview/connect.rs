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
pub struct AddConnectionState {
    port: PortData,
    selected_idx: ListState,
}

impl AddConnectionState {
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
        let cur_itr = available_ports(&self.port, graph, conf);
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
        let con = available_ports(&self.port, graph, locks).nth(idx);
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
            AddConnectionEvent::MoveUp => {
                let cur = self.selected_idx.selected();
                let nxt = match cur {
                    Some(n) => n.checked_sub(1),
                    None => Some(0),
                };
                self.selected_idx.select(nxt);
                Ok(Some(UiAction::Redraw))
            }
            AddConnectionEvent::MoveDown => {
                let cur = self.selected_idx.selected();
                let nxt = match cur {
                    Some(n) => n.checked_add(1),
                    None => Some(0),
                };
                self.selected_idx.select(nxt);
                Ok(Some(UiAction::Redraw))
            }
            AddConnectionEvent::Cancel => {
                self.selected_idx.select(None);
                Ok(Some(UiAction::Close))
            }
            AddConnectionEvent::Select => Ok(Some(UiAction::Close)),
        }
    }
}

pub struct AddConnectionWidget<'a> {
    graph: &'a JackGraph,
    conf: &'a LockConfig,
}

impl<'a> AddConnectionWidget<'a> {
    pub fn new(graph: &'a JackGraph, conf: &'a LockConfig) -> Self {
        Self { graph, conf }
    }
}

impl<'a> AddConnectionWidget<'a> {
    pub fn dims(&self, state: &AddConnectionState) -> (u16, u16) {
        let (max_item_size, count) = available_ports(&state.port, self.graph, self.conf)
            .map(|data| data.name.as_ref().len())
            .fold((0, 0), |(w, h), cur_width| (w.max(cur_width), h + 1));

        const TITLE_LEN: usize = "Select New Port".len() + 3;
        let item_width = max_item_size.max(TITLE_LEN);
        let item_width = item_width as u16;
        let w = item_width + 4; // Left border + left padding + right border + right padding
        let count = count.max(1);
        let h = (count as u16) + 2; // Top border + bottom border
        (w, h)
    }
}

impl<'a> StatefulWidget for AddConnectionWidget<'a> {
    type State = AddConnectionState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.resolve_tree_state(self.graph, self.conf);
        let graph: &JackGraph = self.graph;
        let conf: &LockConfig = self.conf;
        let selected: &mut ListState = &mut state.selected_idx;
        let port: &PortData = &state.port;

        let available_iter = available_ports(port, graph, conf);

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

fn available_ports<'a, 'b: 'a>(
    port: &'a PortData,
    graph: &'b JackGraph,
    conf: &'a LockConfig,
) -> impl Iterator<Item = &'b PortData> + 'a {
    graph.all_ports().filter(move |cur| {
        cur.name != port.name
            && cur.category == port.category
            && cur.direction == port.direction.flip()
            && !graph.is_connected(&port.name, &cur.name)
            && !conf.connection_status(&port.name, &cur.name).should_block()
    })
}

fn make_block<'a>() -> Block<'a> {
    let title_style = Style::default()
        .add_modifier(Modifier::BOLD)
        .add_modifier(Modifier::UNDERLINED);
    let title = Span::styled("Select New Port", title_style);
    Block::default()
        .borders(Borders::all())
        .border_type(BorderType::Double)
        .border_style(Style::default().add_modifier(Modifier::BOLD))
        .title(title)
}

enum AddConnectionEvent {
    MoveUp,
    MoveDown,
    Cancel,
    Select,
}

impl TryFrom<event::KeyEvent> for AddConnectionEvent {
    type Error = ();
    fn try_from(value: event::KeyEvent) -> Result<Self, Self::Error> {
        const UP_CODES: &[KeyCode] = &[KeyCode::Up, KeyCode::Char('w'), KeyCode::Char('k')];
        const DOWN_CODES: &[KeyCode] = &[KeyCode::Down, KeyCode::Char('s'), KeyCode::Char('j')];

        let code = value.code;

        if UP_CODES.contains(&code) {
            Ok(AddConnectionEvent::MoveUp)
        } else if DOWN_CODES.contains(&code) {
            Ok(AddConnectionEvent::MoveDown)
        } else if code == KeyCode::Esc || code == KeyCode::Backspace {
            Ok(AddConnectionEvent::Cancel)
        } else if code == KeyCode::Enter {
            Ok(AddConnectionEvent::Select)
        } else {
            Err(())
        }
    }
}

impl TryFrom<event::Event> for AddConnectionEvent {
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
