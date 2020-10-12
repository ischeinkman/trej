use crate::config::LockConfig;
use crate::graph::JackGraph;
use crate::model::{PortCategory, PortData, PortDirection};

use std::borrow::Cow;

use tui::buffer::Buffer;
use tui::layout::{Constraint, Corner, Rect};
use tui::style::{Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Block, BorderType, Borders, List, ListItem, Widget};

pub fn make_default_dataview(_graph: &JackGraph, _conf: &LockConfig) -> impl Widget {
    DataviewWidget::new([])
}

pub fn make_client_dataview(
    graph: &JackGraph,
    _conf: &LockConfig,
    client_name: &str,
) -> impl Widget {
    let (midi_inputs, midi_outputs, audio_inputs, audio_outputs) = graph
        .client_ports(client_name)
        .map(|port| match (port.category, port.direction) {
            (PortCategory::Midi, PortDirection::In) => (1, 0, 0, 0),
            (PortCategory::Midi, PortDirection::Out) => (0, 1, 0, 0),
            (PortCategory::Audio, PortDirection::In) => (0, 0, 1, 0),
            (PortCategory::Audio, PortDirection::Out) => (0, 0, 0, 1),
            (PortCategory::Unknown, _) => (0, 0, 0, 0),
        })
        .fold((0, 0, 0, 0), |acc, cur| {
            (acc.0 + cur.0, acc.1 + cur.1, acc.2 + cur.2, acc.3 + cur.3)
        });

    let client_widget = DataField::new("Name", format!("\"{}\"", client_name));
    let midiin_widget = DataField::new("Midi Inputs", format!("{}", midi_inputs));

    let midiout_widget = DataField::new("Midi Outputs", format!("{}", midi_outputs));
    let audioin_widget = DataField::new("Audio Inputs", format!("{}", audio_inputs));
    let audioout_widget = DataField::new("Audio Outputs", format!("{}", audio_outputs));
    DataviewWidget::new([
        client_widget,
        midiin_widget,
        midiout_widget,
        audioin_widget,
        audioout_widget,
    ])
}

pub fn make_port_dataview(_graph: &JackGraph, _conf: &LockConfig, port: &PortData) -> impl Widget {
    let kind = match (port.category, port.direction) {
        (PortCategory::Audio, PortDirection::In) => "Audio Input",
        (PortCategory::Audio, PortDirection::Out) => "Audio Output",
        (PortCategory::Midi, PortDirection::In) => "Midi Input",
        (PortCategory::Midi, PortDirection::Out) => "Midi Output",
        (PortCategory::Unknown, PortDirection::In) => "Unknown Input",
        (PortCategory::Unknown, PortDirection::Out) => "Unknown Output",
    };

    let client_widget = DataField::new("Client", format!("\"{}\"", port.name.client_name()));
    let name_widget = DataField::new("Name", format!("\"{}\"", port.name.port_shortname()));
    let kind_widget = DataField::new("Kind", kind);

    DataviewWidget::new([name_widget, client_widget, kind_widget])
}

pub fn make_connection_dataview<'a>(
    _graph: &'a JackGraph,
    _conf: &'a LockConfig,
    port_a: &'a PortData,
    port_b: &'a PortData,
) -> impl Widget + 'a {
    let (input_port, output_port) = if port_a.direction.is_input() {
        (port_a, port_b)
    } else {
        (port_b, port_a)
    };
    let data_kind = match port_a.category {
        PortCategory::Midi => "Midi",
        PortCategory::Audio => "Audio",
        PortCategory::Unknown => "Unknown",
    };

    let output_widget = DataField::new("Sending Port", output_port.name.as_ref());
    let input_widget = DataField::new("Receiving Port", input_port.name.as_ref());

    let data_widget = DataField::new("Data Kind", data_kind);

    DataviewWidget::new([output_widget, input_widget, data_widget])
}

fn dataview_block<'a>() -> Block<'a> {
    Block::default()
        .title("Info")
        .borders(Borders::all())
        .border_type(BorderType::Rounded)
}

pub struct DataField<'a> {
    name: Cow<'a, str>,
    value: Cow<'a, str>,
}

impl<'a> DataField<'a> {
    pub fn new<A: Into<Cow<'a, str>>, B: Into<Cow<'a, str>>>(name: A, value: B) -> Self {
        let name = name.into();
        let value = value.into();
        Self { name, value }
    }
    pub fn name_width(&self) -> usize {
        let name = Cow::Borrowed(&*self.name);
        Span::raw(name).width()
    }
    pub fn value_width(&self) -> usize {
        let value = Cow::Borrowed(&*self.value);
        Span::raw(value).width()
    }
}

pub struct DataviewWidget<'a, T: AsRef<[DataField<'a>]> + 'a> {
    block: Block<'a>,
    name_style: Style,
    value_style: Style,
    margins: Constraint,
    fields: T,
}

impl<'a, T: AsRef<[DataField<'a>]> + 'a> DataviewWidget<'a, T> {
    pub fn new(fields: T) -> Self {
        Self {
            block: dataview_block(),
            name_style: Style::default().add_modifier(Modifier::UNDERLINED),
            value_style: Style::default(),
            margins: Constraint::Percentage(30),
            fields,
        }
    }
}

impl<'a, T: AsRef<[DataField<'a>]> + 'a> Widget for DataviewWidget<'a, T> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let inner = self.block.inner(area);
        self.block.render(area, buf);
        let area = inner;

        let mut name_rect = Rect {
            width: area.width / 2,
            ..area
        };

        let mut value_rect = Rect {
            width: area.width - name_rect.width,
            x: area.x + name_rect.width,
            ..area
        };

        let (name_width, value_width) = self
            .fields
            .as_ref()
            .iter()
            .map(|field| (field.name_width(), field.value_width()))
            .fold((0, 0), |(namea, valuea), (nameb, valueb)| {
                (namea.max(nameb), valuea.max(valueb))
            });
        let name_width = name_width + 1; // Space for the colon
        let margin = self.margins.apply(area.width);
        name_rect.width -= margin;
        name_rect.x += margin;

        value_rect.width -= margin;

        let name_style = self.name_style;
        let value_style = self.value_style;

        let whitespace_alloc = ' '.to_string().repeat(
            name_width
                .max(value_width)
                .max(name_rect.width.into())
                .max(value_rect.width.into()),
        );
        let (name_items, value_items) = self
            .fields
            .as_ref()
            .iter()
            .map(|field| {
                let name_span = Spans(vec![
                    Span::styled(field.name.as_ref(), name_style),
                    Span::styled(":", name_style),
                ]);
                let value_span = Span::styled(field.value.as_ref(), value_style);
                let value_prefix_len =
                    usize::from(value_rect.width).saturating_sub(value_span.width());
                let value_prefix = &whitespace_alloc[..value_prefix_len];
                let value_span = Spans(vec![Span::raw(value_prefix), value_span]);
                (ListItem::new(name_span), ListItem::new(value_span))
            })
            .fold(
                (Vec::new(), Vec::new()),
                |(mut name_acc, mut value_acc), (name, value)| {
                    name_acc.push(name);
                    value_acc.push(value);
                    (name_acc, value_acc)
                },
            );
        List::new(name_items)
            .start_corner(Corner::TopLeft)
            //.block(Block::default().borders(Borders::all()))
            .render(name_rect, buf);
        List::new(value_items)
            .start_corner(Corner::BottomRight)
            //.block(Block::default().borders(Borders::all()))
            .render(value_rect, buf);
    }
}
