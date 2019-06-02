use std::sync::{Arc, RwLock};

use super::eth::EthV2;
use super::AppState;

/// dispatch packet to peers
pub(crate) fn dispatch(state: Arc<RwLock<AppState>>, eth: EthV2) {}
