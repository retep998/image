//! Functions for altering and converting the color of pixelbufs
use std::num:: {
    cast,
    Float,
};

use std::default::Default;

use color::Luma;
use buffer::{ImageBuffer, Pixel};
use traits::Primitive;
use image::GenericImage;
use math::utils::clamp;

/// Convert the supplied image to grayscale
// TODO: is the 'static bound on `I` really required? Can we avoid it?
pub fn grayscale<'a, I: GenericImage + 'static>(image: &I)
    -> ImageBuffer<Luma<<I::Pixel as Pixel>::Subpixel>, Vec<<I::Pixel as Pixel>::Subpixel>>
    where I::Pixel: 'static,
          <I::Pixel as Pixel>::Subpixel: Default + 'static {
    let (width, height) = image.dimensions();
    let mut out = ImageBuffer::new(width, height);

    for y in (0..height) {
        for x in (0..width) {
            let p = image.get_pixel(x, y).to_luma();
            out.put_pixel(x, y, p);
        }
    }

    out
}

/// Invert each pixel within the supplied image
/// This function operates in place.
pub fn invert<I: GenericImage>(image: &mut I) {
    let (width, height) = image.dimensions();

    for y in (0..height) {
        for x in (0..width) {
            let mut p = image.get_pixel(x, y);
            p.invert();

            image.put_pixel(x, y, p);
        }
    }
}

/// Adjust the contrast of the supplied image
/// ```contrast``` is the amount to adjust the contrast by.
/// Negative values decrease the contrast and positive values increase the contrast.
// TODO: Do we really need the 'static bound on `I`? Can we avoid it?
pub fn contrast<I: GenericImage + 'static>(image: &I, contrast: f32)
    -> ImageBuffer<I::Pixel, Vec<<I::Pixel as Pixel>::Subpixel>>
    where I::Pixel: 'static,
          <I::Pixel as Pixel>::Subpixel: 'static {

    let (width, height) = image.dimensions();
    let mut out = ImageBuffer::new(width, height);

    let max: <I::Pixel as Pixel>::Subpixel = Primitive::max_value();
    let max: f32 = cast(max).unwrap();

    let percent = ((100.0 + contrast) / 100.0).powi(2);

    for y in (0..height) {
        for x in (0..width) {
            let f = image.get_pixel(x, y).map(|&: b| {
                let c: f32 = cast(b).unwrap();

                let d = ((c / max - 0.5) * percent  + 0.5) * max;
                let e = clamp(d, 0.0, max);

                cast(e).unwrap()
            });

            out.put_pixel(x, y, f);
        }
    }

    out
}

/// Brighten the supplied image
/// ```value``` is the amount to brighten each pixel by.
/// Negative values decrease the brightness and positive values increase it.
// TODO: Is the 'static bound on `I` really required? Can we avoid it?
pub fn brighten<I: GenericImage + 'static>(image: &I, value: i32)
    -> ImageBuffer<I::Pixel, Vec<<I::Pixel as Pixel>::Subpixel>>
    where I::Pixel: 'static,
          <I::Pixel as Pixel>::Subpixel: 'static {

    let (width, height) = image.dimensions();
    let mut out = ImageBuffer::new(width, height);

    let max: <I::Pixel as Pixel>::Subpixel = Primitive::max_value();
    let max: i32 = cast(max).unwrap();

    for y in (0..height) {
        for x in (0..width) {
            let e = image.get_pixel(x, y).map_with_alpha(|&:b| {
                let c: i32 = cast(b).unwrap();
                let d = clamp(c + value, 0, max);

                cast(d).unwrap()
            }, |&:alpha| alpha);

            out.put_pixel(x, y, e);
        }
    }

    out
}
