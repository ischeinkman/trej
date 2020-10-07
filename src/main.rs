use jack::Client as JackClient;
use std::time::Duration;
use thiserror::*;

use crossterm::{cursor, event, style, terminal, ExecutableCommand, QueueableCommand};
use std::fmt;
use std::io::{self, Read, Write};
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
    let mut stdout = ScreenWrapper::new().unwrap();
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

#[derive(Debug)]
pub struct ScreenWrapper {
    stdout: io::Stdout,
}

impl ScreenWrapper {
    pub fn new() -> crossterm::Result<Self> {
        let mut stdout = io::stdout();
        terminal::enable_raw_mode()?;
        stdout
            .queue(event::EnableMouseCapture)?
            .queue(terminal::EnterAlternateScreen)?
            .queue(cursor::Hide)?;
        stdout.flush()?;
        Ok(Self { stdout })
    }
    pub fn fillln(&mut self, fmt: fmt::Arguments<'_>) -> crossterm::Result<()> {
        self.stdout.write_fmt(fmt)?;
        let cols = terminal::size()?.0;
        let cur_col = cursor::position()?.0;
        let needed = usize::from(cols.saturating_sub(cur_col));
        eprintln!("{}, {}, {}", cols, cur_col, needed);
        self.stdout
            .write_fmt(format_args!("{:n$}", " ", n = needed))?;
        Ok(())
    }
    pub fn writeln(&mut self, fmt: fmt::Arguments<'_>) -> crossterm::Result<()> {
        self.stdout.write_fmt(fmt)?;
        self.stdout.execute(cursor::MoveToNextLine(1))?;
        Ok(())
    }
    pub fn clear(&mut self) -> crossterm::Result<()> {
        self.stdout
            .queue(terminal::Clear(terminal::ClearType::All))?
            .queue(cursor::MoveTo(0, 0))?
            .flush()?;
        Ok(())
    }
}

impl Drop for ScreenWrapper {
    fn drop(&mut self) {
        self.stdout.queue(event::DisableMouseCapture).unwrap();
        self.stdout.queue(cursor::Show).unwrap();
        self.stdout.queue(terminal::LeaveAlternateScreen).unwrap();
        self.stdout.flush().unwrap();
        terminal::disable_raw_mode().unwrap();
    }
}

impl io::Write for ScreenWrapper {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stdout.write(buf)
    }
    fn write_fmt(&mut self, fmt: fmt::Arguments<'_>) -> io::Result<()> {
        self.stdout.write_fmt(fmt)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }
}
