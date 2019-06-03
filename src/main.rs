#[macro_use]
extern crate lazy_static;

use std::env;
use std::fs::File;
use std::io::Read;
use std::net::{SocketAddr, UdpSocket};
use std::os::unix::io::FromRawFd;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use clap::{App, Arg};
use log::{debug, error};

use crate::discovery::{control_thread, discovery_thread, heartbeats_thread, init_peers_hw_addr};
use crate::dispatch::{dispatch_from_peers, DispatchRoutine};
use crate::error::TapDemoError;
use crate::eth::EthV2;
use crate::tap::create_tap;

mod discovery;
mod dispatch;
mod error;
mod eth;
mod tap;

#[derive(Debug)]
pub(crate) struct Peer {
    name: String,
    ctl_addr: SocketAddr,
    data_addr: SocketAddr,
    hw_addr: [u8; 6],
}

impl FromStr for Peer {
    type Err = error::TapDemoError;

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

pub(crate) struct AppState {
    name: String,
    hw_addr: [u8; 6],
    data_sock: UdpSocket,
    tap_dev: File,
    peers: RwLock<Vec<Peer>>,
}

impl AppState {
    pub(crate) fn add_peer(&self, peer: Peer) {
        let mut peers = self.peers.write().unwrap();
        let p = peers.iter_mut().find(|it| it.ctl_addr.eq(&peer.ctl_addr));

        match p {
            Some(mut p) => {
                // update all except addr
                p.name = peer.name;
                p.hw_addr = peer.hw_addr
            }
            None => peers.push(peer),
        }
    }
}

fn main() {
    simple_logger::init().unwrap();

    let matches = App::new("tap demo")
        .version("0.1")
        .author("admin@snowstar.org")
        .about("tap tunnel via udp")
        .arg(
            Arg::with_name("peers")
                .help("peers address, eg, peer1=10.0.0.1,peer2=10.0.0.2")
                .takes_value(true)
                .required(false)
                .long("peers")
                .short("p"),
        )
        .arg(
            Arg::with_name("auto")
                .help("auto discovery peers in lan")
                .long("auto")
                .short("a"),
        )
        .arg(Arg::with_name("discovery").short("d"))
        .get_matches();

    let is_auto = matches.is_present("auto");

    // create tap
    let tap_info = create_tap("tap0");
    if tap_info.is_err() {
        error!("error create tap device {:?}", tap_info);
        return;
    }
    let tap_info = tap_info.unwrap();

    let data_sock = UdpSocket::bind("0.0.0.0:9908").unwrap();
    data_sock
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();

//    let tap_dev: File = unsafe { File::from_raw_fd(tap_info.fd) };

    let state = Arc::new(AppState {
        name: env::var("HOSTNAME")
            .or_else(|_| env::var("HOST"))
            .unwrap_or("peer-01".to_owned()),
        data_sock,
        tap_dev: tap_info.tap_dev,
        hw_addr: tap_info.hw_addr,
        peers: RwLock::new(Vec::new()),
    });

    // try init peers
    if let Some(peers_str) = matches.value_of("peers") {
        let peers_str: Vec<&str> = peers_str.split(",").collect();

        let peers: Vec<Peer> = peers_str
            .into_iter()
            .map(|it| {
                let peer = it.parse();

                peer.unwrap()
            })
            .collect();
        state.peers.write().unwrap().extend(peers);
    }

    {
        if !is_auto && state.peers.read().unwrap().is_empty() {
            error!("peers is empty and `auto` is not set");
            return;
        }
    }

    // heartbeats thread
    {
        let state = state.clone();
        heartbeats_thread(state);
    }

    // discovery thread
    if is_auto {
        let state = state.clone();
        discovery_thread(state);
    }

    // control thread
    {
        let state = state.clone();
        control_thread(state);
    }

    // init peers hw addr
    {
        let state = state.clone();
        let is_empty = { state.peers.read().unwrap().is_empty() };

        if !is_empty {
            init_peers_hw_addr(state);
        }
    }

    // dispatch from peers
    {
        let state = state.clone();
        std::thread::spawn(move || dispatch_from_peers(state));
    }

    let mut buff = vec![0; 1500];
    let dispatch_routine = DispatchRoutine(state.clone());

    loop {
        let mut tap_dev = &state.tap_dev;

        let size = tap_dev.read(&mut buff);

        dbg!(&size);
        if size.is_err() {
            continue;
        }

        let mut dst_mac = [0; 6];
        dst_mac.copy_from_slice(&buff[4..][0..6]);

        let mut src_mac = [0; 6];
        src_mac.copy_from_slice(&buff[4..][6..12]);

        let mut proto_type = [0; 2];
        proto_type.copy_from_slice(&buff[4..][12..14]);

        let eth = EthV2 {
            dst_mac,
            src_mac,
            proto_type: u16::from_be_bytes(proto_type),
            data: buff.clone(),
        };

        let result = dispatch_routine.dispatch_to_peers(eth);

        match result {
            Err(e) => error!("error dispatch to peers, {:?}", e),
            _ => {}
        }
    }
}
