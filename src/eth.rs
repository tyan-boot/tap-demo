#[derive(Debug)]
pub struct EthV2 {
    pub dst_mac: [u8; 6],
    pub src_mac: [u8; 6],
    pub proto_type: u16,
    pub data: Vec<u8>,
}
