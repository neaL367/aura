use crossbeam_channel::{Receiver, Sender, TrySendError, bounded};

use crate::decoder::DecodedFrame;

/// Capacity of the bounded frame channel (frames buffered between decoder and renderer).
///
/// 2 frames (double buffering) absorbs timing jitter while keeping RAM footprint minimal.
pub const FRAME_CHANNEL_CAPACITY: usize = 2;

/// Sending end of a bounded frame channel.
///
/// Owned by the decoder worker thread.  `try_send` applies non-blocking
/// back-pressure: if the channel is full the newest frame is dropped
/// (the send returns `false`) and the caller decides how to retry.
/// Use the interruptible send pattern rather than `send_blocking` to
/// avoid deadlocking when the renderer is paused and the queue is full.
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
    /// Try to send a frame without blocking.  Returns `true` if sent,
    /// `false` if the channel was full (frame dropped) or disconnected.
    ///
    /// Note: `false` is returned for both `Full` (retry appropriate) and
    /// `Disconnected` (receiver gone — caller should exit). Use
    /// `try_send_checked` when you need to distinguish the two cases.
    pub fn try_send(&self, frame: DecodedFrame) -> bool {
        match self.0.try_send(frame) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => false,
            Err(TrySendError::Disconnected(_)) => false,
        }
    }

    /// Try to send a frame without blocking, returning the specific outcome.
    pub fn try_send_checked(&self, frame: DecodedFrame) -> Result<(), TrySendError<DecodedFrame>> {
        self.0.try_send(frame)
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
