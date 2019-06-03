use std::net::{IpAddr, Ipv4Addr};
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use bincode::{deserialize, serialize};
use log::{debug, error};
use serde::{Deserialize, Serialize};

use crate::error::TapDemoError;

use super::{AppState, Peer};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};

lazy_static! {
    static ref IPV4: IpAddr = Ipv4Addr::new(224, 0, 0, 100).into();
}

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

fn send_msg(msg: Msg, sock: &Socket, addr: &SockAddr) -> std::io::Result<usize> {
    let msg_reply = serialize(&msg).unwrap();

    sock.send_to(&msg_reply, &addr)
}

fn check_peer(peer: &Peer) -> Result<(), TapDemoError> {
    let sock = new_sender()?;
    sock.set_write_timeout(Some(Duration::from_secs(5)))?;

    let msg = Msg {
        inner: ControlMsg::Ping,
    };
    let _result = send_msg(msg, &sock, &peer.ctl_addr.into())?;

    let mut buff = vec![0; 512];
    let _result = sock.recv(&mut buff)?;

    let msg: Msg = deserialize(&buff)?;

    if let ControlMsg::Pong = msg.inner {
        return Ok(());
    }

    debug!("peer lost!");

    Err(TapDemoError::PeerLost)
}

fn new_socket() -> std::io::Result<Socket> {
    let socket = Socket::new(Domain::ipv4(), Type::dgram(), Some(Protocol::udp()))?;
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;

    Ok(socket)
}

fn new_sender() -> std::io::Result<Socket> {
    let socket = new_socket()?;

    socket.bind(&SockAddr::from(SocketAddr::new(
        Ipv4Addr::new(0, 0, 0, 0).into(),
        0,
    )))?;

    Ok(socket)
}

pub(crate) fn heartbeats_thread(state: Arc<AppState>) -> JoinHandle<()> {
    debug!("heartbeats_thread start");

    std::thread::spawn(move || loop {
        {
            let mut peers = state.peers.write().unwrap();

            peers.retain(|peer| !check_peer(&peer).is_ok());
        }

        std::thread::sleep(Duration::from_secs(120));
    })
}

pub(crate) fn discovery_thread(state: Arc<AppState>) -> JoinHandle<()> {
    debug!("discovery_thread start");

    std::thread::spawn(move || {
        loop {
            {
                let sock = new_sender().unwrap();

                let req = Msg {
                    inner: ControlMsg::DiscoveryRequest,
                };
                let req = serialize(&req).unwrap();

                sock.send_to(
                    req.as_slice(),
                    &SockAddr::from(SocketAddr::new(*IPV4, 9909)),
                )
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

                            match msg.inner {
                                ControlMsg::DiscoveryReply(reply) => {
                                    if reply.name == state.name {
                                        continue;
                                    }

                                    let mut data_addr = SocketAddr::V4(addr.as_inet().unwrap());
                                    data_addr.set_port(data_addr.port() - 1);

                                    let peer = Peer {
                                        name: reply.name,
                                        ctl_addr: SocketAddr::V4(addr.as_inet().unwrap()),
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

            std::thread::sleep(Duration::from_secs(60));
        }
    })
}

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
                Ok((_size, addr)) => {
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

                            let _ = send_msg(msg_reply, &sock, &addr);
                        }
                        ControlMsg::HwAddrRequest => {
                            let msg_reply = Msg {
                                inner: ControlMsg::HwAddrReply(state.hw_addr),
                            };

                            let _ = send_msg(msg_reply, &sock, &addr);
                        }
                        ControlMsg::Ping => {
                            let msg_reply = Msg {
                                inner: ControlMsg::Pong,
                            };

                            let _ = send_msg(msg_reply, &sock, &addr);
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

pub(crate) fn init_peers_hw_addr(state: Arc<AppState>) {
    debug!("init peers hw addr...");
    let mut peers = state.peers.write().unwrap();

    for peer in &mut *peers {
        if peer.hw_addr != [0; 6] {
            continue;
        }

        let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
        sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();

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
            Err(_err) => {
                error!(
                    "error init peer hw_addr for {}, peer not respond",
                    peer.name
                );
            }
        }
    }

    // check whether all peers are initialized
    let first_uninitialized = peers.iter().find(|it| it.hw_addr == [0; 6]);

    if first_uninitialized.is_some() {
        // schedule next init
        debug!("schedule for next init");
        let state = state.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(15));

            init_peers_hw_addr(state);
        });
    }

    debug!("init done");
}
