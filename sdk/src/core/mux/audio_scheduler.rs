//! Turns bursty dubbed-audio arrivals (one utterance every few seconds, with
//! STT→translate→TTS latency in between) into a continuous PCM stream: pull
//! [`DubAudioScheduler::next_chunk`] on a fixed real-time clock, get real dub
//! audio when queued, silence otherwise. This is what keeps ffmpeg's second
//! input (the dub audio TCP socket, see `mux` module docs) fed at a steady
//! byte rate even though dub audio itself arrives in irregular, few-seconds
//! chunks.
//!
//! Deliberately simple/reactive: no timestamp alignment against `ts_start_ms`/
//! `ts_end_ms` (the backend stamps those with its own wall clock — the SDK has
//! no shared clock with it, so trying to align would just fake a precision
//! that isn't there). No max-gap fallback either — if dub audio is delayed,
//! silence keeps going for as long as it takes (product decision: switching
//! to the original mic mid-target would mix two languages on one stream,
//! which is more confusing than silence).

use std::collections::VecDeque;

pub struct DubAudioScheduler {
    queue: VecDeque<i16>,
}

impl DubAudioScheduler {
    pub fn new() -> Self {
        DubAudioScheduler {
            queue: VecDeque::new(),
        }
    }

    /// A new dubbed utterance arrived (already base64-decoded PCM samples) —
    /// appended to the queue, played after anything already queued (FIFO,
    /// arrival order — utterances are already delivered in speech order).
    pub fn push_dub_audio(&mut self, samples: Vec<i16>) {
        self.queue.extend(samples);
    }

    /// Pulls exactly `n_samples` samples: real dub audio first, silence (0)
    /// padding for whatever the queue doesn't have. Always returns exactly
    /// `n_samples` — the caller writes a steady byte rate to ffmpeg
    /// regardless of whether real dub audio is available right now.
    pub fn next_chunk(&mut self, n_samples: usize) -> Vec<i16> {
        let mut out = Vec::with_capacity(n_samples);
        for _ in 0..n_samples {
            out.push(self.queue.pop_front().unwrap_or(0));
        }
        out
    }
}

impl Default for DubAudioScheduler {
    fn default() -> Self {
        DubAudioScheduler::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_scheduler_yields_silence() {
        let mut sched = DubAudioScheduler::new();
        assert_eq!(sched.next_chunk(4), vec![0, 0, 0, 0]);
    }

    #[test]
    fn queued_audio_is_returned_before_silence_padding() {
        let mut sched = DubAudioScheduler::new();
        sched.push_dub_audio(vec![10, 20]);
        // Ask for more than queued — real samples first, then silence.
        assert_eq!(sched.next_chunk(5), vec![10, 20, 0, 0, 0]);
    }

    #[test]
    fn exact_fit_leaves_no_silence() {
        let mut sched = DubAudioScheduler::new();
        sched.push_dub_audio(vec![1, 2, 3]);
        assert_eq!(sched.next_chunk(3), vec![1, 2, 3]);
        // Queue now empty — next pull is pure silence.
        assert_eq!(sched.next_chunk(2), vec![0, 0]);
    }

    #[test]
    fn long_dub_audio_drains_across_multiple_chunks_in_order() {
        let mut sched = DubAudioScheduler::new();
        sched.push_dub_audio((1..=10).collect());
        assert_eq!(sched.next_chunk(4), vec![1, 2, 3, 4]);
        assert_eq!(sched.next_chunk(4), vec![5, 6, 7, 8]);
        // Only 2 real samples left, padded with silence.
        assert_eq!(sched.next_chunk(4), vec![9, 10, 0, 0]);
    }

    #[test]
    fn two_pushes_are_consumed_fifo_not_interleaved() {
        let mut sched = DubAudioScheduler::new();
        sched.push_dub_audio(vec![1, 2]);
        sched.push_dub_audio(vec![3, 4]);
        assert_eq!(sched.next_chunk(4), vec![1, 2, 3, 4]);
    }
}
