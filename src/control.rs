use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::thread::JoinHandle;

use bincode::deserialize;
use log::{debug, error};
use socket2::SockAddr;

use crate::app::AppState;
use crate::discovery::new_socket;
use crate::discovery::send_msg;
use crate::discovery::IPV4;
use crate::discovery::{init_peer_hw_addr, scan_node};
use crate::msg::*;

pub(crate) fn control_thread(state: Arc<AppState>) -> JoinHandle<()> {
    debug!("control_thread start");

    std::thread::spawn(move || {
        let sock = new_socket().unwrap();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9909);

        match *IPV4 {
            IpAddr::V4(ref ipv4) => {
                sock.join_multicast_v4(ipv4, &Ipv4Addr::new(0, 0, 0, 0))
                    .unwrap();
            }
            IpAddr::V6(_) => unreachable!(),
        }

        sock.bind(&SockAddr::from(addr)).unwrap();

        let mut buff = vec![0; 512];

        loop {
            let size_and_addr = sock.recv_from(&mut buff);

            match size_and_addr {
                Ok((_size, src_addr)) => {
                    let msg = deserialize(&buff);
                    let msg: Msg = msg.unwrap();

                    match msg.inner {
                        ControlMsg::DiscoveryRequest => {
                            let msg_reply = Msg {
                                inner: ControlMsg::DiscoveryReply(MsgDiscoveryReply {
                                    name: state.name.clone(),
                                    hw_addr: state.hw_addr,
                                }),
                            };

                            let _ = send_msg(msg_reply, &sock, &src_addr);
                        }
                        ControlMsg::HwAddrRequest => {
                            let msg_reply = Msg {
                                inner: ControlMsg::HwAddrReply(state.hw_addr),
                            };

                            let _ = send_msg(msg_reply, &sock, &src_addr);
                        }
                        ControlMsg::Ping => {
                            let msg_reply = Msg {
                                inner: ControlMsg::Pong,
                            };

                            let _ = send_msg(msg_reply, &sock, &src_addr);
                        }
                        ControlMsg::AddPeerRequest(mut peer) => {
                            let result = init_peer_hw_addr(&mut peer);

                            let msg_reply = match result {
                                Ok(_) => {
                                    state.add_peer(peer);
                                    Msg {
                                        inner: ControlMsg::AddPeerReply(true),
                                    }
                                }
                                Err(_) => Msg {
                                    inner: ControlMsg::AddPeerReply(false),
                                },
                            };

                            let _ = send_msg(msg_reply, &sock, &src_addr);
                        }
                        ControlMsg::ListPeerRequest => {
                            let peers = { state.peers.read().unwrap().clone() };

                            let msg_reply = Msg {
                                inner: ControlMsg::ListPeerReply(peers),
                            };

                            let _ = send_msg(msg_reply, &sock, &src_addr);
                        }
                        ControlMsg::RemovePeerRequest { name, addr } => {
                            state.remove_peer(name, addr);

                            let msg_reply = Msg {
                                inner: ControlMsg::RemovePeerReply(true),
                            };

                            let _ = send_msg(msg_reply, &sock, &src_addr);
                        }
                        ControlMsg::ScanNodeRequest => {
                            let peers = {
                                let state = Arc::clone(&state);
                                scan_node(state)
                            };

                            let msg_reply = match peers {
                                Ok(peers) => {
                                    state.add_peers(peers.clone());

                                    Msg {
                                        inner: ControlMsg::ScanNodeReply(peers),
                                    }
                                }
                                Err(_) => Msg {
                                    inner: ControlMsg::ScanNodeReply(Vec::new()),
                                },
                            };

                            let _ = send_msg(msg_reply, &sock, &src_addr);
                        }
                        _ => unreachable!(),
                    }
                }
                Err(e) => match e.kind() {
                    std::io::ErrorKind::WouldBlock => {
                        continue;
                    }
                    _ => error!("error recv {:?}", e),
                },
            }
        }
    })
}
