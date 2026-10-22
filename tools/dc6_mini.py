#!/usr/bin/env python3
"""dc6_mini.py — DC6 sprite decoder (ground truth for the Rust d2dc6 crate).

Proven empirically on the D2 digital release v1.12 (2008) files in assets/
(skillicon.dc6 24x48x48, invcap.dc6 56x56, invhp1.dc6, minipanel.dc6,
font8/16/24/30.dc6 256 frames) and completed with the public classic spec.
See evidence/DC6-SPEC.md for the byte-level proofs.

TWO FILE LAYOUTS (auto-detected, both starting with u32 version=6|7, u32 flags,
u32 encoding, u32 filler at offsets 0/4/8/12):

  Layout A — "flat" (EVERY file of the 1.12 digital release; observed flags=0x1,
  encoding=0, filler 0xCDCDCDCD or 0xEEEEEEEE):
      u32 @16 = directions, u32 @20 = framesPerDirection,
      u32 framePtr[totalFrames] @24 (FLAT — no direction indirection).

  Layout B — "classic" (public spec; NOT present in the 1.12 digital archives:
  a full scan of d2char/d2data/d2exp.mpq found 0 encoding=1 files):
      u32 @16 = directions, u32 @20 = framesPerDirection,
      u32 dirPtr[directions] @24, each dirPtr -> u32 framePtr[framesPerDirection].

Frame header (28 bytes) at each frame pointer:
  u32 flip, u32 width, u32 height, u32 offsetX, u32 offsetY, u32 allocSize,
  u32 nextFrame; then u32 dataLen (frameData byte length) at header+28
  and the RLE payload at header+32, padded to a 4-byte boundary with filler
  bytes (0xCD... or 0xEE... — debug fill, must NOT be decoded).

TWO RLE GRAMMARS (rows stored bottom-up: stream row i -> dest row height-1-i):

  "variant" grammar (this release; encoding=0 yet data IS RLE — the field lies):
      b <= 0x7F   : literal run — copy the next b bytes as pixel indices
      b == 0x80   : end of scanline (single byte, NO operand)
      b == 0x81 n : literal run of n
      b >= 0x82   : transparent run of (b - 0x80)
      Every scanline is terminated by 0x80; a row left short of width at EOL
      stays transparent. NOTE the polarity swap vs classic: literal/transparent
      ranges are INVERTED.

  "classic" grammar (public spec, encoding=1):
      b <= 0x7F   : transparent run of b
      b == 0x80 n : n == 0 -> end of scanline; n > 0 -> rewind x by n
                    (re-decode overwrites earlier pixels)
      b == 0x81 n : literal run of n
      b >= 0x82   : literal run of (b & 0x7F) == (b - 0x80)
"""
import struct

FILLER_OK = (0xEEEEEEEE, 0xCDCDCDCD, 0x00000000)


def _valid_frame_header(data, off, version):
    """Check a candidate frame header; return (width,height,offx,offy,flip) or None."""
    if off + 28 > len(data):
        return None
    flip, w, h, ox, oy, alloc, nxt = struct.unpack_from('<7I', data, off)
    if flip > 1 or w == 0 or h == 0 or w > 2048 or h > 2048:
        return None
    return flip, w, h, ox, oy


def _frame_pointers(data):
    """Return (encoding, [frame data offsets]) using layout A, falling back to B."""
    version, flags, encoding = struct.unpack_from('<III', data, 0)
    ndir, nfpd = struct.unpack_from('<II', data, 16)
    if ndir == 0 or ndir > 32 or nfpd == 0 or nfpd > 4096:
        raise ValueError(f'unplausible counts: dirs={ndir} frames={nfpd}')
    total = ndir * nfpd
    # Layout A: flat frame pointers at 24
    ptrs = list(struct.unpack_from(f'<{min(total, (len(data) - 24) // 4)}I', data, 24))
    if all(_valid_frame_header(data, p, version) for p in ptrs[:total]):
        return version, encoding, ptrs[:total]
    # Layout B: direction indirection (classic)
    if len(data) >= 24 + 4 * ndir:
        dir_ptrs = struct.unpack_from(f'<{ndir}I', data, 24)
        ptrs = []
        ok = True
        for dp in dir_ptrs:
            if dp + 4 * nfpd > len(data):
                ok = False
                break
            fps = struct.unpack_from(f'<{nfpd}I', data, dp)
            for fp in fps:
                if _valid_frame_header(data, fp, version) is None:
                    ok = False
                    break
                ptrs.append(fp)
            if not ok:
                break
        if ok:
            return version, encoding, ptrs
    raise ValueError('neither flat nor classic pointer layout validates')


def rle_rows(data: bytes, pos: int, width: int, height: int, classic: bool = False,
             datalen: int = None) -> bytearray:
    """Decode DC6 RLE frame data into a width*height palette-index buffer.

    Scanlines are stored BOTTOM-UP in a single stream: the i-th scanline
    produced by the loop lands on dest row height-1-i. `classic` selects the
    classic grammar (transparent-first, 0x80 n rewind/EOL); default is the
    v1.12 digital-release "variant" grammar (literal-first, bare 0x80 EOL).
    Stops after `datalen` bytes (or end of data) or height scanlines, so the
    4-byte padding junk after the payload is never decoded.
    """
    out = bytearray(width * height)
    n = len(data) if datalen is None else min(len(data), pos + datalen)
    p, x, row = pos, 0, 0
    while p < n and row < height:
        b = data[p]
        p += 1
        if classic:
            if b <= 0x7F:                       # transparent run
                x += b
            elif b == 0x80:                     # rewind n, or EOL when n==0
                if p >= n:
                    break
                r = data[p]
                p += 1
                if r == 0:
                    row += 1
                    x = 0
                else:
                    x = max(0, x - r)           # overwrite: re-decode backtracks
            elif b == 0x81:                     # literal run, explicit count
                if p >= n:
                    break
                r = min(data[p], width - x)
                y = height - 1 - row
                out[y * width + x: y * width + x + r] = data[p + 1:p + 1 + r]
                x += data[p]
                p += 1 + data[p]
            else:                               # literal run, count in opcode
                r = min(b & 0x7F, width - x)
                y = height - 1 - row
                out[y * width + x: y * width + x + r] = data[p:p + r]
                x += b & 0x7F
                p += b & 0x7F
        else:
            if b <= 0x7F:                       # literal run, count in opcode
                r = min(b, width - x)
                y = height - 1 - row
                out[y * width + x: y * width + x + r] = data[p:p + r]
                x += b
                p += b
            elif b == 0x80:                     # end of scanline (no operand)
                row += 1
                x = 0
            elif b == 0x81:                     # literal run, explicit count
                if p >= n:
                    break
                r = min(data[p], width - x)
                y = height - 1 - row
                out[y * width + x: y * width + x + r] = data[p + 1:p + 1 + r]
                x += data[p]
                p += 1 + data[p]
            else:                               # transparent run of b-0x80
                x = min(width, x + (b - 0x80))
    return out


def decode_dc6(data: bytes) -> list:
    """Decode a DC6 file -> list of frames (direction-major order).

    Each frame: {flip, width, height, offsetX, offsetY, pixels} where pixels
    is a bytearray of width*height palette indices in TOP-DOWN row-major order
    (index 0 = transparent). Auto-detects flat vs classic pointer layout and
    variant vs classic RLE (encoding==1 -> classic grammar, else variant).
    """
    version, encoding, ptrs = _frame_pointers(data)
    classic = (encoding == 1)
    frames = []
    for off in ptrs:
        flip, w, h, ox, oy = _valid_frame_header(data, off, version)
        datalen = struct.unpack_from('<I', data, off + 28)[0]
        dstart = off + 32 + (4 if version >= 7 else 0)
        pix = rle_rows(data, dstart, w, h, classic=classic, datalen=datalen)
        frames.append({'flip': flip, 'width': w, 'height': h,
                       'offsetX': ox, 'offsetY': oy, 'pixels': pix})
    return frames


def load_palette(path: str) -> list:
    """Load a 767-byte (256xRGB truncated) D2 palette DAT, zero-padded to 768."""
    data = open(path, 'rb').read()
    data = (data + b'\x00' * 768)[:768]
    return [tuple(data[i * 3:i * 3 + 3]) for i in range(256)]
