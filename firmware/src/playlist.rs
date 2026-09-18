//! Random play order that never repeats an animation back to back.
//!
//! Each cycle is a shuffled permutation of every catalog index, so all
//! animations get equal air time. When a cycle ends a new permutation is
//! drawn; if it would start with the animation that just finished, that
//! first slot is swapped with a random other one. The random source is
//! injected so this module stays hardware-free.

use crate::catalog::MAX_ANIMATIONS;

const _: () = assert!(MAX_ANIMATIONS <= 256, "playlist slots are stored as u8");

pub struct Playlist {
    order: [u8; MAX_ANIMATIONS],
    len: usize,
    next: usize,
    last: Option<u8>,
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
            last: None,
        };
        playlist.shuffle(random);
        playlist
    }

    /// Index of the next animation to play, or `None` for an empty playlist.
    pub fn next(&mut self, random: &mut impl FnMut() -> u32) -> Option<usize> {
        if self.len == 0 {
            return None;
        }
        if self.next >= self.len {
            self.shuffle(random);
            self.next = 0;
        }
        let item = self.order[self.next];
        self.next += 1;
        self.last = Some(item);
        Some(usize::from(item))
    }

    /// Fisher-Yates over the live slots, then guards against an immediate repeat.
    fn shuffle(&mut self, random: &mut impl FnMut() -> u32) {
        for i in (1..self.len).rev() {
            let j = (random() as usize) % (i + 1);
            self.order.swap(i, j);
        }
        if self.len >= 2 && self.last == Some(self.order[0]) {
            let j = 1 + (random() as usize) % (self.len - 1);
            self.order.swap(0, j);
        }
    }
}
