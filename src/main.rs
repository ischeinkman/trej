use jack::Client as JackClient;
use std::time::Duration;
use thiserror::*;

use crossterm::event;
use std::io::Read;
mod config;
mod graph;
use graph::JackGraph;
mod model;
mod ui;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Jack(#[from] jack::Error),

    #[error(transparent)]
    Terminal(#[from] crossterm::ErrorKind),

    #[error(transparent)]
    Graph(#[from] graph::GraphError),
}

fn main() {
    let jackclient = initialze_jack().unwrap();
    let stdout = ui::ScreenWrapper::new().unwrap();
    let config: config::LockConfig = std::env::args()
        .last()
        .and_then(|path| std::fs::OpenOptions::new().read(true).open(path).ok())
        .and_then(|mut fl| {
            let mut buffer = String::new();
            fl.read_to_string(&mut buffer).ok()?;
            Some(buffer)
        })
        .and_then(|data| toml::from_str(&data).ok())
        .unwrap_or_default();
    let grph = JackGraph::new(jackclient).unwrap();
    let update_flag = grph.update_flag();
    let mut gui = ui::GraphUi::new(grph, config, stdout);
    loop {
        gui.display().unwrap();
        while !update_flag.check() && !event::poll(Duration::from_millis(30)).unwrap() {}
        if update_flag.check() {
            gui.on_event(ui::GraphUiEvent::Refresh).unwrap();
        }
        if !event::poll(Duration::from_millis(0)).unwrap() {
            continue;
        }
        match event::read().unwrap() {
            event::Event::Key(event::KeyEvent {
                code: event::KeyCode::Up,
                ..
            }) => {
                eprintln!("Up.");
                gui.on_event(ui::GraphUiEvent::MoveUp).unwrap();
            }
            event::Event::Key(event::KeyEvent {
                code: event::KeyCode::Down,
                ..
            }) => {
                eprintln!("Down.");
                gui.on_event(ui::GraphUiEvent::MoveDown).unwrap();
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
                gui.on_event(ui::GraphUiEvent::ToggleCollapse).unwrap();
            }
            event::Event::Key(event::KeyEvent {
                code: event::KeyCode::Char('c'),
                modifiers: event::KeyModifiers::CONTROL,
            }) => {
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
