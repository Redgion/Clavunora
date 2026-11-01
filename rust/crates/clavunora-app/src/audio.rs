// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
//! Device audio runs outside the UI thread. Rendering is also testable without a sound card.
#[cfg(target_os = "linux")]
use alsa::{
    Direction, ValueOr,
    pcm::{Access, Format, HwParams, PCM},
};
use anyhow::{Context, Result, ensure};
use clavunora_core::midi::PianoState;
use clavunora_core::playback::{Cursor, Sequence};
use serde::{Deserialize, Serialize};
use std::{
    ffi::{CString, c_char, c_int, c_void},
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

#[cfg(any(target_os = "linux", test))]
const RATE: u32 = 48_000;
const FRAMES: usize = 256;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub volume: f32,
    pub ambience: f32,
    pub device: String,
    /// Empty uses the built-in instrument. Nonempty requires a local .sf2/.sf3 and FluidSynth.
    pub soundfont: String,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            volume: 0.5,
            ambience: 0.12,
            device: "default".into(),
            soundfont: String::new(),
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.volume.is_finite() && (0.0..=1.0).contains(&self.volume),
            "Некорректная громкость"
        );
        ensure!(
            self.ambience.is_finite() && (0.0..=0.35).contains(&self.ambience),
            "Некорректный объём помещения"
        );
        ensure!(
            !self.device.trim().is_empty() && !self.device.contains('\0'),
            "Укажите аудиоустройство или default"
        );
        if !self.soundfont.is_empty() {
            let path = Path::new(&self.soundfont);
            ensure!(path.is_file(), "Файл SoundFont не найден");
            ensure!(path.extension().is_some_and(|s|s.eq_ignore_ascii_case("sf2") || s.eq_ignore_ascii_case("sf3")),"Нужен SoundFont .sf2 или .sf3");
            ensure!(
                !self.soundfont.contains('\0'),
                "Некорректный путь SoundFont"
            );
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Default, Debug)]
struct Target {
    velocity: u8,
    generation: u64,
}
#[derive(Clone)]
struct Shared {
    notes: [Target; 128],
    demo: [Target; 128],
    volume: f32,
    ambience: f32,
    enabled: bool,
    click: u64,
    accent: bool,
    test_until: Option<Instant>,
    status: String,
    peak: f32,
    playback: Option<Playback>,
}
impl Default for Shared {
    fn default() -> Self {
        Self {
            notes: [Target::default(); 128],
            demo: [Target::default(); 128],
            volume: 0.5,
            ambience: 0.12,
            enabled: true,
            click: 0,
            accent: false,
            test_until: None,
            status: "Звук ещё не запущен".into(),
            peak: 0.0,
            playback: None,
        }
    }
}
/// A snapshot of the same musical clock used by the lesson. The worker interpolates
/// within audio blocks, never relying on UI frames for note attacks or clicks.
#[derive(Clone)]
pub struct Playback {
    pub sequence: Arc<Sequence>,
    pub epoch: u64,
    pub anchor: Instant,
    pub position: f64,
    pub speed: f64,
    /// Stop at the next waiting chord, even if the UI is delayed.
    pub limit: f64,
    pub metronome: bool,
    pub volume: f32,
}
impl Playback {
    fn position_at(&self, now: Instant) -> f64 {
        self.position + now.saturating_duration_since(self.anchor).as_secs_f64() * self.speed
    }
}
#[derive(Default)]
pub struct AudioService {
    shared: Arc<Mutex<Shared>>,
    stop: Option<Arc<AtomicBool>>,
}
impl AudioService {
    pub fn start(&mut self, config: &Config) -> Result<()> {
        config.validate()?;
        self.stop();
        let shared = Arc::new(Mutex::new(Shared {
            volume: config.volume,
            ambience: config.ambience,
            enabled: config.enabled,
            status: "Подключение звука…".into(),
            ..Shared::default()
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_shared = shared.clone();
        let worker_stop = stop.clone();
        let config = config.clone();
        thread::Builder::new()
            .name("clavunora-audio".into())
            .spawn(move || {
                if let Err(e) = run(&config, &worker_shared, &worker_stop) {
                    worker_shared
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .status = format!(
                        "Звук недоступен: {e:#}. Проверьте настройки и повторите подключение."
                    );
                }
            })?;
        self.shared = shared;
        self.stop = Some(stop);
        Ok(())
    }
    pub fn stop(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop.store(true, Ordering::Relaxed);
        }
        self.shared.lock().unwrap_or_else(|e| e.into_inner()).notes = [Target::default(); 128];
    }
    pub fn status(&self) -> String {
        self.shared
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .status
            .clone()
    }
    pub fn sync(&self, state: &PianoState, retrigger: Option<u8>) {
        let velocities = state.sounding_velocities();
        let mut shared = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        for (pitch, target) in shared.notes.iter_mut().enumerate() {
            let velocity = velocities.get(&(pitch as u8)).copied().unwrap_or(0);
            if velocity > 0 && (target.velocity == 0 || retrigger == Some(pitch as u8)) {
                target.generation = target.generation.wrapping_add(1);
            }
            target.velocity = velocity;
        }
    }
    /// Demonstration is audio only: it never enters PianoState or the scoring engine.
    pub fn preview(&self, notes: &[(u8, u8, u64)]) {
        let mut shared = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        shared.demo = [Target::default(); 128];
        for &(pitch, velocity, generation) in notes {
            let target = &mut shared.demo[usize::from(pitch)];
            if velocity > target.velocity {
                *target = Target {
                    velocity,
                    generation,
                };
            }
        }
    }
    pub fn mix(&self, enabled: bool, volume: f32) {
        let mut shared = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        shared.enabled = enabled;
        shared.volume = volume.clamp(0.0, 1.0);
    }
    pub fn ambience(&self, amount: f32) {
        self.shared
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .ambience = amount.clamp(0.0, 0.35);
    }
    pub fn silence(&self) {
        let mut shared = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        shared.notes = [Target::default(); 128];
        shared.demo = [Target::default(); 128];
        shared.test_until = None;
        shared.playback = None;
    }
    pub fn test_note(&self) {
        let mut shared = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        shared.test_until = Some(Instant::now() + Duration::from_millis(600));
        shared.notes[69].generation = shared.notes[69].generation.wrapping_add(1);
    }
    pub fn click(&self, accent: bool) {
        let mut shared = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        shared.click = shared.click.wrapping_add(1);
        shared.accent = accent;
    }
    pub fn playback(&self, playback: Option<Playback>) {
        self.shared
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .playback = playback;
    }
    #[cfg(test)]
    pub(crate) fn playback_snapshot(&self) -> Option<Playback> {
        self.shared.lock().unwrap().playback.clone()
    }
}
impl Drop for AudioService {
    fn drop(&mut self) {
        self.stop();
    }
}

const TABLE: usize = 2048;
#[derive(Clone, Copy, Default)]
struct Voice {
    phase: [f32; 8],
    partial: [f32; 8],
    unison: f32,
    amplitude: f32,
    age: f32,
    generation: u64,
    velocity: u8,
    noise: u32,
}
struct Builtin {
    voices: [Voice; 128],
    rate: f32,
    sine: Vec<f32>,
    room_left: Vec<f32>,
    room_right: Vec<f32>,
    room_pos: usize,
    ambience: f32,
}
fn wave(table: &[f32], phase: f32) -> f32 {
    let i = phase as usize;
    let fraction = phase - i as f32;
    table[i] * (1.0 - fraction) + table[(i + 1) % TABLE] * fraction
}
impl Builtin {
    fn new(rate: u32) -> Self {
        Self {
            voices: [Voice::default(); 128],
            rate: rate as f32,
            sine: (0..TABLE)
                .map(|i| (std::f32::consts::TAU * i as f32 / TABLE as f32).sin())
                .collect(),
            room_left: vec![0.0; (rate as f32 * 0.031) as usize],
            room_right: vec![0.0; (rate as f32 * 0.043) as usize],
            room_pos: 0,
            ambience: 0.12,
        }
    }
    fn render(&mut self, targets: &[Target; 128], out: &mut [f32]) {
        out.fill(0.0);
        for (pitch, voice) in self.voices.iter_mut().enumerate() {
            let target = targets[pitch];
            let frequency = 440.0 * 2.0_f32.powf((pitch as f32 - 69.0) / 12.0);
            if target.velocity > 0 && (voice.velocity == 0 || target.generation != voice.generation)
            {
                voice.age = 0.0;
                voice.generation = target.generation;
                voice.noise = (pitch as u32 + 1) * 7919;
                let brightness = target.velocity as f32 / 127.0;
                for (h, amplitude) in voice.partial.iter_mut().enumerate() {
                    *amplitude = if h == 0 {
                        1.0
                    } else {
                        0.42 / (h + 1) as f32 * (0.35 + brightness * 0.9)
                    };
                }
            }
            voice.velocity = target.velocity;
            if target.velocity == 0 && voice.amplitude < 0.000001 {
                continue;
            }
            let attack = 1.0 - (-1.0 / (0.003 * self.rate)).exp();
            let release = (-1.0
                / ((0.055 + 0.045 * (1.0 - frequency / 4200.0).clamp(0.0, 1.0)) * self.rate))
                .exp();
            let level = 0.14 * (target.velocity as f32 / 127.0).powf(1.5);
            let pan = ((pitch as f32 - 60.0) / 48.0).clamp(-1.0, 1.0) * 0.28;
            let mut steps = [0.0; 8];
            let mut decays = [0.0; 8];
            for h in 0..8 {
                let number = (h + 1) as f32;
                let partial_frequency = frequency
                    * number
                    * (1.0 + 0.00004 * number * number * (frequency / 440.0).sqrt()).sqrt();
                if partial_frequency < self.rate * 0.45 {
                    steps[h] = partial_frequency / self.rate * TABLE as f32;
                    let seconds = (3.6 / (1.0 + frequency / 1900.0)) / (1.0 + h as f32 * 0.85);
                    decays[h] = (-1.0 / (seconds * self.rate)).exp();
                } else {
                    voice.partial[h] = 0.0;
                }
            }
            for frame in out.as_chunks_mut::<2>().0 {
                if target.velocity > 0 {
                    voice.amplitude += (level - voice.amplitude) * attack;
                } else {
                    voice.amplitude *= release;
                }
                let mut sample = 0.0;
                for h in 0..8 {
                    if steps[h] == 0.0 {
                        continue;
                    }
                    voice.phase[h] = (voice.phase[h] + steps[h]) % TABLE as f32;
                    sample += wave(&self.sine, voice.phase[h]) * voice.partial[h];
                    voice.partial[h] *= decays[h];
                }
                voice.unison =
                    (voice.unison + frequency * 1.0008 / self.rate * TABLE as f32) % TABLE as f32;
                let string = wave(&self.sine, voice.unison) * voice.partial[0] * 0.16;
                voice.noise ^= voice.noise << 13;
                voice.noise ^= voice.noise >> 17;
                voice.noise ^= voice.noise << 5;
                let hammer = if voice.age < 0.025 {
                    (voice.noise as f32 / u32::MAX as f32 * 2.0 - 1.0)
                        * 0.025
                        * (-voice.age * 350.0).exp()
                } else {
                    0.0
                };
                frame[0] += (sample + string + hammer) * voice.amplitude * (1.0 - pan);
                frame[1] += (sample - string * 0.25 + hammer) * voice.amplitude * (1.0 + pan);
                voice.age += 1.0 / self.rate;
            }
        }
        // Short, quiet cross-fed room reflections; click track is mixed afterwards.
        for frame in out.as_chunks_mut::<2>().0 {
            let l = self.room_pos % self.room_left.len();
            let r = self.room_pos % self.room_right.len();
            let left = self.room_left[l];
            let right = self.room_right[r];
            self.room_left[l] = frame[0] + right * 0.18;
            self.room_right[r] = frame[1] + left * 0.18;
            frame[0] += left * self.ambience;
            frame[1] += right * self.ambience;
            self.room_pos = self.room_pos.wrapping_add(1);
        }
    }
}

// All pointers and the library stay on the audio thread. Every symbol uses FluidSynth's C ABI.
// API: https://www.fluidsynth.org/api/group__audio__rendering.html
struct Fluid {
    _lib: libloading::Library,
    settings: *mut c_void,
    synth: *mut c_void,
    delete_settings: unsafe extern "C" fn(*mut c_void),
    delete_synth: unsafe extern "C" fn(*mut c_void),
    note_on: unsafe extern "C" fn(*mut c_void, c_int, c_int, c_int) -> c_int,
    note_off: unsafe extern "C" fn(*mut c_void, c_int, c_int) -> c_int,
    write: unsafe extern "C" fn(
        *mut c_void,
        c_int,
        *mut c_void,
        c_int,
        c_int,
        *mut c_void,
        c_int,
        c_int,
    ) -> c_int,
    previous: [[Target; 128]; 2],
    cc: unsafe extern "C" fn(*mut c_void, c_int, c_int, c_int) -> c_int,
}
// SAFETY: a synth and its library have a single owner. Moving that owner into
// CPAL's callback does not share pointers; all render calls remain serialized.
unsafe impl Send for Fluid {}
impl Fluid {
    fn new(path: &str, rate: u32) -> Result<Self> {
        // SAFETY: symbols below are looked up with signatures from FluidSynth public headers.
        unsafe {
            let names: &[&str] = if cfg!(target_os = "windows") {
                &["libfluidsynth-3.dll", "fluidsynth.dll"]
            } else if cfg!(target_os = "macos") {
                &[
                    "libfluidsynth.3.dylib",
                    "/opt/homebrew/lib/libfluidsynth.3.dylib",
                    "/usr/local/lib/libfluidsynth.3.dylib",
                ]
            } else {
                &["libfluidsynth.so.3"]
            };
            let lib = names
                .iter()
                .find_map(|name| libloading::Library::new(name).ok())
                .context("Установите FluidSynth либо выберите встроенный инструмент")?;
            let new_settings =
                *lib.get::<unsafe extern "C" fn() -> *mut c_void>(b"new_fluid_settings\0")?;
            let new_synth =
                *lib.get::<unsafe extern "C" fn(*mut c_void) -> *mut c_void>(b"new_fluid_synth\0")?;
            let set_num = *lib
                .get::<unsafe extern "C" fn(*mut c_void, *const c_char, f64) -> c_int>(
                    b"fluid_settings_setnum\0",
                )?;
            let load = *lib
                .get::<unsafe extern "C" fn(*mut c_void, *const c_char, c_int) -> c_int>(
                    b"fluid_synth_sfload\0",
                )?;
            let delete_settings = *lib.get(b"delete_fluid_settings\0")?;
            let delete_synth = *lib.get(b"delete_fluid_synth\0")?;
            let note_on = *lib.get(b"fluid_synth_noteon\0")?;
            let note_off = *lib.get(b"fluid_synth_noteoff\0")?;
            let cc = *lib.get(b"fluid_synth_cc\0")?;
            let write = *lib.get(b"fluid_synth_write_float\0")?;
            let path = CString::new(path)?;
            let mut fluid = Self {
                _lib: lib,
                settings: std::ptr::null_mut(),
                synth: std::ptr::null_mut(),
                delete_settings,
                delete_synth,
                note_on,
                note_off,
                write,
                previous: [[Target::default(); 128]; 2],
                cc,
            };
            fluid.settings = new_settings();
            ensure!(!fluid.settings.is_null(), "Не удалось создать FluidSynth");
            ensure!(
                set_num(fluid.settings, c"synth.sample-rate".as_ptr(), rate as f64) >= 0,
                "Не удалось настроить частоту SoundFont"
            );
            set_num(fluid.settings, c"synth.gain".as_ptr(), 0.5);
            fluid.synth = new_synth(fluid.settings);
            ensure!(
                !fluid.synth.is_null(),
                "Не удалось создать синтезатор SoundFont"
            );
            ensure!(
                load(fluid.synth, path.as_ptr(), 1) >= 0,
                "Не удалось загрузить SoundFont"
            );
            Ok(fluid)
        }
    }
    fn render(
        &mut self,
        targets: &[Target; 128],
        backing: &[Target; 128],
        volume: f32,
        out: &mut [f32],
    ) -> Result<()> {
        // SAFETY: synth is live until Drop; the two sources use independent MIDI channels.
        unsafe {
            (self.cc)(self.synth, 1, 7, (volume.clamp(0.0, 1.0) * 127.0) as c_int);
            for (channel, targets) in [targets, backing].into_iter().enumerate() {
                for (pitch, target) in targets.iter().enumerate() {
                    let old = self.previous[channel][pitch];
                    let retrigger = target.velocity > 0
                        && (old.velocity == 0 || old.generation != target.generation);
                    if old.velocity > 0 && (target.velocity == 0 || retrigger) {
                        (self.note_off)(self.synth, channel as c_int, pitch as c_int);
                    }
                    if retrigger {
                        (self.note_on)(
                            self.synth,
                            channel as c_int,
                            pitch as c_int,
                            target.velocity as c_int,
                        );
                    }
                }
                self.previous[channel] = *targets;
            }
            let ptr = out.as_mut_ptr().cast();
            ensure!(
                (self.write)(self.synth, (out.len() / 2) as c_int, ptr, 0, 2, ptr, 1, 2) >= 0,
                "Ошибка синтеза SoundFont"
            );
        }
        Ok(())
    }
}
impl Drop for Fluid {
    fn drop(&mut self) {
        // SAFETY: destructor runs on the owning thread before unloading the library.
        unsafe {
            if !self.synth.is_null() {
                (self.delete_synth)(self.synth);
            }
            if !self.settings.is_null() {
                (self.delete_settings)(self.settings);
            }
        }
    }
}
struct Renderer {
    builtin: Builtin,
    backing: Builtin,
    cursor: Cursor,
    playback_key: Option<(usize, u64)>,
    backing_volume: f32,
    fluid: Option<Fluid>,
    click_seen: u64,
    click_remaining: u32,
    click_phase: f32,
    accent: bool,
}
impl Renderer {
    fn new(soundfont: &str, rate: u32) -> Result<Self> {
        Ok(Self {
            builtin: Builtin::new(rate),
            backing: Builtin::new(rate),
            cursor: Cursor::default(),
            playback_key: None,
            backing_volume: 0.0,
            fluid: if soundfont.is_empty() {
                None
            } else {
                Some(Fluid::new(soundfont, rate)?)
            },
            click_seen: 0,
            click_remaining: 0,
            click_phase: 0.0,
            accent: false,
        })
    }
    #[cfg(test)]
    fn render(&mut self, shared: &Shared, out: &mut [f32]) -> Result<()> {
        self.render_at(shared, out, Instant::now())
    }
    fn render_at(&mut self, shared: &Shared, out: &mut [f32], now: Instant) -> Result<()> {
        let mut targets = shared.notes;
        for (target, demo) in targets.iter_mut().zip(shared.demo) {
            target.velocity = target.velocity.max(demo.velocity);
            if demo.velocity > 0 {
                target.generation ^= demo.generation.rotate_left(32);
            }
        }
        if shared.test_until.is_some_and(|until| until > now) {
            targets[69].velocity = 90;
        }
        if !shared.enabled {
            targets = [Target::default(); 128];
        }
        let key = shared
            .playback
            .as_ref()
            .map(|p| (Arc::as_ptr(&p.sequence) as usize, p.epoch));
        if key != self.playback_key {
            self.cursor = Cursor::default();
            self.playback_key = key;
            self.click_remaining = 0;
        }
        if shared.click != self.click_seen {
            self.click_seen = shared.click;
            self.start_click(shared.accent);
        }
        // 32-frame scheduling bounds note/click quantization to <0.7ms at 48kHz.
        for (block, out) in out.chunks_mut(64).enumerate() {
            let mut backing = [Target::default(); 128];
            // Keep the last channel gain while NoteOff releases the instrument.
            // Setting gain to zero on pause would cut an arbitrary waveform and click.
            let mut gain = self.backing_volume;
            if let Some(p) = &shared.playback {
                let position = p.position_at(now)
                    + block as f64 * 32.0 / f64::from(self.builtin.rate) * p.speed;
                if position < p.limit {
                    let (voices, click) = self.cursor.advance(&p.sequence, position);
                    if shared.enabled && p.volume > 0.0 {
                        for (target, voice) in backing.iter_mut().zip(voices) {
                            *target = Target {
                                velocity: voice.velocity,
                                generation: voice.generation ^ p.epoch.rotate_left(32),
                            };
                        }
                    }
                    gain = p.volume;
                    if p.metronome
                        && let Some(accent) = click
                    {
                        self.start_click(accent);
                    }
                } else {
                    self.click_remaining = 0;
                }
            }
            self.backing_volume = gain;
            if let Some(fluid) = &mut self.fluid {
                fluid.render(&targets, &backing, gain, out)?;
            } else {
                self.builtin.ambience = shared.ambience;
                self.backing.ambience = shared.ambience;
                self.builtin.render(&targets, out);
                let mut buffer = [0.0; 64];
                self.backing.render(&backing, &mut buffer[..out.len()]);
                for (sample, extra) in out.iter_mut().zip(buffer) {
                    *sample += extra * gain;
                }
            }
            for frame in out.as_chunks_mut::<2>().0 {
                if self.click_remaining > 0 {
                    self.click_phase += std::f32::consts::TAU
                        * if self.accent { 1600.0 } else { 1000.0 }
                        / self.builtin.rate;
                    let click = self.click_phase.sin()
                        * 0.12
                        * (self.click_remaining as f32 / (self.builtin.rate * 0.025));
                    for sample in frame.iter_mut() {
                        *sample += click;
                    }
                    self.click_remaining -= 1;
                }
                for sample in frame {
                    *sample = if shared.enabled {
                        (*sample * shared.volume).tanh() * 0.95
                    } else {
                        0.0
                    };
                }
            }
        }
        Ok(())
    }
    fn start_click(&mut self, accent: bool) {
        self.accent = accent;
        self.click_phase = 0.0;
        self.click_remaining = (self.builtin.rate * 0.025) as u32;
    }
}

#[cfg(target_os = "linux")]
fn run(config: &Config, shared: &Arc<Mutex<Shared>>, stop: &AtomicBool) -> Result<()> {
    let pcm = PCM::new(&config.device, Direction::Playback, true)
        .context("Не удалось открыть звуковое устройство")?;
    let rate;
    {
        let params = HwParams::any(&pcm)?;
        params.set_channels(2)?;
        params.set_access(Access::RWInterleaved)?;
        params.set_format(Format::float())?;
        rate = params.set_rate_near(RATE, ValueOr::Nearest)?;
        params.set_period_size_near(FRAMES as i64, ValueOr::Nearest)?;
        params.set_buffer_time_near(30_000, ValueOr::Nearest)?;
        pcm.hw_params(&params)?;
        let sw = pcm.sw_params_current()?;
        sw.set_start_threshold(1)?;
        pcm.sw_params(&sw)?;
    }
    let io = pcm.io_f32()?;
    let mut renderer = Renderer::new(&config.soundfont, rate)?;
    shared.lock().unwrap_or_else(|e| e.into_inner()).status = format!(
        "Подключено: {} · {} · {rate} Гц",
        config.device,
        if config.soundfont.is_empty() {
            "встроенный инструмент"
        } else {
            "SoundFont"
        }
    );
    let mut buffer = [0.0; FRAMES * 2];
    let mut render_at = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        let snapshot = shared.lock().unwrap_or_else(|e| e.into_inner()).clone();
        render_at = render_at.max(Instant::now());
        renderer.render_at(&snapshot, &mut buffer, render_at)?;
        render_at += Duration::from_secs_f64(FRAMES as f64 / f64::from(rate));
        let peak = buffer.iter().map(|v| v.abs()).fold(0.0_f32, f32::max);
        {
            let mut state = shared.lock().unwrap_or_else(|e| e.into_inner());
            state.peak = state.peak.max(peak);
        }
        let mut written = 0;
        let mut recoveries = 0;
        while written < FRAMES && !stop.load(Ordering::Relaxed) {
            match io.writei(&buffer[written * 2..]) {
                Ok(0) => {
                    pcm.wait(Some(20))?;
                }
                Ok(n) => {
                    written += n;
                    recoveries = 0;
                }
                Err(e) if e.errno() == 11 => {
                    pcm.wait(Some(20))?;
                } // EAGAIN: nonblocking PCM full
                Err(e) => {
                    recoveries += 1;
                    ensure!(recoveries <= 3, "Звуковое устройство не принимает данные");
                    pcm.try_recover(e, true)?;
                }
            }
        }
    }
    pcm.drop()?;
    Ok(())
}

#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "android",
    test
))]
#[path = "audio_desktop.rs"]
mod desktop;
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "android"))]
use desktop::run;

/// Short explicit diagnostic through the real ALSA device; does not open a library or save audio.
pub fn check_output(config: &Config) -> Result<()> {
    let mut service = AudioService::default();
    service.start(config)?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let status = service.status();
        ensure!(!status.starts_with("Звук недоступен"), "{status}");
        if status.starts_with("Подключено") {
            break;
        }
        ensure!(Instant::now() < deadline, "Истекло время подключения звука");
        thread::sleep(Duration::from_millis(10));
    }
    service.test_note();
    thread::sleep(Duration::from_millis(850));
    service.click(true);
    thread::sleep(Duration::from_millis(100));
    let status = service.status();
    ensure!(status.starts_with("Подключено"), "{status}");
    let peak = service
        .shared
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .peak;
    ensure!(peak > 0.001, "Синтезатор не выдал звуковой сигнал");
    service.stop();
    println!(
        "{}",
        serde_json::json!({"ok":true,"peak":peak,"audio_output":config.device,"instrument":if config.soundfont.is_empty(){"builtin"}else{"SoundFont"},"status":status})
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn energy(samples: &[f32]) -> f32 {
        samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32
    }
    #[test]
    fn scheduled_clicks_follow_tempo_and_speed_without_ui_updates() {
        use clavunora_core::{lesson::Plan, score::Song};
        let song=Song::from_json(r#"{"title":"tempo","bpm":120,"time_signature":[4,4],"tempo_map":[[1,60]],"notes":[{"pitch":60,"start":0,"duration":3}]}"#).unwrap();
        let now = Instant::now();
        let shared = Shared {
            playback: Some(Playback {
                sequence: Arc::new(Sequence::accompaniment(&song, &Plan::default())),
                epoch: 1,
                anchor: now,
                position: 0.0,
                speed: 0.5,
                limit: 3.0,
                metronome: true,
                volume: 0.0,
            }),
            ..Default::default()
        };
        let mut renderer = Renderer::new("", RATE).unwrap();
        let mut out = vec![0.0; RATE as usize * 2 * 5];
        renderer.render_at(&shared, &mut out, now).unwrap();
        let window = |time: f64| {
            let start = (time * f64::from(RATE)) as usize * 2;
            energy(&out[start..start + 960])
        };
        for time in [0.005, 1.005, 2.005, 4.005] {
            assert!(window(time) > 0.00001, "missing click at {time}");
        }
        for time in [0.5, 1.5, 2.5, 3.0, 4.5] {
            assert_eq!(window(time), 0.0, "off-grid click at {time}");
        }
        assert!(!renderer.accent); // Beat three after the tempo change.
    }
    #[test]
    fn stopping_accompaniment_releases_the_waveform_instead_of_cutting_gain() {
        use clavunora_core::{lesson::Plan, score::Song};
        let song=Song::from_json(r#"{"title":"release","notes":[{"pitch":60,"hand":"R","start":0,"duration":2},{"pitch":48,"hand":"L","start":0,"duration":2}]}"#).unwrap();
        let now = Instant::now();
        let mut shared = Shared {
            ambience: 0.0,
            playback: Some(Playback {
                sequence: Arc::new(Sequence::accompaniment(
                    &song,
                    &Plan {
                        hand: "R".into(),
                        ..Default::default()
                    },
                )),
                epoch: 1,
                anchor: now,
                position: 0.0,
                speed: 1.0,
                limit: 2.0,
                metronome: false,
                volume: 0.7,
            }),
            ..Default::default()
        };
        let mut renderer = Renderer::new("", RATE).unwrap();
        let mut out = vec![0.0; 960];
        renderer.render_at(&shared, &mut out, now).unwrap();
        assert!(energy(&out) > 0.00001);
        shared.playback = None;
        let mut tail = vec![0.0; RATE as usize * 2 * 2];
        renderer
            .render_at(&shared, &mut tail, now + Duration::from_millis(10))
            .unwrap();
        assert!(
            energy(&tail[..960]) > 0.000001,
            "release must not abruptly zero the waveform"
        );
        assert!(
            energy(&tail[tail.len() - 960..]) < 1e-12,
            "released backing must not stick"
        );
    }
    #[test]
    fn waiting_limit_pause_and_new_epoch_stop_backing_without_touching_live_notes() {
        use clavunora_core::{lesson::Plan, score::Song};
        let song=Song::from_json(r#"{"title":"duet","notes":[{"pitch":60,"hand":"R","start":0,"duration":2},{"pitch":60,"hand":"L","start":0,"duration":2}]}"#).unwrap();
        let now = Instant::now();
        let sequence = Arc::new(Sequence::accompaniment(
            &song,
            &Plan {
                hand: "R".into(),
                ..Default::default()
            },
        ));
        let mut shared = Shared {
            playback: Some(Playback {
                sequence,
                epoch: 1,
                anchor: now,
                position: 0.0,
                speed: 1.0,
                limit: 0.05,
                metronome: true,
                volume: 0.5,
            }),
            ..Default::default()
        };
        shared.notes[60] = Target {
            velocity: 80,
            generation: 8,
        };
        let mut renderer = Renderer::new("", RATE).unwrap();
        let mut out = vec![0.0; 960];
        renderer.render_at(&shared, &mut out, now).unwrap();
        assert!(renderer.backing.voices[60].velocity > 0);
        assert_eq!(renderer.builtin.voices[60].generation, 8);
        renderer
            .render_at(&shared, &mut out, now + Duration::from_millis(60))
            .unwrap();
        assert_eq!(renderer.backing.voices[60].velocity, 0);
        assert_eq!(renderer.click_remaining, 0);
        assert_eq!(renderer.builtin.voices[60].velocity, 80);
        shared.playback = None;
        renderer
            .render_at(&shared, &mut out, now + Duration::from_millis(70))
            .unwrap();
        assert_eq!(renderer.builtin.voices[60].generation, 8);
        let mut p = Playback {
            sequence: Arc::new(Sequence::accompaniment(
                &song,
                &Plan {
                    hand: "R".into(),
                    ..Default::default()
                },
            )),
            epoch: 2,
            anchor: now,
            position: 0.0,
            speed: 1.0,
            limit: 2.0,
            metronome: true,
            volume: 0.5,
        };
        shared.playback = Some(p.clone());
        renderer.render_at(&shared, &mut out, now).unwrap();
        let generation = renderer.backing.voices[60].generation;
        p.epoch = 3;
        shared.playback = Some(p);
        renderer.render_at(&shared, &mut out, now).unwrap();
        assert_ne!(renderer.backing.voices[60].generation, generation);
        shared.enabled = false;
        renderer.render_at(&shared, &mut out, now).unwrap();
        assert_eq!(energy(&out), 0.0);
    }
    #[test]
    fn signal_velocity_pitch_retrigger_release_and_mute() {
        let mut r = Renderer::new("", RATE).unwrap();
        let mut s = Shared::default();
        let mut out = vec![0.0; 4800 * 2];
        r.render(&s, &mut out).unwrap();
        assert_eq!(energy(&out), 0.0);
        s.notes[69] = Target {
            velocity: 127,
            generation: 1,
        };
        r.render(&s, &mut out).unwrap();
        let loud = energy(&out);
        assert!(loud > 0.0001);
        let crossings = out
            .as_chunks::<2>()
            .0
            .iter()
            .map(|f| f[0])
            .collect::<Vec<_>>()
            .windows(2)
            .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
            .count();
        assert!((43..=45).contains(&crossings));
        for _ in 0..15 {
            r.render(&s, &mut out).unwrap();
        }
        let decayed = energy(&out);
        s.notes[69].generation += 1;
        r.render(&s, &mut out).unwrap();
        assert!(
            energy(&out) > decayed * 2.0,
            "A repeated held pitch must have a new attack"
        );
        s.notes[69].velocity = 0;
        for _ in 0..12 {
            r.render(&s, &mut out).unwrap();
        }
        assert!(energy(&out) < 1e-12);
        s.notes[69] = Target {
            velocity: 30,
            generation: 2,
        };
        r.render(&s, &mut out).unwrap();
        assert!(energy(&out) < loud / 4.0);
        s.enabled = false;
        r.render(&s, &mut out).unwrap();
        assert_eq!(energy(&out), 0.0);
    }
    #[test]
    fn polyphony_is_finite_and_limited_and_click_ends() {
        let mut r = Renderer::new("", RATE).unwrap();
        let mut s = Shared::default();
        let mut out = vec![0.0; 48000 * 2];
        s.notes.fill(Target {
            velocity: 127,
            generation: 1,
        });
        r.render(&s, &mut out).unwrap();
        assert!(out.iter().all(|v| v.is_finite() && v.abs() <= 0.95));
        let mut r = Renderer::new("", RATE).unwrap();
        s.notes.fill(Target::default());
        s.click = 1;
        r.render(&s, &mut out).unwrap();
        assert!(energy(&out) > 0.0);
        assert!(out[2400..].iter().all(|v| *v == 0.0));
    }
    #[test]
    fn new_timbre_is_stereo_and_velocity_changes_harmonic_balance() {
        fn spectrum(velocity: u8) -> (f64, f64, f32) {
            let mut synth = Builtin::new(RATE);
            synth.ambience = 0.0;
            let mut targets = [Target::default(); 128];
            targets[69] = Target {
                velocity,
                generation: 1,
            };
            let mut out = vec![0.0; 9600 * 2];
            synth.render(&targets, &mut out);
            let power = |hz: f64| {
                let mut re = 0.0;
                let mut im = 0.0;
                for (i, frame) in out.as_chunks::<2>().0.iter().enumerate().skip(1000) {
                    let angle = std::f64::consts::TAU * hz * i as f64 / f64::from(RATE);
                    re += f64::from(frame[0]) * angle.cos();
                    im += f64::from(frame[0]) * angle.sin();
                }
                re * re + im * im
            };
            (
                power(440.0),
                power(880.0),
                out.as_chunks::<2>()
                    .0
                    .iter()
                    .map(|f| (f[0] - f[1]).powi(2))
                    .sum(),
            )
        }
        let soft = spectrum(35);
        let loud = spectrum(120);
        assert!(loud.0 > soft.0 * 4.0);
        assert!(loud.1 / loud.0 > soft.1 / soft.0 * 1.4);
        assert!(loud.2 > 0.001);
        let old: Config = serde_json::from_str("{\"volume\":0.4}").unwrap();
        assert_eq!(old.ambience, 0.12);
    }
    #[test]
    fn state_sync_preserves_sustain_and_same_pitch_from_other_source() {
        use clavunora_core::midi::MidiEvent;
        let audio = AudioService::default();
        let mut state = PianoState::default();
        for (bytes, source) in [
            ([0x90, 60, 110], "a"),
            ([0x90, 60, 50], "b"),
            ([0xb0, 64, 127], "a"),
            ([0x80, 60, 0], "a"),
        ] {
            state.apply(&MidiEvent::parse(&bytes, source).unwrap());
            audio.sync(&state, None);
        }
        assert_eq!(audio.shared.lock().unwrap().notes[60].velocity, 110);
        state.clear_source("a");
        audio.sync(&state, None);
        assert_eq!(audio.shared.lock().unwrap().notes[60].velocity, 50);
        state.panic();
        audio.sync(&state, None);
        assert_eq!(audio.shared.lock().unwrap().notes[60].velocity, 0);
    }
    #[test]
    fn invalid_settings_and_missing_soundfont_do_not_panic() {
        assert!(
            Config {
                volume: f32::NAN,
                ..Config::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            Config {
                soundfont: "/nonexistent/font.sf2".into(),
                ..Config::default()
            }
            .validate()
            .is_err()
        );
        assert!(Fluid::new("/nonexistent/font.sf2", RATE).is_err());
    }
}
