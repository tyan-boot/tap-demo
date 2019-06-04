use std::net::SocketAddr;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use bincode::{deserialize, serialize};
use lazy_static::lazy_static;
use log::{debug, error};

use crate::app::AppState;
use crate::error::{AppResult, TapDemoError};
use crate::msg::*;
use crate::peer::Peer;

use socket2::{Domain, Protocol, SockAddr, Socket, Type};

lazy_static! {
    pub(crate) static ref IPV4: IpAddr = Ipv4Addr::new(224, 0, 0, 100).into();
}

pub(crate) fn send_msg(msg: Msg, sock: &Socket, addr: &SockAddr) -> std::io::Result<usize> {
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

pub(crate) fn new_socket() -> std::io::Result<Socket> {
    let socket = Socket::new(Domain::ipv4(), Type::dgram(), Some(Protocol::udp()))?;
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;

    Ok(socket)
}

pub(crate) fn new_sender() -> std::io::Result<Socket> {
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

pub(crate) fn init_peer_hw_addr(peer: &mut Peer) -> AppResult<()> {
    if peer.hw_addr != [0; 6] {
        return Ok(());
    }

    let sock = new_sender()?;
    sock.set_read_timeout(Some(Duration::from_secs(5)))?;

    let msg = Msg {
        inner: ControlMsg::HwAddrRequest,
    };

    let peer_ctl_addr = peer.ctl_addr.clone();
    send_msg(msg, &sock, &SockAddr::from(peer_ctl_addr))?;

    let mut buff = vec![0; 512];
    let _size = sock.recv(&mut buff)?;

    let msg: Msg = deserialize(&buff)?;

    match msg.inner {
        ControlMsg::HwAddrReply(hw_addr) => {
            peer.hw_addr = hw_addr;
            Ok(())
        }
        _ => Err(TapDemoError::GetHWAddrError),
    }
}

pub(crate) fn init_peers_hw_addr(state: Arc<AppState>) {
    debug!("init peers hw addr...");
    let mut peers = state.peers.write().unwrap();

    for peer in &mut *peers {
        if peer.hw_addr != [0; 6] {
            continue;
        }

        let _result = init_peer_hw_addr(peer);
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
