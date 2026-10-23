#!/usr/bin/env python3
"""mpq_list.py — Enumerate files in an MPQ via StormLib (SFileFindFirstFile).

Usage: python3 tools/mpq_list.py <archive.mpq> [substring-filter]
"""
import ctypes, os, sys

_lib = ctypes.CDLL('libstorm.so.9')

_lib.SFileOpenArchive.argtypes = [ctypes.c_char_p, ctypes.c_uint32, ctypes.c_uint32, ctypes.POINTER(ctypes.c_void_p)]
_lib.SFileOpenArchive.restype = ctypes.c_bool
_lib.SFileCloseArchive.argtypes = [ctypes.c_void_p]
_lib.SFileCloseArchive.restype = ctypes.c_bool
_lib.SFileFindFirstFile.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_void_p, ctypes.c_char_p]
_lib.SFileFindFirstFile.restype = ctypes.c_void_p
_lib.SFileFindNextFile.argtypes = [ctypes.c_void_p]
_lib.SFileFindNextFile.restype = ctypes.c_bool
_lib.SFileFindClose.argtypes = [ctypes.c_void_p]


class SFILE_FIND_DATA(ctypes.Structure):
    _pack_ = 1
    _fields_ = [
        ('cFileName', ctypes.c_char * 260),     # MAX_PATH
        ('dwFileHash', ctypes.c_uint32 * 2),    # 8 bytes
        ('dwFileSize', ctypes.c_uint32),
        ('dwFileFlags', ctypes.c_uint32),
        ('dwCompSize', ctypes.c_uint32),
        ('dwFileTimeLo', ctypes.c_uint32),
        ('dwFileTimeHi', ctypes.c_uint32),
    ]


def list_files(mpq_path):
    h = ctypes.c_void_p()
    if not _lib.SFileOpenArchive(os.fsencode(mpq_path), 0, 0, ctypes.byref(h)):
        raise OSError(f'cannot open {mpq_path}')
    out = []
    try:
        fd = SFILE_FIND_DATA()
        hf = _lib.SFileFindFirstFile(h, b'*', ctypes.byref(fd), None)
        if hf:
            while True:
                out.append((fd.cFileName.split(b'\x00')[0].decode('utf-8', 'replace'),
                            fd.dwFileSize, fd.dwFileFlags))
                if not _lib.SFileFindNextFile(hf):
                    break
            _lib.SFileFindClose(hf)
        return out
    finally:
        _lib.SFileCloseArchive(h)


if __name__ == '__main__':
    files = list_files(sys.argv[1])
    filt = sys.argv[2].lower() if len(sys.argv) > 2 else None
    for name, size, flags in files:
        if filt and filt not in name.lower():
            continue
        print(f'{size:>10,}  {name}')
    print(f'-- {len(files)} files', file=sys.stderr)
