#!/usr/bin/env python3
"""mpq_mini.py — Minimal read-only MPQ v1 parser WITH MPQ cryptography.

Ground-truth implementation used to verify the Rust d2mpq crate byte-for-byte.
Format references: StormLib sources, MPQ docs (hash_string types, crypt table,
block/hash table layout, sector-based compression: zlib/bzip2/pkware).
"""
import struct, zlib, sys, os

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from blast_pkware import blast as _dcl_blast

try:
    import bz2
except ImportError:
    bz2 = None

# ---------------------------------------------------------------- MPQ crypto
_CRYPT = None

def crypt_table():
    global _CRYPT
    if _CRYPT is None:
        t = [0] * 0x500
        seed = 0x00100001
        for i in range(0x100):
            idx = i
            for _ in range(5):
                seed = (seed * 125 + 3) % 0x2AAAAB
                t1 = (seed & 0xFFFF) << 0x10
                seed = (seed * 125 + 3) % 0x2AAAAB
                t2 = seed & 0xFFFF
                t[idx] = t1 | t2
                idx += 0x100
        _CRYPT = t
    return _CRYPT

def hash_string(s: str, typ: int) -> int:
    """MPQ hash: typ 0=name1, 1=name2, 2=slot, 3=file key, 5=key fix ('(def)' style)."""
    t = crypt_table()
    seed1, seed2 = 0x7FED7FED, 0xEEEEEEEE
    for b in s.upper().encode('ascii', 'replace'):
        if b == ord('/'):
            b = ord('\\')
        seed1 = (t[(typ << 8) + b] ^ (seed1 + seed2)) & 0xFFFFFFFF
        seed2 = (b + seed1 + seed2 + (seed2 << 5) + 3) & 0xFFFFFFFF
    return seed1

def decrypt_bytes(data: bytes, key: int) -> bytes:
    t = crypt_table()
    n = len(data) // 4
    vals = list(struct.unpack(f'<{n}I', data[:n * 4]))
    seed2 = 0xEEEEEEEE
    for i in range(n):
        seed2 = (seed2 + t[0x400 + (key & 0xFF)]) & 0xFFFFFFFF
        ch = vals[i] ^ ((key + seed2) & 0xFFFFFFFF)
        key = (((~key) << 0x15) + 0x11111111 & 0xFFFFFFFF) | (key >> 0x0B)
        seed2 = (ch + seed2 + (seed2 << 5) + 3) & 0xFFFFFFFF
        vals[i] = ch & 0xFFFFFFFF
    return struct.pack(f'<{n}I', *vals)

# ------------------------------------------------------------------ PKWARE DCL
_SLASH = {}
def _pk_litlen(b):
    return b, 8

def pkware_decompress(data: bytes, expected: int) -> bytes:
    """PKWARE Data Compression Library (DCL) implode/explode — literal/1 (type 1) & 2."""
    raise NotImplementedError("pkware not needed for D2 data files (zlib/bzip2)")

# ------------------------------------------------------------------- MPQ file
FLAG_FILE = 0x80000000
FLAG_FIX_KEY = 0x00020000
FLAG_PATCH = 0x10000000
FLAG_SINGLE_UNIT = 0x01000000
FLAG_DELETE = 0x02000000
FLAG_SECTOR_CRC = 0x04000000
FLAG_EXISTS = 0x80000000
FLAG_COMPRESS = 0x00000200
FLAG_IMPLODE = 0x00000100
FLAG_ENCRYPTED = 0x00010000
FLAG_KEY_INDEX = 0x03000000  # mask (key index << 24)

class MPQ:
    def __init__(self, path: str):
        self.path = path
        self.f = open(path, 'rb')
        head = self.f.read(0x200)
        if head[0:4] == b'MPQ\x1b':  # user data wrapper
            ud_size, hoff = struct.unpack_from('<II', head, 4)
            self.f.seek(hoff)
            head = self.f.read(0x200)
        assert head[0:4] == b'MPQ\x1a', f'not MPQ: {head[0:4]!r}'
        (hsz, asz, fmtv, sshift, hoff, boff, hent, bent) = struct.unpack_from('<IIHHIIII', head, 4)
        self.header_size, self.archive_size = hsz, asz
        self.version, self.sector_shift = fmtv, sshift
        self.sector_size = 512 << sshift
        self.base = 0
        self.hash_off, self.block_off = hoff, boff
        self.hash_n, self.block_n = hent, bent
        if fmtv >= 3 and hsz >= 0x44:
            (self.hi_off64,) = struct.unpack_from('<Q', head, 0x40)
        else:
            self.hi_off64 = 0
        self._load_tables()

    def _load_tables(self):
        self.f.seek(self.hash_off)
        raw = decrypt_bytes(self.f.read(self.hash_n * 16), hash_string('(hash table)', 3))
        self.hash = [struct.unpack_from('<IIHHI', raw, i * 16) for i in range(self.hash_n)]
        self.f.seek(self.block_off)
        raw = decrypt_bytes(self.f.read(self.block_n * 16), hash_string('(block table)', 3))
        self.block = [struct.unpack_from('<IIII', raw, i * 16) for i in range(self.block_n)]

    def find(self, name: str):
        n = name.replace('/', '\\')
        h1, h2 = hash_string(n, 1), hash_string(n, 2)
        start = hash_string(n, 0) & (self.hash_n - 1)
        for i in range(self.hash_n):
            idx = (start + i) & (self.hash_n - 1)
            e = self.hash[idx]
            if e[2] == 0xFFFF and e[3] == 0xFFFF:  # empty
                return None
            if e[0] == h1 and e[1] == h2 and (e[4] != 0xFFFFFFFF):
                return e[4]
        return None

    def block_info(self, name):
        bi = self.find(name)
        if bi is None or bi >= self.block_n:
            return None
        off, csize, fsize, flags = self.block[bi]
        return {'index': bi, 'offset': off, 'csize': csize, 'fsize': fsize, 'flags': flags}

    def _read(self, off, size):
        self.f.seek(self.base + off)
        return self.f.read(size)

    def file_key(self, name, b):
        key = hash_string(name.replace('/', '\\'), 3)
        if b['flags'] & FLAG_FIX_KEY:
            key = ((key + b['offset']) & 0xFFFFFFFF) ^ b['fsize']
        ki = (b['flags'] & FLAG_KEY_INDEX) >> 24
        if ki:
            key = self.block[ki][0] if ki < self.block_n else key
        return key

    def decompress(self, data: bytes, flags: int) -> bytes:
        if flags & FLAG_IMPLODE:
            return _dcl_blast(data)
        if flags & FLAG_COMPRESS:
            if not data:
                return b''
            mask = data[0]
            body = data[1:]
            if mask == 0:
                return body
            out = body
            for i in range(8):
                if mask & (1 << i):
                    if i == 1:  # zlib (0x02)
                        out = zlib.decompress(out)
                    elif i == 2:  # pkware DCL (0x04) — modern table
                        out = _dcl_blast(out)
                    elif i == 3:  # D2-era Storm: 0x08 = PKWARE DCL (Ghidra: FUN_6fc13ab0 accepts only 0x01/0x08/0x40/0x80)
                        # Prefer bz2 magic when present (patched-era archives), else D2 DCL.
                        if bz2 is not None and out[:3] == b'BZh':
                            out = bz2.decompress(out)
                        else:
                            out = _dcl_blast(out)
            return out
        return data

    def read_file(self, name: str) -> bytes | None:
        b = self.block_info(name)
        if b is None or not (b['flags'] & FLAG_EXISTS) or (b['flags'] & FLAG_DELETE):
            return None
        flags = b['flags']
        if flags & FLAG_SINGLE_UNIT:
            data = self._read(b['offset'], b['csize'])
            if flags & FLAG_ENCRYPTED:
                data = decrypt_bytes(data, self.file_key(name, b))
            return self.decompress(data, flags)
        # sector-based
        n_sectors = (b['fsize'] + self.sector_size - 1) // self.sector_size
        buf = self._read(b['offset'], 4 * (n_sectors + 1))
        if flags & FLAG_ENCRYPTED:
            buf = decrypt_bytes(buf, self.file_key(name, b) - 1)
        offs = list(struct.unpack(f'<{n_sectors + 1}I', buf))
        out = bytearray()
        for s in range(n_sectors):
            chunk = self._read(b['offset'] + offs[s], offs[s + 1] - offs[s])
            if flags & FLAG_ENCRYPTED:
                chunk = decrypt_bytes(chunk, self.file_key(name, b))
            is_last = (s == n_sectors - 1)
            if (flags & (FLAG_COMPRESS | FLAG_IMPLODE)) and not (is_last and len(chunk) >= self.sector_size):
                out += self.decompress(chunk, flags)
            else:
                out += chunk
        return bytes(out[:b['fsize']])

    def listfile(self):
        data = self.read_file('(listfile)')
        if data is None:
            return []
        txt = data.decode('utf-8', 'replace')
        return [l for l in txt.replace('\r\n', '\n').split('\n') if l]

if __name__ == '__main__':
    mpq = MPQ(sys.argv[1])
    print(f"v{mpq.version} sector={mpq.sector_size} hash={mpq.hash_n} block={mpq.block_n}")
    if len(sys.argv) > 2:
        data = mpq.read_file(sys.argv[2])
        if data is None:
            print("NOT FOUND"); sys.exit(1)
        out = sys.argv[3] if len(sys.argv) > 3 else os.path.basename(sys.argv[2])
        open(out, 'wb').write(data)
        print(f"{sys.argv[2]}: {len(data):,} bytes -> {out}")
    else:
        names = mpq.listfile()
        print(f"listfile: {len(names)} entries")
