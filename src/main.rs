use jack::Client as JackClient;
use thiserror::*;

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
    let mut gui = ui::GraphUi::new(grph, config, stdout);
    loop {
        if gui.step().unwrap() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

fn initialze_jack() -> Result<JackClient, Error> {
    let (client, _) = jack::Client::new("Terj", jack::ClientOptions::NO_START_SERVER)?;
    Ok(client)
}
