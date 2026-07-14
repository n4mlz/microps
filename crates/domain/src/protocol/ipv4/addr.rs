use core::{fmt, str::FromStr};

/// IPv4 address in network byte order.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ipv4Addr {
    octets: [u8; 4],
}

impl Ipv4Addr {
    pub const ANY: Self = Self::new([0, 0, 0, 0]);
    pub const BROADCAST: Self = Self::new([255, 255, 255, 255]);

    pub const fn new(octets: [u8; 4]) -> Self {
        Self { octets }
    }

    pub const fn octets(self) -> [u8; 4] {
        self.octets
    }
}

impl From<[u8; 4]> for Ipv4Addr {
    fn from(octets: [u8; 4]) -> Self {
        Self::new(octets)
    }
}

impl fmt::Display for Ipv4Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}.{}.{}",
            self.octets[0], self.octets[1], self.octets[2], self.octets[3]
        )
    }
}

impl FromStr for Ipv4Addr {
    type Err = Ipv4AddrParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut octets = [0; 4];
        let mut parts = s.split('.');

        for octet in &mut octets {
            let part = parts.next().ok_or(Ipv4AddrParseError)?;
            if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(Ipv4AddrParseError);
            }
            *octet = part.parse().map_err(|_| Ipv4AddrParseError)?;
        }

        if parts.next().is_some() {
            return Err(Ipv4AddrParseError);
        }

        Ok(Self::new(octets))
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid IPv4 address")]
pub struct Ipv4AddrParseError;
