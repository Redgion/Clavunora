# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
from pianoflow.core.song import birthday_song
from pianoflow.core.transport import Transport
import pytest


def test_birthday_complete_and_playable_on_computer():
    song = birthday_song()
    assert len(song.notes) == 25
    assert [n.pitch for n in song.notes[:6]] == [60, 60, 62, 60, 65, 64]
    assert [n.pitch for n in song.notes[-6:]] == [70, 70, 69, 65, 67, 65]
    assert all(60 <= n.pitch <= 72 and n.duration > 0 for n in song.notes)
    assert song.duration > 14


def test_transport_monotonic_speed_pause_seek():
    now = [100.]
    transport = Transport(lambda: now[0])
    transport.duration = 20
    transport.play()
    now[0] += 4
    assert transport.current() == 1
    transport.set_speed(.5)
    now[0] += 2
    assert transport.current() == 2
    transport.pause()
    now[0] += 8
    assert transport.current() == 2
    transport.seek(10)
    transport.play()
    now[0] += 2
    assert transport.current() == 11
    transport.seek(100)
    assert transport.current() == 20
    with pytest.raises(ValueError):
        transport.set_speed(0)
