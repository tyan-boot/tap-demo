use std::convert::TryInto;
use std::fs::OpenOptions;
use std::os::raw::{c_char, c_short};
use std::os::unix::io::IntoRawFd;
use std::time::Duration;

use libc::ioctl;

static TUN_DEV: &'static str = "/dev/net/tun";
static IFFTAP: c_short = 2;
static IFF_NO_PI: c_short = 4096;
static TUNSETIFF: u64 = 1074025674;
static SIOCGIFHWADDR: u64 = 0x8927;

#[derive(Debug)]
struct IfReq {
    if_name: [c_char; 16],

    ifr_ifru: [u8; 24],
}

#[derive(Debug)]
pub struct TapInfo {
    pub fd: i32,
    pub hw_addr: [u8; 6],
}

impl IfReq {
    pub fn with_name(name: &str) -> IfReq {
        let mut if_name = [0; 16];

        for (idx, data) in name.as_bytes().iter().enumerate() {
            if_name[idx] = data.clone().try_into().unwrap();
        }

        IfReq {
            if_name,
            ifr_ifru: [0; 24],
        }
    }

    pub fn if_name(&mut self, name: &str) {
        for (idx, data) in name.as_bytes().iter().enumerate() {
            self.if_name[idx] = data.clone().try_into().unwrap();
        }
    }

    pub fn if_flags(&mut self, flags: c_short) {
        self.ifr_ifru[0] = flags as u8;
        self.ifr_ifru[1] = (flags << 8) as u8;
    }

    pub fn if_hwaddr(&self) -> [u8; 6] {
        let mut hwaddr = [0; 6];

        hwaddr.copy_from_slice(&self.ifr_ifru[2..8]);

        hwaddr
    }
}

pub fn create_tap(name: &str) -> Result<TapInfo, crate::error::TapDemoError> {
    let tun_dev = OpenOptions::new().write(true).open(TUN_DEV)?;
    let mut ifreq = IfReq::with_name(name);
    ifreq.if_flags(IFFTAP | IFF_NO_PI);

    unsafe {
        let fd = tun_dev.into_raw_fd();
        let mut rc = ioctl(fd, TUNSETIFF, &ifreq);

        if rc != 0 {
            return Err(crate::error::TapDemoError::TapCreateError(rc));
        }

        // fixme: 没有 sleep 的话， SIOCGIFHWADDR 获取到的 hwaddr 是一个随机的错误值
        std::thread::sleep(Duration::from_millis(100));

        rc = ioctl(fd, SIOCGIFHWADDR, &ifreq);

        if rc != 0 {
            return Err(crate::error::TapDemoError::GetHWAddrError);
        }

        let hw_addr = ifreq.if_hwaddr();

        Ok(TapInfo { fd, hw_addr })
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn test_create_tap() {
        use super::create_tap;

        let result = create_tap("tap0");

        dbg!(&result);

        assert!(result.is_ok());
    }
}
