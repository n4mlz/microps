mod header;
mod packet;
mod pcb;
mod registry;
mod retrans;

pub use header::*;
pub use packet::*;
pub use pcb::*;
pub use registry::*;
pub use retrans::*;
use thiserror::Error;

use super::{Ipv4Addr, Ipv4Endpoint, Ipv4Interface, Ipv4Packet, Ipv4Protocol};
use crate::{Platform, Random, debug, debugdump};

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

#[derive(Debug, thiserror::Error)]
pub enum TcpOpenError<E> {
    #[error("TCP PCB operation failed: {0}")]
    Pcb(#[from] TcpPcbError),
    #[error("TCP route or interface is unavailable")]
    NetworkUnavailable,
    #[error("TCP ISS generation failed")]
    Random(E),
    #[error("TCP segment output failed: {0}")]
    Output(#[from] TcpOutputError<E>),
}

#[derive(Debug, thiserror::Error)]
pub enum TcpSendError<E> {
    #[error("TCP PCB operation failed: {0}")]
    Pcb(#[from] TcpPcbError),
    #[error("TCP PCB is not established: {0:?}")]
    State(TcpState),
    #[error("TCP segment output failed: {0}")]
    Output(#[from] TcpOutputError<E>),
}

#[derive(Debug, thiserror::Error)]
pub enum TcpReceiveError {
    #[error("TCP PCB operation failed: {0}")]
    Pcb(#[from] TcpPcbError),
    #[error("TCP PCB is not established: {0:?}")]
    State(TcpState),
}

#[derive(Debug, thiserror::Error)]
pub enum TcpCloseError<E> {
    #[error("TCP PCB operation failed: {0}")]
    Pcb(#[from] TcpPcbError),
    #[error("TCP segment output failed: {0}")]
    Output(#[from] TcpOutputError<E>),
    #[error("TCP PCB is in an invalid state: {0:?}")]
    State(TcpState),
}

#[derive(Debug, Error)]
pub enum TcpInputError<E> {
    #[error("invalid TCP segment: {0}")]
    Packet(#[from] TcpError),
    #[error("TCP PCB operation failed: {0}")]
    Pcb(#[from] TcpPcbError),
    #[error("TCP segments must use unicast addresses")]
    Broadcast,
    #[error("TCP ISS generation failed")]
    Random(E),
    #[error("TCP response output failed: {0}")]
    Output(#[from] TcpOutputError<E>),
}

pub struct Tcp;

impl Tcp {
    pub fn open<P: Platform + Random + 'static>(
        local: Ipv4Endpoint,
        remote: Ipv4Endpoint,
        mode: TcpOpenMode,
    ) -> Result<TcpPcbKey, TcpOpenError<<P as Random>::Error>> {
        let pcb = P::stack().tcp_pcbs.open();
        if mode == TcpOpenMode::Active {
            if let Err(error_value) = Self::start_active::<P>(pcb, local, remote) {
                P::stack().tcp_pcbs.close(pcb);
                return Err(error_value);
            }
        } else {
            if P::stack().tcp_pcbs.endpoint_in_use(pcb, local, remote) {
                P::stack().tcp_pcbs.close(pcb);
                return Err(TcpOpenError::Pcb(TcpPcbError::AlreadyBound));
            }
            let mut listener = P::stack().tcp_pcbs.get(pcb)?;
            listener.listen(local, remote);
            P::stack().tcp_pcbs.replace(pcb, listener)?;
        }

        loop {
            let state = P::stack().tcp_pcbs.get(pcb)?.state();
            if state == TcpState::Established {
                let mut established = P::stack().tcp_pcbs.get(pcb)?;
                established.set_mss(Self::mss::<P>(established.remote())?);
                P::stack().tcp_pcbs.replace(pcb, established)?;
                return Ok(pcb);
            }
            if state != TcpState::Listen
                && state != TcpState::SynSent
                && state != TcpState::SynReceived
            {
                P::stack().tcp_pcbs.close(pcb);
                return Err(TcpOpenError::Pcb(TcpPcbError::NotFound));
            }
            P::stack().tcp_pcbs.wait_for_state_change(pcb, state)?;
        }
    }

    fn start_active<P: Platform + Random + 'static>(
        pcb: TcpPcbKey,
        mut local: Ipv4Endpoint,
        remote: Ipv4Endpoint,
    ) -> Result<(), TcpOpenError<<P as Random>::Error>> {
        if local.address() == Ipv4Addr::ANY {
            let route = P::stack()
                .ipv4_routes
                .lookup(remote.address())
                .ok_or(TcpOpenError::NetworkUnavailable)?;
            let interface = P::stack()
                .interfaces
                .interface_as::<Ipv4Interface>(route.interface())
                .ok_or(TcpOpenError::NetworkUnavailable)?;
            local = Ipv4Endpoint::new(interface.unicast(), local.port());
        }
        local = P::stack()
            .tcp_pcbs
            .assign_dynamic_port(pcb, local, remote)?;
        let iss = P::random32().map_err(TcpOpenError::Random)?;
        let mut active = P::stack().tcp_pcbs.get(pcb)?;
        active.connect(local, remote, iss);
        Self::output::<P>(&mut active, TcpFlags::SYN, &[])?;
        P::stack().tcp_pcbs.replace(pcb, active)?;
        Ok(())
    }

    pub fn close<P: Platform + Random + 'static>(
        pcb: TcpPcbKey,
    ) -> Result<(), TcpCloseError<<P as Random>::Error>> {
        let mut current = P::stack().tcp_pcbs.get(pcb)?;
        match current.state() {
            TcpState::Closed => Err(TcpCloseError::State(TcpState::Closed)),
            TcpState::Listen | TcpState::SynSent => {
                P::stack().tcp_pcbs.close(pcb);
                Ok(())
            }
            TcpState::SynReceived | TcpState::Established => {
                Self::output::<P>(&mut current, TcpFlags::ACK | TcpFlags::FIN, &[])?;
                current.enter_fin_wait1();
                P::stack().tcp_pcbs.replace(pcb, current)?;
                Ok(())
            }
            TcpState::CloseWait => {
                Self::output::<P>(&mut current, TcpFlags::ACK | TcpFlags::FIN, &[])?;
                current.enter_last_ack();
                P::stack().tcp_pcbs.replace(pcb, current)?;
                Ok(())
            }
            TcpState::FinWait1 | TcpState::FinWait2 | TcpState::LastAck | TcpState::TimeWait => {
                Err(TcpCloseError::State(current.state()))
            }
            state => Err(TcpCloseError::State(state)),
        }
    }

    pub fn send<P: Platform + Random + 'static>(
        pcb: TcpPcbKey,
        data: &[u8],
    ) -> Result<usize, TcpSendError<<P as Random>::Error>> {
        let current = P::stack().tcp_pcbs.get(pcb)?;
        if !matches!(
            current.state(),
            TcpState::Established | TcpState::FinWait1 | TcpState::FinWait2 | TcpState::CloseWait
        ) {
            return Err(TcpSendError::State(current.state()));
        }
        if data.is_empty() {
            return Ok(0);
        }
        let mut sent = 0;
        while sent < data.len() {
            let mut current = P::stack().tcp_pcbs.get(pcb)?;
            if !matches!(
                current.state(),
                TcpState::Established
                    | TcpState::FinWait1
                    | TcpState::FinWait2
                    | TcpState::CloseWait
            ) {
                return Err(TcpSendError::State(current.state()));
            }
            let in_flight = current.snd_nxt().wrapping_sub(current.snd_una());
            let capacity = u32::from(current.snd_wnd()).saturating_sub(in_flight) as usize;
            if capacity == 0 {
                P::stack().tcp_pcbs.wait_for_update(pcb)?;
                continue;
            }
            let length = capacity.min(current.mss()).min(data.len() - sent);
            Self::output::<P>(
                &mut current,
                TcpFlags::ACK | TcpFlags::PSH,
                &data[sent..sent + length],
            )?;
            current.advance_send(length);
            P::stack().tcp_pcbs.replace(pcb, current)?;
            sent += length;
        }
        Ok(sent)
    }

    fn mss<P: Platform + 'static>(
        remote: Ipv4Endpoint,
    ) -> Result<usize, TcpOpenError<<P as Random>::Error>> {
        let route = P::stack()
            .ipv4_routes
            .lookup(remote.address())
            .ok_or(TcpOpenError::NetworkUnavailable)?;
        let interface = P::stack()
            .interfaces
            .interface_as::<Ipv4Interface>(route.interface())
            .ok_or(TcpOpenError::NetworkUnavailable)?;
        let device = interface.device().ok_or(TcpOpenError::NetworkUnavailable)?;
        let devices = P::stack()
            .devices
            .acquire()
            .expect("device registry lock is infallible");
        let mtu = devices
            .get(device)
            .ok_or(TcpOpenError::NetworkUnavailable)?
            .meta()
            .mtu();
        mtu.checked_sub(20 + TCP_HEADER_LEN)
            .filter(|mss| *mss != 0)
            .ok_or(TcpOpenError::NetworkUnavailable)
    }

    pub fn receive<P: Platform + 'static>(
        pcb: TcpPcbKey,
        buffer: &mut [u8],
    ) -> Result<usize, TcpReceiveError> {
        loop {
            let mut current = P::stack().tcp_pcbs.get(pcb)?;
            match current.state() {
                TcpState::Established | TcpState::FinWait1 | TcpState::FinWait2 => {}
                TcpState::CloseWait if !current.has_received_data() => return Ok(0),
                TcpState::CloseWait => {}
                TcpState::Closing | TcpState::LastAck | TcpState::TimeWait => return Ok(0),
                state => return Err(TcpReceiveError::State(state)),
            }
            if buffer.is_empty() {
                return Ok(0);
            }
            let length = current.receive(buffer);
            if length != 0 {
                P::stack().tcp_pcbs.replace(pcb, current)?;
                return Ok(length);
            }
            P::stack().tcp_pcbs.wait_for_update(pcb)?;
        }
    }

    pub fn tick<P: Platform + Random + 'static>() -> Result<(), TcpOutputError<<P as Random>::Error>>
    {
        P::stack()
            .tcp_pcbs
            .expire_time_wait(P::monotonic_time_seconds());
        for retrans in P::stack()
            .tcp_pcbs
            .due_retrans(P::monotonic_time_microseconds())
        {
            Self::output_segment::<P>(
                retrans.seq,
                retrans.ack,
                retrans.flags,
                retrans.window,
                &retrans.payload,
                retrans.local,
                retrans.remote,
            )?;
        }
        Ok(())
    }

    pub fn input<P: Platform + Random + 'static>(
        packet: Ipv4Packet<'_>,
        interface: &Ipv4Interface,
    ) -> Result<(), TcpInputError<<P as Random>::Error>> {
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
                window: packet.header().window_size(),
            },
            packet.header().flags(),
            packet.dst(),
            packet.src(),
            packet.payload(),
        )
    }

    fn segment_arrives<P: Platform + Random + 'static>(
        segment: SegmentInfo,
        flags: TcpFlags,
        local: Ipv4Endpoint,
        remote: Ipv4Endpoint,
        data: &[u8],
    ) -> Result<(), TcpInputError<<P as Random>::Error>> {
        let pcb = P::stack().tcp_pcbs.select(local, remote);
        let Some((pcb, state)) = pcb else {
            return Self::respond_to_unknown::<P>(segment, flags, local, remote);
        };
        if state == TcpState::Closed {
            return Self::respond_to_unknown::<P>(segment, flags, local, remote);
        }
        let mut pcb_state = P::stack().tcp_pcbs.get(pcb)?;
        match state {
            TcpState::Listen => {
                if flags.contains(TcpFlags::ACK) {
                    Self::output_segment::<P>(segment.ack, 0, TcpFlags::RST, 0, &[], local, remote)
                        .map(|_| ())?;
                } else if flags.contains(TcpFlags::SYN) {
                    let iss = P::random32().map_err(TcpInputError::Random)?;
                    let mut accepted = P::stack().tcp_pcbs.get(pcb)?;
                    accepted.accept_syn(local, remote, segment.seq, iss);
                    Self::output::<P>(&mut accepted, TcpFlags::SYN | TcpFlags::ACK, &[])
                        .map(|_| ())?;
                    P::stack().tcp_pcbs.replace(pcb, accepted)?;
                }
            }
            TcpState::SynSent => {
                let ack_acceptable = if flags.contains(TcpFlags::ACK) {
                    if segment.ack <= pcb_state.iss() || segment.ack > pcb_state.snd_nxt() {
                        Self::output_segment::<P>(
                            segment.ack,
                            0,
                            TcpFlags::RST,
                            0,
                            &[],
                            local,
                            remote,
                        )
                        .map(|_| ())?;
                        return Ok(());
                    }
                    true
                } else {
                    false
                };
                if flags.contains(TcpFlags::RST) {
                    if ack_acceptable {
                        P::stack().tcp_pcbs.close(pcb);
                    }
                    return Ok(());
                }
                if !flags.contains(TcpFlags::SYN) {
                    return Ok(());
                }
                if ack_acceptable {
                    pcb_state.accept_syn_ack(segment.seq, segment.ack, segment.window);
                }
                if pcb_state.state() == TcpState::Established {
                    P::stack().tcp_pcbs.replace(pcb, pcb_state.clone())?;
                    Self::output::<P>(&mut pcb_state, TcpFlags::ACK, &[]).map(|_| ())?;
                    debug!("TCP PCB state: {:?}", TcpState::Established);
                } else {
                    pcb_state.accept_simultaneous_syn(segment.seq);
                    Self::output::<P>(&mut pcb_state, TcpFlags::SYN | TcpFlags::ACK, &[])
                        .map(|_| ())?;
                    P::stack().tcp_pcbs.replace(pcb, pcb_state.clone())?;
                }
            }
            TcpState::SynReceived
            | TcpState::Established
            | TcpState::FinWait1
            | TcpState::FinWait2
            | TcpState::CloseWait
            | TcpState::Closing
            | TcpState::LastAck
            | TcpState::TimeWait => {
                if !pcb_state.accept_segment(segment.seq, segment.length) {
                    if !flags.contains(TcpFlags::RST) {
                        Self::output::<P>(&mut pcb_state, TcpFlags::ACK, &[]).map(|_| ())?;
                    }
                    return Ok(());
                }
                if flags.contains(TcpFlags::RST) {
                    P::stack().tcp_pcbs.close(pcb);
                    return Ok(());
                }
                if flags.contains(TcpFlags::SYN) {
                    Self::output::<P>(&mut pcb_state, TcpFlags::RST, &[]).map(|_| ())?;
                    P::stack().tcp_pcbs.close(pcb);
                    return Ok(());
                }
                if state == TcpState::LastAck
                    && flags.contains(TcpFlags::ACK)
                    && segment.ack == pcb_state.snd_nxt()
                {
                    P::stack().tcp_pcbs.close(pcb);
                    return Ok(());
                }
                if flags.contains(TcpFlags::ACK) {
                    match pcb_state.accept_ack(segment.seq, segment.ack, segment.window) {
                        TcpAckResult::TooNew => {
                            Self::output::<P>(&mut pcb_state, TcpFlags::ACK, &[]).map(|_| ())?;
                            return Ok(());
                        }
                        TcpAckResult::Accepted => {
                            if pcb_state.state() == TcpState::Established {
                                debug!("TCP PCB state: {:?}", TcpState::Established);
                            }
                        }
                        TcpAckResult::Old | TcpAckResult::Unchanged | TcpAckResult::Ignored => {}
                    }
                    if state == TcpState::Closing && segment.ack == pcb_state.snd_nxt() {
                        pcb_state.enter_time_wait(P::monotonic_time_seconds());
                    }
                    P::stack().tcp_pcbs.replace(pcb, pcb_state.clone())?;
                } else {
                    return Ok(());
                }
                if matches!(
                    state,
                    TcpState::Established | TcpState::FinWait1 | TcpState::FinWait2
                ) && !data.is_empty()
                {
                    if !pcb_state.accept_payload(segment.seq, data) {
                        Self::output::<P>(&mut pcb_state, TcpFlags::ACK, &[]).map(|_| ())?;
                        return Ok(());
                    }
                    P::stack().tcp_pcbs.replace(pcb, pcb_state.clone())?;
                    Self::output::<P>(&mut pcb_state, TcpFlags::ACK, &[]).map(|_| ())?;
                }
            }
            _ => {}
        }
        if flags.contains(TcpFlags::FIN)
            && matches!(
                state,
                TcpState::SynReceived
                    | TcpState::Established
                    | TcpState::FinWait1
                    | TcpState::FinWait2
                    | TcpState::CloseWait
                    | TcpState::Closing
                    | TcpState::LastAck
                    | TcpState::TimeWait
            )
        {
            if !pcb_state.accept_fin_at(segment.seq, data.len(), P::monotonic_time_seconds()) {
                Self::output::<P>(&mut pcb_state, TcpFlags::ACK, &[]).map(|_| ())?;
                return Ok(());
            }
            if state == TcpState::FinWait1 {
                if segment.ack == pcb_state.snd_nxt() {
                    pcb_state.enter_time_wait(P::monotonic_time_seconds());
                } else {
                    pcb_state.set_closing();
                }
            }
            Self::output::<P>(&mut pcb_state, TcpFlags::ACK, &[]).map(|_| ())?;
            P::stack().tcp_pcbs.replace(pcb, pcb_state.clone())?;
            if state != pcb_state.state() {
                debug!("TCP PCB state: {:?}", pcb_state.state());
            }
        }
        Ok(())
    }

    fn respond_to_unknown<P: Platform + Random + 'static>(
        segment: SegmentInfo,
        flags: TcpFlags,
        local: Ipv4Endpoint,
        remote: Ipv4Endpoint,
    ) -> Result<(), TcpInputError<<P as Random>::Error>> {
        if flags.contains(TcpFlags::RST) {
            return Ok(());
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
        Self::output_segment::<P>(seq, ack, response_flags, 0, &[], local, remote).map(|_| ())?;
        Ok(())
    }

    fn output<P: Platform + Random + 'static>(
        pcb: &mut TcpPcb,
        flags: TcpFlags,
        payload: &[u8],
    ) -> Result<usize, TcpOutputError<<P as Random>::Error>> {
        let seq = if flags.contains(TcpFlags::SYN) {
            pcb.iss()
        } else {
            pcb.snd_nxt()
        };
        if flags.intersects(TcpFlags::SYN | TcpFlags::FIN) || !payload.is_empty() {
            pcb.queue_retrans(seq, flags, payload, P::monotonic_time_microseconds());
        }
        Self::output_segment::<P>(
            seq,
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
    window: u16,
}

#[derive(Debug, thiserror::Error)]
pub enum TcpOutputError<E> {
    #[error("TCP segment construction failed: {0}")]
    Packet(#[from] TcpError),
    #[error("IPv4 output failed: {0}")]
    Ipv4(#[from] super::Ipv4OutputError<E>),
}
