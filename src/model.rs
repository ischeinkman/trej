use thiserror::*;

mod ports;
pub use ports::*;


#[derive(Debug, Error)]
pub enum NameError {
    #[error("Invalid port full name.")]
    InvalidFullname,

    #[error("Port name too long.")]
    PortnameTooLong,

    #[error("Client name too long.")]
    ClientnameTooLong,
}
