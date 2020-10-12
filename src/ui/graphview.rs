use super::TreePath;
use crate::config::LockConfig;
use crate::graph::JackGraph;
use crossterm::event;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;
use tui::backend::Backend;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Modifier, Style};
use tui::text::{Span, Text};
use tui::widgets::{Block, BorderType, Borders, List, ListItem, ListState};
use tui::Terminal;

mod datapanel;
use datapanel::*;

pub struct GraphUiState {
    pub graph: JackGraph,
    pub conf: LockConfig,
    selected_states: (ListState, ListState, ListState),
}

impl GraphUiState {
    pub fn new(graph: JackGraph, conf: LockConfig) -> Self {
        let selected_states = (
            ListState::default(),
            ListState::default(),
            ListState::default(),
        );
        Self {
            graph,
            conf,
            selected_states,
        }
    }
    fn set_selected_path(&mut self, path: TreePath) {
        eprintln!(
            "{:?}, Moving selection {:?} => {:?}",
            std::time::Instant::now(),
            self.selected_path(),
            path
        );
        let client_state = path.client_offset().checked_sub(1);
        let port_state = path.port_offset().checked_sub(1);
        let connection_state = path.connection_offset().checked_sub(1);
        self.selected_states.0.select(client_state);
        self.selected_states.1.select(port_state);
        self.selected_states.2.select(connection_state);
    }
    fn selected_path(&self) -> TreePath {
        let client_offset = self.selected_states.0.selected().map_or(0, |n| n + 1);
        let port_offset = self.selected_states.1.selected().map_or(0, |n| n + 1);
        let connection_offset = self.selected_states.2.selected().map_or(0, |n| n + 1);
        TreePath::from_offsets(client_offset, port_offset, connection_offset)
    }
    pub fn display<B: Backend>(&mut self, output: &mut Terminal<B>) -> Result<(), crate::Error> {
        output.draw(|f| {
            let selected = self.selected_path();
            let graph = &self.graph;
            let conf = &self.conf;

            let (client_list, longest_client, selected_client) = make_list(
                graph.all_clients(),
                |a| a,
                selected.client_offset(),
                "Clients",
                false,
            );

            let port_itr = selected_client
                .map(|cli| graph.client_ports(cli))
                .into_iter()
                .flatten();

            let (port_list, longest_port, selected_port) = make_list(
                port_itr,
                |data| data.name.port_shortname(),
                selected.port_offset(),
                "Ports",
                false,
            );

            let con_itr = selected_port
                .map(|prt| graph.port_connections(&prt.name))
                .into_iter()
                .flatten();

            let (con_list, longest_con, selected_con) = make_list(
                con_itr,
                |data| data.name.as_ref(),
                selected.connection_offset(),
                "Connections",
                true,
            );

            let mut height_resolver = Layout::default()
                .constraints([Constraint::Ratio(2, 3), Constraint::Ratio(1, 3)])
                .split(f.size());

            let info_rect = height_resolver.pop().unwrap();
            let list_rect = height_resolver.pop().unwrap();

            let mut layout = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(33),
                    Constraint::Percentage(33),
                    Constraint::Percentage(33),
                ])
                .split(list_rect);
            let longest_client = longest_client + 1;
            let longest_port = longest_port + 1;
            let longest_con = longest_con + 1;
            respace_rects(&mut layout, &[longest_client, longest_port, longest_con]);

            let con_rect = layout.pop().unwrap();
            let port_rect = layout.pop().unwrap();
            let client_rect = layout.pop().unwrap();

            match (selected_client, selected_port, selected_con) {
                (None, None, None) => {
                    let info = make_default_dataview(graph, conf);
                    f.render_widget(info, info_rect);
                }
                (Some(client), None, None) => {
                    let info = make_client_dataview(graph, conf, client);
                    f.render_widget(info, info_rect);
                }
                (Some(_client), Some(port), None) => {
                    let info = make_port_dataview(graph, conf, port);
                    f.render_widget(info, info_rect);
                }
                (Some(_client), Some(port_a), Some(port_b)) => {
                    let info = make_connection_dataview(graph, conf, port_a, port_b);
                    f.render_widget(info, info_rect);
                }
                _ => todo!(),
            }

            f.render_stateful_widget(client_list, client_rect, &mut self.selected_states.0);
            f.render_stateful_widget(port_list, port_rect, &mut self.selected_states.1);
            f.render_stateful_widget(con_list, con_rect, &mut self.selected_states.2);
        })?;
        Ok(())
    }

    pub fn handle_event(&mut self, evt: GraphUiEvent) -> Result<ShouldShutdown, crate::Error> {
        eprintln!("{:?}, Got event: {:?}", std::time::Instant::now(), evt);
        match evt {
            GraphUiEvent::Quit => Ok(true),
            GraphUiEvent::MoveUp => {
                let cur = self.selected_path();
                let mut nxt = cur
                    .prev_sibling()
                    .filter(|path| path_is_valid(&self.graph, *path))
                    .or_else(|| cur.parent())
                    .unwrap_or(TreePath::Root);
                if cur == TreePath::Root && nxt == TreePath::Root {
                    nxt = nxt.nth_child(0);
                }
                self.set_selected_path(nxt);
                Ok(false)
            }
            GraphUiEvent::MoveDown => {
                let cur = self.selected_path();
                let mut nxt = cur
                    .next_sibling()
                    .filter(|path| path_is_valid(&self.graph, *path))
                    .or_else(|| cur.parent())
                    .unwrap_or(TreePath::Root);
                if cur == TreePath::Root && nxt == TreePath::Root {
                    nxt = nxt.nth_child(0);
                }
                self.set_selected_path(nxt);
                Ok(false)
            }
            GraphUiEvent::MoveLeft => {
                let cur = self.selected_path();
                let nxt = cur
                    .parent()
                    .filter(|path| path_is_valid(&self.graph, *path))
                    .unwrap_or(TreePath::Root);
                self.set_selected_path(nxt);
                Ok(false)
            }
            GraphUiEvent::MoveRight => {
                let cur = self.selected_path();
                let nxt = cur.nth_child(0);
                let nxt = if path_is_valid(&self.graph, nxt) {
                    eprintln!(
                        "{:?}, Moveright: trying {:?} => {:?}, SUCCEED",
                        std::time::Instant::now(),
                        cur,
                        nxt
                    );
                    nxt
                } else {
                    eprintln!(
                        "{:?}, Moveright: trying {:?} => {:?}, FAILED",
                        std::time::Instant::now(),
                        cur,
                        nxt
                    );
                    cur
                };
                self.set_selected_path(nxt);
                Ok(false)
            }
            GraphUiEvent::Refresh => {
                self.graph.update()?;
                Ok(false)
            }
        }
    }

    pub fn poll_event(
        &mut self,
        timeout: Option<Duration>,
    ) -> Result<Option<GraphUiEvent>, crate::Error> {
        if self.graph.needs_update() {
            return Ok(Some(GraphUiEvent::Refresh));
        }
        let ui_poll_res = event::poll(timeout.unwrap_or_else(|| Duration::from_micros(0)));
        let ui_evt_res = ui_poll_res.and_then(|val| match val {
            true => event::read().map(Some),
            false => Ok(None),
        });
        match ui_evt_res {
            Ok(Some(raw_evt)) => Ok(resolve_crossterm_event(raw_evt)),
            Ok(None) if self.graph.needs_update() => Ok(Some(GraphUiEvent::Refresh)),
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
            let n = match $offset.checked_sub(1) {
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
    let client = resolve!(path.client_offset(), graph.all_clients());
    let port = resolve!(path.port_offset(), graph.client_ports(client));
    let _ = resolve!(path.connection_offset(), graph.port_connections(&port.name));
    true
}

fn resolve_crossterm_event(raw: event::Event) -> Option<GraphUiEvent> {
    match raw {
        event::Event::Key(KeyEvent { code, modifiers }) => {
            resolve_crossterm_keyevent(code, modifiers)
        }
        event::Event::Mouse(_mouseevent) => {
            eprintln!("TODO: handle mouse event of {:?}", _mouseevent);
            None
        }
        event::Event::Resize(_cols, _rows) => {
            eprintln!("TODO: handle resize event of {}, {}", _cols, _rows);
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

fn make_list<'a, Itm, Itr, F, S>(
    itr: Itr,
    mapper: F,
    selected: usize,
    title: &'a str,
    last: bool,
) -> (List, u16, Option<&'a Itm>)
where
    Itm: ?Sized + 'a,
    Itr: Iterator<Item = &'a Itm>,
    F: FnMut(&'a Itm) -> S,
    S: Into<Text<'a>>,
{
    let mut mapper = mapper;
    let mut lst = Vec::new();
    let mut selected_item = None;
    let mut longest_entry = title.len();
    for (idx, data) in itr.enumerate() {
        if selected == idx + 1 {
            selected_item = Some(data);
        }
        let entstr: Text<'a> = mapper(data).into();
        if entstr.width() > longest_entry {
            longest_entry = entstr.width();
        }
        lst.push(ListItem::new(entstr));
    }
    let longest_entry = longest_entry as u16;
    let border = if last { Borders::NONE } else { Borders::RIGHT };
    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default().add_modifier(Modifier::UNDERLINED),
        ))
        .border_type(BorderType::Plain)
        .borders(border);
    let component = List::new(lst)
        .block(block)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    (component, longest_entry, selected_item)
}

fn respace_rects(rects: &mut [tui::layout::Rect], minimums: &[u16]) {
    let mut extra_space = 0;
    // Collect all the extra space
    for idx in 0..rects.len() {
        let min_len = minimums.get(idx).copied().unwrap_or_else(u16::max_value);
        let cur_rect = rects.get_mut(idx).unwrap();
        if cur_rect.width <= min_len {
            continue;
        }
        let diff = cur_rect.width.saturating_sub(min_len);
        cur_rect.width = min_len;
        for next_rect in rects.iter_mut().skip(idx + 1) {
            next_rect.x -= diff;
        }
        extra_space += diff;
    }

    // Distribute the minimums
    let mut finished = false;
    while extra_space > 0 && !finished {
        finished = true;
        for idx in 0..rects.len() {
            let cur_rect = rects.get_mut(idx).unwrap();
            let cur_min = minimums.get(idx).copied().unwrap_or(0);
            let needed = cur_min.saturating_sub(cur_rect.width);
            if needed == 0 {
                continue;
            }

            let to_add = extra_space.min(needed);
            cur_rect.width += to_add;
            if cur_rect.width < cur_min {
                finished = false;
            }
            for next_rect in rects.iter_mut().skip(idx + 1) {
                next_rect.x += to_add;
            }
            extra_space -= to_add;
            if extra_space == 0 {
                break;
            }
        }
    }

    // Distribute the extra
    rects.last_mut().unwrap().width += extra_space;
}
