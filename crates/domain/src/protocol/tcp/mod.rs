mod header;
mod packet;
mod registry;

pub use header::*;
pub use packet::*;
pub use registry::*;
use thiserror::Error;

use super::{Ipv4Addr, Ipv4Endpoint, Ipv4Interface, Ipv4Packet, Ipv4Protocol};
use crate::{Platform, Random, debug, debugdump, error};

pub const TCP_HEADER_LEN: usize = 20;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TcpState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpOpenMode {
    Active,
    Passive,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum TcpOpenError {
    #[error("TCP active open is not implemented")]
    ActiveOpenUnsupported,
    #[error("TCP PCB operation failed: {0}")]
    Pcb(#[from] TcpPcbError),
}

#[derive(Debug, thiserror::Error)]
pub enum TcpCloseError<E> {
    #[error("TCP PCB operation failed: {0}")]
    Pcb(#[from] TcpPcbError),
    #[error("TCP segment output failed: {0}")]
    Output(#[from] TcpOutputError<E>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TcpInputError {
    #[error("invalid TCP segment: {0}")]
    Packet(#[from] TcpError),
    #[error("TCP segments must use unicast addresses")]
    Broadcast,
}

pub struct Tcp;

impl Tcp {
    pub fn open<P: Platform + Random + 'static>(
        local: Ipv4Endpoint,
        remote: Ipv4Endpoint,
        mode: TcpOpenMode,
    ) -> Result<TcpPcbKey, TcpOpenError> {
        let pcb = P::stack().tcp_pcbs.open();
        if mode == TcpOpenMode::Active {
            P::stack().tcp_pcbs.close(pcb);
            return Err(TcpOpenError::ActiveOpenUnsupported);
        }
        if P::stack().tcp_pcbs.endpoint_in_use(pcb, local, remote) {
            P::stack().tcp_pcbs.close(pcb);
            return Err(TcpOpenError::Pcb(TcpPcbError::AlreadyBound));
        }
        let mut listener = P::stack().tcp_pcbs.get(pcb)?;
        listener.listen(local, remote);
        P::stack().tcp_pcbs.replace(pcb, listener)?;

        loop {
            let state = P::stack().tcp_pcbs.get(pcb)?.state();
            if state == TcpState::Established {
                return Ok(pcb);
            }
            if state != TcpState::Listen && state != TcpState::SynReceived {
                P::stack().tcp_pcbs.close(pcb);
                return Err(TcpOpenError::Pcb(TcpPcbError::NotFound));
            }
            P::stack().tcp_pcbs.wait_for_state_change(pcb, state)?;
        }
    }

    pub fn close<P: Platform + Random + 'static>(
        pcb: TcpPcbKey,
    ) -> Result<(), TcpCloseError<<P as Random>::Error>> {
        let snapshot = P::stack().tcp_pcbs.get(pcb)?;
        let output = Self::output::<P>(&snapshot, TcpFlags::RST, &[]);
        P::stack().tcp_pcbs.close(pcb);
        output.map(|_| ()).map_err(TcpCloseError::Output)
    }

    pub fn input<P: Platform + Random + 'static>(
        packet: Ipv4Packet<'_>,
        interface: &Ipv4Interface,
    ) -> Result<(), TcpInputError> {
        let packet = TcpPacket::from_ipv4(packet)?;
        if packet.src().address() == Ipv4Addr::BROADCAST
            || packet.src().address() == interface.broadcast()
            || packet.dst().address() == Ipv4Addr::BROADCAST
            || packet.dst().address() == interface.broadcast()
        {
            return Err(TcpInputError::Broadcast);
        }

        debug!(
            "{} => {}, len={}, dev={:?}",
            packet.src(),
            packet.dst(),
            packet.data().len(),
            interface.device()
        );
        debug!("src: {}", packet.header().src_port());
        debug!("dst: {}", packet.header().dst_port());
        debug!("seq: {}", packet.header().seq());
        debug!("ack: {}", packet.header().ack());
        debug!(
            "off: 0x{:02x} ({}), options: {}, payload: {}",
            packet.header().data_offset(),
            packet.header().header_len(),
            packet.options().len(),
            packet.payload().len()
        );
        debug!(
            "flg: 0x{:02x} ({:?})",
            packet.header().flags().bits(),
            packet.header().flags()
        );
        debug!("wnd: {}", packet.header().window_size());
        debug!("sum: 0x{:04x}", packet.header().checksum());
        debug!("up: {}", packet.header().urgent_pointer());
        debugdump(packet.data());
        let length = packet.payload().len()
            + usize::from(packet.header().flags().contains(TcpFlags::SYN))
            + usize::from(packet.header().flags().contains(TcpFlags::FIN));
        Self::segment_arrives::<P>(
            SegmentInfo {
                seq: packet.header().seq(),
                ack: packet.header().ack(),
                length: length as u32,
            },
            packet.header().flags(),
            packet.dst(),
            packet.src(),
        );
        Ok(())
    }

    fn segment_arrives<P: Platform + Random + 'static>(
        segment: SegmentInfo,
        flags: TcpFlags,
        local: Ipv4Endpoint,
        remote: Ipv4Endpoint,
    ) {
        let pcb = P::stack().tcp_pcbs.select(local, remote);
        let Some((pcb, state)) = pcb else {
            return Self::respond_to_unknown::<P>(segment, flags, local, remote);
        };
        if state == TcpState::Closed {
            return Self::respond_to_unknown::<P>(segment, flags, local, remote);
        }
        if flags.contains(TcpFlags::RST) {
            return;
        }
        match state {
            TcpState::Listen => {
                if flags.contains(TcpFlags::ACK) {
                    if let Err(error_value) = Self::output_segment::<P>(
                        segment.ack,
                        0,
                        TcpFlags::RST,
                        0,
                        &[],
                        local,
                        remote,
                    ) {
                        error!("{error_value}");
                    }
                } else if flags.contains(TcpFlags::SYN) {
                    let Ok(iss) = P::random32() else {
                        error!("TCP ISS generation failed");
                        return;
                    };
                    let Ok(mut accepted) = P::stack().tcp_pcbs.get(pcb) else {
                        return;
                    };
                    accepted.accept_syn(local, remote, segment.seq, iss);
                    if P::stack().tcp_pcbs.replace(pcb, accepted).is_err() {
                        return;
                    }
                    if let Err(error_value) =
                        Self::output::<P>(&accepted, TcpFlags::SYN | TcpFlags::ACK, &[])
                    {
                        error!("{error_value}");
                    }
                }
            }
            TcpState::SynSent => {}
            TcpState::SynReceived => {
                if !flags.contains(TcpFlags::ACK) {
                    return;
                }
                let Ok(mut accepted) = P::stack().tcp_pcbs.get(pcb) else {
                    return;
                };
                if accepted.accept_ack(segment.ack) {
                    if let Err(error_value) = P::stack().tcp_pcbs.replace(pcb, accepted) {
                        error!("{error_value}");
                    } else {
                        debug!("TCP PCB state: {:?}", TcpState::Established);
                    }
                } else {
                    if let Err(error_value) = Self::output_segment::<P>(
                        segment.ack,
                        0,
                        TcpFlags::RST,
                        0,
                        &[],
                        local,
                        remote,
                    ) {
                        error!("{error_value}");
                    }
                }
            }
            _ => {}
        }
    }

    fn respond_to_unknown<P: Platform + Random + 'static>(
        segment: SegmentInfo,
        flags: TcpFlags,
        local: Ipv4Endpoint,
        remote: Ipv4Endpoint,
    ) {
        if flags.contains(TcpFlags::RST) {
            return;
        }
        let (seq, ack, response_flags) = if flags.contains(TcpFlags::ACK) {
            (segment.ack, 0, TcpFlags::RST)
        } else {
            (
                0,
                segment.seq.wrapping_add(segment.length),
                TcpFlags::RST | TcpFlags::ACK,
            )
        };
        if let Err(error_value) =
            Self::output_segment::<P>(seq, ack, response_flags, 0, &[], local, remote)
        {
            error!("{error_value}");
        }
    }

    fn output<P: Platform + Random + 'static>(
        pcb: &TcpPcb,
        flags: TcpFlags,
        payload: &[u8],
    ) -> Result<usize, TcpOutputError<<P as Random>::Error>> {
        Self::output_segment::<P>(
            if flags.contains(TcpFlags::SYN) {
                pcb.iss()
            } else {
                pcb.snd_nxt()
            },
            pcb.rcv_nxt(),
            flags,
            pcb.rcv_wnd(),
            payload,
            pcb.local(),
            pcb.remote(),
        )
    }

    fn output_segment<P: Platform + Random + 'static>(
        seq: u32,
        ack: u32,
        flags: TcpFlags,
        window_size: u16,
        payload: &[u8],
        local: Ipv4Endpoint,
        remote: Ipv4Endpoint,
    ) -> Result<usize, TcpOutputError<<P as Random>::Error>> {
        let segment = TcpPacket::build(local, remote, seq, ack, flags, window_size, payload)?;
        let route = P::stack()
            .ipv4_routes
            .lookup(remote.address())
            .ok_or(TcpOutputError::Ipv4(
                super::Ipv4OutputError::DestinationUnreachable,
            ))?;
        let interface = P::stack()
            .interfaces
            .interface_as::<Ipv4Interface>(route.interface())
            .ok_or(TcpOutputError::Ipv4(
                super::Ipv4OutputError::DestinationUnreachable,
            ))?;
        interface
            .output::<P, P>(
                Ipv4Protocol::Tcp as u8,
                &segment,
                local.address(),
                remote.address(),
            )
            .map(|_| payload.len())
            .map_err(TcpOutputError::Ipv4)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SegmentInfo {
    seq: u32,
    ack: u32,
    length: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum TcpOutputError<E> {
    #[error("TCP segment construction failed: {0}")]
    Packet(#[from] TcpError),
    #[error("IPv4 output failed: {0}")]
    Ipv4(#[from] super::Ipv4OutputError<E>),
}
