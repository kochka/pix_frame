//! Conversion pipeline from a decoded [`Animation`] to PXA bytes, following
//! `doc/PXA_spec.md` section 17.

use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use color_quant::NeuQuant;
use image::RgbaImage;
use image::imageops::{self, FilterType};
use pxa_core::{FrameSource, Header, Rgb, encoded_size, write};

use crate::animation::Animation;

/// Duration given to static images when none is configured.
const STATIC_IMAGE_DURATION_MS: u16 = 1000;

/// NeuQuant sampling factor; 1 uses every pixel for the best result.
const NEUQUANT_SAMPLE_FACTOR: i32 = 1;

pub struct Options {
    pub size: Option<(u16, u16)>,
    pub background: Rgb,
    pub default_duration: Option<u16>,
    pub max_colors: u16,
}

pub struct Report {
    pub bytes: Vec<u8>,
    pub width: u16,
    pub height: u16,
    pub frame_count: u32,
    pub palette_size: u16,
    pub bits_per_pixel: u8,
    pub notes: Vec<String>,
}

struct TimedFrame<'a> {
    image: &'a RgbaImage,
    duration_ms: u16,
}

pub fn convert(animation: &Animation, options: &Options) -> Result<Report> {
    let mut notes = Vec::new();

    let frames = normalize_durations(animation, options, &mut notes)?;
    let (width, height) = target_size(animation, options)?;

    // Resize, then composite against the background: one RGB triple per pixel.
    let rgb_frames: Vec<Vec<Rgb>> = frames
        .iter()
        .map(|frame| {
            let resized;
            let image = if (width, height) == (frame.image.width(), frame.image.height()) {
                frame.image
            } else {
                resized = imageops::resize(frame.image, width, height, FilterType::Nearest);
                &resized
            };
            composite(image, options.background)
        })
        .collect();

    let (palette, indexed) = build_palette(&rgb_frames, options, &mut notes)?;

    let width = u16::try_from(width)?;
    let height = u16::try_from(height)?;
    let frame_count = u32::try_from(frames.len())?;
    let palette_size = u16::try_from(palette.len())?;
    let header = Header::new(width, height, frame_count, palette_size)?;

    let mut bytes = vec![0u8; encoded_size(&header)?];
    let sources = frames
        .iter()
        .zip(&indexed)
        .map(|(frame, indices)| FrameSource {
            duration_ms: frame.duration_ms,
            indices,
        });
    write(&header, &palette, sources, &mut bytes)?;

    Ok(Report {
        bytes,
        width,
        height,
        frame_count,
        palette_size,
        bits_per_pixel: header.bits_per_pixel().bits(),
        notes,
    })
}

/// Applies the duration rules of section 10.1 and 17.
fn normalize_durations<'a>(
    animation: &'a Animation,
    options: &Options,
    notes: &mut Vec<String>,
) -> Result<Vec<TimedFrame<'a>>> {
    let mut frames = Vec::with_capacity(animation.frames.len());
    let mut omitted = 0usize;
    let mut clamped = 0usize;

    for frame in &animation.frames {
        let duration_ms = match frame.duration_ms {
            None => {
                let duration = options.default_duration.unwrap_or(STATIC_IMAGE_DURATION_MS);
                notes.push(format!(
                    "static image, using a single {duration} ms frame (set --default-duration to change it)"
                ));
                duration
            }
            Some(0) => match options.default_duration {
                Some(duration) => duration,
                None => {
                    omitted += 1;
                    continue;
                }
            },
            Some(ms) => u16::try_from(ms).unwrap_or_else(|_| {
                clamped += 1;
                u16::MAX
            }),
        };
        frames.push(TimedFrame {
            image: &frame.image,
            duration_ms,
        });
    }

    if omitted > 0 {
        notes.push(format!(
            "{omitted} zero-duration source frame(s) omitted (use --default-duration to keep them)"
        ));
    }
    if clamped > 0 {
        notes.push(format!("{clamped} frame duration(s) clamped to 65535 ms"));
    }
    if frames.is_empty() {
        bail!(
            "no frames left after omitting {omitted} zero-duration frame(s); \
             pass --default-duration <ms> to keep them"
        );
    }
    Ok(frames)
}

fn target_size(animation: &Animation, options: &Options) -> Result<(u32, u32)> {
    match options.size {
        Some((w, h)) => Ok((u32::from(w), u32::from(h))),
        None => {
            if animation.width > u32::from(u16::MAX) || animation.height > u32::from(u16::MAX) {
                bail!(
                    "source is {}x{}, larger than PXA allows; use --size",
                    animation.width,
                    animation.height
                );
            }
            Ok((animation.width, animation.height))
        }
    }
}

/// Alpha-blends every pixel over `background` and drops the alpha channel.
fn composite(image: &RgbaImage, background: Rgb) -> Vec<Rgb> {
    image
        .pixels()
        .map(|pixel| {
            let [r, g, b, a] = pixel.0;
            let alpha = u16::from(a);
            let blend = |src: u8, bg: u8| -> u8 {
                ((u16::from(src) * alpha + u16::from(bg) * (255 - alpha) + 127) / 255) as u8
            };
            [
                blend(r, background[0]),
                blend(g, background[1]),
                blend(b, background[2]),
            ]
        })
        .collect()
}

/// Builds the global palette and maps every frame to palette indices.
///
/// Colors are kept exactly when they fit `max_colors`; otherwise the
/// animation is quantized. Palette order is deterministic: first appearance
/// in logical pixel order, frame by frame.
fn build_palette(
    frames: &[Vec<Rgb>],
    options: &Options,
    notes: &mut Vec<String>,
) -> Result<(Vec<Rgb>, Vec<Vec<u8>>)> {
    let mut palette = ExactPalette::new();
    let exact: Option<Vec<Vec<u8>>> = frames
        .iter()
        .map(|frame| {
            frame
                .iter()
                .map(|&color| palette.index_of(color, options.max_colors))
                .collect::<Option<Vec<u8>>>()
        })
        .collect();

    if let Some(indexed) = exact {
        return Ok((palette.finish(), indexed));
    }

    let unique = count_unique(frames);
    notes.push(format!(
        "source uses {unique} colors, quantizing to {}",
        options.max_colors
    ));

    // NeuQuant trains on RGBA samples of every frame.
    let mut samples = Vec::with_capacity(frames.iter().map(Vec::len).sum::<usize>() * 4);
    for color in frames.iter().flatten() {
        samples.extend_from_slice(color);
        samples.push(255);
    }
    let quantizer = NeuQuant::new(
        NEUQUANT_SAMPLE_FACTOR,
        usize::from(options.max_colors),
        &samples,
    );

    // The trained map may contain duplicates or unused entries, so the final
    // palette is rebuilt from the colors actually used.
    let mut palette = ExactPalette::new();
    let mut indexed = Vec::with_capacity(frames.len());
    for frame in frames {
        let mut indices = Vec::with_capacity(frame.len());
        for &[r, g, b] in frame {
            let slot = quantizer.index_of(&[r, g, b, 255]);
            let [qr, qg, qb, _] = quantizer
                .lookup(slot)
                .context("quantizer returned an invalid color index")?;
            let index = palette
                .index_of([qr, qg, qb], options.max_colors)
                .context("quantizer produced more colors than requested")?;
            indices.push(index);
        }
        indexed.push(indices);
    }
    Ok((palette.finish(), indexed))
}

fn count_unique(frames: &[Vec<Rgb>]) -> usize {
    frames
        .iter()
        .flatten()
        .collect::<std::collections::HashSet<_>>()
        .len()
}

/// Palette built in first-appearance order with a lookup table.
struct ExactPalette {
    colors: Vec<Rgb>,
    lookup: HashMap<Rgb, u8>,
}

impl ExactPalette {
    fn new() -> Self {
        Self {
            colors: Vec::new(),
            lookup: HashMap::new(),
        }
    }

    /// Index of `color`, inserting it when new. `None` once `max` colors exist.
    fn index_of(&mut self, color: Rgb, max: u16) -> Option<u8> {
        if let Some(&index) = self.lookup.get(&color) {
            return Some(index);
        }
        if self.colors.len() >= usize::from(max) {
            return None;
        }
        let index = u8::try_from(self.colors.len()).ok()?;
        self.colors.push(color);
        self.lookup.insert(color, index);
        Some(index)
    }

    fn finish(self) -> Vec<Rgb> {
        self.colors
    }
}
