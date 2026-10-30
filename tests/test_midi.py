# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
import threading
import time
import mido
from pianoflow.midi.service import MidiService


def pump(app, condition, timeout=3):
    end = time.monotonic() + timeout
    while not condition() and time.monotonic() < end:
        app.processEvents()
        time.sleep(.01)
    assert condition()


def test_worker_transport_and_hot_unplug(app, monkeypatch):
    class Port:
        def __init__(self):
            self.closed = False
        def close(self):
            self.closed = True
    class Backend:
        def __init__(self):
            self.names = ["Test piano"]
            self.port = Port()
            self.callback = None
        def get_input_names(self):
            return self.names[:]
        def open_input(self, name, callback):
            assert name == "Test piano"
            self.callback = callback
            return self.port
    backend = Backend()
    monkeypatch.setattr(mido, "Backend", lambda _: backend)
    service = MidiService()
    events, ports, statuses, disconnects = [], [], [], []
    service.event_received.connect(events.append)
    service.ports_changed.connect(ports.append)
    service.status_changed.connect(lambda text, connected: statuses.append(connected))
    service.disconnected.connect(lambda: disconnects.append(True))
    service.start()
    try:
        pump(app, lambda: bool(ports))
        service.connect_port("Test piano")
        pump(app, lambda: True in statuses)
        thread = threading.Thread(target=lambda: backend.callback(mido.Message("note_on", note=60, velocity=100)))
        thread.start()
        thread.join()
        pump(app, lambda: bool(events))
        assert events[0].note == 60 and events[0].velocity == 100
        backend.names = []
        service.refresh()
        pump(app, lambda: backend.port.closed)
        pump(app, lambda: len(disconnects) >= 2)
        assert statuses[-1] is False
    finally:
        service.close()
    assert not service.thread.is_alive()
