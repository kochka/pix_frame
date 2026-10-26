//! Random play order that keeps repeats of an animation far apart.
//!
//! Each cycle is a shuffled permutation of every catalog index, so all
//! animations get equal air time. When a cycle ends a new permutation is
//! drawn whose first third only holds animations that were not in the last
//! third of the previous cycle, so at least a third of the catalog plays
//! between two showings of the same animation. The random source is
//! injected so this module stays hardware-free.

use crate::catalog::MAX_ANIMATIONS;

const _: () = assert!(MAX_ANIMATIONS <= 256, "playlist slots are stored as u8");

pub struct Playlist {
    order: [u8; MAX_ANIMATIONS],
    len: usize,
    next: usize,
}

impl Playlist {
    /// Builds a shuffled cycle over `0..len` (capped at [`MAX_ANIMATIONS`]).
    pub fn new(len: usize, random: &mut impl FnMut() -> u32) -> Self {
        let len = len.min(MAX_ANIMATIONS);
        let mut order = [0u8; MAX_ANIMATIONS];
        for (index, slot) in order.iter_mut().enumerate().take(len) {
            // Lossless: `index < MAX_ANIMATIONS <= 256`.
            *slot = index as u8;
        }
        let mut playlist = Self {
            order,
            len,
            next: 0,
        };
        playlist.shuffle(0, len, random);
        playlist
    }

    /// Index of the next animation to play, or `None` for an empty playlist.
    pub fn next(&mut self, random: &mut impl FnMut() -> u32) -> Option<usize> {
        if self.len == 0 {
            return None;
        }
        if self.next >= self.len {
            self.start_cycle(random);
            self.next = 0;
        }
        let item = self.order[self.next];
        self.next += 1;
        Some(usize::from(item))
    }

    /// Reorders the cycle that just ended into the next one.
    ///
    /// The `gap` animations played last sit at the end of `order`. Shuffling
    /// the other slots first and keeping the head of that shuffle, then
    /// shuffling everything after the head, leaves the head free of recent
    /// animations while every other slot stays uniformly random.
    fn start_cycle(&mut self, random: &mut impl FnMut() -> u32) {
        // At most half, so the head is never forced to repeat the previous head.
        let gap = (self.len / 3).max(1).min(self.len / 2);
        self.shuffle(0, self.len - gap, random);
        self.shuffle(gap, self.len, random);
    }

    /// Fisher-Yates over `order[start..end]`.
    fn shuffle(&mut self, start: usize, end: usize, random: &mut impl FnMut() -> u32) {
        for i in (start + 1..end).rev() {
            let j = start + (random() as usize) % (i - start + 1);
            self.order.swap(i, j);
        }
    }
}
