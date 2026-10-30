# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Bridge Android document-provider URIs to the existing local file readers."""
from contextlib import contextmanager
from pathlib import Path
import tempfile


def read_document(path):
    if not str(path).startswith('content://'):
        return Path(path).read_bytes()
    from PySide6.QtCore import QFile, QIODevice
    source = QFile(str(path))
    if not source.open(QIODevice.OpenModeFlag.ReadOnly):
        raise OSError(source.errorString())
    try:
        data = bytes(source.readAll())
        if source.error() != QFile.FileError.NoError: raise OSError(source.errorString())
        return data
    finally: source.close()


def write_document(path, data):
    if not str(path).startswith('content://'):
        return Path(path).write_bytes(data)
    from PySide6.QtCore import QFile, QIODevice
    target = QFile(str(path))
    if not target.open(QIODevice.OpenModeFlag.WriteOnly | QIODevice.OpenModeFlag.Truncate):
        raise OSError(target.errorString())
    try:
        if target.write(data) != len(data) or not target.flush(): raise OSError(target.errorString())
    finally: target.close()


@contextmanager
def local_document(path, suffix='', output=False):
    if not str(path).startswith('content://'):
        yield Path(path)
        return
    data = b'' if output else read_document(path)
    if not output and data.lstrip(b'\xef\xbb\xbf \r\n\t').startswith(b'<'):
        suffix = '.musicxml'
    with tempfile.TemporaryDirectory(prefix='pianoflow-document-') as directory:
        local = Path(directory)/('document' + suffix)
        if not output: local.write_bytes(data)
        yield local
        if output: write_document(path, local.read_bytes())
