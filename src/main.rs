use std::fs::OpenOptions;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;
use thiserror::*;

mod config;
use config::LockConfig;
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

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    ConfigParser(#[from] toml::de::Error),
}

fn main() {
    let config_path = std::env::args().skip(1).last();
    let mut state = match config_path {
        Some(config) => TrejState::load_file(config).unwrap(),
        None => TrejState::load_no_config().unwrap(),
    };
    //let mut ui = ui::GraphView::new(state);
    let mut ui_state = ui::GraphViewState::new();
    let output = ui::ScreenWrapper::new().unwrap();
    let mut output = tui::Terminal::new(tui::backend::CrosstermBackend::new(output)).unwrap();
    output
        .draw(|f| {
            let w = ui::GraphViewWidget::new(&state.graph(), &state.config());
            f.render_stateful_widget(w, f.size(), &mut ui_state);
        })
        .unwrap();
    loop {
        let has_graph_update = state.graph().needs_update();
        if has_graph_update {
            state.reload().unwrap();
            state.apply_config().unwrap();
        }
        let ui_event_opt = ui::poll_graphui_event(Some(Duration::from_millis(1000))).unwrap();
        let has_ui_event = ui_event_opt.is_some();
        let should_shutdown = ui_event_opt.map_or(false, |evt| ui_state.handle_event(evt).unwrap());

        if should_shutdown {
            return;
        }
        if has_graph_update || has_ui_event {
            output
                .draw(|f| {
                    let w = ui::GraphViewWidget::new(&state.graph(), &state.config());
                    f.render_stateful_widget(w, f.size(), &mut ui_state);
                })
                .unwrap();
        }
    }
}

struct TrejState {
    config: LockConfig,
    config_path: Option<PathBuf>,
    graph: JackGraph,
}

impl TrejState {
    fn init_graph() -> Result<JackGraph, crate::Error> {
        let (rawclient, _) = jack::Client::new("Terj", jack::ClientOptions::NO_START_SERVER)?;
        let mut graph = JackGraph::new(rawclient)?;
        graph.update()?;
        Ok(graph)
    }
    pub fn load_no_config() -> Result<Self, crate::Error> {
        let config = LockConfig::new();
        let graph = Self::init_graph()?;
        let config_path = None;
        Ok(Self {
            config,
            config_path,
            graph,
        })
    }
    pub fn load_file<T: AsRef<Path>>(path: T) -> Result<Self, crate::Error> {
        let config_path = Some(path.as_ref().to_owned());
        let mut conf_fh = OpenOptions::new().read(true).open(&path)?;
        let mut raw_conf = String::new();
        conf_fh.read_to_string(&mut raw_conf)?;
        let config = toml::from_str(&raw_conf)?;
        let graph = Self::init_graph()?;
        Ok(Self {
            config,
            config_path,
            graph,
        })
    }
    pub fn config(&self) -> &LockConfig {
        &self.config
    }
    pub fn graph(&self) -> &JackGraph {
        &self.graph
    }
    pub fn reload_config(&mut self) -> Result<(), crate::Error> {
        let path = match &self.config_path {
            Some(c) => c,
            None => {
                return Ok(());
            }
        };
        let mut conf_fh = OpenOptions::new().read(true).open(path)?;
        let mut raw_conf = String::new();
        conf_fh.read_to_string(&mut raw_conf)?;
        self.config = toml::from_str(&raw_conf)?;
        Ok(())
    }
    pub fn reload_graph(&mut self) -> Result<(), crate::Error> {
        self.graph.update()?;
        Ok(())
    }
    pub fn reload(&mut self) -> Result<(), crate::Error> {
        self.reload_config()?;
        self.reload_graph()?;
        self.apply_config()?;
        Ok(())
    }
    pub fn apply_config(&mut self) -> Result<(), crate::Error> {
        let graph = &mut self.graph;
        let conf = &self.config;
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
            let adata = match graph.port_by_name(a) {
                Some(dt) => dt,
                None => {
                    continue;
                }
            };
            if graph.port_by_name(b).is_none() {
                continue;
            }
            if graph.port_connections(a).any(|other| &other.name == b) {
                continue;
            }
            let (src, dst) = if adata.direction.is_output() {
                (a, b)
            } else {
                (b, a)
            };
            graph.connect(src, dst)?;
        }
        Ok(())
    }
}
