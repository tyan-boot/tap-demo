use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use bincode::{deserialize, serialize};
use log::{debug, error};
use serde::{Deserialize, Serialize};
use std::thread::JoinHandle;

use super::{AppState, Peer};
use crate::error::TapDemoError;
use std::panic::resume_unwind;

#[derive(Serialize, Deserialize, Debug)]
struct MsgDiscoveryReply {
    name: String,
    hw_addr: [u8; 6],
}

#[derive(Serialize, Deserialize, Debug)]
struct Msg {
    inner: ControlMsg,
}

#[derive(Serialize, Deserialize, Debug)]
enum ControlMsg {
    DiscoveryRequest,
    DiscoveryReply(MsgDiscoveryReply),

    HwAddrRequest,
    HwAddrReply([u8; 6]),

    Ping,
    Pong,
}

fn send_msg(msg: Msg, sock: &UdpSocket, addr: &SocketAddr) -> std::io::Result<usize> {
    let msg_reply = serialize(&msg).unwrap();

    sock.send_to(&msg_reply, &addr)
}

fn check_peer(peer: &Peer) -> Result<(), TapDemoError> {
    let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
    sock.set_read_timeout(Some(Duration::from_secs(5)));
    sock.set_write_timeout(Some(Duration::from_secs(5)));

    let msg = Msg {
        inner: ControlMsg::Ping,
    };
    let result = send_msg(msg, &sock, &peer.ctl_addr)?;

    let mut buff = vec![0; 512];
    let result = sock.recv(&mut buff)?;

    let msg: Msg = deserialize(&buff)?;

    if let ControlMsg::Pong = msg.inner {
        return Ok(());
    }

    Err(TapDemoError::PeerLost)
}

pub(crate) fn heartbeats_thread(state: Arc<RwLock<AppState>>) -> JoinHandle<()> {
    loop {
        {
            let mut state = state.write().unwrap();

            state.peers.retain(|peer| check_peer(&peer).is_ok());
        }

        std::thread::sleep(Duration::from_secs(15));
    }
}

pub(crate) fn discovery_thread(state: Arc<RwLock<AppState>>) -> JoinHandle<()> {
    std::thread::spawn(move || {
        loop {
            {
                let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
                sock.set_broadcast(true).unwrap();
                sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();

                let req = Msg {
                    inner: ControlMsg::DiscoveryRequest,
                };
                let req = serialize(&req).unwrap();

                dbg!(&req);

                sock.send_to(req.as_slice(), "255.255.255.255:9909")
                    .unwrap();

                let mut buf = vec![0; 512];

                'recv: loop {
                    let size_and_addr = sock.recv_from(&mut buf);

                    match size_and_addr {
                        Ok((_size, addr)) => {
                            let msg = deserialize(&buf);
                            if msg.is_err() {
                                error!("error parse peer info, ignore");
                                continue 'recv;
                            }

                            let msg: Msg = msg.unwrap();
                            debug!("msg reply {:#?}", msg);

                            match msg.inner {
                                ControlMsg::DiscoveryReply(reply) => {
                                    let mut state = state.write().unwrap();

                                    if reply.name == state.name {
                                        continue;
                                    }

                                    let mut data_addr = addr.clone();
                                    data_addr.set_port(data_addr.port() - 1);

                                    let peer = Peer {
                                        name: reply.name,
                                        ctl_addr: addr,
                                        data_addr,
                                        hw_addr: reply.hw_addr,
                                    };

                                    state.add_peer(peer);
                                }
                                _ => unreachable!(),
                            }
                        }
                        Err(err) => {
                            match err.kind() {
                                std::io::ErrorKind::WouldBlock => {
                                    // stop recv, and wait 15 sec for next round
                                    break 'recv;
                                }
                                _ => {
                                    // todo: ignore or panic
                                }
                            }
                        }
                    }
                }
            }

            std::thread::sleep(Duration::from_secs(15));
        }
    })
}

pub(crate) fn control_thread(state: Arc<RwLock<AppState>>) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let sock = UdpSocket::bind("0.0.0.0:9909").unwrap();

        let mut buff = vec![0; 512];

        loop {
            let size_and_addr = sock.recv_from(&mut buff);

            match size_and_addr {
                Ok((_size, addr)) => {
                    let msg = deserialize(&buff);
                    let msg: Msg = msg.unwrap();

                    debug!("msg recv {:#?}", msg);

                    match msg.inner {
                        ControlMsg::DiscoveryRequest => {
                            let state = state.read().unwrap();

                            let msg_reply = Msg {
                                inner: ControlMsg::DiscoveryReply(MsgDiscoveryReply {
                                    name: state.name.clone(),
                                    hw_addr: state.hw_addr,
                                }),
                            };

                            send_msg(msg_reply, &sock, &addr);
                        }
                        ControlMsg::HwAddrRequest => {
                            let state = state.read().unwrap();
                            let msg_reply = Msg {
                                inner: ControlMsg::HwAddrReply(state.hw_addr),
                            };

                            send_msg(msg_reply, &sock, &addr);
                        }
                        ControlMsg::Ping => {
                            let msg_reply = Msg {
                                inner: ControlMsg::Pong,
                            };

                            send_msg(msg_reply, &sock, &addr);
                        }
                        _ => unreachable!(),
                    }
                }
                Err(e) => {
                    error!("error recv {:?}", e);
                }
            }
        }
    })
}

pub(crate) fn init_peers_hw_addr(state: Arc<RwLock<AppState>>) {
    let mut state_guard = state.write().unwrap();

    for peer in &mut state_guard.peers {
        if peer.hw_addr != [0; 6] {
            continue;
        }

        let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
        sock.set_read_timeout(Some(Duration::from_secs(5)));

        let msg = Msg {
            inner: ControlMsg::HwAddrRequest,
        };
        let msg = serialize(&msg).unwrap();

        sock.send_to(&msg, &peer.ctl_addr).unwrap();

        let mut buff = vec![0; 512];
        let msg = sock.recv(&mut buff);

        match msg {
            Ok(_size) => {
                let msg = deserialize(&buff);

                if msg.is_err() {
                    error!(
                        "error init peer hw_addr for {}, deserialize msg failed",
                        peer.name
                    );
                    continue;
                }

                let msg: Msg = msg.unwrap();

                match msg.inner {
                    ControlMsg::HwAddrReply(hw_addr) => {
                        peer.hw_addr = hw_addr;
                    }
                    _ => {
                        error!("error init peer hw_addr for {}, msg type mismatch, expect HwAddrReply, got {:?}", peer.name, msg.inner);
                    }
                }
            }
            Err(err) => {
                error!(
                    "error init peer hw_addr for {}, peer not respond",
                    peer.name
                );
            }
        }
    }

    // check whether all peers are initialized
    let first_uninitialized = state_guard.peers.iter().find(|it| it.hw_addr == [0; 6]);

    if first_uninitialized.is_some() {
        // schedule next init

        let state = state.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(15));

            init_peers_hw_addr(state);
        });
    }
}
