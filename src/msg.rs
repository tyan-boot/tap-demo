use serde::{Deserialize, Serialize};

use crate::peer::Peer;
use std::net::IpAddr;

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct MsgDiscoveryReply {
    pub(crate) name: String,
    pub(crate) hw_addr: [u8; 6],
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Msg {
    pub(crate) inner: ControlMsg,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) enum ControlMsg {
    DiscoveryRequest,
    DiscoveryReply(MsgDiscoveryReply),

    HwAddrRequest,
    HwAddrReply([u8; 6]),

    Ping,
    Pong,

    AddPeerRequest(Peer),
    AddPeerReply(bool),

    ListPeerRequest,
    ListPeerReply(Vec<Peer>),

    RemovePeerRequest {
        name: Option<String>,
        addr: Option<IpAddr>,
    },
    RemovePeerReply(bool),

    ScanNodeRequest,
    ScanNodeReply(Vec<Peer>),
}
