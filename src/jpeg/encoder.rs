use std::old_io::MemWriter;
use std::old_io;
use std::old_io::IoResult;
use std::iter::range_step;
use std::num::{ Float, SignedInt };

use color;

use super::transform;
use super::decoder::Component;
use super::decoder::UNZIGZAG;
use super::entropy::build_huff_lut;

// Markers
// Baseline DCT
static SOF0: u8 = 0xC0;
// Huffman Tables
static DHT: u8 = 0xC4;
// Start of Image (standalone)
static SOI: u8 = 0xD8;
// End of image (standalone)
static EOI: u8 = 0xD9;
// Start of Scan
static SOS: u8 = 0xDA;
// Quantization Tables
static DQT: u8 = 0xDB;
// Application segments start and end
static APP0: u8 = 0xE0;

// section K.1
// table K.1
static STD_LUMA_QTABLE: [u8; 64] = [
    16, 11, 10, 16, 124, 140, 151, 161,
    12, 12, 14, 19, 126, 158, 160, 155,
    14, 13, 16, 24, 140, 157, 169, 156,
    14, 17, 22, 29, 151, 187, 180, 162,
    18, 22, 37, 56, 168, 109, 103, 177,
    24, 35, 55, 64, 181, 104, 113, 192,
    49, 64, 78, 87, 103, 121, 120, 101,
    72, 92, 95, 98, 112, 100, 103, 199
 ];

// table K.2
static STD_CHROMA_QTABLE: [u8; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99,
    18, 21, 26, 66, 99, 99, 99, 99,
    24, 26, 56, 99, 99, 99, 99, 99,
    47, 66, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99
 ];

// section K.3
// Code lengths and values for table K.3
static STD_LUMA_DC_CODE_LENGTHS: [u8; 16] = [
    0x00, 0x01, 0x05, 0x01, 0x01, 0x01, 0x01, 0x01,
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
];

static STD_LUMA_DC_VALUES: [u8; 12] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
    0x08, 0x09, 0x0A, 0x0B
];

// Code lengths and values for table K.4
static STD_CHROMA_DC_CODE_LENGTHS: [u8; 16] = [
    0x00, 0x03, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
    0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00
];

static STD_CHROMA_DC_VALUES: [u8; 12] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
    0x08, 0x09, 0x0A, 0x0B
];

// Code lengths and values for table k.5
static STD_LUMA_AC_CODE_LENGTHS: [u8; 16] = [
    0x00, 0x02, 0x01, 0x03, 0x03, 0x02, 0x04, 0x03,
    0x05, 0x05, 0x04, 0x04, 0x00, 0x00, 0x01, 0x7D
];

static STD_LUMA_AC_VALUES: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
    0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xA1, 0x08, 0x23, 0x42, 0xB1, 0xC1, 0x15, 0x52, 0xD1, 0xF0,
    0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0A, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2A, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7,
    0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5,
    0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xE1, 0xE2,
    0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8,
    0xF9, 0xFA,
];

// Code lengths and values for table k.6
static STD_CHROMA_AC_CODE_LENGTHS: [u8; 16] = [
    0x00, 0x02, 0x01, 0x02, 0x04, 0x04, 0x03, 0x04,
    0x07, 0x05, 0x04, 0x04, 0x00, 0x01, 0x02, 0x77,
];
static STD_CHROMA_AC_VALUES: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
    0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xA1, 0xB1, 0xC1, 0x09, 0x23, 0x33, 0x52, 0xF0,
    0x15, 0x62, 0x72, 0xD1, 0x0A, 0x16, 0x24, 0x34, 0xE1, 0x25, 0xF1, 0x17, 0x18, 0x19, 0x1A, 0x26,
    0x27, 0x28, 0x29, 0x2A, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
    0x49, 0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
    0x69, 0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5,
    0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3,
    0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA,
    0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8,
    0xF9, 0xFA,
];

static DCCLASS: u8 = 0;
static ACCLASS: u8 = 1;

static LUMADESTINATION: u8 = 0;
static CHROMADESTINATION: u8 = 1;

static LUMAID: u8 = 1;
static CHROMABLUEID: u8 = 2;
static CHROMAREDID: u8 = 3;

/// The representation of a JPEG encoder
pub struct JPEGEncoder<'a, W: 'a> {
    w: &'a mut W,

    components: Vec<Component>,
    tables: Vec<u8>,

    accumulator: u32,
    nbits: u8,

    luma_dctable: Vec<(u8, u16)>,
    luma_actable: Vec<(u8, u16)>,
    chroma_dctable: Vec<(u8, u16)>,
    chroma_actable: Vec<(u8, u16)>,
}

impl<'a, W: Writer> JPEGEncoder<'a, W> {
    /// Create a new encoder that writes its output to ```w```
    pub fn new(w: &mut W) -> JPEGEncoder<W> {
        let ld = build_huff_lut(&STD_LUMA_DC_CODE_LENGTHS, &STD_LUMA_DC_VALUES);
        let la = build_huff_lut(&STD_LUMA_AC_CODE_LENGTHS, &STD_LUMA_AC_VALUES);

        let cd = build_huff_lut(&STD_CHROMA_DC_CODE_LENGTHS, &STD_CHROMA_DC_VALUES);
        let ca = build_huff_lut(&STD_CHROMA_AC_CODE_LENGTHS, &STD_CHROMA_AC_VALUES);

        let components = vec![
            Component {id: LUMAID, h: 1, v: 1, tq: LUMADESTINATION, dc_table: LUMADESTINATION, ac_table: LUMADESTINATION, dc_pred: 0},
            Component {id: CHROMABLUEID, h: 1, v: 1, tq: CHROMADESTINATION, dc_table: CHROMADESTINATION, ac_table: CHROMADESTINATION, dc_pred: 0},
            Component {id: CHROMAREDID, h: 1, v: 1, tq: CHROMADESTINATION, dc_table: CHROMADESTINATION, ac_table: CHROMADESTINATION, dc_pred: 0}
        ];

        let mut tables = Vec::new();
        tables.extend(STD_LUMA_QTABLE.iter().map(|&v| v));
        tables.extend(STD_CHROMA_QTABLE.iter().map(|&v| v));

        JPEGEncoder {
            w: w,

            components: components,
            tables: tables,

            luma_dctable: ld,
            luma_actable: la,
            chroma_dctable: cd,
            chroma_actable: ca,

            accumulator: 0,
            nbits: 0,
        }
    }

    /// Encodes the image ```image```
    /// that has dimensions ```width``` and ```height```
    /// and ```ColorType``` ```c```
    /// The Image in encoded with subsampling ratio 4:2:2
    pub fn encode(&mut self,
                  image: &[u8],
                  width: u32,
                  height: u32,
                  c: color::ColorType) -> IoResult<()> {

        let n = color::num_components(c);
        let num_components = if n == 1 || n == 2 {1}
                             else {3};

        let _ = try!(self.write_segment(SOI, None));

        let buf = build_jfif_header();
        let _   = try!(self.write_segment(APP0, Some(buf)));

        let buf = build_frame_header(8, width as u16, height as u16, &self.components[..num_components]);
        let _   = try!(self.write_segment(SOF0, Some(buf)));

        assert!(self.tables.len() / 64 == 2);
        let numtables = if num_components == 1 {1}
                        else {2};

        let t = self.tables.clone();

        for (i, table) in t[].chunks(64).enumerate().take(numtables) {
            let buf = build_quantization_segment(8, i as u8, table);
            let _   = try!(self.write_segment(DQT, Some(buf)));
        }

        let numcodes = STD_LUMA_DC_CODE_LENGTHS;
        let values   = STD_LUMA_DC_VALUES;

        let buf = build_huffman_segment(DCCLASS, LUMADESTINATION, &numcodes, &values);
        let _   = try!(self.write_segment(DHT, Some(buf)));

        let numcodes = STD_LUMA_AC_CODE_LENGTHS;
        let values   = STD_LUMA_AC_VALUES;

        let buf = build_huffman_segment(ACCLASS, LUMADESTINATION, &numcodes, &values);
        let _   = try!(self.write_segment(DHT, Some(buf)));

        if num_components == 3 {
            let numcodes = STD_CHROMA_DC_CODE_LENGTHS;
            let values   = STD_CHROMA_DC_VALUES;

            let buf = build_huffman_segment(DCCLASS, CHROMADESTINATION, &numcodes, &values);
            let _   = try!(self.write_segment(DHT, Some(buf)));

            let numcodes = STD_CHROMA_AC_CODE_LENGTHS;
            let values   = STD_CHROMA_AC_VALUES;

            let buf = build_huffman_segment(ACCLASS, CHROMADESTINATION, &numcodes, &values);
            let _   = try!(self.write_segment(DHT, Some(buf)));
        }

        let buf = build_scan_header(&self.components[..num_components]);
        let _   = try!(self.write_segment(SOS, Some(buf)));

        match c {
            color::ColorType::RGB(8)   => try!(self.encode_rgb(image, width as usize, height as usize, 3)),
            color::ColorType::RGBA(8)  => try!(self.encode_rgb(image, width as usize, height as usize, 4)),
            color::ColorType::Gray(8)  => try!(self.encode_gray(image, width as usize, height as usize, 1)),
            color::ColorType::GrayA(8) => try!(self.encode_gray(image, width as usize, height as usize, 2)),
            _  => return Err(old_io::IoError {
                kind: old_io::InvalidInput,
                desc: "Unsupported color type. Use 8 bit per channel RGB(A) or Gray(A) instead.",
                detail: Some(format!(
                    "Color type {:?} is not suppored by this JPEG encoder.",
                    c
                ))
            })
        };

        let _ = try!(self.pad_byte());
        self.write_segment(EOI, None)
    }

    fn write_segment(&mut self, marker: u8, data: Option<Vec<u8>>) -> IoResult<()> {
        let _ = try!(self.w.write_u8(0xFF));
        let _ = try!(self.w.write_u8(marker));

        if data.is_some() {
            let b = data.unwrap();
            let _ = try!(self.w.write_be_u16(b.len() as u16 + 2));
            let _ = try!(self.w.write_all(&b[]));
        }

        Ok(())
    }

    fn write_bits(&mut self, bits: u16, size: u8) -> IoResult<()> {
        self.accumulator |= (bits as u32) << (32 - (self.nbits + size)) as usize;
        self.nbits += size;

        while self.nbits >= 8 {
            let byte = (self.accumulator & (0xFFFFFFFFu32 << 24)) >> 24;
            let _ = try!(self.w.write_u8(byte as u8));

            if byte == 0xFF {
                let _ = try!(self.w.write_u8(0x00));
            }

            self.nbits -= 8;
            self.accumulator <<= 8;
        }

        Ok(())
    }

    fn pad_byte(&mut self) -> IoResult<()> {
        self.write_bits(0x7F, 7)
    }

    fn huffman_encode(&mut self, val: u8, table: &[(u8, u16)]) -> IoResult<()> {
        let (size, code) = table[val as usize];

        if size > 16 {
            panic!("bad huffman value");
        }

        self.write_bits(code, size)
    }

    fn write_block(
        &mut self,
        block: &[i32],
        prevdc: i32,
        dctable: &[(u8, u16)],
        actable: &[(u8, u16)]) -> IoResult<i32> {

        // Differential DC encoding
        let dcval = block[0];
        let diff  = dcval - prevdc;
        let (size, value) = encode_coefficient(diff);

        let _ = try!(self.huffman_encode(size, dctable));
        let _ = try!(self.write_bits(value, size));

        // Figure F.2
        let mut zero_run = 0;
        let mut k = 0us;

        loop {
            k += 1;

            if block[UNZIGZAG[k] as usize] == 0 {
                if k == 63 {

                let _ = try!(self.huffman_encode(0x00, actable));
                    break
                }

                zero_run += 1;
            } else {
                while zero_run > 15 {
                    let _ = try!(self.huffman_encode(0xF0, actable));
                    zero_run -= 16;
                }

                let (size, value) = encode_coefficient(block[UNZIGZAG[k] as usize]);
                let symbol = (zero_run << 4) | size;

                let _ = try!(self.huffman_encode(symbol, actable));
                let _ = try!(self.write_bits(value, size));

                zero_run = 0;

                if k == 63 {
                    break
                }
            }
        }

        Ok(dcval)
    }

    fn encode_gray(&mut self, image: &[u8], width: usize, height: usize, bpp: usize) -> IoResult<()> {
        let mut yblock     = [0u8; 64];
        let mut y_dcprev   = 0;
        let mut dct_yblock = [0i32; 64];

        for y in range_step(0, height, 8) {
            for x in range_step(0, width, 8) {
                // RGB -> YCbCr
                copy_blocks_gray(image, x, y, width, bpp, &mut yblock);

                // Level shift and fdct
                // Coeffs are scaled by 8
                transform::fdct(&yblock[], &mut dct_yblock);

                // Quantization
                for i in (0us..64) {
                    dct_yblock[i]   = ((dct_yblock[i] / 8)   as f32 / self.tables[i] as f32).round() as i32;
                }

                let la = self.luma_actable.clone();
                let ld = self.luma_dctable.clone();

                y_dcprev  = try!(self.write_block(&dct_yblock, y_dcprev, &ld[], &la[]));
            }
        }

        Ok(())
    }

    fn encode_rgb(&mut self, image: &[u8], width: usize, height: usize, bpp: usize) -> IoResult<()> {
        let mut y_dcprev = 0;
        let mut cb_dcprev = 0;
        let mut cr_dcprev = 0;

        let mut dct_yblock   = [0i32; 64];
        let mut dct_cb_block = [0i32; 64];
        let mut dct_cr_block = [0i32; 64];

        let mut yblock   = [0u8; 64];
        let mut cb_block = [0u8; 64];
        let mut cr_block = [0u8; 64];

        for y in range_step(0, height, 8) {
            for x in range_step(0, width, 8) {
                // RGB -> YCbCr
                copy_blocks_ycbcr(image, x, y, width, bpp, &mut yblock, &mut cb_block, &mut cr_block);

                // Level shift and fdct
                // Coeffs are scaled by 8
                transform::fdct(&yblock[], &mut dct_yblock);
                transform::fdct(&cb_block[], &mut dct_cb_block);
                transform::fdct(&cr_block[], &mut dct_cr_block);

                // Quantization
                for i in (0us..64) {
                    dct_yblock[i]   = ((dct_yblock[i] / 8)   as f32 / self.tables[i] as f32).round() as i32;
                    dct_cb_block[i] = ((dct_cb_block[i] / 8) as f32 / self.tables[64..][i] as f32).round() as i32;
                    dct_cr_block[i] = ((dct_cr_block[i] / 8) as f32 / self.tables[64..][i] as f32).round() as i32;
                }

                let la = self.luma_actable.clone();
                let ld = self.luma_dctable.clone();
                let cd = self.chroma_dctable.clone();
                let ca = self.chroma_actable.clone();

                y_dcprev  = try!(self.write_block(&dct_yblock, y_dcprev, &ld[], &la[]));
                cb_dcprev = try!(self.write_block(&dct_cb_block, cb_dcprev, &cd[], &ca[]));
                cr_dcprev = try!(self.write_block(&dct_cr_block, cr_dcprev, &cd[], &ca[]));
            }
        }

        Ok(())
    }
}

fn build_jfif_header() -> Vec<u8> {
    let mut m = MemWriter::new();

    let _ = m.write_str("JFIF");
    let _ = m.write_u8(0);
    let _ = m.write_u8(0x01);
    let _ = m.write_u8(0x02);
    let _ = m.write_u8(0);
    let _ = m.write_be_u16(1);
    let _ = m.write_be_u16(1);
    let _ = m.write_u8(0);
    let _ = m.write_u8(0);

    m.into_inner()
}

fn build_frame_header(precision: u8,
                      width: u16,
                      height: u16,
                      components: &[Component]) -> Vec<u8> {

    let mut m = MemWriter::new();

    let _ = m.write_u8(precision);
    let _ = m.write_be_u16(height);
    let _ = m.write_be_u16(width);
    let _ = m.write_u8(components.len() as u8);

    for & comp in components.iter() {
        let _  = m.write_u8(comp.id);
        let hv = (comp.h << 4) | comp.v;
        let _  = m.write_u8(hv);
        let _  = m.write_u8(comp.tq);
    }

    m.into_inner()
}

fn build_scan_header(components: &[Component]) -> Vec<u8> {
    let mut m = MemWriter::new();

    let _ = m.write_u8(components.len() as u8);

    for & comp in components.iter() {
        let _ 	   = m.write_u8(comp.id);
        let tables = (comp.dc_table << 4) | comp.ac_table;
        let _ 	   = m.write_u8(tables);
    }

    // spectral start and end, approx. high and low
    let _ = m.write_u8(0);
    let _ = m.write_u8(63);
    let _ = m.write_u8(0);

    m.into_inner()
}

fn build_huffman_segment(class: u8,
                         destination: u8,
                         numcodes: &[u8],
                         values: &[u8]) -> Vec<u8> {
    let mut m = MemWriter::new();

    let tcth = (class << 4) | destination;
    let _    = m.write_u8(tcth);

    assert!(numcodes.len() == 16);

    let mut sum = 0us;

    for & i in numcodes.iter() {
        let _ = m.write_u8(i);
        sum += i as usize;
    }

    assert!(sum == values.len());

    for & i in values.iter() {
        let _ = m.write_u8(i);
    }

    m.into_inner()
}

fn build_quantization_segment(precision: u8,
                              identifier: u8,
                              qtable: &[u8]) -> Vec<u8> {

    assert!(qtable.len() % 64 == 0);
    let mut m = MemWriter::new();

    let p = if precision == 8 {0}
            else {1};

    let pqtq = (p << 4) | identifier;
    let _    = m.write_u8(pqtq);

    for i in (0us..64) {
        let _ = m.write_u8(qtable[UNZIGZAG[i] as usize]);
    }

    m.into_inner()
}

fn encode_coefficient(coefficient: i32) -> (u8, u16) {
    let mut magnitude = coefficient.abs() as u16;
    let mut num_bits  = 0u8;

    while magnitude > 0 {
        magnitude >>= 1;
        num_bits += 1;
    }

    let mask = (1 << num_bits as usize) - 1;

    let val  = if coefficient < 0 {
        (coefficient - 1) as u16 & mask
    } else {
        coefficient as u16 & mask
    };

    (num_bits, val)
}

fn rgb_to_ycbcr(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let r = r as f32;
    let g = g as f32;
    let b = b as f32;

    let y  =  0.299f32  * r + 0.587f32  * g + 0.114f32  * b;
    let cb = -0.1687f32 * r - 0.3313f32 * g + 0.5f32    * b + 128f32;
    let cr =  0.5f32    * r - 0.4187f32 * g - 0.0813f32 * b + 128f32;

    (y as u8, cb as u8, cr as u8)
}

fn value_at(s: &[u8], index: usize) -> u8 {
    if index < s.len() {
        s[index]
    } else {
        s[s.len() - 1]
    }
}

fn copy_blocks_ycbcr(source: &[u8],
                     x0: usize,
                     y0: usize,
                     width: usize,
                     bpp: usize,
                     yb: &mut [u8; 64],
                     cbb: &mut [u8; 64],
                     crb: &mut [u8; 64]) {

    for y in (0us..8) {
        let ystride = (y0 + y) * bpp * width;

        for x in (0us..8) {
            let xstride = x0 * bpp + x * bpp;

            let r = value_at(source, ystride + xstride + 0);
            let g = value_at(source, ystride + xstride + 1);
            let b = value_at(source, ystride + xstride + 2);

            let (yc, cb, cr) = rgb_to_ycbcr(r, g, b);

            yb[y * 8 + x]  = yc;
            cbb[y * 8 + x] = cb;
            crb[y * 8 + x] = cr;
        }
    }
}

fn copy_blocks_gray(source: &[u8],
                    x0: usize,
                    y0: usize,
                    width: usize,
                    bpp: usize,
                    gb: &mut [u8; 64]) {

    for y in (0us..8) {
        let ystride = (y0 + y) * bpp * width;

        for x in (0us..8) {
            let xstride = x0 * bpp + x * bpp;
            gb[y * 8 + x] = value_at(source, ystride + xstride + 1);
        }
    }
}
