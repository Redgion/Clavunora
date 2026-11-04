// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
use crate::aero::{self, Aero, Icon};
use crate::keyboard_view::{self, Config as KeyboardConfig};
use crate::{
    audio::{AudioService, Config as AudioConfig},
    midi_input::MidiService,
};
use anyhow::Result;
use clavunora_core::{
    catalog,
    lesson::{Plan, RepeatProgress, SmartConfig},
    midi::{Message, MidiEvent, PianoState},
    notation::{
        Config as NotationConfig, Language as NoteLanguage, Page as SheetPage, View as NotationView,
    },
    performance::Options,
    playback::{Config as LessonAudioConfig, Sequence},
    score::note_name,
    session::{Mode, Report, Session},
    store::{Library, PendingAttempt, Record},
    transport::Transport,
};
use eframe::egui::{self, Color32, Pos2, Rect, RichText, Sense, Vec2};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Page {
    Home,
    Library,
    Course,
    Practice,
    FreePlay,
    Achievements,
    Progress,
    Settings,
}
impl Page {
    pub const ALL: [Self; 8] = [
        Self::Home,
        Self::Library,
        Self::Course,
        Self::Practice,
        Self::FreePlay,
        Self::Achievements,
        Self::Progress,
        Self::Settings,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Self::Home => "Главная",
            Self::Library => "Библиотека",
            Self::Course => "Обучение",
            Self::Practice => "Занятие",
            Self::FreePlay => "Свободная игра",
            Self::Achievements => "Достижения",
            Self::Progress => "Прогресс",
            Self::Settings => "Настройки",
        }
    }
}

struct SaveItem {
    finished_at: std::time::SystemTime,
    song_id: String,
    report: Report,
    pending: Option<PendingAttempt>,
}

#[derive(Clone, Copy)]
struct Listening {
    started: Instant,
    index: usize,
    speed: f64,
    start: f64,
    end: f64,
    whole: bool,
}
impl Listening {
    fn position(self) -> f64 {
        self.start + self.started.elapsed().as_secs_f64() * self.speed
    }
}

pub struct PreviewApp {
    store: Library,
    aero: Option<Aero>,
    pub records: Vec<Record>,
    pub page: Page,
    folder: Option<String>,
    reset_page_scroll: bool,
    search: String,
    import_path: String,
    editor: Option<crate::score_editor::Editor>,
    backups: crate::backups::Controls,
    import_preview: Option<crate::midi_import::Preview>,
    import_receiver: Option<std::sync::mpsc::Receiver<crate::midi_import::Reply>>,
    pub data_dir: PathBuf,
    pub dark: bool,
    plain: bool,
    error: String,
    status: String,
    selected: Option<usize>,
    plan: Plan,
    plan_draft: Plan,
    smart_config: SmartConfig,
    smart_draft: SmartConfig,
    repeat_progress: RepeatProgress,
    practice_song: Option<clavunora_core::score::Song>,
    show_fingers: bool,
    state: PianoState,
    transport: Transport,
    session: Option<Session>,
    running: bool,
    performance_view: bool,
    fullscreen_restore: Option<bool>,
    fullscreen_focus_until: Option<Instant>,
    active_since: f64,
    last_report: Option<Report>,
    save_queue: Vec<SaveItem>,
    selected_history: Option<String>,
    error_page: usize,
    repair_notice: Option<String>,
    tolerance: f64,
    latency: f64,
    close_warning: bool,
    allow_close: bool,
    wait_mode: bool,
    waiting: bool,
    started: Instant,
    mouse_note: Option<u8>,
    touch_keyboard: crate::touch_keyboard::TouchKeyboard,
    computer_notes: BTreeSet<u8>,
    ports: Vec<crate::midi_input::Port>,
    port_index: usize,
    midi: MidiService,
    audio: AudioService,
    audio_config: AudioConfig,
    keyboard_config: KeyboardConfig,
    audio_initialized: bool,
    audio_device_draft: String,
    soundfont_draft: String,
    demo: Option<Listening>,
    demo_sequence: Option<std::sync::Arc<Sequence>>,
    notation: NotationConfig,
    sheet_page: Option<usize>,
    metronome: bool,
    metronome_bpm: f64,
    metronome_beats: u32,
    metronome_anchor: Instant,
    lesson_audio: LessonAudioConfig,
    support_sequence: Option<std::sync::Arc<Sequence>>,
    standalone_sequence: std::sync::Arc<Sequence>,
    playback_epoch: u64,
    native_smoke: bool,
    capture: Option<PathBuf>,
    capture_requested: bool,
    capture_support: bool,
    native_frame_reported: bool,
    pub capture_succeeded: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl PreviewApp {
    pub fn new(mut store: Library, data_dir: PathBuf, native_smoke: bool) -> Result<Self> {
        store.seed()?;
        let records = store.songs()?;
        let dark = store
            .preference("rust-preview:dark")?
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        store.achievement_state()?;
        let audio_config: AudioConfig = store
            .preference("rust:audio")?
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        let notation = store
            .preference("rust:notation")?
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        let lesson_audio = store
            .preference("rust:lesson-audio")?
            .and_then(|v| serde_json::from_value(v).ok())
            .filter(LessonAudioConfig::valid)
            .unwrap_or_default();
        let keyboard_config: KeyboardConfig = store
            .preference("rust:keyboard-view")?
            .and_then(|v| serde_json::from_value(v).ok())
            .filter(KeyboardConfig::valid)
            .unwrap_or_default();
        let store_show_fingers = store
            .preference("rust:show-fingers")?
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let plain = store
            .preference("rust:plain-theme")?
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        Ok(Self {
            store,
            aero: None,
            records,
            page: Page::Home,
            folder: None,
            reset_page_scroll: false,
            search: String::new(),
            import_path: String::new(),
            editor: None,
            backups: Default::default(),
            import_preview: None,
            import_receiver: None,
            data_dir,
            dark,
            plain,
            error: String::new(),
            status: "Добро пожаловать в Clavunora".into(),
            selected: None,
            plan: Plan::default(),
            plan_draft: Plan::default(),
            smart_config: SmartConfig::default(),
            smart_draft: SmartConfig::default(),
            repeat_progress: RepeatProgress::default(),
            practice_song: None,
            show_fingers: store_show_fingers,
            state: PianoState::default(),
            transport: Transport::new(0.0)?,
            session: None,
            running: false,
            performance_view: false,
            fullscreen_restore: None,
            fullscreen_focus_until: None,
            active_since: 0.0,
            last_report: None,
            save_queue: vec![],
            selected_history: None,
            error_page: 0,
            repair_notice: None,
            tolerance: 0.15,
            latency: 0.0,
            close_warning: false,
            allow_close: false,
            wait_mode: true,
            waiting: false,
            started: Instant::now(),
            mouse_note: None,
            touch_keyboard: Default::default(),
            computer_notes: BTreeSet::new(),
            ports: vec![],
            port_index: 0,
            midi: MidiService::default(),
            audio: AudioService::default(),
            audio_device_draft: audio_config.device.clone(),
            soundfont_draft: audio_config.soundfont.clone(),
            audio_config,
            keyboard_config,
            audio_initialized: false,
            demo: None,
            demo_sequence: None,
            notation,
            sheet_page: None,
            metronome: false,
            metronome_bpm: 96.0,
            metronome_beats: 4,
            metronome_anchor: Instant::now(),
            lesson_audio,
            support_sequence: None,
            standalone_sequence: std::sync::Arc::new(Sequence::standalone(96.0, 4)),
            playback_epoch: 0,
            native_smoke,
            capture: None,
            capture_requested: false,
            capture_support: false,
            native_frame_reported: false,
            capture_succeeded: Default::default(),
        })
    }
    pub fn set_capture(&mut self, path: PathBuf) {
        self.capture = Some(path);
    }
    pub fn capture_page(&mut self, name: &str, dark: bool) -> Result<()> {
        self.dark = dark;
        self.page = match name {
            "home" | "plain" => Page::Home,
            "library" | "library-songs" | "import-preview" | "xml-preview" | "editor" => {
                Page::Library
            }
            "course" => Page::Course,
            "practice" | "lesson-tools" | "support-tools" | "midi-lost" | "performance"
            | "error-fragment" | "sheet" | "sheet-english" | "sheet-both" | "sheet-performance"
            | "listening" => Page::Practice,
            "freeplay" => Page::FreePlay,
            "achievements" => Page::Achievements,
            "progress" | "error-review" => Page::Progress,
            "settings" | "backup-preview" => Page::Settings,
            _ => anyhow::bail!("Неизвестный экран"),
        };
        if name == "plain" {
            self.plain = true;
            self.dark = false;
        }
        if name == "import-preview" {
            let bytes = include_bytes!("../../../../assets/happy_birthday.mid").to_vec();
            self.import_preview = Some(crate::midi_import::Preview {
                parsed: clavunora_core::midi_file::parse(&bytes, "Пример MIDI")?,
                bytes,
                name: "happy_birthday.mid".into(),
                kind: "midi",
            });
        }
        if name == "xml-preview" {
            let bytes =
                include_bytes!("../../../../tests/fixtures/musicxml-import.musicxml").to_vec();
            self.import_preview = Some(crate::midi_import::Preview {
                parsed: clavunora_core::musicxml::parse(&bytes, "MusicXML")?,
                bytes,
                name: "Study.musicxml".into(),
                kind: "musicxml",
            });
        }
        if name == "editor" {
            let index = self
                .records
                .iter()
                .position(|r| r.id == "course-5")
                .unwrap_or(0);
            self.open_editor(index);
        }
        if name == "library-songs" {
            self.folder = Some("all".into());
        }
        if self.page == Page::Practice {
            let index = if matches!(
                name,
                "lesson-tools"
                    | "support-tools"
                    | "sheet"
                    | "sheet-english"
                    | "sheet-both"
                    | "sheet-performance"
                    | "listening"
            ) {
                self.records
                    .iter()
                    .position(|r| r.id == "course-5")
                    .unwrap_or(0)
            } else {
                0
            };
            self.open_song(index);
        }
        if name == "error-review" || name == "error-fragment" {
            self.verify_rhythm()?;
            let id = self
                .store
                .attempts()?
                .first()
                .map(|r| r.id.clone())
                .ok_or_else(|| anyhow::anyhow!("Нет тестового результата"))?;
            self.selected_history = Some(id.clone());
            if name == "error-review" {
                self.page = Page::Progress;
            } else {
                self.practice_error(&id, 0)?;
            }
        }
        if name == "support-tools" {
            self.capture_support = true;
            let index = self.selected.unwrap();
            self.plan_draft.hand = "R".into();
            self.apply_lesson_plan(index)?;
            self.lesson_audio.metronome = true;
            self.lesson_audio.accompaniment = true;
        }
        if matches!(
            name,
            "sheet" | "sheet-english" | "sheet-both" | "sheet-performance" | "listening"
        ) {
            self.notation = NotationConfig {
                view: if matches!(name, "sheet-both" | "sheet-performance") {
                    NotationView::Both
                } else {
                    NotationView::Sheet
                },
                language: if name == "sheet-english" {
                    NoteLanguage::English
                } else {
                    NoteLanguage::Russian
                },
            };
        }
        if name == "listening" {
            self.start_listening(true);
        }
        if matches!(name, "performance" | "sheet-performance") {
            self.start_practice();
        }
        if name == "backup-preview" {
            let dir = tempfile::tempdir()?;
            let path = dir.path().join("Пример.clavunora-backup");
            clavunora_core::backup::create(&self.store.database_path()?, &path)?;
            self.backups.preview = Some(clavunora_core::backup::preview(&path)?);
        }
        if name == "midi-lost" {
            self.midi = MidiService::capture_disconnected();
            self.midi_change(crate::midi_input::Change::Lost);
        }
        Ok(())
    }
    /// Runs only inside --self-test's temporary library; uses the real UI lifecycle methods.
    pub fn verify_scoring(&mut self) -> Result<(serde_json::Value, serde_json::Value)> {
        let index = self
            .records
            .iter()
            .position(|r| r.id == "course-1")
            .ok_or_else(|| anyhow::anyhow!("Нет тестового урока"))?;
        self.open_song(index);
        self.start_practice();
        let notes = self.records[index].song.notes.clone();
        for n in notes {
            let now = self.now();
            self.transport.seek(n.start, now)?;
            self.advance_practice(now);
            self.feed_at(
                MidiEvent {
                    source: "self-test".into(),
                    channel: 0,
                    message: Message::NoteOn {
                        pitch: n.pitch,
                        velocity: 90,
                    },
                },
                now,
            );
            self.feed_at(
                MidiEvent {
                    source: "self-test".into(),
                    channel: 0,
                    message: Message::NoteOff { pitch: n.pitch },
                },
                now,
            );
        }
        let now = self.now();
        self.transport.seek(self.transport.duration, now)?;
        self.advance_practice(now);
        let rows = self.store.attempts()?;
        anyhow::ensure!(
            rows.len() == 1
                && rows[0].report["complete"] == true
                && rows[0].report["accuracy"] == 100.0,
            "Не сохранён полный результат"
        );
        self.pause_practice();
        self.finish_current(true);
        self.retry_saves();
        anyhow::ensure!(
            self.store.attempts()?.len() == 1,
            "Повторное сохранение результата"
        );
        Ok((
            rows[0].report.clone(),
            serde_json::to_value(&self.records[index].song)?,
        ))
    }
    pub fn verify_rhythm(&mut self) -> Result<serde_json::Value> {
        let index = self
            .records
            .iter()
            .position(|r| r.id == "course-1")
            .ok_or_else(|| anyhow::anyhow!("Нет тестового урока"))?;
        self.open_song(index);
        self.wait_mode = false;
        self.start_practice();
        let notes = self.records[index].song.notes.clone();
        for n in notes.iter().take(2) {
            for (bytes, position) in [
                ([0x90, n.pitch, 90], n.start + 0.25),
                ([0x80, n.pitch, 0], n.end() - 0.4),
            ] {
                let now = self.now();
                self.transport.seek(position, now)?;
                self.feed_at(MidiEvent::parse(&bytes, "self-test").unwrap(), now);
            }
        }
        let now = self.now();
        self.transport.seek(self.transport.duration, now)?;
        self.advance_practice(now);
        let rows = self.store.attempts()?;
        let report = &rows[0].report;
        anyhow::ensure!(
            report["mode"] == "Ритм"
                && report["complete"] == true
                && report["hit"] == 2
                && report["accuracy"] == 50.0,
            "Неверная оценка теста ритма"
        );
        self.selected_history = Some(rows[0].id.clone());
        Ok(report.clone())
    }
    /// Self-test only: a real saved single-hand attempt using the same selection lifecycle as UI.
    pub fn verify_selected_lesson(&mut self) -> Result<(serde_json::Value, serde_json::Value)> {
        let index = self
            .records
            .iter()
            .position(|r| r.id == "course-5")
            .ok_or_else(|| anyhow::anyhow!("Нет урока для двух рук"))?;
        self.open_song(index);
        self.plan_draft.hand = "L".into();
        self.apply_lesson_plan(index)?;
        self.start_practice();
        for n in self.practice_song.as_ref().unwrap().notes.clone() {
            let now = self.now();
            self.transport.seek(n.start, now)?;
            self.advance_practice(now);
            self.feed(MidiEvent::parse(&[0x90, n.pitch, 90], "midi").unwrap());
            self.feed(MidiEvent::parse(&[0x80, n.pitch, 0], "midi").unwrap());
        }
        let now = self.now();
        self.transport.seek(self.transport.duration, now)?;
        self.advance_practice(now);
        let report = self
            .last_report
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Нет результата выбранной руки"))?;
        anyhow::ensure!(
            report.outcome.complete && !report.full_song && report.hand == "L",
            "Неверный статус выбранной руки"
        );
        Ok((
            serde_json::to_value(report)?,
            serde_json::to_value(&self.records[index].song)?,
        ))
    }
    pub fn inspect(&self) -> Result<serde_json::Value> {
        Ok(
            serde_json::json!({"songs":self.records.len(),"favorites":self.records.iter().filter(|r|r.favorite).count(),"attempts":self.store.attempts()?.len(),"recordings":self.store.recording_count()?,"achievements":self.store.preference("achievements")?}),
        )
    }
    fn now(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }
    fn navigate(&mut self, page: Page) {
        if page != self.page {
            self.performance_view = false;
            self.stop_demo();
            if self.page == Page::Practice {
                self.pause_practice();
            }
            let _ = self.transport.pause(self.now());
            self.state.clear_source("computer");
            self.state.clear_source("mouse");
            self.computer_notes.clear();
            self.mouse_note = None;
            self.audio.sync(&self.state, None);
            self.waiting = false;
        }
        if self.page != page {
            self.reset_page_scroll = true;
        }
        self.page = page;
        if page == Page::Progress && self.selected_history.is_none() {
            match self.store.attempts() {
                Ok(rows) => self.selected_history = rows.first().map(|r| r.id.clone()),
                Err(e) => self.error = e.to_string(),
            }
        }
    }
    pub fn open_for_test(&mut self, index: usize) {
        self.open_song(index);
    }
    fn open_song(&mut self, index: usize) {
        self.repair_notice = None;
        self.sheet_page = None;
        self.reset_page_scroll = true;
        self.performance_view = false;
        self.pause_practice();
        self.navigate(Page::Practice);
        self.selected = Some(index);
        if let Err(e) = self
            .store
            .set_preference("rust:last-song", &serde_json::json!(self.records[index].id))
        {
            self.error = e.to_string();
        }
        let song = &self.records[index].song;
        self.plan = self
            .store
            .preference(&format!("rust:lesson:{}", self.records[index].id))
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_value(v).ok())
            .filter(|p: &Plan| p.validate(song).is_ok() && p.selected(song).is_ok())
            .unwrap_or_default();
        self.plan_draft = self.plan.clone();
        self.smart_config = self
            .store
            .preference("rust:smart-repeat")
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_value(v).ok())
            .filter(SmartConfig::valid)
            .unwrap_or_default();
        self.smart_draft = self.smart_config.clone();
        self.practice_song = self.plan.selected(song).ok();
        self.support_sequence = Some(std::sync::Arc::new(Sequence::accompaniment(
            song, &self.plan,
        )));
        self.repeat_progress = RepeatProgress::default();
        self.transport = Transport::new(self.plan.end(song) + 0.5).expect("Validated song");
        let _ = self
            .transport
            .seek((self.plan.start - 1.0).max(-3.0), self.now());
        if self.plan == Plan::default() {
            let _ = self.transport.seek(-3.0, self.now());
        }
        self.session = None;
        self.running = false;
        self.waiting = false;
        self.last_report = None;
        self.status =
            "Нажмите «Начать». В ожидании оцениваются группы нот, в ритме — нажатия и удержание."
                .into();
    }
    fn release_midi(&mut self) {
        self.performance_view = false;
        // End the current attempt before any newly queued input can be scored.
        self.running = false;
        self.finish_current(false);
        self.waiting = false;
        self.repeat_progress.streak = 0;
        let _ = self.transport.pause(self.now());
        self.state.clear_source("midi");
        self.audio.sync(&self.state, None);
    }
    fn midi_change(&mut self, change: crate::midi_input::Change) {
        match change {
            crate::midi_input::Change::Lost => {
                self.release_midi();
                self.status =
                    "Связь с MIDI потеряна. Занятие остановлено; ожидаем инструмент.".into();
            }
            crate::midi_input::Change::Connected => {
                self.state.clear_source("midi");
                self.audio.sync(&self.state, None);
                self.status =
                    "MIDI подключён. Нажмите «Продолжить» в занятии, когда будете готовы.".into();
            }
        }
    }
    fn poll_midi_connection(&mut self) {
        let selected = self.ports.get(self.port_index).map(|p| p.id.clone());
        for change in self.midi.poll() {
            self.midi_change(change);
        }
        self.ports = self.midi.available.clone();
        self.port_index = selected
            .and_then(|id| self.ports.iter().position(|p| p.id == id))
            .unwrap_or(0);
    }
    fn drain_midi(&mut self) {
        while let Ok(input) = self.midi.receiver.try_recv() {
            let now = input
                .received_at
                .saturating_duration_since(self.started)
                .as_secs_f64();
            self.feed_at(input.event, now);
        }
    }
    fn start_practice(&mut self) {
        self.stop_demo();
        if self.running {
            return;
        }
        self.drain_midi();
        let Some(index) = self.selected else {
            return;
        };
        let now = self.now();
        if self
            .practice_song
            .as_ref()
            .unwrap_or(&self.records[index].song)
            .notes
            .iter()
            .all(|n| n.start < self.transport.current(now).max(0.0))
        {
            let _ = self.transport.seek((self.plan.start - 1.0).max(-3.0), now);
        }
        let start = self.transport.current(now).max(self.plan.start);
        let options = Options {
            start,
            speed: self.transport.speed,
            tolerance: self.tolerance,
            latency: self.latency,
            end: Some(self.plan.end(&self.records[index].song)),
        };
        let mode = if self.wait_mode {
            Mode::Waiting
        } else {
            Mode::Rhythm
        };
        match Session::selected(
            &self.records[index].song,
            mode,
            options,
            self.records[index].score_version.clone(),
            &self.plan,
        ) {
            Ok(session) => {
                self.sheet_page = None;
                self.playback_epoch = self.playback_epoch.wrapping_add(1);
                self.session = Some(session);
                self.running = true;
                if !self.performance_view {
                    self.fullscreen_focus_until = Some(Instant::now() + Duration::from_millis(800));
                }
                self.performance_view = true;
                self.waiting = false;
                self.active_since = now;
                self.last_report = None;
                let _ = self.transport.play(now);
                self.status = "Занятие началось".into();
            }
            Err(e) => self.error = e.to_string(),
        }
    }
    fn retry_saves(&mut self) {
        while let Some(item) = self.save_queue.first_mut() {
            if item.pending.is_none() {
                match PendingAttempt::new(item.song_id.clone(), item.report.clone()) {
                    Ok(mut p) => {
                        match item.finished_at.duration_since(std::time::UNIX_EPOCH) {
                            Ok(time) => p.created = time.as_secs_f64(),
                            Err(e) => {
                                self.error = format!("Дата результата не сохранена: {e}");
                                return;
                            }
                        }
                        item.pending = Some(p);
                    }
                    Err(e) => {
                        self.error = format!("Результат не сохранён: {e}");
                        return;
                    }
                }
            }
            if let Err(e) = self.store.save_attempt(item.pending.as_ref().unwrap()) {
                self.error = format!("Результат не сохранён: {e}");
                return;
            }
            self.save_queue.remove(0);
        }
        self.error.clear();
    }
    fn finish_current(&mut self, complete: bool) {
        self.audio.playback(None);
        let Some(mut session) = self.session.take() else {
            return;
        };
        let report = session.finish(complete);
        if complete && !self.plan.repeat {
            self.performance_view = false;
        }
        self.running = false;
        self.waiting = false;
        if !complete {
            self.repeat_progress.streak = 0;
        }
        let _ = self.transport.pause(self.now());
        if report.worth_saving() {
            if let Some(index) = self.selected {
                self.save_queue.push(SaveItem {
                    finished_at: std::time::SystemTime::now(),
                    song_id: self.records[index].id.clone(),
                    report: report.clone(),
                    pending: None,
                });
                self.retry_saves();
            }
            self.status = format!(
                "{} · точность {}% · ошибок {} · {}",
                if report.outcome.complete {
                    "Завершено"
                } else {
                    "Прервано"
                },
                report.outcome.accuracy,
                report.outcome.mistakes.len(),
                if self.save_queue.is_empty() {
                    "сохранено в истории"
                } else {
                    "ожидает сохранения"
                }
            );
        } else {
            self.status = "Попытка остановлена до первых нот; пустой результат не сохранён.".into();
        }
        self.last_report = Some(report);
    }
    fn pause_practice(&mut self) {
        self.drain_midi();
        self.repeat_progress.streak = 0;
        self.finish_current(false);
        self.running = false;
        self.waiting = false;
        let _ = self.transport.pause(self.now());
    }
    fn advance_practice(&mut self, now: f64) {
        if !self.running || !self.transport.playing {
            return;
        }
        let position = self.transport.current(now);
        if self.wait_mode
            && let Some(group) = self.session.as_ref().and_then(Session::expected)
            && position >= group.start
        {
            let time = group.start;
            let _ = self.transport.seek(time, now);
            let _ = self.transport.pause(now);
            self.waiting = true;
        } else if position >= self.transport.duration {
            self.finish_current(true);
            if self.plan.repeat
                && self.save_queue.is_empty()
                && self
                    .last_report
                    .as_ref()
                    .is_some_and(|r| r.outcome.complete)
            {
                let complete_range = self
                    .last_report
                    .as_ref()
                    .is_some_and(|r| r.start == self.plan.start);
                if !complete_range {
                    self.repeat_progress.streak = 0;
                } else if self.plan.smart {
                    let report = self.last_report.as_ref().unwrap();
                    let effective = (report.outcome.accuracy
                        - 100.0 * report.outcome.mistakes.len() as f64
                            / report.outcome.total.max(1) as f64)
                        .max(0.0);
                    let speed = self.repeat_progress.next_speed(
                        self.transport.speed,
                        effective,
                        &self.smart_config,
                    );
                    let _ = self.transport.set_speed(speed, now);
                } else {
                    self.repeat_progress.completed =
                        self.repeat_progress.completed.saturating_add(1);
                }
                let _ = self.transport.seek((self.plan.start - 1.0).max(-3.0), now);
                self.start_practice();
                self.status = format!(
                    "Повтор {} · скорость {:.0}% · удачных подряд {}",
                    self.repeat_progress.completed.saturating_add(1),
                    self.transport.speed * 100.0,
                    self.repeat_progress.streak
                );
            }
        }
    }
    fn feed(&mut self, event: MidiEvent) {
        self.feed_at(event, self.now());
    }
    fn feed_at(&mut self, event: MidiEvent, now: f64) {
        self.advance_practice(now);
        self.state.apply(&event);
        let retrigger = match event.message {
            Message::NoteOn { pitch, velocity } if velocity > 0 => Some(pitch),
            _ => None,
        };
        self.audio.sync(&self.state, retrigger);
        if self.page == Page::Practice
            && self.running
            && now >= self.active_since
            && let Some(session) = &mut self.session
        {
            let soft = session.expected().is_some_and(|g| {
                self.transport.current(now) < g.start || !g.pitches.is_subset(&self.state.pressed())
            });
            match session.feed_timed(
                &event,
                &self.state.pressed(),
                self.transport.current(now),
                self.waiting,
                now,
            ) {
                Ok(true) => {
                    if soft {
                        self.status = "Небольшое отклонение — принято, продолжаем".into();
                    }
                    self.waiting = false;
                    let _ = self.transport.play(now);
                }
                Ok(false) => {}
                Err(e) => {
                    self.error = e.to_string();
                    self.finish_current(false);
                }
            }
        }
    }
    fn request_close(&mut self, ctx: &egui::Context) {
        self.performance_view = false;
        if self.backups.job.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            return;
        }
        self.pause_practice();
        self.retry_saves();
        if let Some(editor) = &mut self.editor
            && editor.dirty()
        {
            editor.discard = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        if !self.save_queue.is_empty() && !self.allow_close {
            self.close_warning = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
    }
    fn tick(&mut self, ctx: &egui::Context) {
        self.poll_backups();
        if self.backups.job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        self.poll_midi_connection();
        if self.midi.watching() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        self.poll_import();
        if self.import_receiver.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if let Some(path) = &self.capture {
            let screenshot = ctx.input(|i| {
                i.events.iter().find_map(|e| {
                    if let egui::Event::Screenshot { image, .. } = e {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
            });
            if let Some(image) = screenshot {
                match image::save_buffer(
                    path,
                    image.as_raw(),
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                ) {
                    Ok(()) => {
                        self.capture_succeeded
                            .store(true, std::sync::atomic::Ordering::Relaxed);
                        eprintln!("Rust native capture saved");
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    Err(e) => {
                        eprintln!("Rust native capture failed: {e}");
                        self.error = format!("Снимок окна: {e}");
                    }
                }
                self.capture = None;
            } else if !self.capture_requested && self.started.elapsed() > Duration::from_millis(250)
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                self.capture_requested = true;
            }
        }

        if !ctx.input(|i| i.focused)
            && self.running
            && self
                .fullscreen_focus_until
                .is_none_or(|until| Instant::now() >= until)
        {
            self.pause_practice();
        }
        self.drain_midi();
        if !ctx.input(|i| i.focused) || !matches!(self.page, Page::Practice | Page::FreePlay) {
            for event in self.touch_keyboard.clear() {
                self.feed(event);
            }
        }
        if !ctx.input(|i| i.focused) {
            self.state.clear_source("computer");
            self.state.clear_source("mouse");
            self.computer_notes.clear();
            self.mouse_note = None;
        }
        self.advance_practice(self.now());
        self.update_audio();
        self.sync_performance_view(ctx);
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            self.request_close(ctx);
        }
        if self.transport.playing || self.midi.connected() || self.demo.is_some() || self.metronome
        {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        if self.audio_initialized {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
        if self.native_smoke && self.started.elapsed() > Duration::from_secs(3) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.native_smoke {
            ctx.request_repaint();
        }
    }
    fn keys(&mut self, ui: &egui::Ui) {
        if self.backups.job.is_some()
            || self.editor.is_some()
            || !matches!(self.page, Page::Practice | Page::FreePlay)
            || !ui.input(|i| i.focused)
        {
            return;
        }
        let keys = [
            egui::Key::A,
            egui::Key::W,
            egui::Key::S,
            egui::Key::E,
            egui::Key::D,
            egui::Key::F,
            egui::Key::T,
            egui::Key::G,
            egui::Key::Y,
            egui::Key::H,
            egui::Key::U,
            egui::Key::J,
            egui::Key::K,
        ];
        for (index, key) in keys.into_iter().enumerate() {
            let pitch = 60 + index as u8;
            let down = ui.input(|i| i.key_down(key));
            if down && self.computer_notes.insert(pitch) {
                self.feed(MidiEvent {
                    source: "computer".into(),
                    channel: 0,
                    message: Message::NoteOn {
                        pitch,
                        velocity: 90,
                    },
                });
            }
            if !down && self.computer_notes.remove(&pitch) {
                self.feed(MidiEvent {
                    source: "computer".into(),
                    channel: 0,
                    message: Message::NoteOff { pitch },
                });
            }
        }
        let sustain = ui.input(|i| i.key_down(egui::Key::Space));
        self.feed(MidiEvent {
            source: "computer".into(),
            channel: 0,
            message: Message::Control {
                control: 64,
                value: if sustain { 127 } else { 0 },
            },
        });
    }
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        if self.performance_view && self.page == Page::Practice {
            self.performance(ui);
            return;
        }
        let previous_page = self.page;
        let previous_dark = self.dark;
        if self.plain {
            self.dark = false;
        }
        aero::style(ui.ctx(), self.dark);
        ui.set_style(ui.ctx().global_style());
        if self.aero.is_none() {
            match Aero::new(ui.ctx()) {
                Ok(aero) => self.aero = Some(aero),
                Err(e) => self.error = format!("Не удалось загрузить оформление: {e}"),
            }
        }
        if let Some(aero) = &mut self.aero {
            aero.plain = self.plain;
            aero.background(ui, self.dark);
        }
        let compact = ui.max_rect().height() < 540.0;
        let (header, _) = ui.allocate_exact_size(
            Vec2::new(
                ui.available_width(),
                if compact {
                    86.0
                } else if self.page == Page::Home {
                    146.0
                } else {
                    128.0
                },
            ),
            Sense::hover(),
        );
        let p = ui.painter().clone();
        let brand_font = egui::FontId::new(
            if header.width() < 650.0 { 43.0 } else { 54.0 },
            if self.plain {
                egui::FontFamily::Proportional
            } else {
                egui::FontFamily::Name("brand".into())
            },
        );
        let name = Pos2::new(
            header.center().x - 105.0,
            header.top() + if compact { 29.0 } else { 43.0 },
        );
        aero::paint_icon(
            &p,
            Rect::from_center_size(Pos2::new(name.x - 41.0, name.y), Vec2::splat(52.0)),
            Icon::Piano,
            true,
        );
        p.text(
            name + Vec2::new(0.0, 2.0),
            egui::Align2::LEFT_CENTER,
            "Clavunora",
            brand_font.clone(),
            Color32::from_black_alpha(120),
        );
        p.text(
            name,
            egui::Align2::LEFT_CENTER,
            "Clavunora",
            brand_font,
            if self.plain {
                aero::text(false)
            } else {
                Color32::from_rgb(242, 254, 255)
            },
        );
        let theme_rect = Rect::from_center_size(
            Pos2::new(
                header.right() - 34.0,
                header.top() + if compact { 29.0 } else { 38.0 },
            ),
            Vec2::new(46.0, 38.0),
        );
        if ui
            .put(theme_rect, egui::Button::new(""))
            .on_hover_text("Переключить светлую / тёмную тему")
            .clicked()
        {
            self.plain = false;
            let _ = self
                .store
                .set_preference("rust:plain-theme", &serde_json::json!(false));
            self.dark = !self.dark;
            if let Err(e) = self
                .store
                .set_preference("rust-preview:dark", &serde_json::json!(self.dark))
            {
                self.error = e.to_string();
            }
        }
        let center = theme_rect.center();
        if self.dark {
            p.circle_stroke(center, 6.0, egui::Stroke::new(1.7_f32, Color32::WHITE));
            for i in 0..8 {
                let direction = Vec2::angled(i as f32 * std::f32::consts::TAU / 8.0);
                p.line_segment(
                    [center + direction * 9.0, center + direction * 13.0],
                    egui::Stroke::new(1.5_f32, Color32::WHITE),
                );
            }
        } else {
            p.circle_filled(center, 10.0, Color32::WHITE);
            p.circle_filled(
                center + Vec2::new(5.0, -4.0),
                9.0,
                Color32::from_rgb(93, 153, 183),
            );
        }
        if self.page != Page::Home {
            if ui
                .put(
                    Rect::from_center_size(
                        Pos2::new(
                            header.center().x,
                            header.top() + if compact { 67.0 } else { 95.0 },
                        ),
                        Vec2::new(120.0, 32.0),
                    ),
                    egui::Button::new("‹ Главная"),
                )
                .on_hover_text(format!("{} → Главная", self.page.title()))
                .clicked()
            {
                self.navigate(Page::Home);
            }
        } else {
            p.text(
                Pos2::new(
                    header.center().x,
                    header.top() + if compact { 67.0 } else { 96.0 },
                ),
                egui::Align2::CENTER_CENTER,
                "Музыка начинается с вас",
                egui::FontId::proportional(18.0),
                if self.plain {
                    aero::text(false)
                } else {
                    Color32::WHITE
                },
            );
        }
        self.keys(ui);
        let body_width = ui.available_width().min(1100.0);
        let body = Rect::from_min_size(
            Pos2::new(
                ui.max_rect().center().x - body_width / 2.0,
                header.bottom() + 8.0,
            ),
            Vec2::new(
                body_width,
                (ui.max_rect().bottom() - header.bottom() - 50.0).max(1.0),
            ),
        );
        if self.page != Page::Home
            && let Some(aero) = &self.aero
        {
            aero.glass(ui.painter(), body, self.dark, false);
        }
        let inner = body.shrink2(Vec2::new(
            if self.page == Page::Home || compact {
                8.0
            } else {
                24.0
            },
            if compact { 8.0 } else { 18.0 },
        ));
        let reset_scroll = std::mem::take(&mut self.reset_page_scroll);
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(inner)
                .id_salt("main-content"),
            |ui| {
                ui.set_max_width(inner.width());
                let mut scroll = egui::ScrollArea::both()
                    .id_salt(("page-content", self.page))
                    .auto_shrink([false, false])
                    .max_height(inner.height());
                if reset_scroll {
                    scroll = scroll.vertical_scroll_offset(0.0);
                }
                scroll.show(ui, |ui| {
                    if !self.error.is_empty() {
                        ui.colored_label(Color32::from_rgb(180, 49, 35), &self.error);
                    }
                    if !self.save_queue.is_empty() {
                        ui.horizontal(|ui| {
                            ui.label(format!(
                                "Несохранённых результатов: {}",
                                self.save_queue.len()
                            ));
                            if ui.button("Повторить сохранение").clicked() {
                                self.retry_saves();
                            }
                        });
                    }
                    if self.close_warning && !self.save_queue.is_empty() {
                        ui.label("Закрытие отменено: есть несохранённые результаты.");
                        if ui
                            .button("Закрыть без сохранения этих результатов")
                            .clicked()
                        {
                            self.allow_close = true;
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                    if self.backups.job.is_some() {
                        ui.label("Работа с резервной копией…");
                        return;
                    }
                    match self.page {
                        Page::Home => self.home(ui),
                        Page::Library => self.library(ui, false),
                        Page::Course => self.library(ui, true),
                        Page::Practice => self.practice(ui),
                        Page::FreePlay => self.freeplay(ui),
                        Page::Achievements => self.achievements(ui),
                        Page::Progress => self.progress(ui),
                        Page::Settings => self.settings(ui),
                    }
                });
            },
        );
        let footer = Rect::from_min_size(
            Pos2::new(body.left(), body.bottom() + 9.0),
            Vec2::new(body.width(), 28.0),
        );
        if let Some(aero) = &self.aero {
            aero.glass(ui.painter(), footer, self.dark, false);
        }
        ui.painter().text(
            footer.left_center() + Vec2::new(12.0, 0.0),
            egui::Align2::LEFT_CENTER,
            &self.status,
            egui::FontId::proportional(12.0),
            aero::text(self.dark),
        );
        self.editor_controls(ui.ctx());
        if self.backups.job.is_some() {
            egui::Modal::new(egui::Id::new("backup-busy")).show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Работа с резервной копией…");
                });
                ui.label("Дождитесь завершения. Окно выбора файла может быть открыто отдельно.");
            });
        }
        if previous_page != self.page || previous_dark != self.dark {
            ui.ctx().request_repaint();
        }
    }
    fn home(&mut self, ui: &mut egui::Ui) {
        let entries = [
            (
                Page::Library,
                Icon::Library,
                "Библиотека",
                "Ваши любимые произведения",
            ),
            (
                Page::Course,
                Icon::Course,
                "Обучение",
                "Шаг за шагом к музыке",
            ),
            (
                Page::FreePlay,
                Icon::Note,
                "Свободная игра",
                "Вдохновение в каждой ноте",
            ),
            (
                Page::Achievements,
                Icon::Trophy,
                "Достижения",
                "Ваши музыкальные победы",
            ),
            (
                Page::Progress,
                Icon::Chart,
                "Прогресс",
                "История вашего пути",
            ),
            (
                Page::Settings,
                Icon::Settings,
                "Настройки",
                "Звук, клавиатура и тема",
            ),
        ];
        let columns = if ui.available_width() < 640.0 { 2 } else { 3 };
        let gap = 18.0;
        let width =
            ((ui.available_width() - gap * (columns - 1) as f32) / columns as f32).min(255.0);
        let height = if ui.ctx().content_rect().height() < 540.0 {
            124.0
        } else if ui.available_height() < 420.0 {
            156.0
        } else {
            183.0
        };
        let total = columns as f32 * width + gap * (columns - 1) as f32;
        let left = (ui.available_width() - total) / 2.0;
        let mut destination = None;
        for row in entries.chunks(columns) {
            ui.horizontal(|ui| {
                ui.add_space(left);
                for (i, (page, icon, title, subtitle)) in row.iter().enumerate() {
                    if i > 0 {
                        ui.add_space(gap - ui.spacing().item_spacing.x);
                    }
                    if let Some(aero) = &self.aero
                        && aero
                            .tile(
                                ui,
                                Vec2::new(width, height),
                                *icon,
                                title,
                                subtitle,
                                self.dark,
                            )
                            .clicked()
                    {
                        destination = Some(*page);
                    }
                }
            });
            ui.add_space(8.0);
        }
        if let Some(page) = destination {
            if page == Page::Library {
                self.folder = None;
                self.search.clear();
            }
            self.navigate(page);
        }
        ui.add_space(10.0);
        let continue_index = self.selected.or_else(|| {
            self.store
                .preference("rust:last-song")
                .ok()
                .flatten()
                .and_then(|v| {
                    v.as_str()
                        .and_then(|id| self.records.iter().position(|r| r.id == id))
                })
        });
        if let Some(index) = continue_index {
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 76.0), Sense::hover());
            let rect = Rect::from_center_size(rect.center(), Vec2::new(total, 76.0));
            let response = ui.interact(rect, ui.id().with("continue"), Sense::click());
            if let Some(aero) = &self.aero {
                aero.glass(ui.painter(), rect, self.dark, response.hovered());
            }
            aero::paint_icon(
                ui.painter(),
                Rect::from_center_size(
                    rect.left_center() + Vec2::new(42.0, 0.0),
                    Vec2::splat(42.0),
                ),
                Icon::Note,
                true,
            );
            ui.painter().text(
                rect.left_top() + Vec2::new(82.0, 23.0),
                egui::Align2::LEFT_CENTER,
                "Продолжить занятие",
                egui::FontId::proportional(17.0),
                aero::text(self.dark),
            );
            ui.painter().text(
                rect.left_bottom() + Vec2::new(82.0, -23.0),
                egui::Align2::LEFT_CENTER,
                &self.records[index].song.title,
                egui::FontId::proportional(14.0),
                aero::muted(self.dark),
            );
            ui.painter().text(
                rect.right_center() - Vec2::new(24.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                "›",
                egui::FontId::proportional(30.0),
                aero::text(self.dark),
            );
            if response.clicked() {
                self.open_song(index);
            }
        } else {
            ui.vertical_centered(|ui| {
                ui.colored_label(
                    if self.plain {
                        aero::text(false)
                    } else {
                        Color32::WHITE
                    },
                    format!(
                        "{} партитур · выберите музыку для первого занятия",
                        self.records.len()
                    ),
                );
            });
        }
    }
    fn folders(&mut self, ui: &mut egui::Ui) {
        let entries = [
            ("level:1", "Начальный", Icon::Folder),
            ("level:2", "Лёгкий", Icon::Folder),
            ("level:3", "Средний", Icon::Folder),
            ("level:4", "Сложный", Icon::Folder),
            ("level:5", "Мастер", Icon::Trophy),
            ("favorites", "Избранное", Icon::Star),
            ("imports", "Мои пьесы", Icon::Note),
            ("all", "Все произведения", Icon::Library),
        ];
        let columns = if ui.available_width() < 340.0 {
            1
        } else if ui.available_width() < 720.0 {
            2
        } else {
            4
        };
        let width =
            ((ui.available_width() - 16.0 * (columns - 1) as f32) / columns as f32).max(130.0);
        let builtin: BTreeSet<String> = catalog::BUNDLED_SCORES
            .iter()
            .map(|(id, _)| (*id).into())
            .chain(
                catalog::course()
                    .into_iter()
                    .chain(catalog::birthday_variants())
                    .map(|(id, _)| id),
            )
            .collect();
        let mut selected = None;
        for row in entries.chunks(columns) {
            ui.horizontal(|ui| {
                for (key, title, icon) in row {
                    let count = self
                        .records
                        .iter()
                        .filter(|r| match *key {
                            "favorites" => r.favorite,
                            "all" => true,
                            "imports" => !builtin.contains(&r.id),
                            _ => key
                                .strip_prefix("level:")
                                .is_some_and(|n| n.parse::<u8>().ok() == Some(r.song.difficulty)),
                        })
                        .count();
                    if let Some(aero) = &self.aero
                        && aero
                            .tile(
                                ui,
                                Vec2::new(width, 160.0),
                                *icon,
                                title,
                                &format!("{count} партитур"),
                                self.dark,
                            )
                            .clicked()
                    {
                        selected = Some((*key).to_string());
                    }
                }
            });
            ui.add_space(6.0);
        }
        if let Some(key) = selected {
            self.folder = Some(key);
            self.search.clear();
            self.navigate(Page::Library);
        }
    }
    fn poll_import(&mut self) {
        if let Some(receiver) = &self.import_receiver {
            match receiver.try_recv() {
                Ok(Ok(preview)) => {
                    self.import_preview = preview;
                    self.import_receiver = None;
                    self.error.clear();
                }
                Ok(Err(e)) => {
                    self.error = e;
                    self.import_receiver = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.error = "Импорт нот прерван".into();
                    self.import_receiver = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
    }
    fn confirm_midi_import(&mut self) -> Result<bool> {
        let preview = self
            .import_preview
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Сначала откройте нотный файл"))?;
        let (_id, added) =
            self.store
                .add_score_file(&preview.parsed.song, &preview.bytes, preview.kind)?;
        let selected = self.selected.map(|i| self.records[i].id.clone());
        self.records = self.store.songs()?;
        self.selected = selected.and_then(|id| self.records.iter().position(|r| r.id == id));
        self.folder = Some("imports".into());
        self.search.clear();
        self.import_preview = None;
        self.error.clear();
        self.status = if added {
            "Произведение добавлено в «Мои пьесы»"
        } else {
            "Этот файл уже есть в библиотеке; существующая пьеса сохранена"
        }
        .into();
        Ok(added)
    }
    fn import_controls(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("Добавить MIDI / MusicXML",|ui| {
            ui.label("Выберите .mid/.midi/.musicxml/.xml или введите путь. Перед сохранением можно проверить ноты и сведения.");
            ui.add(egui::TextEdit::singleline(&mut self.import_path).hint_text("Путь к нотному файлу").desired_width(ui.available_width()));
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(self.import_receiver.is_none()&&self.import_preview.is_none(),egui::Button::new("Выбрать файл…")).clicked(){self.import_receiver=Some(crate::midi_import::start(None));}
                if ui.add_enabled(self.import_receiver.is_none()&&self.import_preview.is_none()&&!self.import_path.trim().is_empty(),egui::Button::new("Открыть по пути")).clicked(){self.import_receiver=Some(crate::midi_import::start(Some(PathBuf::from(self.import_path.trim()))));}
            });
            if self.import_receiver.is_some(){ui.label("Ожидание выбора / чтение файла…");}
        });
        let mut confirm = false;
        let mut cancel = false;
        if let Some(preview) = &mut self.import_preview {
            ui.group(|ui| {
                ui.heading(if preview.kind == "musicxml" {
                    "Предпросмотр MusicXML"
                } else {
                    "Предпросмотр MIDI"
                });
                ui.label(&preview.name);
                ui.horizontal_wrapped(|ui| {
                    ui.label("Название:");
                    ui.text_edit_singleline(&mut preview.parsed.song.title);
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Композитор:");
                    ui.text_edit_singleline(&mut preview.parsed.song.composer);
                });
                ui.add(
                    egui::Slider::new(&mut preview.parsed.song.difficulty, 1..=5)
                        .text("Сложность (выберите вручную)"),
                );
                ui.label(format!(
                    "{} нот · {} дорожек · {:.1} с · {:.0} BPM",
                    preview.parsed.song.notes.len(),
                    preview.parsed.song.track_names.len(),
                    preview.parsed.song.duration(),
                    preview.parsed.song.bpm
                ));
                if preview.kind == "musicxml" {
                    ui.small(format!(
                        "Аппликатура: {} нот с указанием пальцев",
                        preview
                            .parsed
                            .song
                            .notes
                            .iter()
                            .filter(|n| n.finger > 0)
                            .count()
                    ));
                }
                for warning in &preview.parsed.warnings {
                    ui.small(warning);
                }
                ui.collapsing("Дорожки и первые ноты", |ui| {
                    for (i, name) in preview.parsed.song.track_names.iter().enumerate() {
                        ui.label(format!(
                            "{}: {} · {} нот",
                            i + 1,
                            name,
                            preview
                                .parsed
                                .song
                                .notes
                                .iter()
                                .filter(|n| n.track == i)
                                .count()
                        ));
                    }
                    for n in preview.parsed.song.notes.iter().take(12) {
                        ui.small(format!(
                            "{} · {:.2} с · длительность {:.2} с · дорожка {} · канал {}",
                            self.notation.language.name(n.pitch),
                            n.start,
                            n.duration,
                            n.track + 1,
                            n.channel + 1
                        ));
                    }
                });
                ui.horizontal(|ui| {
                    confirm = ui.button("Добавить в библиотеку").clicked();
                    cancel = ui.button("Отмена").clicked();
                });
            });
        }
        if cancel {
            self.import_preview = None;
        }
        if confirm && let Err(e) = self.confirm_midi_import() {
            self.error = e.to_string();
        }
    }
    fn library(&mut self, ui: &mut egui::Ui, course: bool) {
        ui.heading(if course {
            "Обучение"
        } else {
            "Библиотека"
        });
        if !course {
            self.import_controls(ui);
        }
        ui.label(if course {
            "Начните с простого — каждый урок приближает вас к любимой музыке."
        } else {
            "Выберите папку или найдите произведение по названию и композитору."
        });
        ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text("Найти произведение…")
                .desired_width(ui.available_width()),
        );
        if !course && self.folder.is_none() && self.search.is_empty() {
            self.folders(ui);
            return;
        }
        if !course && ui.button("‹ Все папки").clicked() {
            self.folder = None;
            self.search.clear();
            return;
        }
        let query = self.search.to_lowercase();
        let builtin: BTreeSet<String> = catalog::BUNDLED_SCORES
            .iter()
            .map(|(id, _)| (*id).into())
            .chain(
                catalog::course()
                    .into_iter()
                    .chain(catalog::birthday_variants())
                    .map(|(id, _)| id),
            )
            .collect();
        let mut visible: Vec<usize> = self
            .records
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                if course && !r.id.starts_with("course-") {
                    return false;
                }
                if !course && let Some(folder) = &self.folder {
                    if folder.starts_with("level:") && r.song.difficulty.to_string() != folder[6..]
                    {
                        return false;
                    }
                    if folder == "favorites" && !r.favorite {
                        return false;
                    }
                    if folder == "imports" && builtin.contains(&r.id) {
                        return false;
                    }
                }
                format!("{} {} {}", r.song.title, r.song.composer, r.song.work)
                    .to_lowercase()
                    .contains(&query)
            })
            .map(|(i, _)| i)
            .collect();
        if course {
            visible.sort_by_key(|&i| {
                self.records[i]
                    .id
                    .strip_prefix("course-")
                    .and_then(|n| n.parse::<u32>().ok())
                    .unwrap_or(u32::MAX)
            });
            let done = visible
                .iter()
                .filter(|&&i| {
                    self.store
                        .preference(&format!("passed:{}", self.records[i].id))
                        .ok()
                        .flatten()
                        == Some(serde_json::json!(true))
                })
                .count();
            ui.label(format!("Курс: {} урока · пройдено {done}. Ритм · интервалы · гаммы и аккорды · координация рук.",visible.len()));
        }
        if visible.is_empty() {
            ui.add_space(24.0);
            ui.label(if self.folder.as_deref() == Some("favorites") {
                "Здесь будут ваши любимые произведения. Добавьте их в избранное из библиотеки."
            } else {
                "Подходящих произведений пока нет."
            });
            return;
        }
        let columns = if ui.available_width() < 680.0 { 1 } else { 2 };
        let width = (ui.available_width() - 16.0 * (columns - 1) as f32) / columns as f32;
        let mut open = None;
        let mut edit = None;
        let mut favorite = None;
        for row in visible.chunks(columns) {
            ui.horizontal(|ui| {
                for &index in row {
                    let r = &self.records[index];
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 174.0), Sense::hover());
                    if let Some(aero) = &self.aero {
                        aero.glass(ui.painter(), rect, self.dark, false);
                    }
                    aero::paint_icon(
                        ui.painter(),
                        Rect::from_min_size(rect.min + Vec2::new(16.0, 18.0), Vec2::splat(46.0)),
                        if course { Icon::Course } else { Icon::Note },
                        true,
                    );
                    let text = Rect::from_min_max(
                        rect.min + Vec2::new(78.0, 16.0),
                        rect.max - Vec2::new(14.0, 54.0),
                    );
                    ui.scope_builder(
                        egui::UiBuilder::new()
                            .id_salt(("score-text", index))
                            .layout(egui::Layout::top_down(egui::Align::Min))
                            .max_rect(text),
                        |ui| {
                            ui.set_clip_rect(ui.clip_rect().intersect(text));
                            ui.set_max_width(text.width());
                            ui.add(
                                egui::Label::new(
                                    RichText::new(if course {
                                        format!(
                                            "{}. {}",
                                            r.id.strip_prefix("course-").unwrap_or(""),
                                            r.song.title
                                        )
                                    } else {
                                        r.song.title.clone()
                                    })
                                    .size(19.0)
                                    .strong(),
                                )
                                .truncate(),
                            )
                            .on_hover_text(&r.song.title);
                            if !r.song.composer.is_empty() {
                                ui.add(egui::Label::new(&r.song.composer).truncate())
                                    .on_hover_text(&r.song.composer);
                            }
                            ui.add(
                                egui::Label::new(
                                    RichText::new(format!(
                                        "Уровень {} · {} нот · {:.0} с",
                                        r.song.difficulty,
                                        r.song.notes.len(),
                                        r.song.duration()
                                    ))
                                    .small(),
                                )
                                .truncate(),
                            );
                            if course
                                && self
                                    .store
                                    .preference(&format!("passed:{}", r.id))
                                    .ok()
                                    .flatten()
                                    == Some(serde_json::Value::Bool(true))
                            {
                                ui.label("✓ Пройдено");
                            }
                        },
                    );
                    let buttons = Rect::from_min_max(
                        Pos2::new(rect.left() + 16.0, rect.bottom() - 47.0),
                        rect.max - Vec2::new(12.0, 10.0),
                    );
                    ui.scope_builder(
                        egui::UiBuilder::new()
                            .id_salt(("score-actions", index))
                            .max_rect(buttons),
                        |ui| {
                            ui.set_clip_rect(ui.clip_rect().intersect(buttons));
                            ui.horizontal(|ui| {
                                if ui
                                    .button(if course {
                                        "Начать урок"
                                    } else {
                                        "Играть"
                                    })
                                    .clicked()
                                {
                                    open = Some(index);
                                }
                                if ui
                                    .button(if r.favorite {
                                        "★ В избранном"
                                    } else {
                                        "☆ В избранное"
                                    })
                                    .clicked()
                                {
                                    favorite = Some(index);
                                }
                                if ui.button("Разметка").clicked() {
                                    edit = Some(index);
                                }
                            });
                        },
                    );
                    ui.advance_cursor_after_rect(rect);
                }
            });
            ui.add_space(6.0);
        }
        if let Some(index) = favorite {
            let r = &self.records[index];
            match self.store.favorite(&r.id, !r.favorite) {
                Ok(()) => self.records[index].favorite = !r.favorite,
                Err(e) => self.error = e.to_string(),
            }
        }
        if let Some(index) = open {
            self.open_song(index);
        }
        if let Some(index) = edit {
            self.open_editor(index);
        }
    }
    fn open_editor(&mut self, index: usize) {
        self.pause_practice();
        self.stop_demo();
        self.state.clear_source("computer");
        self.state.clear_source("mouse");
        self.computer_notes.clear();
        self.mouse_note = None;
        self.audio.sync(&self.state, None);
        self.editor = Some(crate::score_editor::Editor::new(&self.records[index]));
    }
    fn save_editor(&mut self) -> Result<()> {
        let editor = self
            .editor
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Нет открытого редактора"))?;
        let id = editor.id.clone();
        self.store
            .save_annotations(&id, &editor.version, &editor.song)?;
        self.records = self.store.songs()?;
        self.editor = None;
        if let Some(index) = self.selected
            && self.records[index].id == id
        {
            self.open_song(index);
        }
        self.status = "Разметка сохранена. Она будет использоваться в новых занятиях".into();
        Ok(())
    }
    fn editor_controls(&mut self, ctx: &egui::Context) {
        let action = self.editor.as_mut().map(|editor| editor.show(ctx));
        match action {
            Some(crate::score_editor::Action::Save) => {
                if let Err(e) = self.save_editor()
                    && let Some(editor) = &mut self.editor
                {
                    editor.error = e.to_string();
                }
            }
            Some(crate::score_editor::Action::Cancel) => self.editor = None,
            _ => {}
        }
    }
    fn apply_lesson_plan(&mut self, index: usize) -> Result<()> {
        self.plan_draft.validate(&self.records[index].song)?;
        anyhow::ensure!(
            self.smart_draft.valid(),
            "Некорректные параметры умного повтора"
        );
        let selected = self.plan_draft.selected(&self.records[index].song)?;
        self.repair_notice = None;
        self.sheet_page = None;
        self.pause_practice();
        self.stop_demo();
        let speed = self.transport.speed;
        self.plan = self.plan_draft.clone();
        self.smart_config = self.smart_draft.clone();
        self.practice_song = Some(selected);
        self.support_sequence = Some(std::sync::Arc::new(Sequence::accompaniment(
            &self.records[index].song,
            &self.plan,
        )));
        self.repeat_progress = RepeatProgress::default();
        self.last_report = None;
        self.transport = Transport::new(self.plan.end(&self.records[index].song) + 0.5)?;
        self.transport.set_speed(speed, self.now())?;
        self.transport
            .seek((self.plan.start - 1.0).max(-3.0), self.now())?;
        self.store.set_preference(
            &format!("rust:lesson:{}", self.records[index].id),
            &serde_json::json!(self.plan),
        )?;
        self.store
            .set_preference("rust:smart-repeat", &serde_json::json!(self.smart_config))?;
        if self.save_queue.is_empty() {
            self.error.clear();
        }
        self.status = "Партия и фрагмент выбраны. Нажмите «Начать».".into();
        Ok(())
    }
    fn lesson_controls(&mut self, ui: &mut egui::Ui, index: usize) {
        egui::CollapsingHeader::new("Разучивание: руки, дорожки и фрагмент").default_open(self.native_smoke&&self.capture.is_some()).show(ui,|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut self.plan_draft.hand,String::new(),"Все руки");
                ui.selectable_value(&mut self.plan_draft.hand,"L".into(),"Левая");ui.selectable_value(&mut self.plan_draft.hand,"R".into(),"Правая");
                if ui.checkbox(&mut self.show_fingers,"Подсказки пальцев").changed() && let Err(e)=self.store.set_preference("rust:show-fingers",&serde_json::json!(self.show_fingers)){self.error=e.to_string();}
            });
            if self.records[index].song.notes.iter().all(|n|n.hand.is_empty()){ui.small("Партии рук в этой пьесе не размечены. Для выбора партии используйте дорожки.");}
            let tracks:BTreeSet<_>=self.records[index].song.notes.iter().map(|n|n.track).collect();
            ui.horizontal_wrapped(|ui| {
                if ui.selectable_label(self.plan_draft.tracks.is_none(),"Все дорожки").clicked(){self.plan_draft.tracks=None;}
                for track in &tracks {
                    let mut selected=self.plan_draft.tracks.as_ref().is_none_or(|s|s.contains(track));
                    let name=self.records[index].song.track_names.get(*track).cloned().unwrap_or_else(||format!("Дорожка {}",track+1));
                    if ui.checkbox(&mut selected,name).changed(){
                        let mut chosen=self.plan_draft.tracks.clone().unwrap_or_else(||tracks.clone());
                        if selected {chosen.insert(*track);} else {chosen.remove(track);}
                        self.plan_draft.tracks=if chosen==tracks {None}else{Some(chosen)};
                    }
                }
            });
            let duration=self.records[index].song.duration();let mut fragment=self.plan_draft.end.is_some()||self.plan_draft.start>0.0;
            if ui.checkbox(&mut fragment,"Выбрать фрагмент").changed(){self.plan_draft.start=0.0;self.plan_draft.end=if fragment{Some(duration)}else{None};}
            if fragment {
                let mut end=self.plan_draft.end.unwrap_or(duration);
                ui.horizontal_wrapped(|ui| {
                    ui.add(egui::DragValue::new(&mut self.plan_draft.start).range(0.0..=duration).speed(0.1).prefix("С ").suffix(" с"));
                    ui.add(egui::DragValue::new(&mut end).range(0.0..=duration).speed(0.1).prefix("До ").suffix(" с"));
                });self.plan_draft.end=Some(end);
            }
            ui.horizontal_wrapped(|ui| {ui.checkbox(&mut self.plan_draft.repeat,"Повторять");if self.plan_draft.repeat{ui.checkbox(&mut self.plan_draft.smart,"Умный повтор");}});
            if self.plan_draft.repeat&&self.plan_draft.smart {
                ui.horizontal_wrapped(|ui| {
                    ui.add(egui::DragValue::new(&mut self.smart_draft.repetitions).range(1..=20).prefix("Удачных подряд: "));
                    ui.add(egui::DragValue::new(&mut self.smart_draft.threshold).range(0.0..=100.0).prefix("Порог: ").suffix("%"));
                    let mut step=self.smart_draft.step*100.0;let mut target=self.smart_draft.target*100.0;
                    ui.add(egui::DragValue::new(&mut step).range(1.0..=25.0).prefix("Шаг: ").suffix("%"));ui.add(egui::DragValue::new(&mut target).range(25.0..=200.0).prefix("Цель: ").suffix("%"));
                    self.smart_draft.step=step/100.0;self.smart_draft.target=target/100.0;
                });
                ui.small("Серия удачных проходов повышает скорость. Ошибки ритма и удержания уменьшают оценку для серии.");
            }
            ui.small("Изменения применяются кнопкой и завершают текущую попытку. Отдельная рука, дорожки и цикл не считаются полной пьесой.");
            ui.horizontal(|ui| {
                if ui.button("Применить").clicked() && let Err(e)=self.apply_lesson_plan(index){self.error=e.to_string();}
                if ui.button("Вся пьеса").clicked(){self.plan_draft=Plan::default();if let Err(e)=self.apply_lesson_plan(index){self.error=e.to_string();}}
            });
        });
        if !self.plan.whole(&self.records[index].song) {
            ui.small(format!(
                "Учебная партия: {} · {:.1}–{:.1} с{}",
                match self.plan.hand.as_str() {
                    "L" => "левая рука",
                    "R" => "правая рука",
                    _ => "выбранные дорожки/фрагмент",
                },
                self.plan.start,
                self.plan.end(&self.records[index].song),
                if self.plan.repeat {
                    " · повтор"
                } else {
                    ""
                }
            ));
        }
    }
    fn display_song(&self, index: usize) -> &clavunora_core::score::Song {
        if self.demo.is_some_and(|demo| demo.whole) {
            &self.records[index].song
        } else {
            self.practice_song
                .as_ref()
                .unwrap_or(&self.records[index].song)
        }
    }
    fn start_listening(&mut self, whole: bool) {
        let Some(index) = self.selected else {
            return;
        };
        self.pause_practice();
        self.stop_demo();
        let plan = if whole {
            Plan::default()
        } else {
            self.plan.clone()
        };
        let song = &self.records[index].song;
        self.demo_sequence = Some(std::sync::Arc::new(Sequence::listening(song, &plan)));
        self.playback_epoch = self.playback_epoch.wrapping_add(1);
        self.demo = Some(Listening {
            started: Instant::now(),
            index,
            speed: self.transport.speed,
            start: plan.start,
            end: plan.end(song),
            whole,
        });
        self.sheet_page = None;
        self.status = if whole {
            "Прослушивание всей пьесы: оценки и достижения не начисляются"
        } else {
            "Прослушивание выбранной партии и фрагмента: оценки и достижения не начисляются"
        }
        .into();
        self.update_support_audio();
    }
    fn listening_controls(&mut self, ui: &mut egui::Ui, index: usize) {
        ui.horizontal_wrapped(|ui| {
            let whole_playing = self.demo.is_some_and(|demo| demo.whole);
            if ui
                .button(if whole_playing {
                    "Остановить прослушивание"
                } else {
                    "Послушать всю пьесу"
                })
                .clicked()
            {
                if whole_playing {
                    self.stop_demo();
                } else {
                    self.start_listening(true);
                }
            }
            if !self.plan.whole(&self.records[index].song) {
                let selected_playing = self.demo.is_some_and(|demo| !demo.whole);
                if ui
                    .button(if selected_playing {
                        "Остановить выбранную партию"
                    } else {
                        "Послушать выбранную партию"
                    })
                    .clicked()
                {
                    if selected_playing {
                        self.stop_demo();
                    } else {
                        self.start_listening(false);
                    }
                }
            }
            if let Some(demo) = self.demo {
                ui.small(format!(
                    "Прослушивание · {:.0}% · {:.1} / {:.1} с",
                    demo.speed * 100.0,
                    (demo.position() - demo.start).max(0.0),
                    demo.end - demo.start
                ));
            }
        });
    }
    fn notation_controls(&mut self, ui: &mut egui::Ui, index: usize) {
        let before = self.notation;
        ui.horizontal_wrapped(|ui| {
            ui.label("Вид:");
            ui.selectable_value(
                &mut self.notation.view,
                NotationView::Falling,
                "Падающие ноты",
            );
            ui.selectable_value(&mut self.notation.view, NotationView::Sheet, "Нотный лист");
            ui.selectable_value(&mut self.notation.view, NotationView::Both, "Оба");
            ui.separator();
            ui.label("Названия нот:");
            ui.selectable_value(
                &mut self.notation.language,
                NoteLanguage::Russian,
                "До Ре Ми",
            );
            ui.selectable_value(&mut self.notation.language, NoteLanguage::English, "C D E");
        });
        if self.notation != before {
            self.sheet_page = None;
            if let Err(e) = self
                .store
                .set_preference("rust:notation", &serde_json::json!(self.notation))
            {
                self.error = e.to_string();
            }
        }
        if self.notation.view.sheet()
            && let Some(sequence) = &self.support_sequence
        {
            let position = self
                .demo
                .map_or_else(|| self.transport.current(self.now()), Listening::position);
            let page = SheetPage::new(
                self.display_song(index),
                &sequence.grid,
                ui.available_width(),
                position,
                self.sheet_page,
            );
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        self.demo.is_none() && page.index > 0,
                        egui::Button::new("← Такты"),
                    )
                    .clicked()
                {
                    self.sheet_page = Some(page.index - 1);
                }
                ui.small(format!("Лист {} / {}", page.index + 1, page.count));
                if ui
                    .add_enabled(
                        self.demo.is_none() && page.index + 1 < page.count,
                        egui::Button::new("Такты →"),
                    )
                    .clicked()
                {
                    self.sheet_page = Some(page.index + 1);
                }
                if ui.button("К текущей позиции").clicked() {
                    self.sheet_page = None;
                }
            });
        }
    }
    fn music_views(&self, ui: &mut egui::Ui, index: usize, position: f64) {
        let height = if self.performance_view {
            (ui.available_height() - self.keyboard_height(ui) - 20.0).max(60.0)
        } else {
            (ui.clip_rect().bottom()
                - ui.cursor().top()
                - if self.notation.view.sheet() {
                    260.0
                } else {
                    220.0
                })
            .clamp(
                60.0,
                if self.notation.view.sheet() {
                    320.0
                } else {
                    200.0
                },
            )
        };
        let both = self.notation.view == NotationView::Both;
        if self.notation.view.sheet()
            && let Some(sequence) = &self.support_sequence
        {
            crate::staff_view::show(
                ui,
                self.display_song(index),
                &sequence.grid,
                position,
                crate::staff_view::Options {
                    language: self.notation.language,
                    dark: self.dark || self.performance_view,
                    fingers: self.show_fingers,
                    height: if both { height * 0.65 } else { height },
                    page: if self.performance_view || self.demo.is_some() {
                        None
                    } else {
                        self.sheet_page
                    },
                },
            );
        }
        if self.notation.view.falling() {
            self.roll(
                ui,
                index,
                position,
                if both { height * 0.35 } else { height },
            );
        }
    }
    fn practice(&mut self, ui: &mut egui::Ui) {
        let Some(index) = self.selected else {
            ui.label("Выберите пьесу в библиотеке или обучении.");
            return;
        };
        ui.add(
            egui::Label::new(RichText::new(&self.records[index].song.title).heading()).truncate(),
        )
        .on_hover_text(&self.records[index].song.title);
        if self.midi.waiting() {
            ui.label(&self.midi.message);
        }
        if let Some(notice) = &self.repair_notice {
            ui.add(egui::Label::new(notice).truncate())
                .on_hover_text(notice);
        }
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(if self.running {
                    "Пауза"
                } else {
                    "Начать / продолжить"
                })
                .clicked()
            {
                if self.running {
                    self.pause_practice();
                } else {
                    self.start_practice();
                }
            }
            if ui.button("Сначала").clicked() {
                self.pause_practice();
                let _ = self
                    .transport
                    .seek((self.plan.start - 1.0).max(-3.0), self.now());
                self.repeat_progress = RepeatProgress::default();
                self.last_report = None;
                self.state.panic();
                self.audio.silence();
            }
            let mut speed = self.transport.speed;
            if ui
                .add(egui::Slider::new(&mut speed, 0.25..=2.0).text("Скорость"))
                .changed()
            {
                self.pause_practice();
                let _ = self.transport.set_speed(speed, self.now());
            }
            let mut waiting = self.wait_mode;
            ui.selectable_value(&mut waiting, true, "С ожиданием");
            ui.selectable_value(&mut waiting, false, "Ритм — без остановок");
            if waiting != self.wait_mode {
                self.pause_practice();
                self.wait_mode = waiting;
            }
        });
        self.listening_controls(ui, index);
        let position = self.demo.map_or_else(
            || self.transport.current(self.now()),
            |demo| demo.position(),
        );
        ui.label(format!(
            "{:.1} / {:.1} сек · сыграно групп: {}",
            position,
            self.records[index].song.duration(),
            self.session.as_ref().map_or(0, Session::completed_groups)
        ));
        if self.waiting {
            let required = self
                .session
                .as_ref()
                .and_then(Session::expected)
                .map(|g| {
                    g.pitches
                        .iter()
                        .map(|p| self.notation.language.name(*p))
                        .collect::<Vec<_>>()
                        .join(" + ")
                })
                .unwrap_or_default();
            ui.label(format!("Ожидаю: {required}"));
            if ui.button("Пропустить ноту / аккорд").clicked()
                && self.session.as_mut().is_some_and(Session::skip_waiting)
            {
                self.waiting = false;
                let _ = self.transport.play(self.now());
                self.status = "Пропуск отмечен в результате, продолжаем".into();
            }
        }
        self.music_views(ui, index, position);
        self.keyboard(ui);
        self.notation_controls(ui, index);
        egui::CollapsingHeader::new("Настройки и разучивание").default_open(self.native_smoke && self.capture.is_some()).show(ui, |ui| {
        if ui.button("Руки и аппликатура").clicked() {
            self.open_editor(index);
        }
        self.lesson_controls(ui, index);
        self.support_controls(ui,index);
        ui.collapsing("О пьесе и источнике", |ui| {
            let song = &self.records[index].song;
            ui.label(&song.composer);
            ui.label(&song.description);
            ui.label(&song.source);
        });
        ui.collapsing(
            "Допуск ритма и компенсация задержки",
            |ui| {
                ui.label("Изменение завершает текущую попытку; нажмите «Продолжить» для новой.");
                let mut tolerance = self.tolerance * 1000.0;
                let mut latency = self.latency * 1000.0;
                if ui
                    .add(egui::Slider::new(&mut tolerance, 30.0..=1000.0).text("Допуск, мс"))
                    .changed()
                {
                    self.pause_practice();
                    self.tolerance = tolerance / 1000.0;
                }
                if ui
                    .add(egui::Slider::new(&mut latency, -500.0..=500.0).text("Задержка, мс"))
                    .changed()
                {
                    self.pause_practice();
                    self.latency = latency / 1000.0;
                }
            },
        );
        ui.small(if self.wait_mode{"Ожидание прощает небольшое опережение и несовпадение нажатий аккорда. Длительность удержания не останавливает игру."}else{"Ритм: движение не останавливается из-за ошибок; пропуски и неточности попадут в результат."});
        self.keyboard_controls(ui, false);
        });
        let outside = self.records[index]
            .song
            .notes
            .iter()
            .filter(|n| {
                n.pitch < self.keyboard_config.first || n.pitch > self.keyboard_config.end()
            })
            .count();
        if outside > 0 {
            ui.small(format!("За пределами видимого диапазона: {outside} нот. Диапазон меняется в настройках клавиатуры."));
        }
        if self.show_fingers {
            let expected = self
                .session
                .as_ref()
                .and_then(Session::expected)
                .map_or(position, |g| g.start);
            let hints: Vec<_> = self
                .practice_song
                .as_ref()
                .unwrap_or(&self.records[index].song)
                .notes
                .iter()
                .filter(|n| n.start <= expected + 0.001 && n.end() > expected && n.finger > 0)
                .map(|n| {
                    format!(
                        "{}: {} {}",
                        self.notation.language.name(n.pitch),
                        if n.hand == "L" {
                            "Л"
                        } else if n.hand == "R" {
                            "П"
                        } else {
                            "палец"
                        },
                        n.finger
                    )
                })
                .collect();
            ui.small(if hints.is_empty(){"Аппликатура: 1 — большой палец, 5 — мизинец. Подсказки есть только для размеченных нот.".into()}else{hints.join(" · ")});
        }
        ui.small("Точность — доля найденных нот/групп, а не оценка музыкального исполнения. Ошибки ритма и удержания показаны отдельно.");
        if let Some(report) = &self.last_report {
            ui.label(format!(
                "Последний результат: {}% · {} / {} · {}",
                report.outcome.accuracy,
                report.outcome.hit,
                report.outcome.total,
                if report.outcome.complete {
                    "завершено"
                } else {
                    "прервано"
                }
            ));
        }
    }
    fn leave_performance(&mut self) {
        self.pause_practice();
        self.performance_view = false;
        self.reset_page_scroll = true;
        self.state.panic();
        self.computer_notes.clear();
        self.mouse_note = None;
        self.audio.silence();
    }
    fn sync_performance_view(&mut self, ctx: &egui::Context) {
        let use_native_fullscreen = !self.native_smoke || self.capture.is_some();
        if !self.performance_view {
            self.fullscreen_focus_until = None;
        }
        if !self.save_queue.is_empty() {
            self.performance_view = false;
        }
        if self.performance_view
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.leave_performance();
        }
        if self.performance_view && self.fullscreen_restore.is_none() {
            self.fullscreen_restore = Some(ctx.input(|i| i.viewport().fullscreen.unwrap_or(false)));
            if use_native_fullscreen {
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
            }
        } else if !self.performance_view
            && let Some(previous) = self.fullscreen_restore.take()
            && use_native_fullscreen
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(previous));
        }
    }
    fn performance(&mut self, ui: &mut egui::Ui) {
        let Some(index) = self.selected else {
            self.performance_view = false;
            return;
        };
        // Instrument canvas takes the entire viewport; no page chrome or scroll area.
        aero::style(ui.ctx(), true);
        ui.set_style(ui.ctx().global_style());
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.painter()
            .rect_filled(ui.max_rect(), 0, Color32::from_rgb(16, 27, 38));
        self.keys(ui);
        let position = self.transport.current(self.now());
        self.music_views(ui, index, position);
        self.keyboard(ui);
        egui::Area::new(egui::Id::new("performance-exit"))
            .anchor(egui::Align2::RIGHT_TOP, Vec2::new(-12.0, 12.0))
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                if ui
                    .button(if cfg!(target_os = "android") {
                        "‹ К занятию"
                    } else {
                        "Esc · назад"
                    })
                    .on_hover_text("Пауза и возврат к настройкам занятия")
                    .clicked()
                {
                    self.leave_performance();
                }
            });
        if self.waiting || !self.running {
            let required = self
                .session
                .as_ref()
                .and_then(Session::expected)
                .map(|g| {
                    g.pitches
                        .iter()
                        .map(|p| self.notation.language.name(*p))
                        .collect::<Vec<_>>()
                        .join(" + ")
                })
                .unwrap_or_default();
            egui::Area::new(egui::Id::new("performance-state"))
                .anchor(egui::Align2::LEFT_TOP, Vec2::new(12.0, 12.0))
                .order(egui::Order::Foreground)
                .show(ui.ctx(), |ui| {
                    if self.waiting {
                        ui.label(format!("Ожидаю: {required}"));
                        if ui.small_button("Пропустить").clicked()
                            && self.session.as_mut().is_some_and(Session::skip_waiting)
                        {
                            self.waiting = false;
                            let _ = self.transport.play(self.now());
                        }
                    } else {
                        ui.label("Пауза · Esc — к настройкам");
                    }
                });
        }
        ui.ctx().request_repaint_after(Duration::from_millis(16));
    }
    fn keyboard_height(&self, ui: &egui::Ui) -> f32 {
        if self.performance_view {
            ((ui.clip_rect().height() * 0.24).clamp(64.0, 260.0)
                * self.keyboard_config.zoom.max(0.5))
            .min((ui.clip_rect().height() * 0.48).max(32.0))
        } else {
            ((if self.page == Page::FreePlay {
                190.0
            } else {
                132.0
            }) * self.keyboard_config.zoom)
                .max(64.0)
        }
    }
    fn roll(&self, ui: &mut egui::Ui, index: usize, position: f64, height: f32) {
        let width = ui.available_width().max(200.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(
            rect,
            4,
            if self.dark || self.performance_view {
                Color32::from_rgb(23, 37, 52)
            } else {
                Color32::from_rgb(228, 239, 248)
            },
        );
        let keys = self.keyboard_config.keys(
            self.keyboard_config
                .bounds(rect.shrink2(Vec2::new(10.0, 0.0))),
        );
        let lookahead = 4.0 / f64::from(self.keyboard_config.zoom);
        for n in &self.display_song(index).notes {
            if n.end() < position || n.start > position + lookahead {
                continue;
            }
            let Some(key) = keys.iter().find(|k| k.pitch == n.pitch) else {
                continue;
            };
            let bottom = rect.bottom() - ((n.start - position) / lookahead) as f32 * rect.height();
            let top = bottom - (n.duration / lookahead) as f32 * rect.height();
            let note_rect = Rect::from_min_max(
                Pos2::new(key.rect.left() + 0.5, top.max(rect.top())),
                Pos2::new(key.rect.right() - 0.5, bottom.min(rect.bottom())),
            );
            if note_rect.is_positive() {
                painter.rect_filled(
                    note_rect,
                    3,
                    if n.hand == "L" {
                        Color32::from_rgb(169, 156, 218)
                    } else {
                        Color32::from_rgb(95, 184, 157)
                    },
                );
                if self.show_fingers
                    && n.finger > 0
                    && note_rect.width() > 10.0
                    && note_rect.height() > 12.0
                {
                    painter.text(
                        note_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        n.finger,
                        egui::FontId::proportional(12.0),
                        Color32::from_rgb(16, 51, 68),
                    );
                }
            }
        }
    }
    fn keyboard(&mut self, ui: &mut egui::Ui) -> Rect {
        let mut pressed = self.state.pressed();
        if let Some(demo) = self.demo {
            let time = demo.position();
            pressed.extend(
                self.display_song(demo.index)
                    .notes
                    .iter()
                    .filter(|n| n.start <= time && n.end() > time)
                    .map(|n| n.pitch),
            );
        }
        let height = self.keyboard_height(ui);
        let height = if self.page == Page::Practice && !self.performance_view {
            height.min((ui.clip_rect().bottom() - ui.cursor().top() - 30.0).max(44.0))
        } else {
            height
        };
        let (frame, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), height + 20.0),
            Sense::drag(),
        );
        if let Some(aero) = &self.aero {
            aero.glass(ui.painter(), frame, self.dark, false);
        }
        let rect = self.keyboard_config.bounds(frame.shrink(10.0));
        let keys = self.keyboard_config.keys(rect);
        let mut key_rects = vec![];
        for item in keys.iter().filter(|k| !k.black) {
            let pitch = item.pitch;
            let key = item.rect.shrink2(Vec2::new(0.5, 0.0));
            key_rects.push((pitch, key));
            ui.painter().rect_filled(
                key,
                4,
                if pressed.contains(&pitch) {
                    Color32::from_rgb(87, 217, 231)
                } else {
                    Color32::from_rgb(241, 248, 249)
                },
            );
            ui.painter().rect_stroke(
                key,
                4,
                egui::Stroke::new(1.0_f32, Color32::from_rgb(132, 167, 182)),
                egui::StrokeKind::Inside,
            );
            ui.painter().rect_filled(
                Rect::from_min_max(
                    Pos2::new(key.left() + 2.0, key.bottom() - 9.0),
                    key.max - Vec2::splat(2.0),
                ),
                3,
                if pressed.contains(&pitch) {
                    Color32::from_rgb(27, 167, 188)
                } else {
                    Color32::from_rgb(203, 222, 229)
                },
            );
            if key.width() > 27.0 || (pitch % 12 == 0 && key.width() >= 14.0) {
                ui.painter().text(
                    key.center_bottom() - Vec2::new(0.0, 22.0),
                    egui::Align2::CENTER_CENTER,
                    self.notation.language.name(pitch),
                    egui::FontId::proportional(13.0),
                    Color32::from_rgb(40, 79, 96),
                );
            }
        }
        let mut black_rects = vec![];
        for item in keys.iter().filter(|k| k.black) {
            let pitch = item.pitch;
            let key = item.rect;
            black_rects.push((pitch, key));
            ui.painter().rect_filled(
                key.translate(Vec2::new(1.0, 3.0)),
                4,
                Color32::from_black_alpha(90),
            );
            ui.painter().rect_filled(
                key,
                4,
                if pressed.contains(&pitch) {
                    Color32::from_rgb(23, 167, 196)
                } else {
                    Color32::from_rgb(17, 42, 59)
                },
            );
            ui.painter().rect_filled(
                key.shrink2(Vec2::new(
                    (key.width() * 0.15).min(5.0),
                    (key.height() * 0.15).min(8.0),
                )),
                3,
                if pressed.contains(&pitch) {
                    Color32::from_rgb(58, 206, 225)
                } else {
                    Color32::from_rgb(36, 65, 84)
                },
            );
            ui.painter().line_segment(
                [
                    key.left_top() + Vec2::new((key.width() * 0.15).min(5.0), 5.0),
                    key.right_top() + Vec2::new(-(key.width() * 0.15).min(5.0), 5.0),
                ],
                egui::Stroke::new(1.0_f32, Color32::from_white_alpha(130)),
            );
        }
        let events = ui.input(|i| i.events.clone());
        let has_touch = events
            .iter()
            .any(|event| matches!(event, egui::Event::Touch { .. }));
        for event in self.touch_keyboard.update(&events, &keys) {
            self.feed(event);
        }
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let mouse = if !has_touch
            && !self.touch_keyboard.active()
            && response.is_pointer_button_down_on()
        {
            pointer.and_then(|pos| {
                black_rects
                    .iter()
                    .chain(key_rects.iter())
                    .find(|(_, r)| r.contains(pos))
                    .map(|(pitch, _)| *pitch)
            })
        } else {
            None
        };
        if mouse != self.mouse_note {
            if let Some(pitch) = self.mouse_note.take() {
                self.feed(MidiEvent {
                    source: "mouse".into(),
                    channel: 0,
                    message: Message::NoteOff { pitch },
                });
            }
            if let Some(pitch) = mouse {
                self.feed(MidiEvent {
                    source: "mouse".into(),
                    channel: 0,
                    message: Message::NoteOn {
                        pitch,
                        velocity: 90,
                    },
                });
                self.mouse_note = Some(pitch);
            }
        }
        if !self.performance_view {
            ui.small("Клавиши компьютера: A W S E D F T G Y H U J K · C4–C5. Пробел — Sustain.");
        }
        frame
    }
    fn freeplay(&mut self, ui: &mut egui::Ui) {
        ui.heading("Свободная игра");
        ui.label("Играйте, пробуйте аккорды и находите свою мелодию.");
        ui.horizontal_wrapped(|ui| {
            ui.label(if self.midi.connected() {
                "● MIDI подключён"
            } else {
                "○ Экранный и компьютерный ввод"
            });
            ui.separator();
            ui.label(format!("Динамика: {} / 127", self.state.last_velocity));
            ui.separator();
            ui.label(if self.state.pedal_active() {
                "Sustain включён"
            } else {
                "Sustain выключен"
            });
        });
        if self.midi.waiting() {
            ui.label(&self.midi.message);
        }
        self.keyboard_controls(ui, false);
        ui.add_space(28.0);
        let names = self
            .state
            .sounding()
            .iter()
            .map(|p| self.notation.language.name(*p))
            .collect::<Vec<_>>()
            .join(" · ");
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(if names.is_empty() {
                    "Сыграйте любую ноту"
                } else {
                    &names
                })
                .size(28.0),
            );
        });
        ui.add_space(28.0);
        self.keyboard(ui);
        ui.add_space(18.0);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add(egui::Slider::new(&mut self.audio_config.volume, 0.0..=1.0).text("Громкость"))
                .changed()
            {
                self.audio
                    .mix(self.audio_config.enabled, self.audio_config.volume);
                self.save_audio_config();
            }
            if ui
                .checkbox(&mut self.audio_config.enabled, "Звук")
                .changed()
            {
                self.audio
                    .mix(self.audio_config.enabled, self.audio_config.volume);
                self.save_audio_config();
            }
            if ui.button("Сбросить ноты").clicked() {
                self.state.panic();
                self.audio.silence();
                self.computer_notes.clear();
                self.mouse_note = None;
            }
        });
    }
    fn achievements(&mut self, ui: &mut egui::Ui) {
        ui.heading("Достижения");
        ui.label("Каждый шаг к музыке заслуживает своей награды.");
        match self.store.achievement_state() {
            Ok(state) => {
                let columns = if ui.available_width() < 720.0 { 2 } else { 3 };
                let width = (ui.available_width() - 14.0 * (columns - 1) as f32) / columns as f32;
                for row in (0..clavunora_core::achievements::DEFINITIONS.len())
                    .collect::<Vec<_>>()
                    .chunks(columns)
                {
                    ui.horizontal(|ui| {
                        for &i in row {
                            let def = &clavunora_core::achievements::DEFINITIONS[i];
                            let earned = state
                                .earned
                                .get(def.key)
                                .and_then(serde_json::Value::as_f64)
                                .is_some_and(|v| v > 0.0 && v <= 32503680000.0);
                            let (rect, _) =
                                ui.allocate_exact_size(Vec2::new(width, 268.0), Sense::hover());
                            if let Some(aero) = &self.aero {
                                aero.glass(ui.painter(), rect, self.dark, false);
                            }
                            let center = Pos2::new(rect.center().x, rect.top() + 55.0);
                            ui.painter().circle_filled(
                                center,
                                34.0,
                                if earned {
                                    Color32::from_rgba_unmultiplied(245, 203, 81, 130)
                                } else {
                                    Color32::from_rgba_unmultiplied(31, 94, 125, 130)
                                },
                            );
                            ui.painter().circle_stroke(
                                center,
                                36.0,
                                egui::Stroke::new(
                                    2.0_f32,
                                    if earned {
                                        Color32::from_rgb(255, 231, 162)
                                    } else {
                                        Color32::from_rgb(177, 214, 228)
                                    },
                                ),
                            );
                            aero::paint_icon(
                                ui.painter(),
                                Rect::from_center_size(center, Vec2::splat(47.0)),
                                [
                                    Icon::Note,
                                    Icon::Library,
                                    Icon::Star,
                                    Icon::Piano,
                                    Icon::Chart,
                                    Icon::Course,
                                ][i % 6],
                                earned,
                            );
                            let content = Rect::from_min_max(
                                Pos2::new(rect.left() + 14.0, rect.top() + 102.0),
                                rect.max - Vec2::new(14.0, 12.0),
                            );
                            ui.scope_builder(
                                egui::UiBuilder::new()
                                    .id_salt(("award", i))
                                    .max_rect(content),
                                |ui| {
                                    ui.set_clip_rect(ui.clip_rect().intersect(content));
                                    ui.set_max_width(content.width());
                                    ui.vertical_centered(|ui| {
                                        ui.label(RichText::new(def.title).size(17.0).strong());
                                        ui.small(def.description);
                                        ui.add(
                                            egui::ProgressBar::new(
                                                state.progress[i] as f32 / def.target as f32,
                                            )
                                            .fill(if earned {
                                                Color32::from_rgb(206, 163, 48)
                                            } else {
                                                Color32::from_rgb(26, 165, 198)
                                            })
                                            .text(
                                                format!("{} / {}", state.progress[i], def.target),
                                            ),
                                        );
                                        if earned {
                                            ui.small("Награда получена");
                                        }
                                    });
                                },
                            );
                            ui.advance_cursor_after_rect(rect);
                        }
                    });
                    ui.add_space(6.0);
                }
            }
            Err(e) => self.error = e.to_string(),
        }
    }
    fn practice_error(&mut self, attempt_id: &str, error_index: usize) -> Result<()> {
        // Re-read on click: a cached library may predate an edit in another window.
        let records = self.store.songs()?;
        let attempt = self
            .store
            .attempts()?
            .into_iter()
            .find(|a| a.id == attempt_id)
            .ok_or_else(|| anyhow::anyhow!("Попытка больше не найдена в истории"))?;
        let index = records
            .iter()
            .position(|r| Some(r.id.as_str()) == attempt.song_id.as_deref())
            .ok_or_else(|| anyhow::anyhow!("Пьеса этой попытки удалена или недоступна"))?;
        let repair =
            clavunora_core::error_practice::from_attempt(&records[index], &attempt, error_index)?;
        let selected = repair.plan.selected(&records[index].song)?;
        let mut transport = Transport::new(repair.plan.end(&records[index].song) + 0.5)?;
        transport.set_speed(repair.speed, self.now())?;
        transport.seek((repair.plan.start - 1.0).max(-3.0), self.now())?;
        self.pause_practice();
        self.records = records;
        self.open_song(index);
        self.plan = repair.plan.clone();
        self.plan_draft = repair.plan;
        self.practice_song = Some(selected);
        self.support_sequence = Some(std::sync::Arc::new(Sequence::accompaniment(
            &self.records[index].song,
            &self.plan,
        )));
        self.transport = transport;
        self.wait_mode = repair.mode == Mode::Waiting;
        self.repair_notice = Some(format!(
            "Разбор ошибки «{}» · {} · {:.2} с · фрагмент {:.2}–{:.2} с",
            repair.kind,
            note_name(repair.pitch),
            repair.position,
            self.plan.start,
            self.plan.end(&self.records[index].song)
        ));
        self.status = format!(
            "Фрагмент готов: {:.0}% скорости, повтор включён. Нажмите «Начать».",
            repair.speed * 100.0
        );
        Ok(())
    }
    fn progress(&mut self, ui: &mut egui::Ui) {
        let mut repair = None;
        ui.heading("История занятий");
        ui.label("В ожидании считаются группы нот; в ритме — физические клавиши. Пауза сохраняет незавершённую попытку. Проценты разных режимов не равнозначны.");
        match self.store.attempts() {
            Ok(rows) => {
                if rows.is_empty() {
                    ui.label(
                        "Пока нет занятий. Сыграйте первую пьесу — здесь появятся результаты.",
                    );
                    return;
                }
                ui.collapsing("Сводка занятий", |ui| {
                    let complete = rows
                        .iter()
                        .filter(|r| r.report["complete"].as_bool() == Some(true))
                        .count();
                    let pieces = rows
                        .iter()
                        .filter_map(|r| r.song_id.as_deref())
                        .collect::<BTreeSet<_>>()
                        .len();
                    let stats = [
                        ("Занятия", rows.len()),
                        ("Завершённые занятия", complete),
                        ("Пьесы с результатом", pieces),
                    ];
                    let width = (ui.available_width() - 24.0) / 3.0;
                    ui.horizontal(|ui| {
                        for (title, count) in stats {
                            let (rect, _) =
                                ui.allocate_exact_size(Vec2::new(width, 90.0), Sense::hover());
                            if let Some(aero) = &self.aero {
                                aero.glass(ui.painter(), rect, self.dark, false);
                            }
                            ui.painter().text(
                                rect.center_top() + Vec2::new(0.0, 24.0),
                                egui::Align2::CENTER_CENTER,
                                title,
                                egui::FontId::proportional(14.0),
                                aero::muted(self.dark),
                            );
                            ui.painter().text(
                                rect.center_bottom() - Vec2::new(0.0, 28.0),
                                egui::Align2::CENTER_CENTER,
                                count,
                                egui::FontId::proportional(28.0),
                                aero::text(self.dark),
                            );
                        }
                    });
                    ui.add_space(8.0);
                    if rows.is_empty() {
                        ui.label(
                        "Пока нет занятий. Сыграйте первую пьесу — здесь появятся ваши результаты.",
                    );
                    } else {
                        ui.label("Точность последних занятий");
                        let (rect, _) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), 132.0),
                            Sense::hover(),
                        );
                        let recent: Vec<_> = rows.iter().take(7).rev().collect();
                        let unit = rect.width() / recent.len() as f32;
                        for (i, row) in recent.iter().enumerate() {
                            let value = row.report["accuracy"]
                                .as_f64()
                                .unwrap_or(0.0)
                                .clamp(0.0, 100.0) as f32;
                            let x = rect.left() + unit * i as f32;
                            let bar = Rect::from_min_max(
                                Pos2::new(x + unit * 0.2, rect.bottom() - 25.0 - value * 0.85),
                                Pos2::new(x + unit * 0.8, rect.bottom() - 25.0),
                            );
                            ui.painter()
                                .rect_filled(bar, 5, Color32::from_rgb(42, 176, 196));
                            ui.painter().text(
                                Pos2::new(x + unit / 2.0, bar.top() - 10.0),
                                egui::Align2::CENTER_CENTER,
                                format!("{value:.0}%"),
                                egui::FontId::proportional(13.0),
                                aero::text(self.dark),
                            );
                            ui.painter().text(
                                Pos2::new(x + unit / 2.0, rect.bottom() - 9.0),
                                egui::Align2::CENTER_CENTER,
                                row.date.get(..5).unwrap_or(&row.date),
                                egui::FontId::proportional(12.0),
                                aero::muted(self.dark),
                            );
                        }
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("attempt-selector")
                    .max_height(180.0)
                    .show(ui, |ui| {
                        for row in &rows {
                            let text = format!(
                                "{} · {} · {} · {}% · {}% скорости · {}",
                                row.date,
                                row.report["title"].as_str().unwrap_or("Занятие"),
                                row.report["mode"].as_str().unwrap_or(""),
                                row.report["accuracy"],
                                row.report["speed"].as_f64().unwrap_or(1.0) * 100.0,
                                if row.report["complete"].as_bool() == Some(true) {
                                    "завершено"
                                } else {
                                    "прервано"
                                }
                            );
                            if ui
                                .selectable_label(
                                    self.selected_history.as_deref() == Some(row.id.as_str()),
                                    text,
                                )
                                .clicked()
                            {
                                self.selected_history = Some(row.id.clone());
                                self.error_page = 0;
                            }
                        }
                    });
                if let Some(row) = rows
                    .iter()
                    .find(|r| Some(r.id.as_str()) == self.selected_history.as_deref())
                {
                    ui.separator();
                    ui.label(format!(
                        "Партия: {} · {:.1}–{:.1} с{}",
                        row.report["hand"]
                            .as_str()
                            .filter(|s| !s.is_empty())
                            .unwrap_or("все руки"),
                        row.report["start"].as_f64().unwrap_or(0.0),
                        row.report["end"].as_f64().unwrap_or(0.0),
                        if row.report["repeated"].as_bool() == Some(true) {
                            " · повтор"
                        } else {
                            ""
                        }
                    ));
                    if let Some(tracks) = row.report["tracks"].as_array() {
                        ui.label(format!(
                            "Дорожки: {}",
                            tracks
                                .iter()
                                .filter_map(|t| t.as_u64())
                                .map(|t| (t + 1).to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                    ui.heading("Ошибки выбранной попытки");
                    let unavailable = self
                        .records
                        .iter()
                        .find(|r| Some(r.id.as_str()) == row.song_id.as_deref())
                        .map_or_else(
                            || Some("Пьеса удалена или недоступна".to_owned()),
                            |r| {
                                clavunora_core::error_practice::check_version(r, row)
                                    .err()
                                    .map(|e| e.to_string())
                            },
                        );
                    if let Some(reason) = &unavailable {
                        ui.label(reason);
                    }
                    ui.small("«Разучить этот момент» откроет фрагмент с повтором и сниженной скоростью. Минимум — 25%.");

                    if let Some(errors) = row.report["mistakes"].as_array() {
                        if errors.is_empty() {
                            ui.label(if row.report["complete"].as_bool() == Some(true) {
                                "Ошибок не зарегистрировано"
                            } else {
                                "Прерванная попытка: итоговая оценка ограничена"
                            });
                        }
                        let pages = errors.len().div_ceil(20).max(1);
                        self.error_page = self.error_page.min(pages - 1);
                        if pages > 1 {
                            ui.horizontal(|ui| {
                                if ui
                                    .add_enabled(
                                        self.error_page > 0,
                                        egui::Button::new("Предыдущие"),
                                    )
                                    .clicked()
                                {
                                    self.error_page -= 1;
                                }
                                ui.label(format!(
                                    "Ошибки: страница {} / {pages}",
                                    self.error_page + 1
                                ));
                                if ui
                                    .add_enabled(
                                        self.error_page + 1 < pages,
                                        egui::Button::new("Следующие"),
                                    )
                                    .clicked()
                                {
                                    self.error_page += 1;
                                }
                            });
                        }
                        for (error_index, error) in errors
                            .iter()
                            .enumerate()
                            .skip(self.error_page * 20)
                            .take(20)
                        {
                            ui.push_id(error_index, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(format!(
                                        "{:.2} с · {} · {} · {:+.0} мс",
                                        error["position"].as_f64().unwrap_or(0.0),
                                        note_name(
                                            error["pitch"].as_u64().unwrap_or(0).min(127) as u8
                                        ),
                                        error["kind"].as_str().unwrap_or(""),
                                        error["delta"].as_f64().unwrap_or(0.0) * 1000.0
                                    ));
                                    let button = ui.add_enabled(
                                        unavailable.is_none(),
                                        egui::Button::new("Разучить этот момент"),
                                    );
                                    if button.clicked() {
                                        repair = Some((row.id.clone(), error_index));
                                    }
                                    if let Some(reason) = &unavailable {
                                        button.on_disabled_hover_text(reason);
                                    }
                                });
                            });
                        }
                    } else {
                        ui.label("В старой записи нет подробностей об ошибках.");
                    }
                }
            }
            Err(e) => self.error = e.to_string(),
        }
        if let Some((id, index)) = repair
            && let Err(e) = self.practice_error(&id, index)
        {
            self.error = e.to_string();
        }
    }
    fn stop_demo(&mut self) {
        if self.demo.take().is_some() {
            self.status = "Прослушивание остановлено".into();
        }
        self.demo_sequence = None;
        self.audio.preview(&[]);
        self.audio.playback(None);
    }
    fn save_audio_config(&mut self) {
        if let Err(e) = self
            .store
            .set_preference("rust:audio", &serde_json::json!(self.audio_config))
        {
            self.error = e.to_string();
        }
    }
    fn update_audio(&mut self) {
        self.audio.sync(&self.state, None);
        if self.demo.is_some_and(|demo| demo.position() >= demo.end) {
            self.stop_demo();
            self.status = "Прослушивание завершено. Нажмите «Начать», когда будете готовы".into();
        }
        self.update_support_audio();
    }
    fn update_support_audio(&mut self) {
        let anchor = Instant::now();
        let now = anchor.saturating_duration_since(self.started).as_secs_f64();
        let playback = if self.running && self.transport.playing {
            self.support_sequence
                .as_ref()
                .map(|sequence| crate::audio::Playback {
                    sequence: sequence.clone(),
                    epoch: self.playback_epoch,
                    anchor,
                    position: self.transport.current(now),
                    speed: self.transport.speed,
                    limit: if self.wait_mode {
                        self.session.as_ref().and_then(Session::expected).map_or(
                            self.plan.end(&self.records[self.selected.unwrap()].song),
                            |g| g.start,
                        )
                    } else {
                        self.plan.end(&self.records[self.selected.unwrap()].song)
                    },
                    metronome: self.lesson_audio.metronome,
                    volume: if self.lesson_audio.accompaniment {
                        self.lesson_audio.volume
                    } else {
                        0.0
                    },
                })
        } else if let Some(demo) = self.demo {
            self.demo_sequence
                .as_ref()
                .map(|sequence| crate::audio::Playback {
                    sequence: sequence.clone(),
                    epoch: self.playback_epoch,
                    anchor: demo.started,
                    position: demo.start,
                    speed: demo.speed,
                    limit: demo.end,
                    metronome: self.lesson_audio.metronome,
                    volume: 1.0,
                })
        } else if self.metronome && matches!(self.page, Page::FreePlay | Page::Settings) {
            Some(crate::audio::Playback {
                sequence: self.standalone_sequence.clone(),
                epoch: self.playback_epoch,
                anchor: self.metronome_anchor,
                position: 0.0,
                speed: 1.0,
                limit: f64::INFINITY,
                metronome: true,
                volume: 0.0,
            })
        } else {
            None
        };
        self.audio.playback(playback);
    }
    fn save_lesson_audio(&mut self) {
        if let Err(e) = self
            .store
            .set_preference("rust:lesson-audio", &serde_json::json!(self.lesson_audio))
        {
            self.error = e.to_string();
        }
    }
    fn support_controls(&mut self, ui: &mut egui::Ui, index: usize) {
        let heading = ui.heading("Метроном и сопровождение");
        if self.capture_support {
            heading.scroll_to_me_animation(
                Some(egui::Align::Center),
                egui::style::ScrollAnimation::none(),
            );
        }
        let mut config = self.lesson_audio.clone();
        let mut changed = ui
            .checkbox(&mut config.metronome, "Метроном по темпу пьесы")
            .changed();
        let count = self.support_sequence.as_ref().map_or(0, |s| s.note_count);
        changed |= ui
            .add_enabled(
                count > 0,
                egui::Checkbox::new(&mut config.accompaniment, "Играть другую партию"),
            )
            .changed();
        if count == 0 {
            ui.small("Выберите одну руку или часть дорожек и нажмите «Применить». Для сопровождения другой рукой нужна разметка рук.");
        } else {
            ui.small(format!("Сопровождение: {count} нот. При выборе руки играет противоположная рука, при выборе дорожек — остальные дорожки."));
        }
        changed |= ui
            .add(egui::Slider::new(&mut config.volume, 0.0..=1.0).text("Громкость сопровождения"))
            .changed();
        if changed {
            self.pause_practice();
            self.stop_demo();
            self.lesson_audio = config;
            self.save_lesson_audio();
        }
        let song = &self.records[index].song;
        ui.small(format!("Темп {:.0} BPM · размер {}/{}. Метроном следует карте темпа и скорости занятия. В ожидании оба помощника останавливаются; сопровождение не засчитывается как ваша игра.",song.bpm,song.time_signature[0],song.time_signature[1]));
    }

    fn save_keyboard_config(&mut self) {
        if let Some(pitch) = self.mouse_note.take() {
            self.feed(MidiEvent {
                source: "mouse".into(),
                channel: 0,
                message: Message::NoteOff { pitch },
            });
        }
        if let Err(e) = self.store.set_preference(
            "rust:keyboard-view",
            &serde_json::json!(self.keyboard_config),
        ) {
            self.error = e.to_string();
        }
    }
    fn keyboard_controls(&mut self, ui: &mut egui::Ui, full: bool) {
        let previous = self.keyboard_config.clone();
        ui.horizontal_wrapped(|ui| {
            if full {
                let mut count = self.keyboard_config.count;
                egui::ComboBox::from_id_salt("keyboard-count")
                    .selected_text(format!("{count} клавиш"))
                    .show_ui(ui, |ui| {
                        for n in keyboard_view::COUNTS {
                            ui.selectable_value(&mut count, n, format!("{n} клавиш"));
                        }
                    });
                if count != self.keyboard_config.count {
                    self.keyboard_config.preset(count);
                }
            }
            let mut zoom = self.keyboard_config.zoom * 100.0;
            if ui
                .add(egui::Slider::new(&mut zoom, 25.0..=100.0).text("Масштаб клавиш, %"))
                .changed()
            {
                self.keyboard_config.zoom = zoom / 100.0;
            }
            if ui.button("Вписать").clicked() {
                self.keyboard_config.zoom = 1.0;
            }
        });
        if full {
            ui.add(
                egui::Slider::new(
                    &mut self.keyboard_config.first,
                    0..=(128 - self.keyboard_config.count),
                )
                .text("Начальная MIDI-нота"),
            );
            ui.small("Количество включает белые и чёрные клавиши. Масштаб уменьшает клавиатуру и расширяет видимую по времени область падающих нот.");
            ui.small("Это настройка отображения: диапазон внешнего инструмента и оценка игры не ограничиваются.");
        }
        ui.small(format!(
            "Диапазон: {} — {} · {} клавиш",
            note_name(self.keyboard_config.first),
            note_name(self.keyboard_config.end()),
            self.keyboard_config.count
        ));
        if previous != self.keyboard_config {
            self.save_keyboard_config();
        }
    }
    fn prepare_backup(&mut self) -> Result<std::path::PathBuf> {
        self.pause_practice();
        self.stop_demo();
        self.retry_saves();
        anyhow::ensure!(
            self.save_queue.is_empty(),
            "Сначала сохраните ожидающие результаты занятия"
        );
        anyhow::ensure!(self.editor.is_none(), "Сначала закройте редактор разметки");
        self.store.database_path()
    }
    fn poll_backups(&mut self) {
        let reply = self.backups.job.as_ref().and_then(|r| match r.try_recv() {
            Ok(reply) => Some(reply),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(_) => Some(Err(
                "Операция с копией прервана. Проверьте текущую библиотеку".into(),
            )),
        });
        let Some(reply) = reply else { return };
        self.backups.job = None;
        match reply {
            Ok(Some(crate::backups::Outcome::Created(path, summary))) => {
                self.backups.info = format!(
                    "Копия создана: {} · {} пьес · {} результатов",
                    path.display(),
                    summary.songs,
                    summary.attempts
                );
            }
            Ok(Some(crate::backups::Outcome::Preview(preview))) => {
                self.backups.info.clear();
                self.backups.preview = Some(preview);
            }
            Ok(Some(crate::backups::Outcome::Restored(safety))) => {
                let result = (|| -> Result<Self> {
                    Self::new(
                        Library::open(&self.store.database_path()?)?,
                        self.data_dir.clone(),
                        self.native_smoke,
                    )
                })();
                match result {
                    Ok(mut replacement) => {
                        replacement.page = Page::Settings;
                        replacement.backups.info = format!(
                            "Данные восстановлены. Прежняя библиотека сохранена: {}",
                            safety.display()
                        );
                        replacement.status = "Резервная копия восстановлена".into();
                        *self = replacement;
                    }
                    Err(e) => {
                        self.backups.info = format!(
                            "Копия записана, но не удалось обновить окно: {e}. Перезапустите приложение. Прежние данные: {}",
                            safety.display()
                        )
                    }
                }
            }
            Ok(None) => self.backups.info = "Операция отменена. Данные не изменены".into(),
            Err(e) => self.backups.info = e,
        }
    }
    fn backup_controls(&mut self, ui: &mut egui::Ui) {
        ui.heading("Резервные копии");
        ui.label("Сохраните библиотеку, разметку, избранное, прогресс и настройки в один файл.");
        ui.add(
            egui::TextEdit::singleline(&mut self.backups.path)
                .hint_text("Полный путь к .clavunora-backup — или оставьте пустым для выбора файла")
                .desired_width(ui.available_width()),
        );
        let path = (!self.backups.path.trim().is_empty())
            .then(|| std::path::PathBuf::from(self.backups.path.trim()));
        let mut create = false;
        let mut preview = false;
        ui.horizontal_wrapped(|ui| {
            create = ui.button("Создать копию").clicked();
            preview = ui.button("Выбрать копию для восстановления").clicked();
        });
        if create || preview {
            match self.prepare_backup() {
                Ok(source) => {
                    self.backups.preview = None;
                    self.backups.info.clear();
                    self.backups.job = Some(crate::backups::start(if create {
                        crate::backups::Task::Create {
                            source,
                            destination: path,
                        }
                    } else {
                        crate::backups::Task::Preview(path)
                    }));
                }
                Err(e) => self.backups.info = e.to_string(),
            }
        }
        let mut restore = false;
        let mut cancel = false;
        if let Some(p) = &self.backups.preview {
            ui.group(|ui| {
                ui.label(format!("Проверенная копия: {}",p.name));
                ui.label(format!("{} пьес · {} результатов · {} настроек · {} сохранённых записей",p.summary.songs,p.summary.attempts,p.summary.preferences,p.summary.recordings));
                ui.label("Восстановление полностью заменит текущие данные. Перед заменой приложение создаст страховочную копию в папке backups вашего профиля.");
                ui.small("После восстановления MIDI нужно подключить заново. Настройки звука и оформления загрузятся из копии.");
                ui.horizontal_wrapped(|ui| {
                    restore=ui.button("Заменить данные этой копией").clicked();
                    cancel=ui.button("Отмена").clicked();
                });
            });
        }
        if cancel {
            self.backups.preview = None;
        }
        if restore {
            match self.prepare_backup() {
                Ok(target) => {
                    self.midi.disconnect();
                    self.state.panic();
                    self.audio.silence();
                    if let Some(preview) = self.backups.preview.take() {
                        self.backups.job =
                            Some(crate::backups::start(crate::backups::Task::Restore {
                                target,
                                preview,
                                safety: self.data_dir.join("backups"),
                            }));
                    }
                }
                Err(e) => self.backups.info = e.to_string(),
            }
        }
        if !self.backups.info.is_empty() {
            ui.add(egui::Label::new(&self.backups.info).wrap());
        }
        ui.small(
            "Файл копии не перезаписывается. Старые ZIP-копии Python здесь не поддерживаются.",
        );
    }
    fn settings(&mut self, ui: &mut egui::Ui) {
        self.backup_controls(ui);
        ui.separator();
        ui.heading("Настройки");
        let old = (self.plain, self.dark);
        ui.horizontal_wrapped(|ui| {
            if ui.selectable_label(self.plain, "Обычная").clicked() {
                self.plain = true;
                self.dark = false;
            }
            if ui
                .selectable_label(!self.plain && !self.dark, "Стекло · светлая")
                .clicked()
            {
                self.plain = false;
                self.dark = false;
            }
            if ui
                .selectable_label(!self.plain && self.dark, "Стекло · тёмная")
                .clicked()
            {
                self.plain = false;
                self.dark = true;
            }
        });
        if old != (self.plain, self.dark) {
            let _ = self
                .store
                .set_preference("rust:plain-theme", &serde_json::json!(self.plain));
            let _ = self
                .store
                .set_preference("rust-preview:dark", &serde_json::json!(self.dark));
            ui.ctx().request_repaint();
        }
        ui.add_space(14.0);
        ui.heading("Клавиатура на экране");
        self.keyboard_controls(ui, true);
        ui.separator();
        ui.heading("Звук");
        let changed = ui
            .checkbox(&mut self.audio_config.enabled, "Озвучивать нажатия")
            .changed()
            | ui.add(egui::Slider::new(&mut self.audio_config.volume, 0.0..=1.0).text("Громкость"))
                .changed()
            | ui.add(
                egui::Slider::new(&mut self.audio_config.ambience, 0.0..=0.35)
                    .text("Объём встроенного инструмента"),
            )
            .changed();
        if changed {
            self.audio.ambience(self.audio_config.ambience);
            self.audio
                .mix(self.audio_config.enabled, self.audio_config.volume);
            self.save_audio_config();
        }
        ui.label(self.audio.status());
        ui.horizontal_wrapped(|ui| {
            if ui.button("Проверить звук: ля").clicked() {
                self.audio.test_note();
            }
            if ui.button("Остановить все звуки").clicked() {
                self.stop_demo();
                self.state.panic();
                self.audio.silence();
                self.computer_notes.clear();
                self.mouse_note = None;
                self.metronome = false;
                self.audio.sync(&self.state, None);
            }
        });
        ui.collapsing("Звуковое устройство и инструмент",|ui| {
            ui.label(if cfg!(target_os="linux") {"ALSA: default использует системный выход; можно указать pulse или pipewire, если они доступны."}else{"default — системный аудиовыход. Для другого устройства укажите его точное имя или идентификатор."});
            ui.text_edit_singleline(&mut self.audio_device_draft);
            ui.label("SoundFont .sf2/.sf3: пустое поле — встроенный инструмент. Для SoundFont нужна системная библиотека FluidSynth.");
            ui.text_edit_singleline(&mut self.soundfont_draft);
            if ui.button("Применить / переподключить звук").clicked() {
                self.stop_demo();
                let candidate=AudioConfig {device:self.audio_device_draft.clone(),soundfont:self.soundfont_draft.clone(),..self.audio_config.clone()};
                match self.audio.start(&candidate) {
                    Ok(())=>{self.audio_config=candidate;self.audio_initialized=true;self.save_audio_config();},
                    Err(e)=>self.error=e.to_string(),
                }
            }
            if ui.button("Вернуть встроенный инструмент").clicked() {
                self.audio_config.soundfont.clear();
                self.soundfont_draft.clear();
                match self.audio.start(&self.audio_config) {
                    Ok(())=>{self.audio_initialized=true;self.save_audio_config();},
                    Err(e)=>self.error=e.to_string(),
                }
            }
        });
        ui.separator();
        ui.heading("Метроном для свободной игры");
        let changed = ui
            .checkbox(&mut self.metronome, "Включить метроном")
            .changed()
            | ui.add(
                egui::Slider::new(&mut self.metronome_bpm, 30.0..=240.0).text("Ударов в минуту"),
            )
            .changed()
            | ui.add(egui::Slider::new(&mut self.metronome_beats, 1..=12).text("Долей в такте"))
                .changed();
        if changed {
            self.metronome_anchor = Instant::now();
            self.playback_epoch = self.playback_epoch.wrapping_add(1);
            self.standalone_sequence = std::sync::Arc::new(Sequence::standalone(
                self.metronome_bpm,
                self.metronome_beats,
            ));
        }
        ui.small("Работает в настройках и свободной игре. В занятии включите метроном под пианино: он следует темпу пьесы. Первый удар такта выделен.");
        ui.separator();
        ui.heading("MIDI-ввод");
        if !self.native_smoke {
            self.midi.monitor();
        }
        if ui.button("Обновить MIDI-порты").clicked() {
            self.midi.refresh();
        }
        egui::ComboBox::from_id_salt("midi-port")
            .selected_text(
                self.ports
                    .get(self.port_index)
                    .map(|p| p.name.as_str())
                    .unwrap_or("Нет выбранного порта"),
            )
            .show_ui(ui, |ui| {
                for (i, name) in self.ports.iter().enumerate() {
                    ui.selectable_value(&mut self.port_index, i, &name.name);
                }
            });
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(!self.ports.is_empty(), egui::Button::new("Подключить"))
                .clicked()
            {
                self.release_midi();
                match self.midi.connect(&self.ports[self.port_index]) {
                    Ok(()) => {
                        self.status =
                            "MIDI подключён. Перейдите в свободную игру или занятие.".into();
                        self.error.clear();
                    }
                    Err(e) => self.error = e.to_string(),
                }
            }
            if ui.button("Отключить").clicked() {
                self.midi.disconnect();
                self.release_midi();
                self.status = "MIDI отключён".into();
            }
        });
        ui.label(&self.midi.message);
        ui.small("Список проверяется раз в секунду. После восстановления занятие остаётся на паузе. «Отключить» отменяет автоматическое подключение.");
        ui.separator();
        ui.collapsing("О программе и лицензии",|ui| {
            ui.label(format!("Clavunora {}",env!("CARGO_PKG_VERSION")));
            ui.label("Copyright (C) 2026 redgion and Clavunora contributors");
            ui.label("Код: GNU GPLv3. Разрешены использование, изменение и распространение, в том числе за плату, с соблюдением GPL. Программа предоставляется без гарантий.");
            ui.label("Ноты, шрифты и зависимости сохраняют собственные лицензии; часть нот допускает только некоммерческое использование.");
            ui.hyperlink_to("Исходники и релизы","https://github.com/Redgion/PianoTrainer");
            ui.hyperlink_to("GNU GPLv3","https://www.gnu.org/licenses/gpl-3.0.html");
            ui.collapsing("Полный текст GPLv3",|ui| {egui::ScrollArea::vertical().max_height(240.0).show(ui,|ui|{ui.label(include_str!("../../../../LICENSE"));});});
        });
        ui.label("Библиотека, прогресс и настройки хранятся в вашем профиле и сохраняются при обновлении приложения.");
        ui.monospace(self.data_dir.display().to_string());
        ui.label("Импорт существующей базы выполняется отдельной командой и создаёт копию. Резервные копии доступны в начале настроек.");
    }
}
impl eframe::App for PreviewApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(target_os = "android")]
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::BrowserBack)) {
            if self.backups.job.is_some() {
                return;
            }
            if let Some(editor) = &mut self.editor {
                if editor.dirty() {
                    editor.discard = true;
                } else {
                    self.editor = None;
                }
                return;
            }
            if self.performance_view {
                self.leave_performance();
                return;
            }
            if self.page != Page::Home {
                self.navigate(Page::Home);
                return;
            }
            self.pause_practice();
            self.stop_demo();
            self.retry_saves();
            self.audio.stop();
            self.audio_initialized = false;
            self.state.clear_source("computer");
            self.state.clear_source("mouse");
            self.computer_notes.clear();
            self.mouse_note = None;
            if self.midi.connected() {
                self.midi.disconnect();
                self.release_midi();
            }
            for event in self.touch_keyboard.clear() {
                self.feed(event);
            }
            if let Err(error) = crate::platform::minimize_android() {
                self.error = error.to_string();
            }
            return;
        }
        #[cfg(target_os = "android")]
        if !ctx.input(|i| i.focused) {
            self.pause_practice();
            self.stop_demo();
            if self.midi.connected() {
                self.midi.disconnect();
                self.release_midi();
            }
            self.state.clear_source("computer");
            self.state.clear_source("mouse");
            self.computer_notes.clear();
            self.mouse_note = None;
            for event in self.touch_keyboard.clear() {
                self.feed(event);
            }
            self.audio.stop();
            self.audio_initialized = false;
            return;
        }
        if !self.audio_initialized && !self.native_smoke {
            self.audio_initialized = true;
            if let Err(e) = self.audio.start(&self.audio_config) {
                self.error = e.to_string();
            }
        }
        self.tick(ctx);
        if self.native_smoke && !self.native_frame_reported {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        }
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if !self.allow_close {
            self.pause_practice();
            self.retry_saves();
            if !self.save_queue.is_empty() {
                eprintln!(
                    "Clavunora: {} результатов не сохранены",
                    self.save_queue.len()
                );
            }
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.native_smoke && !self.native_frame_reported {
            eprintln!("Rust native UI: first frame");
            self.native_frame_reported = true;
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.inner_margin(0))
            .show_inside(ui, |ui| self.draw(ui));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accompaniment_is_audio_only_and_waits_pauses_and_navigates_with_lesson() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = PreviewApp::new(
            Library::open(&dir.path().join("db")).unwrap(),
            dir.path().to_path_buf(),
            true,
        )
        .unwrap();
        let index = app.records.iter().position(|r| r.id == "course-5").unwrap();
        app.open_song(index);
        app.plan_draft.hand = "R".into();
        app.apply_lesson_plan(index).unwrap();
        app.lesson_audio = LessonAudioConfig {
            metronome: true,
            accompaniment: true,
            volume: 0.4,
        };
        app.start_practice();
        app.update_audio();
        let p = app.audio.playback_snapshot().unwrap();
        assert!(p.sequence.note_count > 0);
        assert_eq!(p.volume, 0.4);
        assert!(p.metronome);
        assert_eq!(
            p.limit,
            app.session.as_ref().unwrap().expected().unwrap().start
        );
        assert!(app.state.pressed().is_empty());
        assert!(app.store.attempts().unwrap().is_empty());
        let now = app.now();
        app.transport.seek(p.limit, now).unwrap();
        app.advance_practice(now);
        app.update_audio();
        assert!(app.waiting);
        assert!(app.audio.playback_snapshot().is_none());
        let required = app
            .session
            .as_ref()
            .unwrap()
            .expected()
            .unwrap()
            .pitches
            .clone();
        for pitch in required {
            app.feed(MidiEvent::parse(&[0x90, pitch, 90], "midi").unwrap());
            app.feed(MidiEvent::parse(&[0x80, pitch, 0], "midi").unwrap());
        }
        app.update_audio();
        assert!(!app.waiting);
        assert!(app.audio.playback_snapshot().is_some());
        app.pause_practice();
        assert!(app.audio.playback_snapshot().is_none());
        let report = app.last_report.as_ref().unwrap();
        assert_eq!(report.hand, "R");
        assert!(!report.full_song);
        assert!(report.outcome.hit > 0);
        assert!(report.outcome.hit < report.outcome.total);
        app.start_practice();
        app.update_audio();
        app.navigate(Page::Home);
        assert!(app.audio.playback_snapshot().is_none());
    }
    #[test]
    fn rhythm_accompaniment_uses_fragment_speed_and_restarts_on_repeat_without_awarding_hits() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = PreviewApp::new(
            Library::open(&dir.path().join("db")).unwrap(),
            dir.path().to_path_buf(),
            true,
        )
        .unwrap();
        let index = app.records.iter().position(|r| r.id == "course-5").unwrap();
        app.open_song(index);
        app.plan_draft = Plan {
            hand: "L".into(),
            start: 0.5,
            end: Some(3.0),
            repeat: true,
            ..Default::default()
        };
        app.apply_lesson_plan(index).unwrap();
        app.wait_mode = false;
        app.lesson_audio = LessonAudioConfig {
            metronome: true,
            accompaniment: true,
            volume: 0.6,
        };
        app.transport.set_speed(0.5, app.now()).unwrap();
        app.start_practice();
        app.update_audio();
        let first = app.audio.playback_snapshot().unwrap();
        assert_eq!(first.speed, 0.5);
        assert_eq!(first.limit, 3.0);
        let now = app.now();
        app.transport.seek(app.transport.duration, now).unwrap();
        app.advance_practice(now);
        app.update_audio();
        let repeated = app.audio.playback_snapshot().unwrap();
        assert_ne!(first.epoch, repeated.epoch);
        assert_eq!(repeated.limit, 3.0);
        assert_eq!(repeated.speed, 0.5);
        assert!(app.running && app.transport.playing);
        let report = app.last_report.as_ref();
        assert!(report.is_none()); // New pass has started; completed report is in history.
        let attempts = app.store.attempts().unwrap();
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].report["hit"], 0);
        assert_eq!(attempts[0].report["full_song"], false);
        assert!(app.state.pressed().is_empty());
        app.plan.repeat = false;
        let now = app.now();
        app.transport.seek(app.transport.duration, now).unwrap();
        app.advance_practice(now);
        app.update_audio();
        assert!(!app.running);
        assert!(app.audio.playback_snapshot().is_none());
    }
    #[test]
    fn lesson_support_settings_persist_and_free_metronome_never_leaks_into_paused_practice() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let mut app = PreviewApp::new(
            Library::open(&path).unwrap(),
            dir.path().to_path_buf(),
            true,
        )
        .unwrap();
        app.lesson_audio = LessonAudioConfig {
            metronome: true,
            accompaniment: true,
            volume: 0.3,
        };
        app.save_lesson_audio();
        drop(app);
        let mut app = PreviewApp::new(
            Library::open(&path).unwrap(),
            dir.path().to_path_buf(),
            true,
        )
        .unwrap();
        assert!(app.lesson_audio.metronome && app.lesson_audio.accompaniment);
        assert_eq!(app.lesson_audio.volume, 0.3);
        app.metronome = true;
        app.navigate(Page::FreePlay);
        app.update_audio();
        assert!(app.audio.playback_snapshot().is_some());
        app.open_song(0);
        app.update_audio();
        assert!(app.audio.playback_snapshot().is_none());
        app.start_practice();
        app.update_audio();
        assert_eq!(app.audio.playback_snapshot().unwrap().volume, 0.3);
        app.midi_change(crate::midi_input::Change::Lost);
        assert!(app.audio.playback_snapshot().is_none());
        assert!(!app.running);
    }
    #[test]
    fn backups_preview_cancel_and_confirm_refresh_library_and_settings() {
        fn wait(app: &mut PreviewApp) {
            let start = Instant::now();
            while app.backups.job.is_some() {
                app.poll_backups();
                assert!(
                    start.elapsed() < Duration::from_secs(120),
                    "Backup worker did not finish; status: {}",
                    app.backups.info
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("db");
        let file = dir.path().join("copy.clavunora-backup");
        let mut app =
            PreviewApp::new(Library::open(&db).unwrap(), dir.path().into(), true).unwrap();
        app.store
            .set_preference("rust:plain-theme", &serde_json::json!(true))
            .unwrap();
        let source = app.prepare_backup().unwrap();
        app.backups.job = Some(crate::backups::start(crate::backups::Task::Create {
            source,
            destination: Some(file.clone()),
        }));
        app.request_close(&egui::Context::default());
        assert!(app.backups.job.is_some());
        wait(&mut app);
        assert!(file.is_file());
        assert!(app.backups.info.contains("Копия создана"));
        app.store.favorite("course-1", true).unwrap();
        app.store
            .set_preference("rust:plain-theme", &serde_json::json!(false))
            .unwrap();
        app.backups.job = Some(crate::backups::start(crate::backups::Task::Preview(Some(
            file.clone(),
        ))));
        wait(&mut app);
        assert!(app.backups.preview.is_some());
        app.backups.preview = None;
        assert!(
            app.store
                .songs()
                .unwrap()
                .into_iter()
                .find(|r| r.id == "course-1")
                .unwrap()
                .favorite
        );
        app.backups.job = Some(crate::backups::start(crate::backups::Task::Preview(Some(
            file,
        ))));
        wait(&mut app);
        let preview = app.backups.preview.take().unwrap();
        app.backups.job = Some(crate::backups::start(crate::backups::Task::Restore {
            target: db,
            preview,
            safety: dir.path().join("backups"),
        }));
        wait(&mut app);
        assert!(app.plain);
        assert_eq!(app.page, Page::Settings);
        assert!(
            !app.records
                .iter()
                .find(|r| r.id == "course-1")
                .unwrap()
                .favorite
        );
        assert!(app.backups.info.contains("Прежняя библиотека сохранена"));
    }
    #[test]
    fn midi_loss_stops_attempt_releases_sustain_and_never_auto_resumes() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = PreviewApp::new(
            Library::open(&dir.path().join("db")).unwrap(),
            dir.path().into(),
            true,
        )
        .unwrap();
        let index = app.records.iter().position(|r| r.id == "course-1").unwrap();
        app.open_song(index);
        app.start_practice();
        let now = app.now();
        app.transport.seek(0.0, now).unwrap();
        app.advance_practice(now);
        for bytes in [
            &[0x90, 60, 90],
            &[0xb0, 64, 127],
            &[0x80, 60, 0],
            &[0x91, 64, 90],
        ] {
            app.feed(MidiEvent::parse(bytes, "midi").unwrap());
        }
        app.feed(MidiEvent::parse(&[0x90, 67, 80], "computer").unwrap());
        assert!(app.state.pedal_active());
        app.midi_change(crate::midi_input::Change::Lost);
        assert!(!app.running);
        assert!(!app.transport.playing);
        assert!(!app.waiting);
        assert!(!app.state.pedal_active());
        assert_eq!(app.state.sounding(), BTreeSet::from([67]));
        let attempts = app.store.attempts().unwrap();
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].report["complete"], false);
        app.midi_change(crate::midi_input::Change::Connected);
        assert!(!app.running);
        assert!(!app.transport.playing);
        assert_eq!(app.store.attempts().unwrap().len(), 1);
    }
    #[test]
    fn selected_hand_plan_is_saved_and_invalid_changes_do_not_interrupt_current_attempt() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let mut app = PreviewApp::new(
            Library::open(&path).unwrap(),
            dir.path().to_path_buf(),
            true,
        )
        .unwrap();
        let index = app.records.iter().position(|r| r.id == "course-5").unwrap();
        app.open_song(index);
        app.plan_draft.hand = "L".into();
        app.apply_lesson_plan(index).unwrap();
        app.start_practice();
        assert!(
            app.practice_song
                .as_ref()
                .unwrap()
                .notes
                .iter()
                .all(|n| n.hand == "L")
        );
        app.plan_draft.tracks = Some(BTreeSet::new());
        assert!(app.apply_lesson_plan(index).is_err());
        assert!(app.running && app.session.is_some());
        app.pause_practice();
        drop(app);
        let mut app = PreviewApp::new(
            Library::open(&path).unwrap(),
            dir.path().to_path_buf(),
            true,
        )
        .unwrap();
        app.open_song(index);
        assert_eq!(app.plan.hand, "L");
    }
    #[test]
    fn smart_repeat_saves_each_pass_and_pause_cancels_restart() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = PreviewApp::new(
            Library::open(&dir.path().join("db")).unwrap(),
            dir.path().to_path_buf(),
            true,
        )
        .unwrap();
        let index = app.records.iter().position(|r| r.id == "course-1").unwrap();
        app.open_song(index);
        app.plan_draft.repeat = true;
        app.plan_draft.smart = true;
        app.smart_draft.repetitions = 1;
        app.apply_lesson_plan(index).unwrap();
        app.transport.set_speed(0.5, app.now()).unwrap();
        app.start_practice();
        for note in app.practice_song.as_ref().unwrap().notes.clone() {
            let now = app.now();
            app.transport.seek(note.start, now).unwrap();
            app.advance_practice(now);
            app.feed(MidiEvent::parse(&[0x90, note.pitch, 90], "midi").unwrap());
            app.feed(MidiEvent::parse(&[0x80, note.pitch, 0], "midi").unwrap());
        }
        let now = app.now();
        app.transport.seek(app.transport.duration, now).unwrap();
        app.advance_practice(now);
        assert!(app.running);
        assert_eq!(app.repeat_progress.completed, 1);
        assert_eq!(app.transport.speed, 0.55);
        let saved = app.store.attempts().unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].report["speed"], 0.5);
        assert_eq!(saved[0].report["full_song"], false);
        assert_eq!(saved[0].report["repeated"], true);
        let awards = app.store.preference("achievements").unwrap().unwrap();
        assert!(awards.get("first-piece").is_none());
        app.pause_practice();
        assert!(!app.running);
        app.advance_practice(app.now() + 100.0);
        assert!(!app.running);
        assert_eq!(app.store.attempts().unwrap().len(), 1);
    }
    #[test]
    fn resumed_partial_repeat_does_not_raise_speed_or_award_a_full_pass() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = PreviewApp::new(
            Library::open(&dir.path().join("db")).unwrap(),
            dir.path().to_path_buf(),
            true,
        )
        .unwrap();
        let index = app.records.iter().position(|r| r.id == "course-1").unwrap();
        app.open_song(index);
        app.plan_draft.repeat = true;
        app.plan_draft.smart = true;
        app.smart_draft.repetitions = 1;
        app.apply_lesson_plan(index).unwrap();
        app.transport.set_speed(0.5, app.now()).unwrap();
        app.transport.seek(1.5, app.now()).unwrap();
        app.start_practice();
        for note in app
            .practice_song
            .as_ref()
            .unwrap()
            .notes
            .clone()
            .into_iter()
            .filter(|n| n.start >= 1.5)
        {
            let now = app.now();
            app.transport.seek(note.start, now).unwrap();
            app.advance_practice(now);
            app.feed(MidiEvent::parse(&[0x90, note.pitch, 90], "midi").unwrap());
            app.feed(MidiEvent::parse(&[0x80, note.pitch, 0], "midi").unwrap());
        }
        let now = app.now();
        app.transport.seek(app.transport.duration, now).unwrap();
        app.advance_practice(now);
        assert!(app.running);
        assert_eq!(app.transport.speed, 0.5);
        assert_eq!(app.repeat_progress.completed, 0);
        let rows = app.store.attempts().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].report["start"], 1.5);
    }
    #[test]
    fn changing_plan_finishes_the_old_configuration_and_empty_fragment_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = PreviewApp::new(
            Library::open(&dir.path().join("db")).unwrap(),
            dir.path().to_path_buf(),
            true,
        )
        .unwrap();
        let index = app.records.iter().position(|r| r.id == "course-1").unwrap();
        app.open_song(index);
        app.start_practice();
        let now = app.now();
        app.transport.seek(0.0, now).unwrap();
        app.advance_practice(now);
        app.feed(MidiEvent::parse(&[0x90, 60, 90], "midi").unwrap());
        app.plan_draft.start = 0.75;
        app.plan_draft.end = Some(1.5);
        app.apply_lesson_plan(index).unwrap();
        let rows = app.store.attempts().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].report["start"], 0.0);
        assert_eq!(rows[0].report["complete"], false);
        app.plan_draft.start = 1.0;
        app.plan_draft.end = Some(1.0);
        assert!(app.apply_lesson_plan(index).is_err());
        assert_eq!(app.plan.start, 0.75);
    }
    #[test]
    fn editor_cancel_does_not_save_and_saved_annotations_reach_practice() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let mut app =
            PreviewApp::new(Library::open(&path).unwrap(), dir.path().into(), true).unwrap();
        let index = app.records.iter().position(|r| r.id == "course-1").unwrap();
        let original = app.records[index].song.clone();
        app.open_song(index);
        app.open_editor(index);
        app.editor.as_mut().unwrap().assign(&[0], "L", Some(4));
        assert_eq!(app.records[index].song, original);
        assert_eq!(
            app.store
                .songs()
                .unwrap()
                .into_iter()
                .find(|r| r.id == "course-1")
                .unwrap()
                .song,
            original
        );
        app.editor = None;
        app.open_editor(index);
        assert!(!app.editor.as_ref().unwrap().dirty());
        app.editor.as_mut().unwrap().assign(&[0], "L", Some(4));
        let ctx = egui::Context::default();
        app.request_close(&ctx);
        assert!(app.editor.as_ref().unwrap().discard);
        app.save_editor().unwrap();
        assert!(app.editor.is_none());
        assert_eq!(app.practice_song.as_ref().unwrap().notes[0].finger, 4);
        assert_eq!(app.practice_song.as_ref().unwrap().notes[0].hand, "L");
        app.open_editor(index);
        app.editor.as_mut().unwrap().assign(&[0], "R", Some(2));
        let record = &app.records[index];
        let mut external = record.song.clone();
        external.notes[1].finger = 3;
        app.store
            .save_annotations(&record.id, &record.score_version, &external)
            .unwrap();
        assert!(app.save_editor().is_err());
        assert!(app.editor.is_some());
    }
    #[test]
    fn musicxml_preview_preserves_fingering_and_only_saves_on_confirmation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Study.musicxml");
        std::fs::write(
            &path,
            include_bytes!("../../../../tests/fixtures/musicxml-import.musicxml"),
        )
        .unwrap();
        let mut app = PreviewApp::new(
            Library::open(&dir.path().join("db")).unwrap(),
            dir.path().to_path_buf(),
            true,
        )
        .unwrap();
        let count = app.records.len();
        let preview = crate::midi_import::read(&path).unwrap();
        assert_eq!(preview.kind, "musicxml");
        assert!(preview.parsed.song.notes.iter().any(|n| n.finger == 1));
        app.import_preview = Some(preview);
        assert_eq!(app.store.songs().unwrap().len(), count);
        assert!(app.confirm_midi_import().unwrap());
        assert!(
            app.records
                .iter()
                .any(|r| r.id.starts_with("imported-musicxml:") && r.song.title == "Study")
        );
        app.import_preview = Some(crate::midi_import::read(&path).unwrap());
        assert!(!app.confirm_midi_import().unwrap());
        assert_eq!(app.records.len(), count + 1);
        let mxl = dir.path().join("x.mxl");
        std::fs::write(&mxl, b"PKinvalid").unwrap();
        assert!(crate::midi_import::read(&mxl).is_err());
        assert_eq!(app.store.songs().unwrap().len(), count + 1);
    }
    #[test]
    fn midi_preview_requires_confirmation_and_duplicate_preserves_edits_and_favorite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let midi = dir.path().join("Моя пьеса.mid");
        std::fs::write(
            &midi,
            include_bytes!("../../../../assets/happy_birthday.mid"),
        )
        .unwrap();
        let mut app = PreviewApp::new(
            Library::open(&path).unwrap(),
            dir.path().to_path_buf(),
            true,
        )
        .unwrap();
        let original = app.records.len();
        app.import_preview = Some(crate::midi_import::read(&midi).unwrap());
        assert_eq!(app.store.songs().unwrap().len(), original);
        app.import_preview.as_mut().unwrap().parsed.song.title = "Моя редакция".into();
        assert!(app.confirm_midi_import().unwrap());
        assert_eq!(app.records.len(), original + 1);
        let record = app
            .records
            .iter()
            .find(|r| r.id.starts_with("imported-midi:"))
            .unwrap();
        let id = record.id.clone();
        app.store.favorite(&id, true).unwrap();
        app.import_preview = Some(crate::midi_import::read(&midi).unwrap());
        assert!(!app.confirm_midi_import().unwrap());
        let record = app.records.iter().find(|r| r.id == id).unwrap();
        assert_eq!(record.song.title, "Моя редакция");
        assert!(record.favorite);
        drop(app);
        let app = PreviewApp::new(
            Library::open(&path).unwrap(),
            dir.path().to_path_buf(),
            true,
        )
        .unwrap();
        assert_eq!(app.records.len(), original + 1);
        let bad = dir.path().join("bad.mid");
        std::fs::write(&bad, b"broken").unwrap();
        assert!(crate::midi_import::read(&bad).is_err());
        assert_eq!(app.store.songs().unwrap().len(), original + 1);
    }
    #[test]
    fn plain_theme_preference_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let store = Library::open(&path).unwrap();
        store
            .set_preference("rust:plain-theme", &serde_json::json!(true))
            .unwrap();
        let app = PreviewApp::new(store, dir.path().to_path_buf(), true).unwrap();
        assert!(app.plain);
        drop(app);
        let app = PreviewApp::new(
            Library::open(&path).unwrap(),
            dir.path().to_path_buf(),
            true,
        )
        .unwrap();
        assert!(app.plain);
    }
    #[test]
    fn error_review_opens_slow_repeat_and_checks_fresh_version_before_leaving() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let mut app =
            PreviewApp::new(Library::open(&path).unwrap(), dir.path().into(), true).unwrap();
        let report = app.verify_rhythm().unwrap();
        let attempt = app.store.attempts().unwrap().remove(0);
        let preference = serde_json::json!({"start":0.75,"end":1.5});
        app.store
            .set_preference("rust:lesson:course-1", &preference)
            .unwrap();
        app.navigate(Page::Progress);
        app.practice_error(&attempt.id, 0).unwrap();
        assert_eq!(app.page, Page::Practice);
        assert!(!app.running);
        assert!(!app.performance_view);
        assert!(app.plan.repeat && !app.plan.smart);
        assert!(!app.wait_mode);
        assert_eq!(app.transport.speed, 0.75);
        assert!(app.repair_notice.is_some());
        assert_eq!(app.plan_draft, app.plan);
        assert_eq!(
            app.store.preference("rust:lesson:course-1").unwrap(),
            Some(preference)
        );
        assert_eq!(app.store.attempts().unwrap().len(), 1);
        assert_eq!(app.store.attempts().unwrap()[0].report, report);
        app.start_practice();
        assert!(app.performance_view && app.running);
        let index = app.selected.unwrap();
        let record = &app.records[index];
        let mut edited = record.song.clone();
        edited.notes[0].finger = 2;
        app.store
            .save_annotations(&record.id, &record.score_version, &edited)
            .unwrap();
        assert!(app.practice_error(&attempt.id, 0).is_err());
        assert!(app.running);
        let fragment_report = app.session.as_mut().unwrap().finish(true);
        assert!(!fragment_report.full_song);
        assert!(!fragment_report.passed());
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute("DELETE FROM songs WHERE id='course-1'", [])
            .unwrap();
        assert!(app.practice_error(&attempt.id, 0).is_err());
        assert_eq!(app.store.attempts().unwrap()[0].report, report);
    }
    #[test]
    fn fullscreen_transition_does_not_pause_but_real_focus_loss_still_does() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = PreviewApp::new(
            Library::open(&dir.path().join("db")).unwrap(),
            dir.path().into(),
            true,
        )
        .unwrap();
        let ctx = egui::Context::default();
        let index = app.records.iter().position(|r| r.id == "course-1").unwrap();
        app.open_song(index);
        app.start_practice();
        let raw = || egui::RawInput {
            focused: false,
            ..Default::default()
        };
        let _ = ctx.run_ui(raw(), |ui| app.tick(ui.ctx()));
        assert!(
            app.running,
            "Transient fullscreen focus loss stopped the lesson"
        );
        app.fullscreen_focus_until = Some(Instant::now() - Duration::from_millis(1));
        let _ = ctx.run_ui(raw(), |ui| app.tick(ui.ctx()));
        assert!(!app.running, "Sustained focus loss must still pause");
    }
    #[test]
    fn start_enters_instrument_only_fullscreen_and_escape_restores_window() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = PreviewApp::new(
            Library::open(&dir.path().join("db")).unwrap(),
            dir.path().into(),
            true,
        )
        .unwrap();
        let ctx = egui::Context::default();
        aero::install_fonts(&ctx);
        let index = app.records.iter().position(|r| r.id == "course-1").unwrap();
        app.open_song(index);
        assert!(!app.performance_view);
        app.start_practice();
        assert!(app.performance_view);
        app.native_smoke = false;
        let raw = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
            ..Default::default()
        };
        let output = ctx.run_ui(raw(), |ui| {
            app.sync_performance_view(ui.ctx());
            app.draw(ui)
        });
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::Fullscreen(true)))
        );
        let texts: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|s| {
                if let egui::Shape::Text(t) = &s.shape {
                    Some(t.galley.text())
                } else {
                    None
                }
            })
            .collect();
        for forbidden in [
            "Clavunora",
            "Настройки и разучивание",
            "Скорость",
            "Начать / продолжить",
        ] {
            assert!(
                !texts.iter().any(|t| t.contains(forbidden)),
                "Chrome leaked into performance: {forbidden}"
            );
        }
        assert!(texts.contains(&"C4"));
        let mut input = raw();
        input.events.push(egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        });
        let output = ctx.run_ui(input, |ui| {
            app.sync_performance_view(ui.ctx());
            app.draw(ui)
        });
        assert!(!app.performance_view);
        assert!(!app.running);
        assert!(!app.transport.playing);
        assert!(app.fullscreen_restore.is_none());
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::Fullscreen(false)))
        );
        app.fullscreen_restore = Some(true);
        let output = ctx.run_ui(raw(), |ui| app.sync_performance_view(ui.ctx()));
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::Fullscreen(true)))
        );
    }
    #[test]
    fn choosing_a_song_after_scrolling_library_shows_piano_in_viewport() {
        for size in [Vec2::new(1200.0, 800.0), Vec2::new(760.0, 540.0)] {
            let dir = tempfile::tempdir().unwrap();
            let mut app = PreviewApp::new(
                Library::open(&dir.path().join("db")).unwrap(),
                dir.path().into(),
                true,
            )
            .unwrap();
            let ctx = egui::Context::default();
            aero::install_fonts(&ctx);
            app.page = Page::Library;
            app.folder = Some("all".into());
            for frame in 0..4 {
                let events = if frame > 0 {
                    vec![
                        egui::Event::PointerMoved(Pos2::new(size.x * 0.5, size.y * 0.6)),
                        egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            phase: egui::TouchPhase::Move,
                            delta: Vec2::new(0.0, -1200.0),
                            modifiers: Default::default(),
                        },
                    ]
                } else {
                    vec![]
                };
                let _ = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                        events,
                        ..Default::default()
                    },
                    |ui| app.draw(ui),
                );
            }
            let index = app.records.iter().position(|r| r.id == "course-1").unwrap();
            app.open_song(index);
            assert_eq!(app.page, Page::Practice);
            assert!(app.reset_page_scroll);
            for _ in 0..2 {
                let output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                        ..Default::default()
                    },
                    |ui| app.draw(ui),
                );
                let keys: Vec<_> = output
                    .shapes
                    .iter()
                    .filter_map(|shape| {
                        if let egui::Shape::Text(text) = &shape.shape {
                            (text.galley.text() == "C4").then_some((
                                text.pos,
                                text.galley.size(),
                                shape.clip_rect,
                            ))
                        } else {
                            None
                        }
                    })
                    .collect();
                assert!(!keys.is_empty(), "Piano is missing at {size:?}");
                for (pos, extent, clip) in keys {
                    assert!(
                        pos.y >= clip.top() && pos.y + extent.y <= clip.bottom(),
                        "Piano is below viewport: {pos:?} {clip:?}"
                    );
                }
            }
        }
    }
    #[test]
    fn song_card_text_never_escapes_library_scroll_viewport() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = PreviewApp::new(
            Library::open(&dir.path().join("db")).unwrap(),
            dir.path().to_path_buf(),
            true,
        )
        .unwrap();
        app.page = Page::Library;
        app.folder = Some("all".into());
        let titles: BTreeSet<_> = app.records.iter().map(|r| r.song.title.clone()).collect();
        let ctx = egui::Context::default();
        aero::install_fonts(&ctx);
        for size in [Vec2::new(1200.0, 800.0), Vec2::new(760.0, 540.0)] {
            for _ in 0..2 {
                let output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                        ..Default::default()
                    },
                    |ui| app.draw(ui),
                );
                let mut checked = 0;
                for shape in output.shapes {
                    if let egui::Shape::Text(text) = shape.shape
                        && titles.contains(text.galley.text())
                    {
                        assert!(
                            shape.clip_rect.top() >= 150.0,
                            "Song title is painted above the scroll area: {:?}",
                            shape.clip_rect
                        );
                        assert!(
                            shape.clip_rect.bottom() <= size.y - 50.0,
                            "Song title is painted below the scroll area: {:?}",
                            shape.clip_rect
                        );
                        checked += 1;
                    }
                }
                assert!(checked > 0, "No song cards were drawn");
            }
        }
    }
    #[test]
    fn glass_home_routes_all_six_tiles_back_button_and_theme_control() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = PreviewApp::new(
            Library::open(&dir.path().join("db")).unwrap(),
            dir.path().to_path_buf(),
            true,
        )
        .unwrap();
        let ctx = egui::Context::default();
        aero::install_fonts(&ctx);
        let render = |app: &mut PreviewApp, events: Vec<egui::Event>| {
            let _ = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ui| app.draw(ui),
            );
        };
        let click = |app: &mut PreviewApp, pos: Pos2| {
            render(
                app,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: Default::default(),
                    },
                ],
            );
            render(
                app,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                }],
            );
            render(app, vec![]);
        };
        render(&mut app, vec![]);
        for (i, page) in [
            Page::Library,
            Page::Course,
            Page::FreePlay,
            Page::Achievements,
            Page::Progress,
            Page::Settings,
        ]
        .into_iter()
        .enumerate()
        {
            click(
                &mut app,
                Pos2::new(
                    327.0 + (i % 3) as f32 * 273.0,
                    264.0 + (i / 3) as f32 * 201.0,
                ),
            );
            assert_eq!(app.page, page, "Home tile {i}");
            click(&mut app, Pos2::new(600.0, 95.0));
            assert_eq!(app.page, Page::Home);
        }
        click(&mut app, Pos2::new(1166.0, 38.0));
        assert!(app.dark);
        assert_eq!(
            app.store.preference("rust-preview:dark").unwrap(),
            Some(serde_json::json!(true))
        );
    }
    #[test]
    fn keyboard_view_survives_restart_and_does_not_filter_midi_or_pause_a_session() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let mut app = PreviewApp::new(
            Library::open(&path).unwrap(),
            dir.path().to_path_buf(),
            true,
        )
        .unwrap();
        app.open_song(app.records.iter().position(|r| r.id == "course-1").unwrap());
        app.start_practice();
        app.keyboard_config.preset(49);
        app.keyboard_config.zoom = 0.5;
        app.save_keyboard_config();
        assert!(app.running && app.session.is_some());
        app.feed(MidiEvent::parse(&[0x90, 108, 90], "midi").unwrap());
        assert!(app.state.pressed().contains(&108));
        let expected = app.keyboard_config.clone();
        drop(app);
        let app = PreviewApp::new(
            Library::open(&path).unwrap(),
            dir.path().to_path_buf(),
            true,
        )
        .unwrap();
        assert_eq!(app.keyboard_config, expected);
        app.store
            .set_preference(
                "rust:keyboard-view",
                &serde_json::json!({"count":88,"first":120,"zoom":0.5}),
            )
            .unwrap();
        drop(app);
        let app = PreviewApp::new(
            Library::open(&path).unwrap(),
            dir.path().to_path_buf(),
            true,
        )
        .unwrap();
        assert_eq!(app.keyboard_config, KeyboardConfig::default());
    }
    #[test]
    fn piano_mouse_hits_black_keys_before_white_keys_and_releases_outside() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = PreviewApp::new(
            Library::open(&dir.path().join("db")).unwrap(),
            dir.path().to_path_buf(),
            true,
        )
        .unwrap();
        app.page = Page::FreePlay;
        let ctx = egui::Context::default();
        let mut frame = Rect::NOTHING;
        let mut render = |app: &mut PreviewApp, events: Vec<egui::Event>| {
            let _ = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 500.0))),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ui| {
                    frame = app.keyboard(ui);
                },
            );
            frame
        };
        let frame = render(&mut app, vec![]);
        let rect = frame.shrink(10.0);
        let width = rect.width() / 8.0;
        let black = Pos2::new(rect.left() + width, rect.top() + 25.0);
        let press = |pos| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        };
        render(
            &mut app,
            vec![egui::Event::PointerMoved(black), press(black)],
        );
        assert_eq!(app.state.pressed(), BTreeSet::from([61]));
        let white = Pos2::new(rect.left() + width * 0.5, rect.bottom() - 20.0);
        render(&mut app, vec![egui::Event::PointerMoved(white)]);
        assert_eq!(app.state.pressed(), BTreeSet::from([60]));
        render(
            &mut app,
            vec![egui::Event::PointerMoved(Pos2::new(
                rect.right() + 20.0,
                rect.bottom() + 10.0,
            ))],
        );
        assert!(app.state.pressed().is_empty());
        assert!(app.mouse_note.is_none());
    }
    #[test]
    fn preview_uses_full_or_selected_score_speed_and_stops_at_end_without_progress() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = PreviewApp::new(
            Library::open(&dir.path().join("db")).unwrap(),
            dir.path().to_path_buf(),
            true,
        )
        .unwrap();
        let index = app.records.iter().position(|r| r.id == "course-5").unwrap();
        app.open_song(index);
        app.plan_draft = Plan {
            hand: "R".into(),
            start: 1.0,
            end: Some(2.5),
            ..Default::default()
        };
        app.apply_lesson_plan(index).unwrap();
        let plan = app.plan.clone();
        app.transport.set_speed(0.5, app.now()).unwrap();
        app.lesson_audio.accompaniment = false;
        app.lesson_audio.volume = 0.0;
        app.start_listening(true);
        let whole = app.audio.playback_snapshot().unwrap();
        assert_eq!(
            whole.sequence.note_count,
            app.records[index].song.notes.len()
        );
        assert_eq!(whole.position, 0.0);
        assert_eq!(whole.speed, 0.5);
        assert_eq!(whole.volume, 1.0);
        assert_eq!(whole.limit, app.records[index].song.duration());
        assert!(app.display_song(index).notes.iter().any(|n| n.hand == "L"));
        app.start_listening(false);
        let selected = app.audio.playback_snapshot().unwrap();
        assert_eq!(selected.position, 1.0);
        assert_eq!(selected.limit, 2.5);
        assert_eq!(selected.sequence.note_count, 2);
        assert!(app.display_song(index).notes.iter().all(|n| n.hand == "R"));
        assert_eq!(app.plan, plan);
        app.demo.as_mut().unwrap().started = Instant::now() - Duration::from_secs(10);
        app.update_audio();
        assert!(app.demo.is_none() && app.audio.playback_snapshot().is_none());
        assert!(app.store.attempts().unwrap().is_empty());
        assert!(app.store.preference("achievements").unwrap().is_none());
        assert!(app.state.pressed().is_empty());
        assert!(app.session.is_none());
        app.start_listening(true);
        app.start_practice();
        app.update_audio();
        assert!(app.demo.is_none());
        assert_eq!(app.plan, plan);
        assert!(app.wait_mode);
        assert_eq!(app.audio.playback_snapshot().unwrap().volume, 0.0);
    }
    #[test]
    fn sheet_and_note_language_controls_persist_without_changing_score_or_progress() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let mut app = PreviewApp::new(
            Library::open(&path).unwrap(),
            dir.path().to_path_buf(),
            true,
        )
        .unwrap();
        let index = app.records.iter().position(|r| r.id == "course-5").unwrap();
        app.open_song(index);
        let original = app.records[index].score_version.clone();
        let ctx = egui::Context::default();
        aero::install_fonts(&ctx);
        let render = |app: &mut PreviewApp, events: Vec<egui::Event>| {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
                    events,
                    focused: true,
                    ..Default::default()
                },
                |ui| app.notation_controls(ui, index),
            )
        };
        for target in ["Нотный лист", "До Ре Ми"] {
            let output = render(&mut app, vec![]);
            let pos = output
                .shapes
                .iter()
                .find_map(|shape| {
                    if let egui::Shape::Text(text) = &shape.shape {
                        if text.galley.text() == target {
                            Some(text.pos + text.galley.size() * 0.5)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .unwrap();
            for pressed in [true, false] {
                render(
                    &mut app,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                );
            }
        }
        assert_eq!(
            app.notation,
            NotationConfig {
                view: NotationView::Sheet,
                language: NoteLanguage::Russian
            }
        );
        assert_eq!(app.records[index].score_version, original);
        assert!(app.store.attempts().unwrap().is_empty());
        drop(app);
        let app = PreviewApp::new(
            Library::open(&path).unwrap(),
            dir.path().to_path_buf(),
            true,
        )
        .unwrap();
        assert_eq!(app.notation.view, NotationView::Sheet);
        assert_eq!(app.notation.language, NoteLanguage::Russian);
    }
    #[test]
    fn sheet_only_and_combined_fullscreen_keep_the_piano_visible_on_small_windows() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = PreviewApp::new(
            Library::open(&dir.path().join("db")).unwrap(),
            dir.path().to_path_buf(),
            true,
        )
        .unwrap();
        let index = app.records.iter().position(|r| r.id == "course-5").unwrap();
        app.open_song(index);
        app.start_practice();
        let ctx = egui::Context::default();
        aero::install_fonts(&ctx);
        for view in [NotationView::Sheet, NotationView::Both] {
            for size in [
                Vec2::new(640.0, 320.0),
                Vec2::new(800.0, 360.0),
                Vec2::new(760.0, 540.0),
                Vec2::new(1200.0, 800.0),
            ] {
                app.notation.view = view;
                let output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                        focused: true,
                        ..Default::default()
                    },
                    |ui| app.draw(ui),
                );
                let key = output
                    .shapes
                    .iter()
                    .find_map(|shape| {
                        if let egui::Shape::Text(text) = &shape.shape {
                            (text.galley.text() == "C5").then_some((
                                text.pos,
                                text.galley.size(),
                                shape.clip_rect,
                            ))
                        } else {
                            None
                        }
                    })
                    .expect("piano label must exist");
                assert!(
                    key.0.y + key.1.y <= key.2.bottom(),
                    "piano is outside viewport for {view:?} {size:?}"
                );
                assert!(app.performance_view);
            }
        }
        assert!(app.store.attempts().unwrap().is_empty());
    }
    #[test]
    fn listening_never_scores_and_is_stopped_before_practice_or_navigation() {
        let dir = tempfile::tempdir().unwrap();
        let store = Library::open(&dir.path().join("db")).unwrap();
        let mut app = PreviewApp::new(store, dir.path().to_path_buf(), true).unwrap();
        app.open_song(0);
        app.start_listening(true);
        app.update_audio();
        assert!(app.store.attempts().unwrap().is_empty());
        assert!(app.state.pressed().is_empty());
        assert!(app.store.preference("achievements").unwrap().is_none());
        app.start_practice();
        assert!(app.demo.is_none());
        app.pause_practice();
        app.start_listening(true);
        app.navigate(Page::Home);
        assert!(app.demo.is_none());
        assert!(app.store.attempts().unwrap().is_empty());
    }
    #[test]
    fn sound_settings_survive_restart_without_opening_a_device() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let store = Library::open(&path).unwrap();
        let mut app = PreviewApp::new(store, dir.path().to_path_buf(), true).unwrap();
        app.audio_config.volume = 0.23;
        app.audio_config.enabled = false;
        app.save_audio_config();
        drop(app);
        let app = PreviewApp::new(
            Library::open(&path).unwrap(),
            dir.path().to_path_buf(),
            true,
        )
        .unwrap();
        assert!(!app.audio_config.enabled);
        assert_eq!(app.audio_config.volume, 0.23);
        assert!(!app.audio_initialized);
    }
    #[test]
    fn waiting_input_is_saved_as_partial_on_navigation() {
        let dir = tempfile::tempdir().unwrap();
        let store = Library::open(&dir.path().join("library.sqlite3")).unwrap();
        let mut app = PreviewApp::new(store, dir.path().to_path_buf(), false).unwrap();
        let index = app.records.iter().position(|r| r.id == "course-1").unwrap();
        app.open_song(index);
        app.start_practice();
        let now = app.now();
        app.transport.seek(0.0, now).unwrap();
        app.advance_practice(now);
        app.feed(MidiEvent::parse(&[0x90, 64, 90], "midi").unwrap());
        assert_eq!(app.session.as_ref().unwrap().completed_groups(), 0);
        app.feed(MidiEvent::parse(&[0x90, 60, 90], "computer").unwrap());
        assert_eq!(app.session.as_ref().unwrap().completed_groups(), 1);
        assert!(!app.waiting);
        app.navigate(Page::Home);
        assert!(!app.transport.playing);
        assert!(!app.state.pressed().contains(&60));
        let rows = app.store.attempts().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].report["complete"], false);
        assert_eq!(rows[0].report["hit"], 1);
        assert_eq!(rows[0].report["mistakes"].as_array().unwrap().len(), 1);
        assert!(app.store.preference("passed:course-1").unwrap().is_none());
    }
    #[test]
    fn preview_theme_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.sqlite3");
        {
            let app = PreviewApp::new(
                Library::open(&path).unwrap(),
                dir.path().to_path_buf(),
                false,
            )
            .unwrap();
            app.store
                .set_preference("rust-preview:dark", &serde_json::json!(true))
                .unwrap();
        }
        let app = PreviewApp::new(
            Library::open(&path).unwrap(),
            dir.path().to_path_buf(),
            false,
        )
        .unwrap();
        assert!(app.dark);
    }
    #[test]
    fn full_waiting_attempt_is_stored_once_and_reopens() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.sqlite3");
        {
            let mut app = PreviewApp::new(
                Library::open(&path).unwrap(),
                dir.path().to_path_buf(),
                false,
            )
            .unwrap();
            app.verify_scoring().unwrap();
            assert_eq!(
                app.store.preference("passed:course-1").unwrap(),
                Some(serde_json::Value::Bool(true))
            );
        }
        let app = PreviewApp::new(
            Library::open(&path).unwrap(),
            dir.path().to_path_buf(),
            false,
        )
        .unwrap();
        assert_eq!(app.store.attempts().unwrap().len(), 1);
    }
    #[test]
    fn mode_and_speed_changes_preserve_original_attempt_configuration() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = PreviewApp::new(
            Library::open(&dir.path().join("db")).unwrap(),
            dir.path().to_path_buf(),
            false,
        )
        .unwrap();
        let index = app.records.iter().position(|r| r.id == "course-1").unwrap();
        app.open_song(index);
        app.start_practice();
        let now = app.now();
        app.transport.seek(0.0, now).unwrap();
        app.advance_practice(now);
        app.feed_at(MidiEvent::parse(&[0x90, 60, 90], "midi").unwrap(), now);
        app.pause_practice();
        app.wait_mode = false;
        app.transport.set_speed(0.5, app.now()).unwrap();
        let report = &app.store.attempts().unwrap()[0].report;
        assert_eq!(report["mode"], "Ожидание");
        assert_eq!(report["speed"], 1.0);
        assert_eq!(report["complete"], false);
        let now = app.now();
        app.transport.seek(-3.0, now).unwrap();
        app.start_practice();
        let now = app.now();
        app.transport.seek(0.0, now).unwrap();
        app.feed_at(MidiEvent::parse(&[0x90, 60, 90], "midi").unwrap(), now);
        app.pause_practice();
        let rows = app.store.attempts().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].report["mode"], "Ритм");
        assert_eq!(rows[0].report["speed"], 0.5);
        assert_eq!(rows[0].report["complete"], false);
    }
    #[test]
    fn database_failure_keeps_result_for_retry_and_blocks_silent_close() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let mut app = PreviewApp::new(
            Library::open(&path).unwrap(),
            dir.path().to_path_buf(),
            false,
        )
        .unwrap();
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute_batch("CREATE TRIGGER fail_attempt BEFORE INSERT ON attempts BEGIN SELECT RAISE(ABORT,'test failure'); END;").unwrap();
        let index = app.records.iter().position(|r| r.id == "course-1").unwrap();
        app.open_song(index);
        app.start_practice();
        let now = app.now();
        app.transport.seek(0.0, now).unwrap();
        app.feed_at(MidiEvent::parse(&[0x90, 60, 90], "midi").unwrap(), now);
        app.pause_practice();
        assert_eq!(app.save_queue.len(), 1);
        let id = app.save_queue[0].pending.as_ref().unwrap().id.clone();
        let ended = app.save_queue[0]
            .finished_at
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        assert_eq!(app.save_queue[0].pending.as_ref().unwrap().created, ended);
        assert!(app.store.attempts().unwrap().is_empty());
        app.request_close(&egui::Context::default());
        assert!(app.close_warning);
        assert_eq!(app.save_queue[0].pending.as_ref().unwrap().id, id);
        db.execute_batch("DROP TRIGGER fail_attempt").unwrap();
        app.retry_saves();
        assert!(app.save_queue.is_empty());
        app.retry_saves();
        assert_eq!(app.store.attempts().unwrap().len(), 1);
    }
    #[test]
    fn queued_input_uses_capture_time_not_the_later_render_time() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = PreviewApp::new(
            Library::open(&dir.path().join("db")).unwrap(),
            dir.path().to_path_buf(),
            false,
        )
        .unwrap();
        let index = app.records.iter().position(|r| r.id == "course-1").unwrap();
        app.open_song(index);
        app.wait_mode = false;
        app.start_practice();
        let captured = app.now();
        app.transport.seek(0.0, captured).unwrap();
        // Simulate a delayed UI frame. The queued packet still belongs to position zero.
        app.started -= Duration::from_secs(1);
        app.feed_at(MidiEvent::parse(&[0x90, 60, 90], "midi").unwrap(), captured);
        app.pause_practice();
        let row = &app.store.attempts().unwrap()[0].report;
        assert_eq!(row["hit"], 1);
        assert!(row["mistakes"].as_array().unwrap().is_empty());
    }
    #[test]
    fn events_captured_before_start_do_not_score_in_the_new_session() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = PreviewApp::new(
            Library::open(&dir.path().join("db")).unwrap(),
            dir.path().to_path_buf(),
            false,
        )
        .unwrap();
        let index = app.records.iter().position(|r| r.id == "course-1").unwrap();
        app.open_song(index);
        app.wait_mode = false;
        let stale = app.now();
        app.start_practice();
        let now = app.now();
        app.transport.seek(0.0, now).unwrap();
        app.feed_at(MidiEvent::parse(&[0x90, 60, 90], "midi").unwrap(), stale);
        app.pause_practice();
        assert!(app.store.attempts().unwrap().is_empty());
    }
}
