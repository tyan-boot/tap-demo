use std::env;
use std::fs::File;
use std::io::Read;
use std::net::{IpAddr, UdpSocket};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use clap::ArgMatches;
use log::error;

use crate::control::control_thread;
use crate::discovery::{discovery_thread, heartbeats_thread, init_peers_hw_addr};
use crate::dispatch::{dispatch_from_peers, DispatchRoutine};
use crate::error::AppResult;
use crate::eth::EthV2;
use crate::peer::Peer;
use crate::tap::{create_tap as inner_create_tap, TapInfo};

pub(crate) struct AppState {
    pub(crate) name: String,
    pub(crate) hw_addr: [u8; 6],
    pub(crate) data_sock: UdpSocket,
    pub(crate) tap_dev: File,
    pub(crate) peers: RwLock<Vec<Peer>>,
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

    pub(crate) fn add_peers(&self, peers: Vec<Peer>) {
        for peer in peers {
            self.add_peer(peer);
        }
    }

    pub(crate) fn remove_peer(&self, name: Option<String>, addr: Option<IpAddr>) {
        let mut peers = self.peers.write().unwrap();

        peers.retain(move |it| {
            let name_eq = if name.is_some() {
                name.as_ref().unwrap().eq(&it.name)
            } else {
                false
            };

            let addr_eq = if addr.is_some() {
                addr.as_ref().unwrap().eq(&it.ctl_addr.ip())
            } else {
                false
            };

            !(name_eq | addr_eq)
        });
    }
}

fn create_tap() -> AppResult<TapInfo> {
    // create tap
    inner_create_tap("tap0")
}

fn create_data_sock() -> AppResult<UdpSocket> {
    let data_sock = UdpSocket::bind("0.0.0.0:9908")?;
    data_sock.set_write_timeout(Some(Duration::from_secs(5)))?;

    Ok(data_sock)
}

fn parse_peers_str(peers_str: &str) -> AppResult<Vec<Peer>> {
    let peers_str: Vec<&str> = peers_str.split(",").collect();

    let peers: Vec<Peer> = peers_str
        .into_iter()
        .map(|it| {
            let peer = it.parse();

            peer.unwrap()
        })
        .collect();

    Ok(peers)
}

pub(crate) fn run(args: &ArgMatches) -> AppResult<()> {
    let tap_info = create_tap()?;
    let data_sock = create_data_sock()?;
    let is_auto = args.is_present("auto");

    // init peers from args
    let init_peers = match args.value_of("peers") {
        Some(peers_str) => parse_peers_str(peers_str)?,
        None => Vec::new(),
    };

    let state = Arc::new(AppState {
        name: env::var("HOSTNAME")
            .or_else(|_| env::var("HOST"))
            .unwrap_or("peer-01".to_owned()),
        data_sock,
        tap_dev: tap_info.tap_dev,
        hw_addr: tap_info.hw_addr,
        peers: RwLock::new(init_peers),
    });

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

        if size.is_err() {
            continue;
        }

        let mut dst_mac = [0; 6];
        dst_mac.copy_from_slice(&buff[0..6]);

        let mut src_mac = [0; 6];
        src_mac.copy_from_slice(&buff[6..12]);

        let mut proto_type = [0; 2];
        proto_type.copy_from_slice(&buff[12..14]);

        let eth = EthV2 {
            dst_mac,
            src_mac,
            proto_type: u16::from_be_bytes(proto_type),
            data: &buff,
        };

        let result = dispatch_routine.dispatch_to_peers(eth);

        match result {
            Err(e) => error!("error dispatch to peers, {:?}", e),
            _ => {}
        }
    }
}
