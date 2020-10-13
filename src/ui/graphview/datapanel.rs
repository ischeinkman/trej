use crate::config::{LockConfig, LockStatus};
use crate::graph::JackGraph;
use crate::model::{PortCategory, PortData, PortDirection};

use std::borrow::Cow;

use tui::buffer::Buffer;
use tui::layout::{Constraint, Corner, Rect};
use tui::style::{Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Block, BorderType, Borders, List, ListItem, Widget};

/// Used to wrap the field list on the dataview in a generic way
/// so that `make_dataview` returns a single unified type no matter
/// the number of fields per dataview kind.
enum ArrayWrapper<DefaultArr, ClientArr, PortArr, ConArr> {
    Default(DefaultArr),
    Client(ClientArr),
    Port(PortArr),
    Con(ConArr),
}

impl<DefaultArr, ClientArr, PortArr, ConArr> ArrayWrapper<DefaultArr, ClientArr, PortArr, ConArr> {}

impl<T, DefaultArr, ClientArr, PortArr, ConArr> AsRef<[T]>
    for ArrayWrapper<DefaultArr, ClientArr, PortArr, ConArr>
where
    DefaultArr: AsRef<[T]>,
    ClientArr: AsRef<[T]>,
    PortArr: AsRef<[T]>,
    ConArr: AsRef<[T]>,
{
    fn as_ref(&self) -> &[T] {
        match self {
            ArrayWrapper::Default(a) => a.as_ref(),
            ArrayWrapper::Client(a) => a.as_ref(),
            ArrayWrapper::Port(a) => a.as_ref(),
            ArrayWrapper::Con(a) => a.as_ref(),
        }
    }
}

/// Makes the default, root-level data view panel.
fn make_default_dataview<'a>(
    _graph: &JackGraph,
    _conf: &LockConfig,
) -> DataviewWidget<'a, impl AsRef<[DataField<'a>]> + 'a> {
    DataviewWidget::new([])
}

/// Makes the data view panel for a JACK Client. 
fn make_client_dataview<'a>(
    graph: &JackGraph,
    conf: &LockConfig,
    client_name: &str,
) -> DataviewWidget<'a, impl AsRef<[DataField<'a>]> + 'a> {
    let lock = conf.client_status(client_name);
    let lock_str = match lock {
        LockStatus::None => "Unlocked",
        LockStatus::Block => "Blocking New",
        LockStatus::Force => "Forcing Old",
        LockStatus::Full => "Locked",
    };
    let lock_widget = DataField::new("Lock Status", lock_str);
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
        lock_widget,
    ])
}

/// Makes the data view panel for a JACK Port. 
fn make_port_dataview<'a>(
    _graph: &JackGraph,
    conf: &LockConfig,
    port: &PortData,
) -> DataviewWidget<'a, impl AsRef<[DataField<'a>]> + 'a> {
    let lock = conf.port_status(&port.name);
    let lock_str = match lock {
        LockStatus::None => "Unlocked",
        LockStatus::Block => "Blocking New",
        LockStatus::Force => "Forcing Old",
        LockStatus::Full => "Locked",
    };
    let lock_widget = DataField::new("Lock Status", lock_str);
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

    DataviewWidget::new([name_widget, client_widget, kind_widget, lock_widget])
}

/// Makes the data view panel for a connection between two ports.  
fn make_connection_dataview<'a>(
    _graph: &'a JackGraph,
    conf: &'a LockConfig,
    port_a: &'a PortData,
    port_b: &'a PortData,
) -> DataviewWidget<'a, impl AsRef<[DataField<'a>]> + 'a> {
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
    let lock = conf.connection_status(&input_port.name, &output_port.name);
    let lock_str = match lock {
        LockStatus::None => "Unlocked",
        LockStatus::Block => "Unlocked",
        LockStatus::Force => "Locked",
        LockStatus::Full => "Locked",
    };
    let lock_widget = DataField::new("Lock Status", lock_str);

    let output_widget = DataField::new("Sending Port", output_port.name.as_ref());
    let input_widget = DataField::new("Receiving Port", input_port.name.as_ref());

    let data_widget = DataField::new("Data Kind", data_kind);

    DataviewWidget::new([output_widget, input_widget, data_widget, lock_widget])
}


/// Makes the `Block` that wraps the data view panel.
fn dataview_block<'a>() -> Block<'a> {
    Block::default()
        .title("Info")
        .borders(Borders::all())
        .border_type(BorderType::Rounded)
}

pub struct DataviewWidget<'a, T> {
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

    /// Maps the inner `fields` type using the provided mapper.
    /// Mainly used in `make_dataview` to wrap the `fields` list
    /// in an `ArrayWrapper`, which means we can unify all field
    /// list counts into a single enum without having to specify it.
    fn map_items<U, F>(self, mapper: F) -> DataviewWidget<'a, U>
    where
        U: AsRef<[DataField<'a>]> + 'a,
        F: FnOnce(T) -> U,
    {
        DataviewWidget {
            fields: (mapper)(self.fields),
            block: self.block,
            name_style: self.name_style,
            value_style: self.value_style,
            margins: self.margins,
        }
    }
}

pub fn make_dataview<'a>(
    path: super::TreePath,
    graph: &'a JackGraph,
    conf: &'a LockConfig,
) -> DataviewWidget<'a, impl AsRef<[DataField<'a>]> + 'a> {
    let res = match path {
        super::TreePath::Root => {
            make_default_dataview(graph, conf).map_items(ArrayWrapper::Default)
        }
        super::TreePath::Client { client } => {
            let client_name = graph.all_clients().nth(client).unwrap();
            make_client_dataview(graph, conf, client_name).map_items(ArrayWrapper::Client)
        }
        super::TreePath::Port { client, port } => {
            let client_name = graph.all_clients().nth(client);
            let port = client_name
                .and_then(|c| graph.client_ports(c).nth(port))
                .unwrap();
            make_port_dataview(graph, conf, port).map_items(ArrayWrapper::Port)
        }
        super::TreePath::Connection {
            client,
            port,
            connection,
        } => {
            let client_name = graph.all_clients().nth(client);
            let port = client_name.and_then(move |c| graph.client_ports(c).nth(port));
            let con = port
                .as_ref()
                .and_then(move |p| graph.port_connections(&p.name).nth(connection));

            let (port_a, port_b) = port.zip(con).unwrap();
            make_connection_dataview(graph, conf, port_a, port_b).map_items(ArrayWrapper::Con)
        }
    };
    res
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
            .render(name_rect, buf);
        List::new(value_items)
            .start_corner(Corner::BottomRight)
            .render(value_rect, buf);
    }
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
