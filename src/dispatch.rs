use std::sync::Arc;

use crate::app::AppState;
use crate::error::TapDemoError;
use crate::eth::EthV2;

use log::error;
use std::io::Write;

pub(crate) struct DispatchRoutine(pub(crate) Arc<AppState>);

impl DispatchRoutine {
    /// dispatch packet to peers
    pub(crate) fn dispatch_to_peers(&self, eth: EthV2) -> Result<(), TapDemoError> {
        let peers = self.0.peers.write().unwrap();
        // for brd
        if eth.dst_mac == [255, 255, 255, 255, 255, 255] {
            for peer in &*peers {
                // don't send to self
                if peer.hw_addr == self.0.hw_addr {
                    continue;
                }
                let _result = self.0.data_sock.send_to(&eth.data, &peer.data_addr)?;
            }
        } else {
            let peer = peers.iter().find(|&it| it.hw_addr == eth.dst_mac);

            match peer {
                Some(peer) => {
                    self.0.data_sock.send_to(&eth.data, &peer.data_addr)?;
                }
                None => {
                    error!("unknown dst {:x?}", eth.dst_mac);
                }
            }
        }

        Ok(())
    }
}

pub(crate) fn dispatch_from_peers(state: Arc<AppState>) {
    let data_sock = &state.data_sock;
    let mut buff = vec![0; 1500];
    let mut tap_dev = &state.tap_dev;

    loop {
        let _result = data_sock.recv(&mut buff);
        let _result = tap_dev.write(&buff);
    }
}
