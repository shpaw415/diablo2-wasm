#!/usr/bin/env python3
"""blast_pkware.py — Python port of Mark Adler's blast.c (PKWARE DCL explode).
Source: zlib contrib/blast/blast.c v1.3 (evidence/blast.c). Used as ground truth
to verify the Rust d2mpq crate's pkware decompression.
"""
MAXBITS = 13
MAXWIN = 4096

LITLEN = bytes([
    11, 124, 8, 7, 28, 7, 188, 13, 76, 4, 10, 8, 12, 10, 12, 10, 8, 23, 8,
    9, 7, 6, 7, 8, 7, 6, 55, 8, 23, 24, 12, 11, 7, 9, 11, 12, 6, 7, 22, 5,
    7, 24, 6, 11, 9, 6, 7, 22, 7, 11, 38, 7, 9, 8, 25, 11, 8, 11, 9, 12,
    8, 12, 5, 38, 5, 38, 5, 11, 7, 5, 6, 21, 6, 10, 53, 8, 7, 24, 10, 27,
    44, 253, 253, 253, 252, 252, 252, 13, 12, 45, 12, 45, 12, 61, 12, 45,
    44, 173])
LENLEN = bytes([2, 35, 36, 53, 38, 23])
DISTLEN = bytes([2, 20, 53, 230, 247, 151, 248])
BASE = [3, 2, 4, 5, 6, 7, 8, 9, 10, 12, 16, 24, 40, 72, 136, 264]
EXTRA = [0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8]

class State:
    def __init__(self, data: bytes):
        self.data = data
        self.pos = 0
        self.bitbuf = 0
        self.bitcnt = 0
        self.out = bytearray()
        self.first = True
        self._built = False
        self.litcnt = [0]*(MAXBITS+1); self.litsym = [0]*256
        self.lencnt = [0]*(MAXBITS+1); self.lensym = [0]*16
        self.distcnt = [0]*(MAXBITS+1); self.distsym = [0]*64

    def bits(self, need: int) -> int:
        val = self.bitbuf
        while self.bitcnt < need:
            if self.pos >= len(self.data):
                raise EOFError("out of input")
            val |= self.data[self.pos] << self.bitcnt
            self.pos += 1
            self.bitcnt += 8
        self.bitbuf = val >> need
        self.bitcnt -= need
        return val & ((1 << need) - 1)

    def decode(self, count, symbol):
        code = first = index = 0
        blen = 1
        n = 1
        bitbuf = self.bitbuf
        left = self.bitcnt
        ci = 1
        while True:
            while left > 0:
                left -= 1
                code |= (bitbuf & 1) ^ 1
                bitbuf >>= 1
                cnt = count[ci]; ci += 1
                if code < first + cnt:
                    self.bitbuf = bitbuf
                    self.bitcnt = (self.bitcnt - blen) & 7
                    return symbol[index + (code - first)]
                index += cnt
                first += cnt
                first <<= 1
                code <<= 1
                blen += 1
            left = (MAXBITS + 1) - blen
            if left == 0:
                break
            if self.pos >= len(self.data):
                raise EOFError("out of input")
            bitbuf = self.data[self.pos]; self.pos += 1
            if left > 8: left = 8
        raise ValueError("bad code")

    def _construct(self, rep, count, symbol):
        length = []
        for r in rep:
            n_rep = (r >> 4) + 1
            ln = r & 15
            length.extend([ln] * n_rep)
        n = len(length)
        for i in range(MAXBITS + 1):
            count[i] = 0
        for ln in length:
            count[ln] += 1
        if count[0] == n:
            return 0
        left = 1
        for ln in range(1, MAXBITS + 1):
            left <<= 1
            left -= count[ln]
            if left < 0:
                return left
        offs = [0] * (MAXBITS + 2)
        for ln in range(1, MAXBITS):
            offs[ln + 1] = offs[ln] + count[ln]
        for sym, ln in enumerate(length):
            if ln != 0:
                symbol[offs[ln]] = sym
                offs[ln] += 1
        return left

    def _build(self):
        self._construct(LITLEN, self.litcnt, self.litsym)
        self._construct(LENLEN, self.lencnt, self.lensym)
        self._construct(DISTLEN, self.distcnt, self.distsym)
        self._built = True

    def run(self) -> bytes:
        if not self._built:
            self._build()
        lit = self.bits(8)
        if lit > 1:
            raise ValueError(f"bad lit flag {lit}")
        dict_bits = self.bits(8)
        if dict_bits < 4 or dict_bits > 6:
            raise ValueError(f"bad dict {dict_bits}")
        while True:
            if self.bits(1):
                symbol = self.decode(self.lencnt, self.lensym)
                ln = BASE[symbol] + self.bits(EXTRA[symbol])
                if ln == 519:
                    break
                sym2 = 2 if ln == 2 else dict_bits
                dist = (self.decode(self.distcnt, self.distsym) << sym2) + self.bits(sym2) + 1
                if self.first and dist > len(self.out):
                    raise ValueError("distance too far back")
                for _ in range(ln):
                    self.out.append(self.out[-dist])
            else:
                if lit:
                    symbol = self.decode(self.litcnt, self.litsym)
                else:
                    symbol = self.bits(8)
                self.out.append(symbol)
        return bytes(self.out)

def blast(data: bytes) -> bytes:
    """Decompress a full PKWARE DCL stream (including its 2-byte header)."""
    return State(data).run()
