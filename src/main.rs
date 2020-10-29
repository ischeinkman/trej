use std::io;
use std::time::Duration;
use thiserror::*;

mod config;
mod graph;
mod model;
mod ui;

mod daemon;

mod state;
use state::TrejState;

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

    #[error(transparent)]
    NameParser(#[from] crate::model::NameError),

    #[error(transparent)]
    ConfigWatcher(#[from] notify::Error),

    #[error(transparent)]
    DaemonArgParser(#[from] crate::daemon::ArgError),
}

const SHOULD_BE_DAEMON: bool = true;
use daemon::{DaemonArgs, TrejDaemon};

fn main() {
    if SHOULD_BE_DAEMON {
        let args = DaemonArgs::from_args(std::env::args()).unwrap();
        let (daemon, _) = TrejDaemon::new(args).unwrap();
        return daemon.run().unwrap();
    }
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
        let ui_event_opt = ui_state
            .handle_pending_event(
                &mut state.graph,
                &mut state.config,
                Some(Duration::from_millis(1000)),
            )
            .unwrap();

        match ui_event_opt {
            Some(ui::UiAction::Close) => {
                return;
            }
            None if !has_graph_update => {
                // No updates in state or UI, so no redrawing
            }
            _ => {
                output
                    .draw(|f| {
                        let w = ui::GraphViewWidget::new(&state.graph(), &state.config());
                        f.render_stateful_widget(w, f.size(), &mut ui_state);
                    })
                    .unwrap();
            }
        }
    }
}
