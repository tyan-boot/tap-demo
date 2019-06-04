#[derive(Debug)]
pub struct EthV2<'a> {
    pub dst_mac: [u8; 6],
    pub src_mac: [u8; 6],
    pub proto_type: u16,
    pub data: &'a [u8],
}
