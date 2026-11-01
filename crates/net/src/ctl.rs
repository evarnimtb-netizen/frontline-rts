//! The reliable, ordered control stream both sides keep (S2 "Transport":
//! control messages are re-sent every 100 ms until acked).
//!
//! Each message gets the next `cseq`. The receiver processes only the next
//! `cseq` it expects and answers every control datagram with a cumulative
//! [`Ack`](crate::protocol::Ack), so a lost or reordered message is simply
//! resent; nothing is processed twice or out of order.

use std::collections::VecDeque;

/// Resend interval for unacked control messages (µs).
pub const RESEND_US: u64 = 100_000;
/// Unacked messages resent per interval; the receiver takes them in order,
/// so resending the whole backlog after a long gap would only add load.
pub const RESEND_WINDOW: usize = 4;
/// Messages kept unacked at most. Past it the stream is broken for good
/// (dropping any message would leave a `cseq` gap the receiver never gets
/// past), so [`CtlOut::push`] refuses the message and flags the queue;
/// the owner drops the peer. Never reached by an honest peer in S2 (lobby
/// broadcasts are coalesced, 1 hash per second, rate-limited pauses, bye).
pub const MAX_QUEUED: usize = 1024;

#[derive(Debug)]
pub struct CtlOut<T> {
    next: u32,
    /// `(cseq, message, last sent)`; `last sent` is `None` until the first
    /// send.
    queue: VecDeque<(u32, T, Option<u64>)>,
    overflowed: bool,
}

impl<T: Clone> CtlOut<T> {
    pub fn new() -> CtlOut<T> {
        CtlOut {
            next: 0,
            queue: VecDeque::new(),
            overflowed: false,
        }
    }

    /// Queues `msg`; it goes out on the next [`CtlOut::due`]. With
    /// [`MAX_QUEUED`] messages unacked, `msg` is refused (`None`) and the
    /// queue is flagged [`CtlOut::overflowed`].
    pub fn push(&mut self, msg: T) -> Option<u32> {
        if self.queue.len() >= MAX_QUEUED {
            self.overflowed = true;
            return None;
        }
        let c = self.next;
        self.next = self.next.wrapping_add(1);
        self.queue.push_back((c, msg, None));
        Some(c)
    }

    /// A message was refused: the peer stopped acking.
    pub fn overflowed(&self) -> bool {
        self.overflowed
    }

    /// Forgets every queued message (the peer is gone).
    pub fn clear(&mut self) {
        self.queue.clear();
    }

    /// The peer processed every `cseq < next`.
    pub fn ack(&mut self, next: u32) {
        // Ignore acks for messages never sent (a forged or corrupt ack).
        if next.wrapping_sub(self.queue.front().map_or(self.next, |q| q.0))
            > self.queue.len() as u32
        {
            return;
        }
        while let Some((c, _, _)) = self.queue.front() {
            if c.wrapping_sub(next) as i32 >= 0 {
                break;
            }
            self.queue.pop_front();
        }
    }

    /// Messages to send now: every never-sent one, and the oldest
    /// [`RESEND_WINDOW`] whose last send is [`RESEND_US`] ago.
    pub fn due(&mut self, now: u64) -> Vec<(u32, T)> {
        let mut out = Vec::new();
        for (i, (c, m, last)) in self.queue.iter_mut().enumerate() {
            let send = match *last {
                None => true,
                Some(t) => i < RESEND_WINDOW && now.saturating_sub(t) >= RESEND_US,
            };
            if send {
                *last = Some(now);
                out.push((*c, m.clone()));
            }
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }
}

impl<T: Clone> Default for CtlOut<T> {
    fn default() -> CtlOut<T> {
        CtlOut::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resends_until_acked_in_order() {
        let mut q = CtlOut::new();
        q.push("a");
        q.push("b");
        assert_eq!(q.due(0), vec![(0, "a"), (1, "b")]);
        assert!(q.due(50_000).is_empty());
        q.ack(1);
        assert_eq!(q.due(100_000), vec![(1, "b")]);
        // A forged ack past anything sent is ignored.
        q.ack(1000);
        assert_eq!(q.len(), 1);
        q.ack(2);
        assert!(q.is_empty());
    }

    #[test]
    fn overflow_refuses_and_flags() {
        let mut q = CtlOut::new();
        for i in 0..MAX_QUEUED {
            assert_eq!(q.push(i), Some(i as u32));
        }
        assert!(!q.overflowed());
        assert_eq!(q.push(9999), None);
        assert!(q.overflowed());
        // Nothing was dropped from the front: the stream stays gap-free.
        assert_eq!(q.due(0)[0], (0, 0));
        assert_eq!(q.len(), MAX_QUEUED);
    }
}
