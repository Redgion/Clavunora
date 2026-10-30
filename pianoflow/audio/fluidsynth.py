# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Optional SoundFont synthesis through libfluidsynth and Qt audio output.
API: https://www.fluidsynth.org/api/fluidsynth_simple_8c-example.html
"""
import ctypes as C
from ctypes.util import find_library
import threading
from pathlib import Path
from PySide6.QtCore import QIODevice
from PySide6.QtMultimedia import QAudioFormat, QAudioSink, QMediaDevices


class FluidStream(QIODevice):
    def __init__(self, soundfont, parent=None):
        super().__init__(parent)
        library=find_library('fluidsynth')
        if not library: raise RuntimeError('Библиотека FluidSynth не установлена. Можно использовать встроенный звук.')
        if not Path(soundfont).is_file(): raise RuntimeError('Выберите существующий SoundFont .sf2 или .sf3')
        self.lib=C.CDLL(library); self.lock=threading.Lock(); self.targets=set(); self.settings=None; self.synth=None
        signatures={
            'new_fluid_settings': (C.c_void_p,[]), 'delete_fluid_settings':(None,[C.c_void_p]),
            'fluid_settings_setnum':(C.c_int,[C.c_void_p,C.c_char_p,C.c_double]),
            'new_fluid_synth':(C.c_void_p,[C.c_void_p]),'delete_fluid_synth':(None,[C.c_void_p]),
            'fluid_synth_sfload':(C.c_int,[C.c_void_p,C.c_char_p,C.c_int]),
            'fluid_synth_noteon':(C.c_int,[C.c_void_p,C.c_int,C.c_int,C.c_int]),
            'fluid_synth_noteoff':(C.c_int,[C.c_void_p,C.c_int,C.c_int]),
            'fluid_synth_write_s16':(C.c_int,[C.c_void_p,C.c_int,C.c_void_p,C.c_int,C.c_int,C.c_void_p,C.c_int,C.c_int]),
        }
        for name,(result,args) in signatures.items():
            fn=getattr(self.lib,name); fn.restype=result; fn.argtypes=args
        try:
            self.settings=self.lib.new_fluid_settings()
            if not self.settings: raise RuntimeError('Не удалось создать настройки FluidSynth')
            self.lib.fluid_settings_setnum(self.settings,b'synth.sample-rate',44100.)
            self.lib.fluid_settings_setnum(self.settings,b'synth.gain',.5)
            self.synth=self.lib.new_fluid_synth(self.settings)
            if not self.synth: raise RuntimeError('Не удалось создать FluidSynth')
            if self.lib.fluid_synth_sfload(self.synth,str(Path(soundfont)).encode(),1)<0: raise RuntimeError('Не удалось загрузить SoundFont')
            self.open(QIODevice.OpenModeFlag.ReadOnly)
        except Exception:
            self.close(); raise

    def bytesAvailable(self): return 8192+super().bytesAvailable()

    def set_notes(self,notes):
        with self.lock:
            notes=set(notes)
            if not self.synth: return
            for pitch in self.targets-notes: self.lib.fluid_synth_noteoff(self.synth,0,pitch)
            for pitch in notes-self.targets: self.lib.fluid_synth_noteon(self.synth,0,pitch,88)
            self.targets=notes

    def readData(self,maxlen):
        frames=maxlen//4
        if not frames: return b''
        buffer=(C.c_short*(frames*2))()
        with self.lock:
            if self.synth: self.lib.fluid_synth_write_s16(self.synth,frames,buffer,0,2,buffer,1,2)
        return bytes(buffer)

    def writeData(self,data): return -1

    def close(self):
        with self.lock:
            if self.synth: self.lib.delete_fluid_synth(self.synth); self.synth=None
            if self.settings: self.lib.delete_fluid_settings(self.settings); self.settings=None
        super().close()


class FluidAudio:
    def __init__(self,parent,soundfont,device=None,volume=.65):
        device=device or QMediaDevices.defaultAudioOutput()
        fmt=QAudioFormat(); fmt.setSampleRate(44100); fmt.setChannelCount(2); fmt.setSampleFormat(QAudioFormat.SampleFormat.Int16)
        if device.isNull() or not device.isFormatSupported(fmt): raise RuntimeError('Устройство не поддерживает stereo PCM16 44100 Гц')
        self.stream=FluidStream(soundfont,parent)
        self.sink=QAudioSink(device,fmt,parent); self.sink.setVolume(volume); self.sink.setBufferSize(8192); self.sink.start(self.stream)
        self.click_sink=None

    def set_notes(self,notes): self.stream.set_notes(notes)

    def click(self,accent=False):
        # Dedicated short percussion note; does not alter channel 0 piano state.
        with self.stream.lock:
            if self.stream.synth:
                self.stream.lib.fluid_synth_noteon(self.stream.synth,9,76 if accent else 77,90)
                self.stream.lib.fluid_synth_noteoff(self.stream.synth,9,76 if accent else 77)

    def close(self):
        self.sink.stop(); self.stream.close()
