use alloc::{collections::VecDeque, sync::Arc, vec::Vec};

use crate::{DeviceKey, Lock, Platform};

/// An owned frame waiting for processing outside device input context.
#[derive(Debug, PartialEq, Eq)]
pub struct ReceivedFrame {
    device: DeviceKey,
    frame_type: u16,
    data: Vec<u8>,
}

impl ReceivedFrame {
    pub fn new(device: DeviceKey, frame_type: u16, data: &[u8]) -> Self {
        Self {
            device,
            frame_type,
            data: data.to_vec(),
        }
    }

    pub fn device(&self) -> DeviceKey {
        self.device
    }

    pub fn frame_type(&self) -> u16 {
        self.frame_type
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

/// The owned input queue storage.
#[derive(Debug)]
pub struct InputQueueInner<P: Platform> {
    frames: P::Mutex<VecDeque<ReceivedFrame>>,
}

impl<P: Platform> Default for InputQueueInner<P> {
    fn default() -> Self {
        Self {
            frames: P::Mutex::new(VecDeque::new()),
        }
    }
}

impl<P: Platform> InputQueueInner<P> {
    pub fn push(&self, frame: ReceivedFrame) {
        P::Mutex::acquire(&self.frames)
            .expect("input queue lock is infallible")
            .push_back(frame);
    }

    pub fn pop(&self) -> Option<ReceivedFrame> {
        P::Mutex::acquire(&self.frames)
            .expect("input queue lock is infallible")
            .pop_front()
    }

    pub fn len(&self) -> usize {
        P::Mutex::acquire(&self.frames)
            .expect("input queue lock is infallible")
            .len()
    }
}

/// Shared input queue owned by the stack and devices.
pub type InputQueue<P> = Arc<InputQueueInner<P>>;
