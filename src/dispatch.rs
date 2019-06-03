use std::sync::{Arc, RwLock};

use super::eth::EthV2;
use super::AppState;
use super::error::TapDemoError;

use log::error;

/// dispatch packet to peers
pub(crate) fn dispatch_to_peers(state: Arc<RwLock<AppState>>, eth: EthV2) -> Result<(), TapDemoError> {
    let state = state.read().unwrap();

    // for brd
    if eth.dst_mac == [255, 255, 255, 255, 255, 255] {
        for peer in &state.peers {
            // don't send to self
            if peer.hw_addr == state.hw_addr {
                continue;
            }

            let _result = state.data_sock.send_to(&eth.data, &peer.data_addr)?;
        }
    } else {
        let peer = state.peers.iter().find(|&it| it.hw_addr == eth.dst_mac);

        match peer {
            Some(peer) => {
                state.data_sock.send_to(&eth.data, &peer.data_addr)?;
            }
            None => {
                error!("unknown dst {:x?}", eth.data);
            }
        }
    }

    Ok(())
}

pub(crate) fn dispatch_from_peers(_state: Arc<RwLock<AppState>>) {}