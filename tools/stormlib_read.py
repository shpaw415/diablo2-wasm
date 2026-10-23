#!/usr/bin/env python3
"""stormlib_read.py — minimal ctypes wrapper for StormLib (libstorm.so.9).
Reads any file from any MPQ regardless of encryption/compression.
"""
import ctypes, os

_lib = ctypes.CDLL('libstorm.so.9')

_lib.SFileOpenArchive.argtypes = [ctypes.c_char_p, ctypes.c_uint32, ctypes.c_uint32, ctypes.POINTER(ctypes.c_void_p)]
_lib.SFileOpenArchive.restype = ctypes.c_bool
_lib.SFileOpenFileEx.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_uint32, ctypes.POINTER(ctypes.c_void_p)]
_lib.SFileOpenFileEx.restype = ctypes.c_bool
_lib.SFileGetFileSize.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint32)]
_lib.SFileGetFileSize.restype = ctypes.c_uint32
_lib.SFileReadFile.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_uint32, ctypes.POINTER(ctypes.c_uint32), ctypes.c_void_p]
_lib.SFileReadFile.restype = ctypes.c_bool
_lib.SFileCloseFile.argtypes = [ctypes.c_void_p]
_lib.SFileCloseArchive.argtypes = [ctypes.c_void_p]


def read_file(mpq_path, name):
    h = ctypes.c_void_p()
    if not _lib.SFileOpenArchive(os.fsencode(mpq_path), 0, 0, ctypes.byref(h)):
        raise OSError(f'cannot open {mpq_path}')
    try:
        hf = ctypes.c_void_p()
        if not _lib.SFileOpenFileEx(h, name.encode(), 0, ctypes.byref(hf)):
            return None
        try:
            size = _lib.SFileGetFileSize(hf, None)
            buf = ctypes.create_string_buffer(size)
            got = ctypes.c_uint32(0)
            if not _lib.SFileReadFile(hf, buf, size, ctypes.byref(got), None):
                raise OSError('read failed')
            return buf.raw[:got.value]
        finally:
            _lib.SFileCloseFile(hf)
    finally:
        _lib.SFileCloseArchive(h)


if __name__ == '__main__':
    import sys
    d = read_file(sys.argv[1], sys.argv[2])
    print('None' if d is None else f'{len(d):,} bytes')
