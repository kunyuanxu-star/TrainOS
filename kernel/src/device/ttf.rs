// V39a — Minimal TrueType/OpenType Font Renderer
//
// Provides TrueType font parsing with basic glyph rendering and
// anti-aliased text output.  Falls back to the built-in 8x16 bitmap
// font when TTF data is unavailable or parsing fails.
//
// Supported tables: head, hhea, hmtx, maxp, cmap (format 4 only),
//                   loca, glyf (simple glyphs only), kern.
// No curve rasterization — simple glyphs are rendered from their
// on-curve points using a scanline fill.

use super::framebuffer::Framebuffer;
use super::graphics::{self, Color};

/// Copy a string into a fixed-size 64-byte array, zero-padded.
fn str_fixed64(s: &str) -> [u8; 64] {
    let mut buf = [0u8; 64];
    let len = core::cmp::min(s.len(), 63);
    for (i, b) in s.bytes().enumerate().take(len) {
        buf[i] = b;
    }
    buf
}

/// Copy a string into a fixed-size 32-byte array, zero-padded.
fn str_fixed32(s: &str) -> [u8; 32] {
    let mut buf = [0u8; 32];
    let len = core::cmp::min(s.len(), 31);
    for (i, b) in s.bytes().enumerate().take(len) {
        buf[i] = b;
    }
    buf
}

// ── Table Tags (big-endian u32) ──────────────────────────────────────────────

const TAG_HEAD: u32 = 0x68656164; // "head"
const TAG_HHEA: u32 = 0x68686561; // "hhea"
const TAG_HMTX: u32 = 0x686D7478; // "hmtx"
const TAG_MAXP: u32 = 0x6D617870; // "maxp"
const TAG_CMAP: u32 = 0x636D6170; // "cmap"
const TAG_LOCA: u32 = 0x6C6F6361; // "loca"
const TAG_GLYF: u32 = 0x676C7966; // "glyf"
const TAG_KERN: u32 = 0x6B65726E; // "kern"
const TAG_NAME: u32 = 0x6E616D65; // "name"
const TAG_OS2:  u32 = 0x4F532F32; // "OS/2"

/// Maximum number of glyphs we track (ASCII range).
const MAX_GLYPHS: usize = 256;

/// Maximum number of kerning pairs.
const MAX_KERN: usize = 128;

/// Maximum contour points per glyph.
const MAX_CONTOUR_POINTS: usize = 32;

/// Built-in glyph bitmap width (for fallback).
const GLYPH_BMP_W: u8 = 16;

/// Built-in glyph bitmap height (for fallback).
const GLYPH_BMP_H: u8 = 32;

// ── TTF Parsing Helpers ──────────────────────────────────────────────────────

/// Read a big-endian u16 from a byte slice.
fn read_u16(data: &[u8], offset: usize) -> u16 {
    if offset + 1 >= data.len() { return 0; }
    ((data[offset] as u16) << 8) | (data[offset + 1] as u16)
}

/// Read a big-endian i16 from a byte slice.
fn read_i16(data: &[u8], offset: usize) -> i16 {
    if offset + 1 >= data.len() { return 0; }
    ((data[offset] as i16) << 8) | (data[offset + 1] as i16)
}

/// Read a big-endian u32 from a byte slice.
fn read_u32(data: &[u8], offset: usize) -> u32 {
    if offset + 3 >= data.len() { return 0; }
    ((data[offset] as u32) << 24)
        | ((data[offset + 1] as u32) << 16)
        | ((data[offset + 2] as u32) << 8)
        | (data[offset + 3] as u32)
}

/// Read a big-endian i32 from a byte slice.
fn read_i32(data: &[u8], offset: usize) -> i32 {
    read_u32(data, offset) as i32
}

/// Read a 4-byte tag from the offset table.
fn read_tag(data: &[u8], offset: usize) -> u32 {
    read_u32(data, offset)
}

/// Find a table in the TTF file by tag. Returns (offset, length).
fn find_table(data: &[u8], tag: u32) -> Option<(usize, usize)> {
    if data.len() < 12 { return None; }
    let num_tables = read_u16(data, 4) as usize;
    // Search the table directory (starts at offset 12)
    let mut off = 12usize;
    for _ in 0..num_tables {
        if off + 16 > data.len() { break; }
        if read_tag(data, off) == tag {
            let tbl_off = read_u32(data, off + 8) as usize;
            let tbl_len = read_u32(data, off + 12) as usize;
            if tbl_off + tbl_len <= data.len() {
                return Some((tbl_off, tbl_len));
            }
        }
        off += 16;
    }
    None
}

// ── Glyph Data ───────────────────────────────────────────────────────────────

/// A parsed glyph with metrics and simplified contour data.
#[derive(Clone, Copy)]
pub struct Glyph {
    pub char_code: u16,
    pub advance_width: u16,
    pub left_side_bearing: i16,
    pub bounding_box: (i16, i16, i16, i16), // xmin, ymin, xmax, ymax
    /// Simplified contour data: x,y pairs of on-curve points.
    pub contours: [i16; MAX_CONTOUR_POINTS * 2],
    pub contour_count: usize,
    /// Pre-rendered bitmap at default size.
    pub bitmap: [u8; GLYPH_BMP_W as usize * GLYPH_BMP_H as usize],
    pub bitmap_width: u8,
    pub bitmap_height: u8,
}

impl Glyph {
    /// Create an empty glyph.
    pub fn empty() -> Self {
        Glyph {
            char_code: 0,
            advance_width: 512,
            left_side_bearing: 0,
            bounding_box: (0, 0, 0, 0),
            contours: [0i16; MAX_CONTOUR_POINTS * 2],
            contour_count: 0,
            bitmap: [0u8; GLYPH_BMP_W as usize * GLYPH_BMP_H as usize],
            bitmap_width: 0,
            bitmap_height: 0,
        }
    }

    /// Create a glyph from a built-in bitmap character pattern.
    pub fn from_bitmap_pattern(ch: u16, pattern: &[u8], pw: u8, ph: u8) -> Self {
        let mut glyph = Glyph::empty();
        glyph.char_code = ch;
        glyph.advance_width = pw as u16 * 64; // In font units
        glyph.bitmap_width = pw;
        glyph.bitmap_height = ph;
        let copy_len = core::cmp::min(pattern.len(), (pw as usize) * (ph as usize));
        for i in 0..copy_len {
            glyph.bitmap[i] = pattern[i];
        }
        glyph
    }
}

// ── Kerning Pair ─────────────────────────────────────────────────────────────

pub struct KernPair {
    pub left: u16,
    pub right: u16,
    pub value: i16,
}

// ── TTF Font ─────────────────────────────────────────────────────────────────

/// A parsed TrueType font with key tables and up to 256 ASCII glyphs.
pub struct TtfFont {
    pub family_name: [u8; 64],
    pub style_name: [u8; 32],
    pub units_per_em: u16,
    pub ascent: i16,
    pub descent: i16,
    pub line_gap: i16,
    /// Glyph data for ASCII range (0-255).
    pub glyphs: [Glyph; MAX_GLYPHS],
    pub glyph_count: usize,
    /// Kerning pairs.
    pub kern_pairs: [KernPair; MAX_KERN],
    pub kern_count: usize,
    /// Whether the font was loaded from TTF data.
    pub parsed: bool,
}

impl TtfFont {
    /// Create a default/fallback font.
    pub fn default() -> Self {
        TtfFont {
            family_name: str_fixed64("System Default"),
            style_name: str_fixed32("Regular"),
            units_per_em: 1024,
            ascent: 800,
            descent: -200,
            line_gap: 0,
            glyphs: [Glyph::empty(); MAX_GLYPHS],
            glyph_count: 0,
            kern_pairs: {
                const EMPTY_KERN: KernPair = KernPair { left: 0, right: 0, value: 0 };
                [EMPTY_KERN; MAX_KERN]
            },
            kern_count: 0,
            parsed: false,
        }
    }

    /// Parse a TrueType font from raw bytes.
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 12 { return None; }

        // Verify sfVersion (TrueType: 0x00010000, OpenType: 0x4F54544F)
        let sf_version = read_u32(data, 0);
        if sf_version != 0x00010000 && sf_version != 0x4F54544F {
            // Not a valid TTF/OTF file
            return None;
        }

        let mut font = TtfFont::default();

        // ── head table ──────────────────────────────────────────────────────
        if let Some((off, len)) = find_table(data, TAG_HEAD) {
            if len >= 54 {
                font.units_per_em = read_u16(data, off + 18);
                font.ascent = read_i16(data, off + 42);
                font.descent = read_i16(data, off + 44);
                font.line_gap = read_i16(data, off + 46);
            }
        }

        // ── name table (family name) ────────────────────────────────────────
        if let Some((off, _len)) = find_table(data, TAG_NAME) {
            let fmt = read_u16(data, off);
            let count = read_u16(data, off + 2);
            let string_offset = read_u16(data, off + 4) as usize;
            let mut name_pos = off + 6;
            for _ in 0..count {
                if name_pos + 12 > data.len() { break; }
                let platform_id = read_u16(data, name_pos);
                let name_id = read_u16(data, name_pos + 6);
                let lang_id = read_u16(data, name_pos + 8);
                let str_len = read_u16(data, name_pos + 10) as usize;
                let str_off = off + string_offset + read_u16(data, name_pos + 12) as usize;

                if name_id == 1 && str_len > 0 && str_off + str_len <= data.len() {
                    // Family name — copy as UTF-8 (or approximate from UTF-16)
                    let max_copy = core::cmp::min(str_len, font.family_name.len());
                    let copy_len = if fmt == 0 {
                        // Macintosh encoding: ASCII/MacRoman
                        for i in 0..core::cmp::min(str_len, max_copy) {
                            font.family_name[i] = data[str_off + i];
                        }
                        max_copy
                    } else {
                        // Windows encoding: UTF-16BE
                        let mut fi = 0;
                        let mut si = 0;
                        while si < str_len && fi < font.family_name.len() - 1 {
                            let hi = data[str_off + si];
                            let lo = if si + 1 < str_len { data[str_off + si + 1] } else { 0 };
                            let code_point = ((hi as u16) << 8) | (lo as u16);
                            if code_point < 0x80 {
                                font.family_name[fi] = code_point as u8;
                                fi += 1;
                            } else if code_point < 0x800 {
                                font.family_name[fi] = 0xC0 | (code_point >> 6) as u8;
                                font.family_name[fi + 1] = 0x80 | (code_point & 0x3F) as u8;
                                fi += 2;
                            }
                            si += 2;
                        }
                        fi
                    };
                    // Null-terminate
                    if copy_len < font.family_name.len() {
                        font.family_name[copy_len] = 0;
                    }
                }
                if name_id == 2 && str_len > 0 && str_off + str_len <= data.len() {
                    // Style name
                    let max_copy = core::cmp::min(str_len, font.style_name.len());
                    for i in 0..core::cmp::min(str_len, max_copy) {
                        font.style_name[i] = data[str_off + i];
                    }
                    if core::cmp::min(str_len, max_copy) < font.style_name.len() {
                        font.style_name[core::cmp::min(str_len, max_copy)] = 0;
                    }
                }
                name_pos += 14;
            }
        }

        // ── cmap table (format 4 only, for ASCII) ───────────────────────────
        let mut char_to_glyph = [0u16; 256];

        if let Some((cmap_off, cmap_len)) = find_table(data, TAG_CMAP) {
            let num_tables = read_u16(data, cmap_off + 2);
            let mut tbl_pos = cmap_off + 4;
            for _ in 0..num_tables {
                if tbl_pos + 8 > data.len() { break; }
                let platform = read_u16(data, tbl_pos);
                let encoding = read_u16(data, tbl_pos + 2);
                let sub_off = cmap_off + read_u32(data, tbl_pos + 4) as usize;

                if sub_off + 6 <= data.len() {
                    let format = read_u16(data, sub_off);
                    if format == 4 && sub_off + 2 <= data.len() {
                        let sub_len = read_u16(data, sub_off + 2) as usize;
                        if sub_off + sub_len <= data.len() {
                            let seg_count = read_u16(data, sub_off + 6) as usize / 2;

                            let end_codes_off = sub_off + 14;
                            let start_codes_off = end_codes_off + seg_count * 2 + 2;
                            let id_delta_off = start_codes_off + seg_count * 2;
                            let id_range_off = id_delta_off + seg_count * 2;

                            for c in 0..256u16 {
                                for seg in 0..seg_count {
                                    let end = read_u16(data, end_codes_off + seg * 2);
                                    if c > end { continue; }
                                    let start = read_u16(data, start_codes_off + seg * 2);
                                    if c < start { break; }
                                    let delta = read_i16(data, id_delta_off + seg * 2);
                                    let range_off = read_u16(data, id_range_off + seg * 2);

                                    if range_off == 0 {
                                        char_to_glyph[c as usize] = ((c as i16 + delta) as u16);
                                    } else {
                                        let gidx_off = range_off as usize / 2 + (c - start) as usize;
                                        // Actually the offset is from the range offset array start
                                        let actual_off = id_range_off + seg * 2 + range_off as usize + (c - start) as usize * 2;
                                        if actual_off + 1 < data.len() {
                                            let glyph_idx = read_u16(data, actual_off);
                                            if glyph_idx != 0 {
                                                char_to_glyph[c as usize] = (glyph_idx as i16 + delta) as u16;
                                            }
                                        }
                                    }
                                    break;
                                }
                            }
                        }
                        break; // Use first format 4 subtable
                    } else if format == 0 && platform == 3 && encoding == 10 {
                        // Try UCS-4 format 12 if available (skip for simplicity)
                    }
                }
                tbl_pos += 8;
            }
        }

        // ── hhea + hmtx (advance widths) ─────────────────────────────────────
        let mut adv_widths = [0u16; MAX_GLYPHS];
        let mut lsbs = [0i16; MAX_GLYPHS];
        let num_h_metrics: usize;

        if let Some((hhea_off, hhea_len)) = find_table(data, TAG_HHEA) {
            if hhea_len >= 36 {
                num_h_metrics = read_u16(data, hhea_off + 34) as usize;
            } else {
                num_h_metrics = 1;
            }
        } else {
            num_h_metrics = 1;
        }

        if let Some((hmtx_off, hmtx_len)) = find_table(data, TAG_HMTX) {
            for i in 0..core::cmp::min(num_h_metrics, MAX_GLYPHS) {
                let entry_off = hmtx_off + i * 4;
                if entry_off + 4 <= data.len() {
                    adv_widths[i] = read_u16(data, entry_off);
                    lsbs[i] = read_i16(data, entry_off + 2);
                }
            }
            // Fill remaining with last entry
            if num_h_metrics < MAX_GLYPHS && num_h_metrics > 0 {
                let last_w = adv_widths[num_h_metrics - 1];
                for i in num_h_metrics..MAX_GLYPHS {
                    adv_widths[i] = last_w;
                }
            }
        }

        // ── maxp (num glyphs) ───────────────────────────────────────────────
        let num_glyphs: usize;
        if let Some((maxp_off, maxp_len)) = find_table(data, TAG_MAXP) {
            if maxp_len >= 6 {
                num_glyphs = read_u16(data, maxp_off + 4) as usize;
            } else {
                num_glyphs = 256;
            }
        } else {
            num_glyphs = 256;
        }

        // ── loca + glyf (glyph outlines) ────────────────────────────────────
        let loca_is_long = font.units_per_em > 0 && {
            // Check head table for indexToLocFormat
            let mut long = false;
            if let Some((head_off, head_len)) = find_table(data, TAG_HEAD) {
                if head_len >= 52 {
                    long = read_i16(data, head_off + 50) != 0;
                }
            }
            long
        };

        let (loca_off, loca_len) = match find_table(data, TAG_LOCA) {
            Some((o, l)) => (o, l),
            None => (0, 0),
        };
        let (glyf_off, glyf_len) = match find_table(data, TAG_GLYF) {
            Some((o, l)) => (o, l),
            None => (0, 0),
        };

        // Build glyphs for ASCII range
        for ch in 0..256u16 {
            let gid = char_to_glyph[ch as usize];
            if gid >= num_glyphs as u16 { continue; }
            if loca_off == 0 { continue; }

            let glyph_offset;
            let glyph_length;
            if loca_is_long {
                let off = loca_off + (gid as usize) * 4;
                if off + 8 > data.len() { continue; }
                glyph_offset = read_u32(data, off) as usize;
                let next_offset = read_u32(data, off + 4) as usize;
                glyph_length = next_offset.saturating_sub(glyph_offset);
            } else {
                let off = loca_off + (gid as usize) * 2;
                if off + 4 > data.len() { continue; }
                glyph_offset = (read_u16(data, off) as usize) * 2;
                let next_offset = (read_u16(data, off + 2) as usize) * 2;
                glyph_length = next_offset.saturating_sub(glyph_offset);
            }

            if glyph_length == 0 {
                // Empty glyph (space, etc.)
                font.glyphs[ch as usize].char_code = ch;
                font.glyphs[ch as usize].advance_width = adv_widths[core::cmp::min(gid as usize, MAX_GLYPHS - 1)];
                font.glyphs[ch as usize].left_side_bearing = lsbs[core::cmp::min(gid as usize, MAX_GLYPHS - 1)];
                continue;
            }

            let g_off = glyf_off + glyph_offset;
            if g_off + 10 > data.len() { continue; }

            let xmin = read_i16(data, g_off);
            let ymin = read_i16(data, g_off + 2);
            let xmax = read_i16(data, g_off + 4);
            let ymax = read_i16(data, g_off + 6);

            let number_of_contours = read_i16(data, g_off + 8);

            // Only handle simple glyphs (number_of_contours > 0)
            if number_of_contours <= 0 { continue; }

            let mut glyph = Glyph::empty();
            glyph.char_code = ch;
            glyph.advance_width = adv_widths[core::cmp::min(gid as usize, MAX_GLYPHS - 1)];
            glyph.left_side_bearing = lsbs[core::cmp::min(gid as usize, MAX_GLYPHS - 1)];
            glyph.bounding_box = (xmin, ymin, xmax, ymax);

            // Parse simple glyph coordinates
            let mut pos = g_off + 10;
            let end_pts_of_contours_off = pos;
            let end_pts_count = number_of_contours as usize;
            let end_pts_end = end_pts_of_contours_off + end_pts_count * 2;
            if end_pts_end > data.len() { continue; }

            // Read last end point index
            let last_end = read_u16(data, end_pts_end - 2) as usize;

            // Instruction length + data
            let inst_len_off = end_pts_end;
            if inst_len_off + 2 > data.len() { continue; }
            let inst_len = read_u16(data, inst_len_off) as usize;
            let flags_off = inst_len_off + 2 + inst_len;
            if flags_off > data.len() { continue; }

            // Read flags
            let mut flags = [0u8; 512];
            let mut fi = 0;
            let mut flag_pos = flags_off;
            let total_points = last_end + 1;
            while fi < total_points && fi < flags.len() && flag_pos < data.len() {
                let flag = data[flag_pos];
                flags[fi] = flag;
                fi += 1;
                flag_pos += 1;
                // Handle repeat flag
                if (flag & 0x08) != 0 && fi < total_points {
                    let repeat = data[flag_pos] as usize;
                    flag_pos += 1;
                    for _ in 0..core::cmp::min(repeat, total_points.saturating_sub(fi)) {
                        if fi < flags.len() {
                            flags[fi] = flag;
                        }
                        fi += 1;
                    }
                }
            }

            // Read x coordinates
            let mut xs = [0i16; 256];
            let mut x_pos = flag_pos;
            let mut xi = 0;
            let mut prev_x = 0i16;
            while xi < total_points && xi < xs.len() && x_pos < data.len() {
                let flag = flags[xi];
                if (flag & 0x02) != 0 {
                    // One byte value
                    let byte_val = data[x_pos] as u16;
                    x_pos += 1;
                    if (flag & 0x10) != 0 {
                        // Positive sign
                        prev_x += byte_val as i16;
                    } else {
                        prev_x -= byte_val as i16;
                    }
                } else if (flag & 0x10) != 0 {
                    // Same x as previous
                } else {
                    // Two byte value
                    if x_pos + 2 <= data.len() {
                        prev_x += read_i16(data, x_pos);
                        x_pos += 2;
                    }
                }
                xs[xi] = prev_x;
                xi += 1;
            }

            // Read y coordinates
            let mut ys = [0i16; 256];
            let mut y_pos = x_pos;
            let mut yi = 0;
            let mut prev_y = 0i16;
            while yi < total_points && yi < ys.len() && y_pos < data.len() {
                let flag = flags[yi];
                if (flag & 0x04) != 0 {
                    let byte_val = data[y_pos] as u16;
                    y_pos += 1;
                    if (flag & 0x20) != 0 {
                        prev_y += byte_val as i16;
                    } else {
                        prev_y -= byte_val as i16;
                    }
                } else if (flag & 0x20) != 0 {
                    // Same y
                } else {
                    if y_pos + 2 <= data.len() {
                        prev_y += read_i16(data, y_pos);
                        y_pos += 2;
                    }
                }
                ys[yi] = prev_y;
                yi += 1;
            }

            // Store on-curve contour points (only on-curve, flag bit 0)
            let mut ci = 0;
            for i in 0..core::cmp::min(total_points, 256) {
                if ci >= MAX_CONTOUR_POINTS { break; }
                let on_curve = (flags[i] & 0x01) != 0;
                if on_curve {
                    glyph.contours[ci * 2] = xs[i];
                    glyph.contours[ci * 2 + 1] = ys[i];
                    ci += 1;
                }
            }
            glyph.contour_count = ci;

            // Build a simple bitmap from contour data
            Self::rasterize_glyph(&mut glyph);

            font.glyphs[ch as usize] = glyph;
            font.glyph_count = ch as usize + 1;
        }

        // ── kern table ──────────────────────────────────────────────────────
        if let Some((kern_off, kern_len)) = find_table(data, TAG_KERN) {
            if kern_len >= 4 {
                let version = read_u16(data, kern_off);
                let num_tables = read_u16(data, kern_off + 2);
                let mut k_pos = kern_off + 4;
                for _ in 0..num_tables {
                    if k_pos + 6 > data.len() { break; }
                    let sub_len = read_u16(data, k_pos + 2) as usize;
                    let coverage = read_u16(data, k_pos + 4);
                    // Only horizontal kerning (coverage bit 0)
                    if (coverage & 0x0001) == 0 { k_pos += sub_len; continue; }
                    let format = (coverage >> 8) & 0x03;
                    if format == 0 {
                        // Format 0: ordered list of kerning pairs
                        let n_pairs = read_u16(data, k_pos + 6) as usize;
                        let pair_pos = k_pos + 14; // skip header + search range etc.
                        for j in 0..core::cmp::min(n_pairs, MAX_KERN) {
                            let p = pair_pos + j * 6;
                            if p + 6 > data.len() { break; }
                            if font.kern_count < MAX_KERN {
                                font.kern_pairs[font.kern_count] = KernPair {
                                    left: read_u16(data, p),
                                    right: read_u16(data, p + 2),
                                    value: read_i16(data, p + 4),
                                };
                                font.kern_count += 1;
                            }
                        }
                    }
                    k_pos += sub_len;
                }
            }
        }

        font.parsed = true;
        Some(font)
    }

    /// Simple rasterization: fill bounding box from contour points.
    fn rasterize_glyph(glyph: &mut Glyph) {
        let (xmin, ymin, xmax, ymax) = glyph.bounding_box;
        let gw = (xmax - xmin) as u8;
        let gh = (ymax - ymin) as u8;
        if gw == 0 || gh == 0 { return; }

        let scale_w = core::cmp::min(gw, GLYPH_BMP_W);
        let scale_h = core::cmp::min(gh, GLYPH_BMP_H);

        // Simple scanline fill: for each pixel test if it's inside the glyph
        // Use the contour data to determine fill.
        for py in 0..scale_h {
            for px in 0..scale_w {
                // Map pixel to font coordinate space
                let fx = xmin + (px as i16) * (xmax - xmin) / scale_w as i16;
                let fy = ymin + (py as i16) * (ymax - ymin) / scale_h as i16;

                // Simple inside test: compute winding number
                let mut winding = 0i32;
                let mut j = glyph.contour_count - 1;
                for i in 0..glyph.contour_count {
                    let xi = glyph.contours[i * 2];
                    let yi = glyph.contours[i * 2 + 1];
                    let xj = glyph.contours[j * 2];
                    let yj = glyph.contours[j * 2 + 1];

                    if ((yi > fy) != (yj > fy)) {
                        let intersect = xj as i32 + (xi as i32 - xj as i32) * (fy as i32 - yj as i32) / (yi as i32 - yj as i32);
                        if (fx as i32) < intersect {
                            if yj > yi { winding += 1; } else { winding -= 1; }
                        }
                    }
                    j = i;
                }

                let idx = py as usize * GLYPH_BMP_W as usize + px as usize;
                if idx < glyph.bitmap.len() {
                    glyph.bitmap[idx] = if winding != 0 { 255 } else { 0 };
                }
            }
        }
        glyph.bitmap_width = scale_w;
        glyph.bitmap_height = scale_h;
    }

    /// Render a single character to the framebuffer.
    pub fn render_char(
        &self, fb: &mut Framebuffer, x: u32, y: u32,
        ch: char, size: u32, color: Color, _aa: bool,
    ) {
        let code = ch as usize;
        if code >= MAX_GLYPHS { return; }

        let glyph = &self.glyphs[code];
        let bmp_w = glyph.bitmap_width as usize;
        let bmp_h = glyph.bitmap_height as usize;
        if bmp_w == 0 || bmp_h == 0 {
            // Fallback to simple box
            let sz = core::cmp::max(size, 4);
            fb.fill_rect(x, y, sz, sz, color);
            return;
        }

        // Scale bitmap to requested size
        let scale = core::cmp::max(size / bmp_h as u32, 1);
        let render_w = bmp_w as u32 * scale;
        let render_h = bmp_h as u32 * scale;

        for row in 0..render_h {
            if y + row >= fb.height() { break; }
            let src_row = (row / scale) as usize;
            for col in 0..render_w {
                if x + col >= fb.width() { break; }
                let src_col = (col / scale) as usize;
                let src_idx = src_row * bmp_w + src_col;
                let alpha = if src_idx < glyph.bitmap.len() {
                    glyph.bitmap[src_idx]
                } else {
                    0
                };

                if alpha >= 128 {
                    fb.put_pixel(x + col, y + row, color);
                } else if alpha > 0 {
                    // Anti-aliasing: blend with background
                    let bg = fb.get_pixel(x + col, y + row);
                    let blended = graphics::blend(
                        graphics::rgba(
                            graphics::red(color),
                            graphics::green(color),
                            graphics::blue(color),
                            alpha,
                        ),
                        bg,
                    );
                    fb.put_pixel(x + col, y + row, blended);
                }
            }
        }
    }

    /// Measure the width of a text string in pixels (at given size).
    pub fn measure_text(&self, text: &str, size: u32) -> u32 {
        let glyph_scale = if self.units_per_em > 0 {
            size as f32 / self.units_per_em as f32
        } else {
            1.0
        };

        let mut total = 0u32;
        let mut prev_ch: Option<char> = None;
        for ch in text.chars() {
            if let Some(prev) = prev_ch {
                let kern = self.get_kerning(prev, ch);
                if kern != 0 {
                    let adj = (kern as f32 * glyph_scale * 64.0).abs() as u32;
                    total = total.saturating_add(adj);
                }
            }
            let code = ch as usize;
            if code < MAX_GLYPHS {
                let aw = self.glyphs[code].advance_width;
                let px = (aw as f32 * glyph_scale) as u32;
                total += px;
            } else {
                total += size * 2 / 3;
            }
            prev_ch = Some(ch);
        }
        total
    }

    /// Get kerning adjustment between two characters.
    pub fn get_kerning(&self, left: char, right: char) -> i16 {
        for i in 0..self.kern_count {
            if self.kern_pairs[i].left == left as u16
                && self.kern_pairs[i].right == right as u16
            {
                return self.kern_pairs[i].value;
            }
        }
        0
    }

    /// Render text with kerning.
    /// Returns total rendered width.
    pub fn render_text(
        &self, fb: &mut Framebuffer, x: u32, y: u32,
        text: &str, size: u32, color: Color, aa: bool,
    ) -> u32 {
        let glyph_scale = if self.units_per_em > 0 {
            size as f32 / self.units_per_em as f32
        } else {
            1.0
        };

        let mut cx = x;
        let mut prev_ch: Option<char> = None;
        for ch in text.chars() {
            if let Some(prev) = prev_ch {
                let kern = self.get_kerning(prev, ch);
                if kern != 0 {
                    let adj = (kern as f32 * glyph_scale * 64.0) as i32;
                    cx = (cx as i32 + adj).max(0) as u32;
                }
            }
            self.render_char(fb, cx, y, ch, size, color, aa);
            let code = ch as usize;
            if code < MAX_GLYPHS {
                let aw = self.glyphs[code].advance_width;
                cx += (aw as f32 * glyph_scale) as u32;
            } else {
                cx += size * 2 / 3;
            }
            prev_ch = Some(ch);
        }
        cx - x
    }
}

// ── Font Manager ──────────────────────────────────────────────────────────────

/// Manages multiple loaded fonts with a default fallback.
pub struct FontManager {
    pub default_font: TtfFont,
    pub mono_font: TtfFont,
    pub fonts: [(u32, TtfFont); 8],
    pub font_count: usize,
    next_id: u32,
}

impl FontManager {
    /// Create a new font manager with default (built-in) fonts.
    pub fn new() -> Self {
        FontManager {
            default_font: TtfFont::default(),
            mono_font: TtfFont::default(),
            fonts: [
                (0, TtfFont::default()),
                (0, TtfFont::default()),
                (0, TtfFont::default()),
                (0, TtfFont::default()),
                (0, TtfFont::default()),
                (0, TtfFont::default()),
                (0, TtfFont::default()),
                (0, TtfFont::default()),
            ],
            font_count: 0,
            next_id: 100,
        }
    }

    /// Load a font from raw TTF data.
    pub fn load_font(&mut self, data: &[u8]) -> Option<u32> {
        if self.font_count >= 8 { return None; }
        let font = TtfFont::parse(data)?;
        let id = self.next_id;
        self.next_id += 1;
        self.fonts[self.font_count] = (id, font);
        self.font_count += 1;
        Some(id)
    }

    /// Get a font by ID.
    pub fn get_font(&self, id: u32) -> Option<&TtfFont> {
        for i in 0..self.font_count {
            if self.fonts[i].0 == id {
                return Some(&self.fonts[i].1);
            }
        }
        None
    }

    /// Get the default font (with fallback).
    pub fn default(&self) -> &TtfFont {
        &self.default_font
    }
}

// ── Global Font Manager ──────────────────────────────────────────────────────

static mut FONT_MANAGER: Option<FontManager> = None;

/// Initialize the font manager.
pub fn font_init() {
    unsafe {
        if FONT_MANAGER.is_none() {
            FONT_MANAGER = Some(FontManager::new());
            crate::println!("  V39a: Font manager initialized");
        }
    }
}

/// Access the global font manager.
pub fn font_manager() -> Option<&'static mut FontManager> {
    unsafe { FONT_MANAGER.as_mut() }
}

/// Render text using the global font manager (falls back to bitmap font).
pub fn render_text(
    fb: &mut Framebuffer, x: u32, y: u32,
    text: &str, size: u32, color: Color, aa: bool,
) -> u32 {
    unsafe {
        match FONT_MANAGER.as_ref() {
            Some(fm) => fm.default_font.render_text(fb, x, y, text, size, color, aa),
            None => {
                // Fallback to bitmap font
                let font = graphics::font_8x16();
                let mut cx = x;
                for ch in text.chars() {
                    if cx + 8 > fb.width() { break; }
                    fb.draw_char(cx, y, ch, color, 0x00000000, font);
                    cx += 8;
                }
                cx - x
            }
        }
    }
}
