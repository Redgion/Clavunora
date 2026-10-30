# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""ALSA/RtMidi transport owned by a worker thread, delivered through Qt signals."""
import queue
import threading
import time
from PySide6.QtCore import QObject, Signal
from pianoflow.core.state import MidiEvent


class MidiService(QObject):
    event_received = Signal(object)
    ports_changed = Signal(list)
    status_changed = Signal(str, bool)
    disconnected = Signal()

    def __init__(self, parent=None):
        super().__init__(parent)
        self.commands = queue.Queue()
        self.stop_event = threading.Event()
        self.thread = threading.Thread(target=self._run, daemon=True, name="PianoFlow MIDI")

    def start(self):
        from pianoflow.core.platform import is_android
        if is_android():
            self.status_changed.emit('Android: USB/Bluetooth MIDI ещё не реализован', False)
            return
        self.thread.start()

    def connect_port(self, name: str):
        self.commands.put(("connect", name))

    def disconnect_port(self):
        self.commands.put(("disconnect", ""))

    def refresh(self):
        self.commands.put(("refresh", ""))

    def close(self):
        self.stop_event.set()
        if self.thread.is_alive():
            self.thread.join(timeout=3)

    def _callback(self, message):
        if message.type not in ("note_on", "note_off", "control_change"):
            return
        self.event_received.emit(MidiEvent(
            message.type, getattr(message, "note", 0), getattr(message, "velocity", 0),
            message.channel, getattr(message, "control", 0), getattr(message, "value", 0),
            timestamp=time.monotonic()))

    def _run(self):
        port = None
        name = ""
        previous = None
        next_scan = 0.0
        try:
            import mido
            backend = mido.Backend("mido.backends.rtmidi")
            while not self.stop_event.is_set():
                try:
                    command, target = self.commands.get(timeout=0.1)
                except queue.Empty:
                    command, target = "", ""
                try:
                    if command in ("connect", "disconnect"):
                        if port is not None:
                            port.close()
                            port = None
                        name = ""
                        self.disconnected.emit()
                        if command == "connect":
                            port = backend.open_input(target, callback=self._callback)
                            name = target
                            self.status_changed.emit(target, True)
                        else:
                            self.status_changed.emit("MIDI отключён", False)
                    if command == "refresh" or time.monotonic() >= next_scan:
                        next_scan = time.monotonic() + 2
                        names = backend.get_input_names()
                        if names != previous:
                            self.ports_changed.emit(names)
                            previous = names
                        if port is not None and name not in names:
                            port.close()
                            port = None
                            name = ""
                            self.disconnected.emit()
                            self.status_changed.emit("Устройство отключено", False)
                except Exception as exc:
                    self.status_changed.emit(f"MIDI: {exc}", False)
                    if port is not None:
                        port.close()
                        port = None
                    name = ""
                    self.disconnected.emit()
                    next_scan = time.monotonic() + 2
        except Exception as exc:
            self.status_changed.emit(f"MIDI недоступен: {exc}", False)
        finally:
            if port is not None:
                port.close()
