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
/// Messages kept unacked before the oldest are dropped. Never reached by an
/// honest peer in S2 (lobby edits, 1 hash per second, pause, bye).
pub const MAX_QUEUED: usize = 1024;

#[derive(Debug)]
pub struct CtlOut<T> {
    next: u32,
    /// `(cseq, message, last sent)`; `last sent` is `None` until the first
    /// send.
    queue: VecDeque<(u32, T, Option<u64>)>,
}

impl<T: Clone> CtlOut<T> {
    pub fn new() -> CtlOut<T> {
        CtlOut {
            next: 0,
            queue: VecDeque::new(),
        }
    }

    /// Queues `msg`; it goes out on the next [`CtlOut::due`].
    pub fn push(&mut self, msg: T) -> u32 {
        let c = self.next;
        self.next = self.next.wrapping_add(1);
        if self.queue.len() >= MAX_QUEUED {
            self.queue.pop_front();
        }
        self.queue.push_back((c, msg, None));
        c
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
}
