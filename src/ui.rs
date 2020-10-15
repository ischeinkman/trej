use crossterm::{cursor, event, terminal, ExecutableCommand, QueueableCommand};
use std::fmt;
use std::io::{self, Write};
use std::ops::{Deref, DerefMut};

mod graphview;
mod treepath;
use treepath::*;

pub use graphview::*;

#[derive(Debug)]
pub struct ScreenWrapper {
    stdout: io::Stdout,
}

impl ScreenWrapper {
    #[allow(dead_code)]
    pub fn new() -> crossterm::Result<Self> {
        let mut stdout = io::stdout();
        terminal::enable_raw_mode()?;
        stdout
            .queue(event::EnableMouseCapture)?
            .queue(terminal::EnterAlternateScreen)?
            .queue(cursor::Hide)?
            .queue(terminal::Clear(terminal::ClearType::All))?
            .queue(cursor::MoveTo(1, 1))?;
        stdout.flush()?;
        Ok(Self { stdout })
    }
    #[allow(dead_code)]
    pub fn writeln(&mut self, fmt: fmt::Arguments<'_>) -> crossterm::Result<()> {
        self.stdout.write_fmt(fmt)?;
        self.stdout.execute(cursor::MoveToNextLine(1))?;
        Ok(())
    }
    #[allow(dead_code)]
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
    fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }
    fn write_fmt(&mut self, fmt: fmt::Arguments<'_>) -> io::Result<()> {
        self.stdout.write_fmt(fmt)
    }
}

impl Deref for ScreenWrapper {
    type Target = io::Stdout;
    fn deref(&self) -> &Self::Target {
        &self.stdout
    }
}

impl DerefMut for ScreenWrapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.stdout
    }
}


#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum UiAction {
    Redraw, 
    Close,
}