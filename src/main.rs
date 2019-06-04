#[macro_use]
extern crate lazy_static;

use std::env;
use std::fs::File;
use std::io::Read;
use std::net::{SocketAddr, UdpSocket};
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use clap::{App, Arg, SubCommand};
use log::error;

use crate::app::run;
use crate::discovery::{control_thread, discovery_thread, heartbeats_thread, init_peers_hw_addr};
use crate::dispatch::{dispatch_from_peers, DispatchRoutine};
use crate::error::TapDemoError;
use crate::eth::EthV2;
use crate::tap::create_tap;
use std::ptr::NonNull;
use std::thread::JoinHandle;

mod app;
mod discovery;
mod dispatch;
mod error;
mod eth;
mod peer;
mod tap;

fn main() {
    simple_logger::init().unwrap();

    let matches = App::new("tap demo")
        .version("0.1")
        .author("admin@snowstar.org")
        .about("tap tunnel via udp")
        .subcommand(
            SubCommand::with_name("start")
                .help("start main loop")
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
                .help("peers manage")
                .subcommand(SubCommand::with_name("list").help("list peers"))
                .subcommand(SubCommand::with_name("add").help("add peer")),
        )
        .get_matches();

    if let Some(arg) = matches.subcommand_matches("start") {
        run(arg);
        return;
    }
}
