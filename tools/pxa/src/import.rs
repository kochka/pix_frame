//! Source decoders. Every supported format converges on [`Animation`].

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use anyhow::{Context, Result, bail};
use image::codecs::gif::GifDecoder;
use image::codecs::png::PngDecoder;
use image::codecs::webp::WebPDecoder;
use image::{AnimationDecoder, DynamicImage, ImageFormat, ImageReader};

use crate::animation::{Animation, SourceFrame};

/// Loads any supported source file.
pub fn load(path: &Path) -> Result<Animation> {
    let format = ImageReader::open(path)?
        .with_guessed_format()?
        .format()
        .context("unrecognized image format")?;
    let reader = BufReader::new(File::open(path)?);

    match format {
        ImageFormat::Gif => from_animation(GifDecoder::new(reader)?),
        ImageFormat::Png => {
            let decoder = PngDecoder::new(reader)?;
            if decoder.is_apng()? {
                from_animation(decoder.apng()?)
            } else {
                from_static(DynamicImage::from_decoder(decoder)?)
            }
        }
        ImageFormat::WebP => {
            let decoder = WebPDecoder::new(reader)?;
            if decoder.has_animation() {
                from_animation(decoder)
            } else {
                from_static(DynamicImage::from_decoder(decoder)?)
            }
        }
        other => bail!("unsupported source format {other:?}; use GIF, PNG, APNG or WebP"),
    }
}

fn from_animation<'a, D: AnimationDecoder<'a>>(decoder: D) -> Result<Animation> {
    let mut frames = Vec::new();
    let mut size = None;

    for (index, frame) in decoder.into_frames().enumerate() {
        let frame = frame.with_context(|| format!("cannot decode source frame {index}"))?;
        let (numer, denom) = frame.delay().numer_denom_ms();
        let duration_ms = if denom == 0 {
            0
        } else {
            // Round to the nearest millisecond.
            u32::try_from((u64::from(numer) * 2 + u64::from(denom)) / (2 * u64::from(denom)))
                .unwrap_or(u32::MAX)
        };
        if frame.left() != 0 || frame.top() != 0 {
            bail!(
                "source frame {index} is not a complete image (offset {}, {})",
                frame.left(),
                frame.top()
            );
        }

        let image = frame.into_buffer();
        let dimensions = image.dimensions();
        match size {
            None => size = Some(dimensions),
            Some(expected) if expected != dimensions => bail!(
                "source frame {index} is {}x{}, expected {}x{}",
                dimensions.0,
                dimensions.1,
                expected.0,
                expected.1
            ),
            Some(_) => {}
        }

        frames.push(SourceFrame {
            image,
            duration_ms: Some(duration_ms),
        });
    }

    let (width, height) = size.context("source contains no frames")?;
    Ok(Animation {
        width,
        height,
        frames,
    })
}

fn from_static(image: DynamicImage) -> Result<Animation> {
    let image = image.into_rgba8();
    let (width, height) = image.dimensions();
    Ok(Animation {
        width,
        height,
        frames: vec![SourceFrame {
            image,
            duration_ms: None,
        }],
    })
}
