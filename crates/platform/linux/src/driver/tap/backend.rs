use microps::{
    DeviceBackend, DeviceError, DeviceKey, IrqLine, Platform, ReceivedFrame, debug, debugdump,
    info,
    protocol::{EtherType, EthernetFrame, FRAME_LEN_MAX, FRAME_LEN_MIN, HEADER_LEN, MacAddr},
};

use super::raw::Tap;
use crate::os::signal_number;

const READ_BUFFER_LEN: usize = FRAME_LEN_MAX;

pub const fn irq() -> IrqLine {
    IrqLine::DeviceInput
}

pub struct EtherTapDevice {
    name: String,
    address: MacAddr,
    device_key: Option<DeviceKey>,
    tap: Option<Tap>,
}

impl EtherTapDevice {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            address: MacAddr::ANY,
            device_key: None,
            tap: None,
        }
    }

    fn tap(&mut self) -> Result<&mut Tap, DeviceError> {
        self.tap.as_mut().ok_or(DeviceError::NotOpen)
    }

    fn handle_frame<P: Platform + 'static>(&mut self, frame: &[u8]) -> Result<(), DeviceError> {
        let frame =
            EthernetFrame::try_from(frame).map_err(|error| backend_error(error.to_string()))?;
        if frame.header().destination() != self.address
            && frame.header().destination() != MacAddr::BROADCAST
        {
            return Ok(());
        }
        let device = self.device_key.ok_or(DeviceError::MissingDeviceKey)?;
        let frame_type = frame.header().ether_type();
        debug!(
            "dev={}, type=0x{frame_type:04x}, len={}",
            self.name,
            frame.payload().len()
        );
        debugdump(frame.payload());
        P::stack()
            .input_queue
            .push(ReceivedFrame::new(device, frame_type, frame.payload()));
        Ok(())
    }
}

impl<P: Platform + 'static> DeviceBackend<P> for EtherTapDevice {
    fn set_device_key(&mut self, device: DeviceKey) {
        self.device_key = Some(device);
    }

    fn open(&mut self) -> Result<(), DeviceError> {
        let tap = Tap::open(&self.name).map_err(|error| backend_error(error.to_string()))?;
        self.address = MacAddr::from(
            tap.hardware_address(&self.name)
                .map_err(|error| backend_error(error.to_string()))?,
        );
        info!("dev={}, addr={}", self.name, self.address);
        tap.configure_async(signal_number(irq()))
            .map_err(|error| backend_error(error.to_string()))?;
        self.tap = Some(tap);
        Ok(())
    }

    fn close(&mut self) -> Result<(), DeviceError> {
        self.tap = None;
        Ok(())
    }

    fn output(
        &mut self,
        frame_type: u16,
        data: &[u8],
        dst: Option<&[u8]>,
    ) -> Result<(), DeviceError> {
        let destination = dst.ok_or(DeviceError::MissingDestination)?;
        let destination: [u8; 6] =
            destination
                .try_into()
                .map_err(|_| DeviceError::InvalidDestination {
                    len: destination.len(),
                })?;
        if data.len() > FRAME_LEN_MAX - HEADER_LEN {
            return Err(DeviceError::PayloadTooLarge {
                mtu: FRAME_LEN_MAX - HEADER_LEN,
                len: data.len(),
            });
        }
        let ether_type =
            EtherType::try_from(frame_type).map_err(|error| backend_error(error.to_string()))?;
        let mut frame =
            EthernetFrame::build(self.address, MacAddr::from(destination), ether_type, data)
                .map_err(|error| backend_error(error.to_string()))?;
        frame.resize(frame.len().max(FRAME_LEN_MIN), 0);
        debug!(
            "dev={}, type=0x{frame_type:04x}, len={}",
            self.name,
            frame.len()
        );
        debugdump(&frame);
        let tap = self.tap()?;
        let written = tap
            .write_frame(&frame)
            .map_err(|error| backend_error(error.to_string()))?;
        if written != frame.len() {
            return Err(backend_error(format!(
                "short TAP write: {written}/{}",
                frame.len()
            )));
        }
        Ok(())
    }

    fn input(&mut self) -> Result<(), DeviceError> {
        let mut buffer = [0; READ_BUFFER_LEN];
        loop {
            let length = match self.tap()?.read_frame(&mut buffer) {
                Ok(0) => return Ok(()),
                Ok(length) => length,
                Err(error) if matches!(error.raw_os_error(), Some(errno) if errno == libc::EAGAIN || errno == libc::EWOULDBLOCK) =>
                {
                    return Ok(());
                }
                Err(error) if error.raw_os_error() == Some(libc::EINTR) => continue,
                Err(error) => return Err(backend_error(error.to_string())),
            };
            self.handle_frame::<P>(&buffer[..length])?;
        }
    }
}

fn backend_error(message: impl Into<String>) -> DeviceError {
    DeviceError::Backend {
        message: message.into(),
    }
}
