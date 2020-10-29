use crate::config::LockConfig;
use crate::model::PortFullname;

use jack::Client as JackClient;
use jack::PortId;
use notify::{self, RecommendedWatcher, RecursiveMode, Watcher};

use std::convert::TryFrom;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::mpsc;

mod args;
pub use args::{ArgError, DaemonArgs, StartServerFlag};

pub struct TrejDaemon {
    args: DaemonArgs,
    event_stream: mpsc::Receiver<DaemonMessage>,
    config: LockConfig,
    client: jack::AsyncClient<GraphNotifier, ()>,
    _watcher: notify::RecommendedWatcher,
}

impl TrejDaemon {
    pub fn new(args: DaemonArgs) -> Result<(Self, mpsc::SyncSender<DaemonMessage>), crate::Error> {
        let config = read_config(args.config_path())?;

        let (snd, event_stream) = mpsc::sync_channel(32);
        let _watcher = make_watcher(&args, snd.clone())?;
        let client = make_client(&args, snd.clone())?;

        Ok((
            Self {
                args,
                event_stream,
                config,
                client,
                _watcher,
            },
            snd,
        ))
    }
    pub fn run(mut self) -> Result<(), crate::Error> {
        loop {
            match self.event_stream.recv() {
                Err(_) => {
                    eprintln!("Channel closed. Breaking.");
                    break;
                }
                Ok(DaemonMessage::ConfigUpdated) => {
                    eprintln!("Got config update evt.");
                    let new_config = read_config(self.args.config_path())?;
                    if new_config != self.config {
                        eprintln!("Applying new config.");
                        self.config = new_config;
                        apply_config(&self.config, &self.client.as_client())?;
                    }
                    else {
                        eprintln!("Config is unchanged.");
                    }
                }
                Ok(DaemonMessage::GraphUpdated) => {
                    eprintln!("Got graph update evt.");
                    apply_config(&self.config, &self.client.as_client())?;
                }
            }
        }
        Ok(())
    }
}

fn read_config(path: &Path) -> Result<LockConfig, crate::Error> {
    let mut fh = File::open(path)?;
    let mut buffer = String::new();
    fh.read_to_string(&mut buffer)?;
    let retvl = toml::from_str(&buffer)?;
    Ok(retvl)
}

fn make_watcher(
    args: &DaemonArgs,
    sender: mpsc::SyncSender<DaemonMessage>,
) -> Result<notify::RecommendedWatcher, crate::Error> {
    let mut watcher = RecommendedWatcher::new_immediate(move |_| {
        if sender.try_send(DaemonMessage::ConfigUpdated).is_ok() {}
    })?;
    watcher.watch(args.config_path(), RecursiveMode::NonRecursive)?;
    Ok(watcher)
}

fn make_client(
    args: &DaemonArgs,
    sender: mpsc::SyncSender<DaemonMessage>,
) -> Result<jack::AsyncClient<GraphNotifier, ()>, crate::Error> {
    let force_start = args.server_flag() == StartServerFlag::StartServer;
    let try_start = args.server_flag() == StartServerFlag::StartIfStopped;

    let flags = if force_start {
        jack::ClientOptions::empty()
    } else {
        jack::ClientOptions::NO_START_SERVER
    };
    let retry_flags = jack::ClientStatus::SERVER_FAILED | jack::ClientStatus::SERVER_ERROR;
    let first_res = jack::Client::new(args.client_name(), flags);
    let second_res = match first_res {
        Ok((_, status)) if force_start && !status.contains(jack::ClientStatus::SERVER_STARTED) => {
            Err(jack::Error::ClientActivationError)
        }
        Ok((client, _)) => Ok(client),
        Err(jack::Error::ClientError(flags)) if try_start && flags.intersects(retry_flags) => {
            jack::Client::new(args.client_name(), jack::ClientOptions::empty()).map(|(cli, _)| cli)
        }
        Err(other) => Err(other),
    };

    let raw_client = second_res?;
    let notifier = GraphNotifier { channel: sender };
    let client = raw_client.activate_async(notifier, ())?;
    Ok(client)
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum DaemonMessage {
    GraphUpdated,
    ConfigUpdated,
}

fn apply_config(conf: &LockConfig, client: &JackClient) -> Result<(), crate::Error> {
    let port_names = client
        .ports(None, None, jack::PortFlags::empty())
        .into_iter()
        .map(PortFullname::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    let port_data: Vec<_> = port_names
        .iter()
        .map(|name| client.port_by_name(name.as_ref()))
        .collect();
    for (port_a_idx, port_a) in port_names.iter().enumerate() {
        let a_data_opt = port_data.get(port_a_idx).and_then(|d| d.as_ref());
        let a_data = match a_data_opt {
            Some(d) => d,
            None => {
                continue;
            }
        };
        let port_b_iter = port_names.iter().skip(1 + port_a_idx);
        for port_b in port_b_iter {
            if !a_data.is_connected_to(port_b.as_ref())? {
                continue;
            }
            let lock_status = conf.connection_status(port_a, port_b);
            if lock_status.should_block() {
                let a_is_src = a_data.flags().contains(jack::PortFlags::IS_OUTPUT);
                let (src, dst) = if a_is_src {
                    (port_a, port_b)
                } else {
                    (port_b, port_a)
                };
                client.disconnect_ports_by_name(src.as_ref(), dst.as_ref())?;
            }
        }
    }
    for (port_a, port_b) in conf.forced_connections() {
        let mut a_data = None;
        let mut b_data = None;
        for (cur_idx, cur_name) in port_names.iter().enumerate() {
            if port_a == cur_name {
                a_data = port_data.get(cur_idx).and_then(|d| d.as_ref());
                if b_data.is_some() {
                    break;
                }
            } else if port_b == cur_name {
                b_data = port_data.get(cur_idx).and_then(|d| d.as_ref());
                if a_data.is_some() {
                    break;
                }
            }
        }
        let (a_data, b_data) = if let Some(dt) = a_data.zip(b_data) {
            dt
        } else {
            continue;
        };
        if a_data.port_type()? != b_data.port_type()? || a_data.is_connected_to(port_b.as_ref())? {
            continue;
        }
        let a_is_input = a_data.flags().contains(jack::PortFlags::IS_INPUT);
        let b_is_input = b_data.flags().contains(jack::PortFlags::IS_INPUT);
        let (src, dst) = match (a_is_input, b_is_input) {
            (false, true) => (port_a, port_b),
            (true, false) => (port_b, port_a),
            _ => {
                continue;
            }
        };
        client.connect_ports_by_name(src.as_ref(), dst.as_ref())?;
    }
    Ok(())
}

struct GraphNotifier {
    channel: mpsc::SyncSender<DaemonMessage>,
}

impl GraphNotifier {
    pub fn notify(&self) -> jack::Control {
        match self.channel.try_send(DaemonMessage::GraphUpdated) {
            Ok(()) | Err(mpsc::TrySendError::Full(_)) => jack::Control::Continue,
            Err(mpsc::TrySendError::Disconnected(_)) => jack::Control::Quit,
        }
    }
}

impl jack::NotificationHandler for GraphNotifier {
    fn port_registration(&mut self, _: &JackClient, _: PortId, _: bool) {
        self.notify();
    }
    fn ports_connected(&mut self, _: &JackClient, _: PortId, _: PortId, _: bool) {
        self.notify();
    }
    fn port_rename(&mut self, _: &JackClient, _: PortId, _: &str, _: &str) -> jack::Control {
        self.notify()
    }
    fn client_registration(&mut self, _: &JackClient, _: &str, _: bool) {
        self.notify();
    }
    fn graph_reorder(&mut self, _: &JackClient) -> jack::Control {
        self.notify()
    }
}
