use jack::Client as JackClient;
use thiserror::*;

use crossterm::{cursor, event, style, terminal, ExecutableCommand, QueueableCommand};
use std::io::{self, Write};
mod config;
mod model;
mod graph;
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
            let mode = modes.iter().find(|(mdx, _)| idx == *mdx).copied().map_or(0, |(_, m)| m);
            let attr = if idx == selected_idx {
                style::Attribute::Reverse
            } else {
                style::Attribute::Reset
            };
            write!(stdout, "{}{:02} : {}", attr, idx, client).unwrap();
            stdout.execute(cursor::MoveToNextLine(1)).unwrap();
            if mode == 0 {
                continue;
            }
            for (port_name, port_connections) in graph.client_connections(client) {
                let data = jackclient.port_by_name(&port_name.as_ref()).unwrap();
                let is_midi = data.port_type().unwrap().to_lowercase().contains("midi");
                let is_input = data.flags().contains(jack::PortFlags::IS_INPUT);

                let arrow = match (is_midi, is_input) {
                    (true, true) => "<-M-",
                    (false, true) => "<-A-",
                    (true, false) => "-M->",
                    (false, false) => "-A->",
                };
                write!(stdout, "     |{} {}", arrow, port_name.port_shortname()).unwrap();
                stdout.execute(cursor::MoveToNextLine(1)).unwrap();
                if mode <= 1 {
                    continue;
                }
                for con_name in port_connections {
                    write!(stdout, "           |{} {}", arrow, con_name.as_ref()).unwrap();
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
