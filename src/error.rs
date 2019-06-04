#[derive(Debug)]
pub enum TapDemoError {
    IOError(std::io::Error),
    TapCreateError(i32),
    GetHWAddrError,
    PeerParseError,
    PeerAddressParseError(String),
    TapSetupError,

    PeerLost,
    MsgDeserializeError(bincode::Error),
}

impl From<std::io::Error> for TapDemoError {
    fn from(err: std::io::Error) -> Self {
        TapDemoError::IOError(err)
    }
}

impl From<std::net::AddrParseError> for TapDemoError {
    fn from(err: std::net::AddrParseError) -> Self {
        TapDemoError::PeerAddressParseError(err.to_string())
    }
}

impl From<bincode::Error> for TapDemoError {
    fn from(err: bincode::Error) -> Self {
        TapDemoError::MsgDeserializeError(err)
    }
}

pub type AppResult<T> = Result<T, TapDemoError>;
