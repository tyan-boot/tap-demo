use std::net::SocketAddr;
use std::str::FromStr;

use crate::error::TapDemoError;

#[derive(Debug)]
pub(crate) struct Peer {
    pub(crate) name: String,
    pub(crate) ctl_addr: SocketAddr,
    pub(crate) data_addr: SocketAddr,
    pub(crate) hw_addr: [u8; 6],
}

impl FromStr for Peer {
    type Err = TapDemoError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pairs: Vec<&str> = s.split("=").collect();

        if pairs.len() != 2 {
            return Err(TapDemoError::PeerParseError);
        }

        let ctl_addr: SocketAddr = pairs[1].parse()?;
        let mut data_addr = ctl_addr.clone();
        data_addr.set_port(data_addr.port() - 1);

        Ok(Peer {
            name: pairs[0].to_owned(),
            ctl_addr,
            data_addr,
            hw_addr: [0; 6],
        })
    }
}
