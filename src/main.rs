use std::net::{Ipv4Addr, SocketAddrV4};

use bincode::deserialize;
use clap::{App, Arg, SubCommand};
use log::{error, info};
use prettytable::{cell, row, Table};
use socket2::SockAddr;

use crate::app::run;
use crate::discovery::{new_sender, send_msg};
use crate::error::TapDemoError;
use crate::msg::{ControlMsg, Msg};
use crate::peer::Peer;

use std::time::Duration;

mod app;
mod control;
mod discovery;
mod dispatch;
mod error;
mod eth;
mod msg;
mod peer;
mod tap;

fn display_peers(peers: &Vec<Peer>) {
    let mut table = Table::new();
    table.add_row(row!("Name", "IP Address", "MAC Address"));

    for peer in peers {
        let hw_addr = format!(
            "{:x}:{:x}:{:x}:{:x}:{:x}:{:x}",
            peer.hw_addr[0],
            peer.hw_addr[1],
            peer.hw_addr[2],
            peer.hw_addr[3],
            peer.hw_addr[4],
            peer.hw_addr[5],
        );

        table.add_row(row!(peer.name, peer.ctl_addr.to_string(), hw_addr));
    }

    table.printstd();
}

fn main() {
    simple_logger::init().unwrap();

    let matches = App::new("tap demo")
        .version("0.1")
        .author("admin@snowstar.org")
        .about("tap tunnel via udp")
        .subcommand(
            SubCommand::with_name("start")
                .about("start main loop")
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
                ),
        )
        .subcommand(
            SubCommand::with_name("peers")
                .about("peers manage")
                .subcommand(SubCommand::with_name("list").about("list peers"))
                .subcommand(
                    SubCommand::with_name("add")
                        .about("add peer")
                        .arg(
                            Arg::with_name("peer name")
                                .takes_value(true)
                                .required(true)
                                .help("eg, peer-01"),
                        )
                        .arg(
                            Arg::with_name("peer address")
                                .takes_value(true)
                                .required(true)
                                .help("eg, 10.0.0.1:9909"),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("remove")
                        .about("remove peer")
                        .arg(
                            Arg::with_name("peer name")
                                .short("n")
                                .long("name")
                                .help("peer name")
                                .takes_value(true),
                        )
                        .arg(
                            Arg::with_name("peer ip address")
                                .short("h")
                                .long("host")
                                .help("peer ip address")
                                .takes_value(true),
                        ),
                )
                .subcommand(SubCommand::with_name("scan").about("scan nodes")),
        )
        .get_matches();

    if let Some(arg) = matches.subcommand_matches("start") {
        run(arg);
        return;
    }

    if let Some(peers_cmd) = matches.subcommand_matches("peers") {
        let ctl_addr = SockAddr::from(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 9909));
        let sock = new_sender().unwrap();

        if let Some(add_peer) = peers_cmd.subcommand_matches("add") {
            let peer_name = add_peer.value_of("peer name").unwrap();
            let peer_address = add_peer.value_of("peer address").unwrap();

            let peer: Result<Peer, TapDemoError> =
                format!("{}={}", peer_name, peer_address).parse();

            match peer {
                Ok(peer) => {
                    let msg = Msg {
                        inner: ControlMsg::AddPeerRequest(peer),
                    };

                    let _ = send_msg(msg, &sock, &ctl_addr);

                    let mut buff = vec![0; 512];
                    sock.set_read_timeout(Some(Duration::from_secs(10)))
                        .unwrap();
                    let _ = sock.recv(&mut buff).unwrap();

                    let msg: Msg = deserialize(&buff).unwrap();

                    match msg.inner {
                        ControlMsg::AddPeerReply(succ) => {
                            if succ {
                                info!("add success");
                            } else {
                                error!("add failed")
                            }
                        }
                        _ => error!("add failed"),
                    }
                }
                Err(_) => error!("error parse peer"),
            }
        }

        if let Some(_) = peers_cmd.subcommand_matches("list") {
            let msg = Msg {
                inner: ControlMsg::ListPeerRequest,
            };

            let _result = send_msg(msg, &sock, &ctl_addr);

            let mut buff = vec![0; 4096];

            let _ = sock.recv(&mut buff).unwrap();
            let msg: Msg = deserialize(&buff).unwrap();

            match msg.inner {
                ControlMsg::ListPeerReply(peers) => {
                    display_peers(&peers);
                }
                _ => error!("response error"),
            }
        }

        if let Some(remove_peer) = peers_cmd.subcommand_matches("remove") {
            let peer_name = remove_peer.value_of("peer name").map(|it| it.to_owned());
            let peer_address = remove_peer
                .value_of("peer ip address")
                .and_then(|it| it.parse().ok());

            let msg = Msg {
                inner: ControlMsg::RemovePeerRequest {
                    name: peer_name.into(),
                    addr: peer_address,
                },
            };

            let _ = send_msg(msg, &sock, &ctl_addr);
        }

        if let Some(_) = peers_cmd.subcommand_matches("scan") {
            sock.set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();

            let msg = Msg {
                inner: ControlMsg::ScanNodeRequest,
            };

            let _ = send_msg(msg, &sock, &ctl_addr);

            let mut buff = vec![0; 4096];
            let _ = sock.recv(&mut buff).unwrap();
            let msg: Msg = deserialize(&buff).unwrap();

            match msg.inner {
                ControlMsg::ScanNodeReply(peers) => {
                    display_peers(&peers);
                }
                _ => error!("response error"),
            }
        }
    }
}
