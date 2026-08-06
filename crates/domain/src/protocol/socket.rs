use slotmap::{SlotMap, new_key_type};
use thiserror::Error;

use super::{
    Ipv4Endpoint, Tcp, TcpCloseError, TcpOpenError, TcpPcbError, TcpPcbKey, TcpReceiveError,
    TcpSendError, Udp, UdpOutputError, UdpPcbError, UdpPcbKey,
};
use crate::{Lock, Platform, Random};

new_key_type! {
    pub struct SocketKey;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketDomain {
    Ipv4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketType {
    Stream,
    Datagram,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketProtocol {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Copy)]
enum SocketTransport {
    Tcp(TcpPcbKey),
    Udp(UdpPcbKey),
}

#[derive(Debug, Clone, Copy)]
struct SocketEntry {
    domain: SocketDomain,
    kind: SocketType,
    transport: SocketTransport,
}

#[derive(Debug)]
pub struct SocketRegistry<P: Platform> {
    sockets: P::Mutex<SlotMap<SocketKey, SocketEntry>>,
}

#[derive(Debug, Error)]
pub enum SocketError<E> {
    #[error("unsupported socket domain")]
    Domain,
    #[error("unsupported socket type")]
    Type,
    #[error("socket protocol does not match socket type")]
    Protocol,
    #[error("socket does not exist")]
    NotFound,
    #[error("TCP operation failed: {0}")]
    TcpOpen(#[from] TcpOpenError<E>),
    #[error("TCP close failed: {0}")]
    TcpClose(#[from] TcpCloseError<E>),
    #[error("TCP send failed: {0}")]
    TcpSend(#[from] TcpSendError<E>),
    #[error("TCP receive failed: {0}")]
    TcpReceive(#[from] TcpReceiveError),
    #[error("TCP PCB operation failed: {0}")]
    TcpPcb(#[from] TcpPcbError),
    #[error("UDP output failed: {0}")]
    UdpOutput(#[from] UdpOutputError<E>),
    #[error("UDP PCB operation failed: {0}")]
    UdpPcb(#[from] UdpPcbError),
}

pub struct Socket;

impl Socket {
    pub fn open<P: Platform + Random + 'static>(
        domain: SocketDomain,
        kind: SocketType,
        protocol: Option<SocketProtocol>,
    ) -> Result<SocketKey, SocketError<<P as Random>::Error>> {
        if domain != SocketDomain::Ipv4 {
            return Err(SocketError::Domain);
        }
        let transport = match (kind, protocol) {
            (SocketType::Stream, None | Some(SocketProtocol::Tcp)) => {
                SocketTransport::Tcp(P::stack().tcp_pcbs.open())
            }
            (SocketType::Datagram, None | Some(SocketProtocol::Udp)) => {
                SocketTransport::Udp(P::stack().udp_pcbs.open())
            }
            _ => return Err(SocketError::Protocol),
        };
        Ok(P::stack().sockets.insert(SocketEntry {
            domain,
            kind,
            transport,
        }))
    }

    pub fn close<P: Platform + Random + 'static>(
        socket: SocketKey,
    ) -> Result<(), SocketError<<P as Random>::Error>> {
        let entry = P::stack()
            .sockets
            .remove(socket)
            .ok_or(SocketError::NotFound)?;
        match entry.transport {
            SocketTransport::Tcp(pcb) => Tcp::close::<P>(pcb).map_err(SocketError::TcpClose),
            SocketTransport::Udp(pcb) => {
                P::stack().udp_pcbs.close(pcb).map_err(SocketError::UdpPcb)
            }
        }
    }

    pub fn bind<P: Platform + 'static>(
        socket: SocketKey,
        local: Ipv4Endpoint,
    ) -> Result<(), SocketError<<P as Random>::Error>> {
        let entry = P::stack()
            .sockets
            .get(socket)
            .ok_or(SocketError::NotFound)?;
        match entry.transport {
            SocketTransport::Tcp(pcb) => Tcp::bind::<P>(pcb, local).map_err(SocketError::TcpPcb),
            SocketTransport::Udp(pcb) => P::stack()
                .udp_pcbs
                .bind(pcb, local)
                .map_err(SocketError::UdpPcb),
        }
    }

    pub fn listen<P: Platform + 'static>(
        socket: SocketKey,
        backlog: usize,
    ) -> Result<(), SocketError<<P as Random>::Error>> {
        let entry = P::stack()
            .sockets
            .get(socket)
            .ok_or(SocketError::NotFound)?;
        if entry.kind != SocketType::Stream {
            return Err(SocketError::Type);
        }
        let SocketTransport::Tcp(pcb) = entry.transport else {
            unreachable!("stream sockets always use TCP")
        };
        Tcp::listen::<P>(pcb, backlog).map_err(SocketError::TcpPcb)
    }

    pub fn accept<P: Platform + Random + 'static>(
        socket: SocketKey,
    ) -> Result<(SocketKey, Ipv4Endpoint), SocketError<<P as Random>::Error>> {
        let entry = P::stack()
            .sockets
            .get(socket)
            .ok_or(SocketError::NotFound)?;
        if entry.kind != SocketType::Stream {
            return Err(SocketError::Type);
        }
        let SocketTransport::Tcp(pcb) = entry.transport else {
            unreachable!("stream sockets always use TCP")
        };
        let (accepted, remote) = Tcp::accept::<P>(pcb).map_err(SocketError::TcpOpen)?;
        let child = P::stack().sockets.insert(SocketEntry {
            domain: entry.domain,
            kind: SocketType::Stream,
            transport: SocketTransport::Tcp(accepted),
        });
        Ok((child, remote))
    }

    pub fn connect<P: Platform + Random + 'static>(
        socket: SocketKey,
        remote: Ipv4Endpoint,
    ) -> Result<(), SocketError<<P as Random>::Error>> {
        let entry = P::stack()
            .sockets
            .get(socket)
            .ok_or(SocketError::NotFound)?;
        let SocketTransport::Tcp(pcb) = entry.transport else {
            return Err(SocketError::Type);
        };
        Tcp::connect::<P>(pcb, remote).map_err(SocketError::TcpOpen)
    }

    pub fn recv<P: Platform + 'static>(
        socket: SocketKey,
        buffer: &mut [u8],
    ) -> Result<usize, SocketError<<P as Random>::Error>> {
        let entry = P::stack()
            .sockets
            .get(socket)
            .ok_or(SocketError::NotFound)?;
        let SocketTransport::Tcp(pcb) = entry.transport else {
            return Err(SocketError::Type);
        };
        Tcp::receive::<P>(pcb, buffer).map_err(SocketError::TcpReceive)
    }

    pub fn send<P: Platform + Random + 'static>(
        socket: SocketKey,
        data: &[u8],
    ) -> Result<usize, SocketError<<P as Random>::Error>> {
        let entry = P::stack()
            .sockets
            .get(socket)
            .ok_or(SocketError::NotFound)?;
        let SocketTransport::Tcp(pcb) = entry.transport else {
            return Err(SocketError::Type);
        };
        Tcp::send::<P>(pcb, data).map_err(SocketError::TcpSend)
    }

    pub fn recv_from<P: Platform + 'static>(
        socket: SocketKey,
        buffer: &mut [u8],
    ) -> Result<(usize, Ipv4Endpoint), SocketError<<P as Random>::Error>> {
        let entry = P::stack()
            .sockets
            .get(socket)
            .ok_or(SocketError::NotFound)?;
        let SocketTransport::Udp(pcb) = entry.transport else {
            return Err(SocketError::Type);
        };
        Udp::recv_from::<P>(pcb, buffer).map_err(SocketError::UdpPcb)
    }

    pub fn send_to<P: Platform + Random + 'static>(
        socket: SocketKey,
        data: &[u8],
        remote: Ipv4Endpoint,
    ) -> Result<usize, SocketError<<P as Random>::Error>> {
        let entry = P::stack()
            .sockets
            .get(socket)
            .ok_or(SocketError::NotFound)?;
        let SocketTransport::Udp(pcb) = entry.transport else {
            return Err(SocketError::Type);
        };
        Udp::send_to::<P>(pcb, data, remote).map_err(SocketError::UdpOutput)
    }
}

impl<P: Platform> Default for SocketRegistry<P> {
    fn default() -> Self {
        Self {
            sockets: P::Mutex::new(SlotMap::default()),
        }
    }
}

impl<P: Platform> SocketRegistry<P> {
    fn insert(&self, entry: SocketEntry) -> SocketKey {
        self.sockets
            .acquire()
            .expect("socket registry lock is infallible")
            .insert(entry)
    }

    fn get(&self, socket: SocketKey) -> Option<SocketEntry> {
        self.sockets
            .acquire()
            .expect("socket registry lock is infallible")
            .get(socket)
            .copied()
    }

    fn remove(&self, socket: SocketKey) -> Option<SocketEntry> {
        self.sockets
            .acquire()
            .expect("socket registry lock is infallible")
            .remove(socket)
    }
}
