use std::convert::TryInto;
use std::fs::{File, OpenOptions};
use std::os::raw::{c_char, c_short};
use std::os::unix::io::{FromRawFd, IntoRawFd};
use std::time::Duration;

use libc::ioctl;
use std::net::{IpAddr, Ipv4Addr};

static TUN_DEV: &'static str = "/dev/net/tun";
static IFF_TAP: c_short = 0x0002;
static IFF_NO_PI: c_short = 0x1000;
static IFF_UP: c_short = 0x0001;

static TUNSETIFF: u64 = 1074025674;
static SIOCGIFHWADDR: u64 = 0x8927;
static SIOCSIFFLAGS: u64 = 0x8914;
static SIOCGIFFLAGS: u64 = 0x8913;
static SIOCSIFADDR: u64 = 0x8916;

#[derive(Debug)]
#[repr(C)]
struct IfReq {
    if_name: [c_char; 16],

    ifr_ifru: [u8; 24],
}

#[derive(Debug)]
pub struct TapInfo {
    pub tap_dev: File,
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
        self.if_name = [0; 16];

        for (idx, data) in name.as_bytes().iter().enumerate() {
            self.if_name[idx] = data.clone().try_into().unwrap();
        }
    }

    pub fn if_flags(&mut self, flags: c_short) {
        self.ifr_ifru[0] |= flags as u8;
        self.ifr_ifru[1] |= (flags >> 8) as u8;
    }

    pub fn if_hwaddr(&self) -> [u8; 6] {
        let mut hwaddr = [0; 6];

        hwaddr.copy_from_slice(&self.ifr_ifru[2..8]);

        hwaddr
    }

    pub fn if_add_ipv4(&mut self, ipv4: Ipv4Addr) {
        use libc::AF_INET;

        // sin_family
        self.ifr_ifru[0] = AF_INET as u8;
        self.ifr_ifru[1] = 0; // since we known AF_INET is 1

        // sin_port
        self.ifr_ifru[2] = 0;
        self.ifr_ifru[3] = 0;

        // sin_addr
        self.ifr_ifru[4..8].copy_from_slice(&ipv4.octets());
    }
}

pub fn create_tap(name: &str) -> Result<TapInfo, crate::error::TapDemoError> {
    let tun_dev = OpenOptions::new().write(true).read(true).open(TUN_DEV)?;
    let mut ifreq = IfReq::with_name(name);
    ifreq.if_flags(IFF_TAP | IFF_NO_PI);

    unsafe {
        let fd = tun_dev.into_raw_fd();
        let mut rc = ioctl(fd, TUNSETIFF, &ifreq);
        if rc != 0 {
            return Err(crate::error::TapDemoError::TapCreateError(rc));
        }

        // fixme: 没有 sleep 的话， SIOCGIFHWADDR 获取到的 hwaddr 是一个随机的错误值
        std::thread::sleep(Duration::from_millis(1000));

        rc = ioctl(fd, SIOCGIFHWADDR, &ifreq);

        if rc != 0 {
            return Err(crate::error::TapDemoError::GetHWAddrError);
        }

        let hw_addr = ifreq.if_hwaddr();

        use libc::{socket, AF_INET, SOCK_DGRAM};

        let skfd = socket(AF_INET, SOCK_DGRAM, 0);
        let mut ifreq = IfReq::with_name(name);
        rc = ioctl(skfd, SIOCGIFFLAGS, &ifreq);
        if rc != 0 {
            return Err(crate::error::TapDemoError::TapSetupError);
        }

        ifreq.if_flags(IFF_UP);
        rc = ioctl(skfd, SIOCSIFFLAGS, &ifreq);
        if rc != 0 {
            return Err(crate::error::TapDemoError::TapSetupError);
        }

        // todo: ip prefix, should use rtnetlink
        //        ifreq.if_add_ipv4(Ipv4Addr::new(10, 0, 0, 1));
        //        rc = ioctl(skfd, SIOCSIFADDR, &ifreq);

        libc::close(skfd);

        Ok(TapInfo {
            tap_dev: File::from_raw_fd(fd),
            hw_addr,
        })
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
