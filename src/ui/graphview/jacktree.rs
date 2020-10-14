use tui::buffer::Buffer;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Modifier, Style};
use tui::text::{Span, Text};
use tui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, StatefulWidget};

use crate::graph::JackGraph;

use super::TreePath;

#[derive(Debug, Default)]
pub struct JackTreeState {
    client_state: ListState,
    port_state: ListState,
    connection_state: ListState,
}

impl JackTreeState {
    pub fn select(&mut self, path: TreePath) {
        self.client_state.select(path.client_idx());
        self.port_state.select(path.port_idx());
        self.connection_state.select(path.connection_idx());
    }
    pub fn selected(&self) -> TreePath {
        let client_idx = self.client_state.selected();
        let port_idx = self.port_state.selected();
        let connection_idx = self.connection_state.selected();
        TreePath::new(client_idx, port_idx, connection_idx)
    }
}
pub struct JackTree<'a> {
    graph: &'a JackGraph,
}

impl<'a> JackTree<'a> {
    pub fn new(graph: &'a JackGraph) -> Self {
        Self { graph }
    }
}

impl<'a> StatefulWidget for JackTree<'a> {
    type State = JackTreeState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let graph = self.graph;
        let selected = state.selected();
        let (client_list, longest_client, selected_client) = make_list(
            graph.all_clients(),
            |a| a,
            selected.client_idx(),
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
            selected.port_idx(),
            "Ports",
            false,
        );

        let con_itr = selected_port
            .map(|prt| graph.port_connections(&prt.name))
            .into_iter()
            .flatten();

        let (con_list, longest_con, _selected_con) = make_list(
            con_itr,
            |data| data.name.as_ref(),
            selected.connection_idx(),
            "Connections",
            true,
        );

        let mut layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33),
                Constraint::Percentage(33),
                Constraint::Percentage(33),
            ])
            .split(area);
        let longest_client = longest_client + 1;
        let longest_port = longest_port + 1;
        let longest_con = longest_con + 1;
        respace_rects(&mut layout, &[longest_client, longest_port, longest_con]);

        let con_rect = layout.pop().unwrap();
        let port_rect = layout.pop().unwrap();
        let client_rect = layout.pop().unwrap();

        StatefulWidget::render(client_list, client_rect, buf, &mut state.client_state);
        StatefulWidget::render(port_list, port_rect, buf, &mut state.port_state);
        StatefulWidget::render(con_list, con_rect, buf, &mut state.connection_state);
    }
}

fn make_list<'a, Itm, Itr, F, S>(
    itr: Itr,
    mapper: F,
    selected: Option<usize>,
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
        if selected == Some(idx) {
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

fn respace_rects(rects: &mut [Rect], minimums: &[u16]) {
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
