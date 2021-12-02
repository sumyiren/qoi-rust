use std::slice;

use crate::colorspace::ColorSpace;
use crate::consts::{
    QOI_COLOR, QOI_DIFF_16, QOI_DIFF_24, QOI_DIFF_8, QOI_HEADER_SIZE, QOI_INDEX, QOI_PADDING,
    QOI_RUN_16, QOI_RUN_8,
};
use crate::error::{Error, Result};
use crate::header::Header;
use crate::pixel::{Pixel, SupportedChannels};
use crate::utils::unlikely;

struct WriteBuf {
    start: *const u8,
    current: *mut u8,
}

impl WriteBuf {
    pub const unsafe fn new(ptr: *mut u8) -> Self {
        Self { start: ptr, current: ptr }
    }

    #[inline]
    pub fn write<const N: usize>(&mut self, v: [u8; N]) {
        unsafe {
            let mut i = 0;
            while i < N {
                self.current.add(i).write(v[i]);
                i += 1;
            }
            self.current = self.current.add(N);
        }
    }

    #[inline]
    pub fn push(&mut self, v: u8) {
        unsafe {
            self.current.write(v);
            self.current = self.current.add(1);
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        unsafe { self.current.offset_from(self.start).max(0) as usize }
    }
}

#[inline]
fn encode_diff_canonical<const N: usize>(
    px: Pixel<N>, px_prev: Pixel<N>, buf: &mut WriteBuf,
) -> Option<(bool, bool, bool, bool)> {
    let vr = i16::from(px.r()) - i16::from(px_prev.r());
    let vg = i16::from(px.g()) - i16::from(px_prev.g());
    let vb = i16::from(px.b()) - i16::from(px_prev.b());
    let va = i16::from(px.a_or(0)) - i16::from(px_prev.a_or(0));

    let (vr_16, vg_16, vb_16, va_16) = (vr + 16, vg + 16, vb + 16, va + 16);
    if vr_16 | vg_16 | vb_16 | va_16 | 31 == 31 {
        loop {
            if va == 0 {
                let (vr_2, vg_2, vb_2) = (vr + 2, vg + 2, vb + 2);
                if vr_2 | vg_2 | vb_2 | 3 == 3 {
                    buf.write([QOI_DIFF_8 | (vr_2 << 4 | vg_2 << 2 | vb_2) as u8]);
                    break;
                }
                let (vg_8, vb_8) = (vg + 8, vb + 8);
                if vg_8 | vb_8 | 15 == 15 {
                    buf.write([QOI_DIFF_16 | vr_16 as u8, (vg_8 << 4 | vb_8) as u8]);
                    break;
                }
            }
            buf.write([
                QOI_DIFF_24 | (vr_16 >> 1) as u8,
                (vr_16 << 7 | vg_16 << 2 | vb_16 >> 3) as u8,
                (vb_16 << 5 | va_16) as u8,
            ]);
            break;
        }
        None
    } else {
        Some((vr != 0, vg != 0, vb != 0, va != 0))
    }
}

#[inline]
fn encode_diff_wrapping<const N: usize>(
    px: Pixel<N>, px_prev: Pixel<N>, buf: &mut WriteBuf,
) -> Option<(bool, bool, bool, bool)> {
    let vr = px.r().wrapping_sub(px_prev.r());
    let vg = px.g().wrapping_sub(px_prev.g());
    let vb = px.b().wrapping_sub(px_prev.b());
    let va = px.a_or(0).wrapping_sub(px_prev.a_or(0));

    let (vr_16, vg_16, vb_16, va_16) =
        (vr.wrapping_add(16), vg.wrapping_add(16), vb.wrapping_add(16), va.wrapping_add(16));

    if vr_16 | vg_16 | vb_16 | va_16 | 31 == 31 {
        loop {
            if va == 0 {
                let (vr_2, vg_2, vb_2) =
                    (vr.wrapping_add(2), vg.wrapping_add(2), vb.wrapping_add(2));
                if vr_2 | vg_2 | vb_2 | 3 == 3 {
                    buf.write([QOI_DIFF_8 | vr_2 << 4 | vg_2 << 2 | vb_2]);
                    break;
                }
                let (vg_8, vb_8) = (vg.wrapping_add(8), vb.wrapping_add(8));
                if vg_8 | vb_8 | 15 == 15 {
                    buf.write([QOI_DIFF_16 | vr_16, vg_8 << 4 | vb_8]);
                    break;
                }
            }
            buf.write([
                QOI_DIFF_24 | vr_16 >> 1,
                vr_16 << 7 | vg_16 << 2 | vb_16 >> 3,
                vb_16 << 5 | va_16,
            ]);
            break;
        }
        None
    } else {
        Some((vr != 0, vg != 0, vb != 0, va != 0))
    }
}

fn qoi_encode_impl<const CHANNELS: usize, const CANONICAL: bool>(
    out: &mut [u8], data: &[u8], width: u32, height: u32, colorspace: ColorSpace,
) -> Result<usize>
where
    Pixel<CHANNELS>: SupportedChannels,
{
    let max_len = encode_size_required(width, height, CHANNELS as u8);
    if unlikely(out.len() < max_len) {
        return Err(Error::OutputBufferTooSmall { size: out.len(), required: max_len });
    }

    let n_pixels = (width as usize) * (height as usize);
    if unlikely(data.is_empty()) {
        return Err(Error::EmptyImage { width, height });
    } else if unlikely(n_pixels * CHANNELS != data.len()) {
        return Err(Error::BadEncodingDataSize { size: data.len(), expected: n_pixels * CHANNELS });
    }

    let pixels = unsafe {
        // Safety: we've verified that n_pixels * N == data.len()
        slice::from_raw_parts::<Pixel<CHANNELS>>(data.as_ptr().cast(), n_pixels)
    };

    let mut buf = unsafe {
        // Safety: all write ops are guaranteed to not go outside allocation
        WriteBuf::new(out.as_mut_ptr())
    };

    let header =
        Header { width, height, channels: CHANNELS as u8, colorspace, ..Header::default() };
    buf.write(header.to_bytes());

    let mut index = [Pixel::new(); 64];
    let mut px_prev = Pixel::new().with_a(0xff);
    let mut run = 0_u16;

    let next_run = |buf: &mut WriteBuf, run: &mut u16| {
        let mut r = *run;
        if r < 33 {
            r -= 1;
            buf.push(QOI_RUN_8 | (r as u8));
        } else {
            r -= 33;
            buf.write([QOI_RUN_16 | ((r >> 8) as u8), (r & 0xff) as u8]);
        }
        *run = 0;
    };

    for (i, &px) in pixels.iter().enumerate() {
        if px == px_prev {
            run += 1;
            if run == 0x2020 || i == n_pixels - 1 {
                next_run(&mut buf, &mut run);
            }
        } else {
            if run != 0 {
                next_run(&mut buf, &mut run);
            }
            let index_pos = px.hash_index();
            let index_px = unsafe {
                // Safety: hash_index() is computed mod 64, so it will never go out of bounds
                index.get_unchecked_mut(usize::from(index_pos))
            };
            if *index_px == px {
                buf.push(QOI_INDEX | index_pos);
            } else {
                *index_px = px;

                let nonzero = if CANONICAL {
                    encode_diff_canonical::<CHANNELS>(px, px_prev, &mut buf)
                } else {
                    encode_diff_wrapping::<CHANNELS>(px, px_prev, &mut buf)
                };

                if let Some((r, g, b, a)) = nonzero {
                    let c = ((r as u8) << 3) | ((g as u8) << 2) | ((b as u8) << 1) | (a as u8);
                    buf.push(QOI_COLOR | c);
                    if r {
                        buf.push(px.r());
                    }
                    if g {
                        buf.push(px.g());
                    }
                    if b {
                        buf.push(px.b());
                    }
                    if a {
                        buf.push(px.a_or(0));
                    }
                }
            }
            px_prev = px;
        }
    }

    buf.write([0; QOI_PADDING]);
    Ok(buf.len())
}

#[inline]
pub fn encode_to_buf_impl<const CANONICAL: bool>(
    out: &mut [u8], data: &[u8], width: u32, height: u32, channels: u8, colorspace: ColorSpace,
) -> Result<usize> {
    match channels {
        3 => qoi_encode_impl::<3, CANONICAL>(out, data, width, height, colorspace),
        4 => qoi_encode_impl::<4, CANONICAL>(out, data, width, height, colorspace),
        _ => Err(Error::InvalidChannels { channels }),
    }
}

#[inline]
pub fn encode_to_vec_impl<const CANONICAL: bool>(
    data: &[u8], width: u32, height: u32, channels: u8, colorspace: ColorSpace,
) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(encode_size_required(width, height, channels));
    unsafe {
        out.set_len(out.capacity());
    }
    let size =
        encode_to_buf_impl::<CANONICAL>(&mut out, data, width, height, channels, colorspace)?;
    out.truncate(size);
    Ok(out)
}

#[inline]
pub fn encode_size_required(width: u32, height: u32, channels: u8) -> usize {
    let (width, height) = (width as usize, height as usize);
    let n_pixels = width.saturating_mul(height);
    QOI_HEADER_SIZE + n_pixels.saturating_mul(usize::from(channels)) + n_pixels + QOI_PADDING
}

#[inline]
pub fn qoi_encode_to_vec(
    data: impl AsRef<[u8]>, width: u32, height: u32, channels: u8,
    colorspace: impl Into<ColorSpace>,
) -> Result<Vec<u8>> {
    encode_to_vec_impl::<false>(data.as_ref(), width, height, channels, colorspace.into())
}

#[inline]
pub fn qoi_encode_to_buf(
    mut out: impl AsMut<[u8]>, data: impl AsRef<[u8]>, width: u32, height: u32, channels: u8,
    colorspace: impl Into<ColorSpace>,
) -> Result<usize> {
    encode_to_buf_impl::<false>(
        out.as_mut(),
        data.as_ref(),
        width,
        height,
        channels,
        colorspace.into(),
    )
}
