use crossterm::{cursor, event, terminal, QueueableCommand};
use std::fmt;
use std::io::{self, Write};
use std::ops::{Deref, DerefMut};


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
            .queue(cursor::Hide)?
            .queue(terminal::Clear(terminal::ClearType::All))?
            .queue(cursor::MoveTo(1, 1))?;
        stdout.flush()?;
        Ok(Self { stdout })
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
