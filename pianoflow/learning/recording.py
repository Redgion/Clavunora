# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
import time
import mido


class Recorder:
    def __init__(self, clock=time.monotonic):
        self.clock = clock
        self.started = None
        self.events = []
        self.held = set()
        self.channels = set()

    @property
    def recording(self):
        return self.started is not None

    def start(self):
        self.events.clear()
        self.held.clear()
        self.channels.clear()
        self.started = self.clock()

    def feed(self, event):
        if not self.recording or event.type not in ('note_on', 'note_off', 'control_change'):
            return
        data = {'type': event.type, 'channel': event.channel}
        if event.type == 'control_change':
            data.update(control=event.control, value=event.value)
        else:
            data.update(note=event.note, velocity=event.velocity)
            key = (event.channel, event.note)
            if event.type == 'note_on' and event.velocity:
                self.held.add(key)
            else:
                self.held.discard(key)
        self.channels.add(event.channel)
        self.events.append((self.clock() - self.started, data))

    def stop(self):
        if self.recording:
            t = self.clock() - self.started
            for channel, note in sorted(self.held):
                self.events.append((t, dict(type='note_off', channel=channel, note=note, velocity=0)))
            for channel in sorted(self.channels):
                self.events.append((t, dict(type='control_change', channel=channel, control=64, value=0)))
            self.started = None
            self.held.clear()

    def save(self, path):
        self.stop()
        midi = mido.MidiFile(ticks_per_beat=480)
        track = mido.MidiTrack()
        midi.tracks.append(track)
        track.append(mido.MetaMessage('set_tempo', tempo=500000))
        previous = 0
        for seconds, data in self.events:
            tick = round(seconds * 960)
            track.append(mido.Message(**data, time=max(0, tick - previous)))
            previous = tick
        midi.save(path)
