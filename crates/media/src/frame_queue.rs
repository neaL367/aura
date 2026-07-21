use crossbeam_channel::{Receiver, Sender, TrySendError, bounded};

use crate::decoder::DecodedFrame;

/// Capacity of the bounded frame channel (frames buffered between decoder and renderer).
///
/// 3 frames is sufficient to absorb minor timing jitter without unbounded growth.
pub const FRAME_CHANNEL_CAPACITY: usize = 3;

/// Sending end of a bounded frame channel.
///
/// Owned by the decoder worker thread.  `send_frame` applies back-pressure:
/// if the channel is full it drops the oldest frame (for video) or blocks
/// (for GIF, where timing matters).
pub struct FrameSender(Sender<DecodedFrame>);

/// Receiving end of a bounded frame channel.
///
/// Owned by the render thread.  Non-blocking: returns `None` if no frame
/// is ready, allowing the renderer to display the previous frame.
pub struct FrameReceiver(Receiver<DecodedFrame>);

/// Create a matched sender/receiver pair with a fixed capacity.
pub fn frame_channel() -> (FrameSender, FrameReceiver) {
    let (tx, rx) = bounded(FRAME_CHANNEL_CAPACITY);
    (FrameSender(tx), FrameReceiver(rx))
}

impl FrameSender {
    /// Try to send a frame.  Returns `true` if sent, `false` if channel was full.
    pub fn try_send(&self, frame: DecodedFrame) -> bool {
        match self.0.try_send(frame) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => false,
            Err(TrySendError::Disconnected(_)) => false,
        }
    }

    /// Block until the frame is sent or the channel is disconnected.
    pub fn send_blocking(&self, frame: DecodedFrame) -> bool {
        self.0.send(frame).is_ok()
    }
}

impl FrameReceiver {
    /// Try to receive a frame without blocking.
    pub fn try_recv(&self) -> Option<DecodedFrame> {
        self.0.try_recv().ok()
    }
}
