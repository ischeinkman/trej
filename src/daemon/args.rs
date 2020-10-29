use std::path::{Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ArgError {
    #[error("Invalid client name passed.")]
    InvalidClientName,
    #[error("Invalid flag passed: \"{0}\"")]
    InvalidFlag(String),
    #[error("Invalid config file passed: \"{0}\"")]
    InvalidPath(String),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum StartServerFlag {
    StartServer,
    StartIfStopped,
    NoStart,
}

impl Default for StartServerFlag {
    fn default() -> Self {
        StartServerFlag::StartIfStopped
    }
}

pub struct DaemonArgs {
    config_path: PathBuf,
    server_flag: Option<StartServerFlag>,
    client_name: Option<String>,
}

impl DaemonArgs {
    pub fn from_args<I: Iterator<Item = S>, S: AsRef<str>>(iter: I) -> Result<Self, ArgError> {
        let mut iter = iter.skip(1).peekable();
        let mut client_name = None;
        let mut server_flag = None;
        let mut config_path = None;
        while let Some(cur_key) = iter.next() {
            if cur_key.as_ref() == "-c" {
                let cur_val = iter
                    .next()
                    .map(|s| s.as_ref().to_owned())
                    .unwrap_or_default();
                if cur_val.trim().is_empty() || cur_val.len() >= *jack::CLIENT_NAME_SIZE {
                    return Err(ArgError::InvalidClientName);
                }
                client_name = Some(cur_val);
            } else if cur_key.as_ref() == "-n" {
                server_flag = Some(StartServerFlag::NoStart);
            } else if cur_key.as_ref() == "-s" {
                server_flag = Some(StartServerFlag::StartServer);
            } else if cur_key.as_ref() == "-r" {
                server_flag = Some(StartServerFlag::StartIfStopped);
            } else {
                let raw_path = cur_key.as_ref();
                let path = PathBuf::from(raw_path);
                if !path.is_file() {
                    return Err(ArgError::InvalidPath(raw_path.to_owned()));
                }
                config_path = Some(path);
            }
        }
        let config_path = config_path.ok_or_else(|| ArgError::InvalidPath(String::new()))?;
        Ok(Self {
            config_path,
            server_flag,
            client_name,
        })
    }
    pub fn config_path(&self) -> &Path {
        &self.config_path
    }
    pub fn client_name(&self) -> &str {
        const DEFAULT_NAME: &str = "trejdaemon";
        self.client_name.as_deref().unwrap_or(DEFAULT_NAME)
    }
    pub fn server_flag(&self) -> StartServerFlag {
        self.server_flag.unwrap_or_default()
    }
}
