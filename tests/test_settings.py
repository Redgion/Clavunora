# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
import pytest
from pianoflow.settings.store import SettingsStore


def test_roundtrip(tmp_path):
    path = tmp_path / "config" / "settings.json"
    settings = SettingsStore(path)
    settings.values.update(show_labels=False, keyboard_size=61, midi_port="Piano")
    settings.save()
    restored = SettingsStore(path)
    assert restored.values == settings.values


def test_invalid_types_and_range(tmp_path):
    path = tmp_path / "settings.json"
    path.write_text('{"keyboard_size": 1, "show_labels": "no"}')
    settings = SettingsStore(path)
    assert settings.values["keyboard_size"] == 88
    assert settings.values["show_labels"] is True


def test_corrupt_preserved(tmp_path):
    path = tmp_path / "settings.json"
    path.write_text("{bad")
    settings = SettingsStore(path)
    assert settings.error
    with pytest.raises(OSError):
        settings.save()
    assert path.read_text() == "{bad"
