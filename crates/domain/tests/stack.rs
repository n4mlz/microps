use core::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, OnceLock};

use microps::{
    Irq, IrqLine, Lock, Platform, Random, Stack, Time,
    protocol::{
        Ipv4Addr, Ipv4Endpoint, TcpAckResult, TcpFlags, TcpOpenError, TcpOpenMode, TcpPcb,
        TcpState, UdpPcbError,
    },
};

struct MockRuntime;

static STACK: OnceLock<Stack<MockRuntime>> = OnceLock::new();

#[derive(Debug, Default)]
struct TestMutex<T>(Mutex<T>, Condvar);

impl<T> Lock<T> for TestMutex<T> {
    type Error = core::convert::Infallible;
    type Guard<'a>
        = MutexGuard<'a, T>
    where
        T: 'a;

    fn new(value: T) -> Self {
        Self(Mutex::new(value), Condvar::new())
    }

    fn acquire(&self) -> Result<Self::Guard<'_>, Self::Error> {
        Ok(self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()))
    }

    fn wait<'a>(&'a self, guard: Self::Guard<'a>) -> Result<Self::Guard<'a>, Self::Error> {
        Ok(self
            .1
            .wait(guard)
            .unwrap_or_else(|poisoned| poisoned.into_inner()))
    }

    fn wake_all(&self) {
        self.1.notify_all();
    }
}

static INIT_CALLS: AtomicUsize = AtomicUsize::new(0);
static SHUTDOWN_CALLS: AtomicUsize = AtomicUsize::new(0);

impl Platform for MockRuntime {
    type Error = core::convert::Infallible;
    type Mutex<T: Send> = TestMutex<T>;

    fn stack() -> &'static Stack<Self> {
        STACK.get_or_init(Stack::new)
    }

    fn init() -> Result<(), <Self as Platform>::Error> {
        INIT_CALLS.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn shutdown() {
        SHUTDOWN_CALLS.fetch_add(1, Ordering::SeqCst);
    }
}

impl Random for MockRuntime {
    type Error = core::convert::Infallible;

    fn random16() -> Result<u16, Self::Error> {
        Ok(0)
    }
}

impl Time for MockRuntime {
    fn monotonic_time_microseconds() -> u64 {
        0
    }
}

impl Irq for MockRuntime {
    type Error = core::convert::Infallible;

    fn register(_: IrqLine, _: Box<dyn Fn(IrqLine) + Send + Sync>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn raise(_: IrqLine) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[test]
fn stack_lifecycle_calls_runtime_hooks() {
    INIT_CALLS.store(0, Ordering::SeqCst);
    SHUTDOWN_CALLS.store(0, Ordering::SeqCst);

    Stack::<MockRuntime>::init().unwrap();
    Stack::<MockRuntime>::shutdown();

    assert_eq!(INIT_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(SHUTDOWN_CALLS.load(Ordering::SeqCst), 1);
}

#[test]
fn udp_registry_opens_binds_and_releases_sockets() {
    let stack = MockRuntime::stack();
    let first = stack.udp_pcbs.open();
    let second = stack.udp_pcbs.open();
    let endpoint = Ipv4Endpoint::new(Ipv4Addr::from([192, 0, 2, 2]), 7);

    assert_eq!(stack.udp_pcbs.bind(first, endpoint), Ok(()));
    assert_eq!(
        stack.udp_pcbs.bind(second, endpoint),
        Err(UdpPcbError::AlreadyBound)
    );
    assert_eq!(stack.udp_pcbs.close(first), Ok(()));
    assert_eq!(stack.udp_pcbs.bind(second, endpoint), Ok(()));
    assert_eq!(stack.udp_pcbs.close(second), Ok(()));
    assert_eq!(stack.udp_pcbs.close(second), Err(UdpPcbError::NotFound));
}

#[test]
fn socket_api_selects_an_ipv4_transport() {
    use microps::protocol::{Socket, SocketDomain, SocketProtocol, SocketType};

    let socket = Socket::open::<MockRuntime>(
        SocketDomain::Ipv4,
        SocketType::Datagram,
        Some(SocketProtocol::Udp),
    )
    .unwrap();
    Socket::bind::<MockRuntime>(socket, Ipv4Endpoint::new(Ipv4Addr::ANY, 40001)).unwrap();
    Socket::close::<MockRuntime>(socket).unwrap();
    assert!(
        Socket::open::<MockRuntime>(
            SocketDomain::Ipv4,
            SocketType::Stream,
            Some(SocketProtocol::Udp),
        )
        .is_err()
    );
}

#[test]
fn tcp_registry_listens_and_rejects_duplicate_endpoints() {
    let stack = MockRuntime::stack();
    let first = stack.tcp_pcbs.open();
    let second = stack.tcp_pcbs.open();
    let local = Ipv4Endpoint::new(Ipv4Addr::ANY, 7);
    let remote = Ipv4Endpoint::new(Ipv4Addr::ANY, 0);

    let mut listener = stack.tcp_pcbs.get(first).unwrap();
    listener.listen(local, remote);
    assert_eq!(stack.tcp_pcbs.replace(first, listener), Ok(()));
    assert_eq!(stack.tcp_pcbs.get(first).unwrap().state(), TcpState::Listen);
    assert!(stack.tcp_pcbs.endpoint_in_use(second, local, remote));
    assert!(stack.tcp_pcbs.close(first));
    assert!(stack.tcp_pcbs.close(second));
}

#[test]
fn tcp_active_open_requires_a_route() {
    assert!(matches!(
        microps::protocol::Tcp::open::<MockRuntime>(
            Ipv4Endpoint::new(Ipv4Addr::ANY, 7),
            Ipv4Endpoint::new(Ipv4Addr::ANY, 0),
            TcpOpenMode::Active,
        ),
        Err(TcpOpenError::NetworkUnavailable)
    ));
}

#[test]
fn tcp_pcb_accepts_acks_and_buffers_payload() {
    let local = Ipv4Endpoint::new(Ipv4Addr::ANY, 7);
    let remote = Ipv4Endpoint::new(Ipv4Addr::from([192, 0, 2, 2]), 50000);
    let mut pcb = TcpPcb::new();

    pcb.listen(local, Ipv4Endpoint::new(Ipv4Addr::ANY, 0));
    pcb.accept_syn(local, remote, 100, 200);
    assert_eq!(pcb.state(), TcpState::SynReceived);
    assert_eq!(pcb.accept_ack(101, 201, 4096), TcpAckResult::Accepted);
    assert_eq!(pcb.state(), TcpState::Established);
    assert!(pcb.accept_segment(101, 3));
    assert!(pcb.accept_payload(101, b"hey"));

    let mut buffer = [0; 3];
    assert_eq!(pcb.receive(&mut buffer), 3);
    assert_eq!(&buffer, b"hey");
}

#[test]
fn tcp_pcb_completes_an_active_handshake() {
    let local = Ipv4Endpoint::new(Ipv4Addr::from([192, 0, 2, 2]), 50000);
    let remote = Ipv4Endpoint::new(Ipv4Addr::from([192, 0, 2, 1]), 7);
    let mut pcb = TcpPcb::new();

    pcb.connect(local, remote, 300);
    assert_eq!(pcb.state(), TcpState::SynSent);
    pcb.queue_retrans(300, TcpFlags::SYN, &[], 0);
    pcb.accept_syn_ack(700, 301, 4096);

    assert_eq!(pcb.state(), TcpState::Established);
    assert_eq!(pcb.rcv_nxt(), 701);
    assert_eq!(pcb.snd_una(), 301);
    assert_eq!(pcb.snd_wnd(), 4096);
    assert!(pcb.due_retrans(200_000).is_empty());
}

#[test]
fn tcp_pcb_enters_close_wait_after_fin_and_last_ack_after_local_close() {
    let local = Ipv4Endpoint::new(Ipv4Addr::ANY, 7);
    let remote = Ipv4Endpoint::new(Ipv4Addr::from([192, 0, 2, 2]), 50000);
    let mut pcb = TcpPcb::new();

    pcb.accept_syn(local, remote, 100, 200);
    pcb.accept_ack(101, 201, 4096);
    assert!(pcb.accept_fin(101, 0));
    assert_eq!(pcb.state(), TcpState::CloseWait);
    assert_eq!(pcb.rcv_nxt(), 102);

    pcb.enter_last_ack();
    assert_eq!(pcb.state(), TcpState::LastAck);
    assert_eq!(pcb.snd_nxt(), 202);
}

#[test]
fn tcp_pcb_retransmits_with_backoff_and_cleans_up_acked_data() {
    let local = Ipv4Endpoint::new(Ipv4Addr::ANY, 7);
    let remote = Ipv4Endpoint::new(Ipv4Addr::from([192, 0, 2, 2]), 50000);
    let mut pcb = TcpPcb::new();

    pcb.accept_syn(local, remote, 100, 200);
    pcb.queue_retrans(200, TcpFlags::SYN, &[], 0);
    assert!(pcb.due_retrans(199_999).is_empty());
    assert_eq!(pcb.due_retrans(200_000).len(), 1);
    assert!(pcb.due_retrans(599_999).is_empty());
    assert_eq!(pcb.due_retrans(600_000).len(), 1);

    assert_eq!(pcb.accept_ack(101, 201, 4096), TcpAckResult::Accepted);
    assert!(pcb.due_retrans(12_000_001).is_empty());
}
