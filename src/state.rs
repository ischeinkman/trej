use crate::config::LockConfig;
use crate::graph::JackGraph;

use std::fs::OpenOptions;
use std::io::Read;
use std::path::{Path, PathBuf};

pub(crate) struct TrejState {
    pub config: LockConfig,
    pub config_path: Option<PathBuf>,
    pub graph: JackGraph,
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
