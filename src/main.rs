use jack::Client as JackClient;
use thiserror::*;

use crossterm::{cursor, event, style, terminal, ExecutableCommand, QueueableCommand};
use std::io::{self, Read, Write};
mod config;
mod graph;
mod model;
use graph::JackGraph;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Jack(#[from] jack::Error),
}

fn main() {
    let jackclient = initialze_jack().unwrap();
    let mut stdout = io::stdout();
    terminal::enable_raw_mode().unwrap();
    let stdout = stdout
        .queue(event::EnableMouseCapture)
        .unwrap()
        .queue(terminal::EnterAlternateScreen)
        .unwrap()
        .queue(cursor::Hide)
        .unwrap();
    stdout.flush().unwrap();
    let mut selected_idx = 0;
    let mut modes = Vec::new();
    let mut config: config::LockConfig = std::env::args()
        .last()
        .and_then(|path| std::fs::OpenOptions::new().read(true).open(path).ok())
        .and_then(|mut fl| {
            let mut buffer = String::new();
            fl.read_to_string(&mut buffer).ok()?;
            Some(buffer)
        })
        .and_then(|data| toml::from_str(&data).ok())
        .unwrap_or_default();
    loop {
        stdout
            .queue(terminal::Clear(terminal::ClearType::All))
            .unwrap()
            .queue(cursor::MoveTo(0, 0))
            .unwrap()
            .flush()
            .unwrap();
        let graph: JackGraph = JackGraph::parse_client(&jackclient).unwrap();
        for (idx, client) in graph.all_clients().enumerate() {
            let mode = modes
                .iter()
                .find(|(mdx, _)| idx == *mdx)
                .copied()
                .map_or(0, |(_, m)| m);
            let attr = if idx == selected_idx {
                style::Attribute::Reverse
            } else {
                style::Attribute::Reset
            };
            let client_lock = config.client_status(client);
            let client_lock_str = match client_lock {
                config::LockStatus::None => "<None  >",
                config::LockStatus::Force => "<Forced>",
                config::LockStatus::Block => "<Block >",
                config::LockStatus::Full => "<Full  >",
            };
            write!(
                stdout,
                "{}{:02} : {} ({})",
                attr, idx, client, client_lock_str
            )
            .unwrap();
            stdout.execute(cursor::MoveToNextLine(1)).unwrap();
            if mode == 0 {
                continue;
            }
            for (port_name, port_connections) in graph.client_connections(client) {
                let data = jackclient.port_by_name(&port_name.as_ref()).unwrap();
                let is_midi = data.port_type().unwrap().to_lowercase().contains("midi");
                let is_input = data.flags().contains(jack::PortFlags::IS_INPUT);

                let port_lock = config.port_status(port_name);
                let port_lock_str = match port_lock {
                    config::LockStatus::None => "<None  >",
                    config::LockStatus::Force => "<Forced>",
                    config::LockStatus::Block => "<Block >",
                    config::LockStatus::Full => "<Full  >",
                };

                let arrow = match (is_midi, is_input) {
                    (true, true) => "<-M-",
                    (false, true) => "<-A-",
                    (true, false) => "-M->",
                    (false, false) => "-A->",
                };
                write!(
                    stdout,
                    "     |{} {} ({})",
                    arrow,
                    port_name.port_shortname(),
                    port_lock_str
                )
                .unwrap();
                stdout.execute(cursor::MoveToNextLine(1)).unwrap();
                if mode <= 1 {
                    continue;
                }
                for con_name in port_connections {
                    let con_lock = config.connection_status(port_name, con_name);
                    let con_lock_str = match con_lock {
                        config::LockStatus::None => "<None  >",
                        config::LockStatus::Force => "<Forced>",
                        config::LockStatus::Block => "<Block >",
                        config::LockStatus::Full => "<Full  >",
                    };
                    write!(
                        stdout,
                        "           |{} {} ({})",
                        arrow,
                        con_name.as_ref(),
                        con_lock_str
                    )
                    .unwrap();
                    stdout.execute(cursor::MoveToNextLine(1)).unwrap();
                }
            }
        }
        match event::read().unwrap() {
            event::Event::Key(event::KeyEvent {
                code: event::KeyCode::Up,
                ..
            }) => {
                eprintln!("Up.");
                selected_idx = selected_idx.saturating_sub(1);
            }
            event::Event::Key(event::KeyEvent {
                code: event::KeyCode::Down,
                ..
            }) => {
                eprintln!("Down.");
                selected_idx = selected_idx.saturating_add(1);
            }
            event::Event::Key(event::KeyEvent {
                code: event::KeyCode::Enter,
                ..
            })
            | event::Event::Key(event::KeyEvent {
                code: event::KeyCode::Char(' '),
                ..
            }) => {
                eprintln!("Select.");
                let evt = modes.iter_mut().find(|(idx, _)| *idx == selected_idx);
                let mode = match evt {
                    Some((_, md)) => md,
                    None => {
                        modes.push((selected_idx, 3u8));
                        &mut modes.last_mut().unwrap().1
                    }
                };
                *mode = (*mode).saturating_add(1) % 3;
            }
            event::Event::Key(event::KeyEvent {
                code: event::KeyCode::Char('c'),
                modifiers: event::KeyModifiers::CONTROL,
            }) => {
                stdout.queue(event::DisableMouseCapture).unwrap();
                stdout.queue(cursor::Show).unwrap();
                stdout.queue(terminal::LeaveAlternateScreen).unwrap();
                stdout.flush().unwrap();
                terminal::disable_raw_mode().unwrap();
                return;
            }
            event::Event::Key(event::KeyEvent {
                code: event::KeyCode::Char('k'),
                ..
            }) => {
                for prt in graph.all_ports() {
                    if config.get_port_lock(prt).is_none() {
                        let client_lock = config.client_status(prt.client_name());
                        config.set_port_lock(prt.clone(), client_lock);
                    }
                }
                for (a, b) in graph.all_connections() {
                    config.add_connection(a.clone(), b.clone());
                }
                for cl in graph.all_clients() {
                    if config.get_client_lock(cl).is_none() {
                        config.set_client_lock(cl.to_owned(), config::LockStatus::None);
                    }
                }
            }
            event::Event::Key(event::KeyEvent {
                code: event::KeyCode::Char('d'),
                ..
            }) => {
                let mut outfile = std::fs::OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .write(true)
                    .open("outconf.toml")
                    .unwrap();
                let outdata = toml::to_string_pretty(&config).unwrap();
                write!(&mut outfile, "{}", outdata).unwrap();
            }
            other => {
                eprintln!("Other: {:?}", other);
            }
        }
    }
}

fn initialze_jack() -> Result<JackClient, Error> {
    let (client, _) = jack::Client::new("Terj", jack::ClientOptions::NO_START_SERVER)?;
    Ok(client)
}
