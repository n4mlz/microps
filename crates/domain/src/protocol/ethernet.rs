use core::{fmt, str::FromStr};

use thiserror::Error;

/// The length of an Ethernet MAC address in bytes.
pub const ADDRESS_LEN: usize = 6;

/// The length of an Ethernet frame header without an optional VLAN tag.
pub const HEADER_LEN: usize = 14;

/// Common EtherTypes assigned by IEEE/IANA.
pub mod ethertype {
    pub const IPV4: u16 = 0x0800;
    pub const ARP: u16 = 0x0806;
    pub const IPV6: u16 = 0x86dd;
}

/// An Ethernet MAC address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EthernetAddress([u8; ADDRESS_LEN]);

impl EthernetAddress {
    pub const ANY: Self = Self([0; ADDRESS_LEN]);
    pub const BROADCAST: Self = Self([u8::MAX; ADDRESS_LEN]);

    pub const fn new(octets: [u8; ADDRESS_LEN]) -> Self {
        Self(octets)
    }

    pub const fn octets(self) -> [u8; ADDRESS_LEN] {
        self.0
    }
}

impl From<[u8; ADDRESS_LEN]> for EthernetAddress {
    fn from(octets: [u8; ADDRESS_LEN]) -> Self {
        Self::new(octets)
    }
}

impl fmt::Display for EthernetAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, octet) in self.0.iter().enumerate() {
            if index != 0 {
                formatter.write_str(":")?;
            }
            write!(formatter, "{octet:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for EthernetAddress {
    type Err = EthernetAddressParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let bytes = value.as_bytes();
        if bytes.len() != ADDRESS_LEN * 3 - 1 {
            return Err(EthernetAddressParseError);
        }

        let mut octets = [0; ADDRESS_LEN];
        for (index, octet) in octets.iter_mut().enumerate() {
            let offset = index * 3;
            if index != 0 && bytes[offset - 1] != b':' {
                return Err(EthernetAddressParseError);
            }
            let high = hex_digit(bytes[offset]).ok_or(EthernetAddressParseError)?;
            let low = hex_digit(bytes[offset + 1]).ok_or(EthernetAddressParseError)?;
            *octet = (high << 4) | low;
        }

        Ok(Self(octets))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("invalid Ethernet MAC address")]
pub struct EthernetAddressParseError;

/// A borrowed Ethernet frame with its header fields decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EthernetFrame<'a> {
    destination: EthernetAddress,
    source: EthernetAddress,
    ethertype: u16,
    payload: &'a [u8],
}

impl EthernetFrame<'_> {
    pub const fn destination(&self) -> EthernetAddress {
        self.destination
    }

    pub const fn source(&self) -> EthernetAddress {
        self.source
    }

    pub const fn ethertype(&self) -> u16 {
        self.ethertype
    }

    pub const fn payload(&self) -> &[u8] {
        self.payload
    }
}

impl<'a> TryFrom<&'a [u8]> for EthernetFrame<'a> {
    type Error = EthernetError;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        if data.len() < HEADER_LEN {
            return Err(EthernetError::TooShort { len: data.len() });
        }

        let mut destination = [0; ADDRESS_LEN];
        destination.copy_from_slice(&data[..ADDRESS_LEN]);
        let mut source = [0; ADDRESS_LEN];
        source.copy_from_slice(&data[ADDRESS_LEN..ADDRESS_LEN * 2]);

        Ok(Self {
            destination: EthernetAddress::from(destination),
            source: EthernetAddress::from(source),
            ethertype: u16::from_be_bytes([data[12], data[13]]),
            payload: &data[HEADER_LEN..],
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum EthernetError {
    #[error("Ethernet frame is too short: {len} bytes")]
    TooShort { len: usize },
}

const fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
