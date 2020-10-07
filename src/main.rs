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
        apply_config(&gui.config, &mut gui.graph).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

fn initialze_jack() -> Result<JackClient, Error> {
    let (client, _) = jack::Client::new("Terj", jack::ClientOptions::NO_START_SERVER)?;
    Ok(client)
}

fn apply_config(conf: &config::LockConfig, graph: &mut graph::JackGraph) -> Result<(), crate::Error> {
    let should_disconnect = graph
        .all_connections()
        .filter(|(a, b)| conf.connection_status(&a.name, &b.name).should_block())
        .map(|(a, b)| (a.clone(), b.clone()))
        .collect::<Vec<_>>();
    for (a, b) in should_disconnect {
        let (src, dst) = if a.direction.is_output() {
            (a, b)
        } else {
            (b, a)
        };
        graph.disconnect(&src.name, &dst.name)?;
    }
    for (a, b) in conf.forced_connections() {
        if graph.port_connections(a).any(|other| &other.name == b) {
            continue;
        }
        let adata = graph.port_by_name(a).unwrap();
        let (src, dst) = if adata.direction.is_output() {
            (a, b)
        } else {
            (b, a)
        };
        graph.connect(src, dst)?;
    }
    Ok(())
}
