# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Versioned offline repertoire. Installation never overwrites user edits."""
from importlib.resources import files
import json
from pianoflow.library.store import decode


def manifest():
    return json.loads(files('pianoflow').joinpath('resources/catalog/index.json').read_text(encoding='utf-8'))


def bundled_songs():
    root = files('pianoflow').joinpath('resources/catalog')
    for entry in manifest()['entries']:
        if entry.get('score'):
            yield entry['id'], decode(root.joinpath(entry['score']).read_text(encoding='utf-8'))


def install_catalog(library):
    for identifier, song in bundled_songs():
        marker = 'catalog-installed:' + identifier
        if not library.preference(marker, False):
            library.add(song, identifier)
            library.set_preference(marker, True)


def coverage(song):
    total = len(song.notes)
    hands = sum(bool(n.hand) for n in song.notes)
    fingers = sum(bool(n.hand and n.finger) for n in song.notes)
    return f'Разметка рук: {hands}/{total} нот · Подсказки пальцев: {fingers}/{total} нот'
