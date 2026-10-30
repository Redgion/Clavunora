// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
use anyhow::{Context, Result};
use clavunora_core::midi::MidiEvent;
use midir::{Ignore, MidiInput, MidiInputConnection};
use std::{
    sync::mpsc::{self, Receiver, SyncSender},
    time::{Duration, Instant},
};
pub struct TimedInput {
    pub event: MidiEvent,
    pub received_at: Instant,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Port {
    pub id: String,
    pub name: String,
}
impl Port {
    // ALSA appends a volatile client:port address to the display name.
    fn identity(&self) -> &str {
        self.name
            .strip_suffix(&format!(" {}", self.id))
            .unwrap_or(&self.name)
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum Change {
    Lost,
    Connected,
}
struct Monitor {
    request: SyncSender<()>,
    snapshots: Receiver<std::result::Result<Vec<Port>, String>>,
}
impl Monitor {
    fn start() -> Result<Self> {
        let (request, commands) = mpsc::sync_channel(1);
        let (sender, snapshots) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("clavunora-midi-ports".into())
            .spawn(move || {
                loop {
                    let result = scan().map_err(|e| e.to_string());
                    match sender.try_send(result) {
                        Ok(()) | Err(mpsc::TrySendError::Full(_)) => {}
                        Err(mpsc::TrySendError::Disconnected(_)) => break,
                    }
                    match commands.recv_timeout(Duration::from_secs(1)) {
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                }
            })?;
        Ok(Self { request, snapshots })
    }
}
fn scan() -> Result<Vec<Port>> {
    let input =
        MidiInput::new("Clavunora port monitor").map_err(|e| anyhow::anyhow!(e.to_string()))?;
    input
        .ports()
        .iter()
        .map(|p| {
            Ok(Port {
                id: p.id(),
                name: input
                    .port_name(p)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?,
            })
        })
        .collect()
}
/// No callback or monitor writes to the library or practice state.
pub struct MidiService {
    connection: Option<MidiInputConnection<()>>,
    active: Option<Port>,
    desired: Option<Port>,
    monitor: Option<Monitor>,
    pub available: Vec<Port>,
    pub message: String,
    pub receiver: Receiver<TimedInput>,
}
impl Default for MidiService {
    fn default() -> Self {
        let (_, receiver) = mpsc::channel();
        Self {
            connection: None,
            active: None,
            desired: None,
            monitor: None,
            available: vec![],
            message: "MIDI не подключён. Экранный и компьютерный ввод доступны.".into(),
            receiver,
        }
    }
}
impl MidiService {
    /// A disconnected fixture for the isolated --capture UI, without opening ALSA.
    pub fn capture_disconnected() -> Self {
        let mut service = Self {
            desired: Some(Port {
                id: "20:0".into(),
                name: "Учебное пианино 20:0".into(),
            }),
            ..Self::default()
        };
        service.observe(Ok(vec![]));
        service
    }
    pub fn ports() -> Result<Vec<String>> {
        Ok(scan()?.into_iter().map(|p| p.name).collect())
    }
    pub fn monitor(&mut self) {
        if self.monitor.is_none() {
            match Monitor::start() {
                Ok(m) => self.monitor = Some(m),
                Err(e) => self.message = format!("Не удалось запустить проверку MIDI: {e}"),
            }
        }
    }
    pub fn refresh(&mut self) {
        self.monitor();
        if let Some(m) = &self.monitor {
            let _ = m.request.try_send(());
        }
    }
    pub fn watching(&self) -> bool {
        self.monitor.is_some()
    }
    pub fn waiting(&self) -> bool {
        self.desired.is_some() && !self.connected()
    }
    pub fn poll(&mut self) -> Vec<Change> {
        let mut changes = vec![];
        let result = self
            .monitor
            .as_ref()
            .and_then(|m| m.snapshots.try_recv().ok());
        if let Some(result) = result {
            changes.extend(self.observe(result));
        }
        changes
    }
    fn observe(&mut self, result: std::result::Result<Vec<Port>, String>) -> Vec<Change> {
        self.observe_with(result, Self::open)
    }
    fn observe_with(
        &mut self,
        result: std::result::Result<Vec<Port>, String>,
        open: impl FnOnce(&mut Self, &Port) -> Result<()>,
    ) -> Vec<Change> {
        let mut changes = vec![];
        match result {
            Err(error) => {
                if self.active.is_some() {
                    self.drop_connection();
                    changes.push(Change::Lost);
                }
                self.available.clear();
                self.message =
                    format!("Проверка MIDI недоступна: {error}. Повторяем автоматически.");
                return changes;
            }
            Ok(ports) => self.available = ports,
        }
        if self
            .active
            .as_ref()
            .is_some_and(|p| !self.available.contains(p))
        {
            self.drop_connection();
            changes.push(Change::Lost);
        }
        if !self.connected()
            && let Some(wanted) = &self.desired
        {
            let matches: Vec<_> = self
                .available
                .iter()
                .filter(|p| p.identity() == wanted.identity())
                .cloned()
                .collect();
            self.message=match matches.len() {
                0 => format!("Связь потеряна: {}. Ожидаем возвращения инструмента.",wanted.identity()),
                1 => match open(self, &matches[0]) {
                    Ok(()) => { self.desired=Some(matches[0].clone()); changes.push(Change::Connected); self.connected_message() },
                    Err(e) => format!("Не удалось восстановить MIDI: {e}. Повторяем автоматически."),
                },
                _ => "Найдено несколько одинаковых MIDI-портов. Выберите нужный и нажмите «Подключить».".into(),
            };
        }
        changes
    }
    pub fn connect(&mut self, expected: &Port) -> Result<()> {
        self.disconnect();
        self.monitor();
        match self.open(expected) {
            Ok(()) => {
                self.desired = Some(expected.clone());
                self.message = self.connected_message();
                Ok(())
            }
            Err(e) => {
                self.message = format!("MIDI не подключён: {e}");
                Err(e)
            }
        }
    }
    fn open(&mut self, expected: &Port) -> Result<()> {
        let mut input = MidiInput::new("Clavunora").map_err(|e| anyhow::anyhow!(e.to_string()))?;
        input.ignore(Ignore::None);
        let port = input
            .ports()
            .into_iter()
            .find(|p| p.id() == expected.id)
            .context("MIDI-порт исчез; обновите список")?;
        let name = input
            .port_name(&port)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        anyhow::ensure!(
            name == expected.name,
            "MIDI-порт изменился; выберите инструмент заново"
        );
        let (sender, receiver) = mpsc::channel();
        let connection = input
            .connect(
                &port,
                "Clavunora live input",
                move |_, bytes, _| {
                    let received_at = Instant::now();
                    if let Some(event) = MidiEvent::parse(bytes, "midi") {
                        let _ = sender.send(TimedInput { event, received_at });
                    }
                },
                (),
            )
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        self.receiver = receiver;
        self.connection = Some(connection);
        self.active = Some(expected.clone());
        Ok(())
    }
    fn connected_message(&self) -> String {
        format!(
            "MIDI подключён: {}. При потере связи занятие остановится; соединение восстановится автоматически.",
            self.active.as_ref().map_or("", |p| p.identity())
        )
    }
    fn drop_connection(&mut self) {
        self.connection.take();
        self.active = None;
        let (_, receiver) = mpsc::channel();
        self.receiver = receiver;
    }
    pub fn disconnect(&mut self) {
        self.desired = None;
        self.drop_connection();
        self.message = "MIDI отключён. Автоматическое подключение отменено.".into();
    }
    pub fn connected(&self) -> bool {
        self.active.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn port(id: &str, label: &str) -> Port {
        Port {
            id: id.into(),
            name: format!("{label} {id}"),
        }
    }
    #[test]
    fn unique_return_reconnects_on_new_address_and_failed_open_is_retried() {
        let p = port("20:0", "Piano:Piano");
        let mut m = MidiService {
            active: Some(p.clone()),
            desired: Some(p),
            ..Default::default()
        };
        let new_port = port("99:0", "Piano:Piano");
        let ports = vec![new_port.clone()];
        assert_eq!(
            m.observe_with(Ok(ports.clone()), |_, _| anyhow::bail!("busy")),
            vec![Change::Lost]
        );
        assert!(m.waiting());
        assert!(m.message.contains("busy"));
        assert_eq!(
            m.observe_with(Ok(ports), |service, p| {
                service.active = Some(p.clone());
                Ok(())
            }),
            vec![Change::Connected]
        );
        assert!(m.connected());
        assert!(!m.waiting());
        assert_eq!(m.desired, Some(new_port.clone()));
        assert!(
            m.observe_with(Ok(vec![new_port]), |_, _| panic!("already connected"))
                .is_empty()
        );
    }
    #[test]
    fn disappearance_releases_queued_events_and_keeps_reconnect_target() {
        let p = port("20:0", "Piano:Piano");
        let mut m = MidiService {
            active: Some(p.clone()),
            desired: Some(p),
            ..Default::default()
        };
        let (sender, receiver) = mpsc::channel();
        m.receiver = receiver;
        sender
            .send(TimedInput {
                event: MidiEvent::parse(&[0x90, 60, 90], "midi").unwrap(),
                received_at: Instant::now(),
            })
            .unwrap();
        assert_eq!(m.observe(Ok(vec![])), vec![Change::Lost]);
        assert!(!m.connected());
        assert!(m.waiting());
        assert!(m.receiver.try_recv().is_err());
        assert!(m.observe(Ok(vec![])).is_empty());
        m.disconnect();
        assert!(!m.waiting());
        assert!(m.observe(Ok(vec![port("21:0", "Piano:Piano")])).is_empty());
    }
    #[test]
    fn duplicate_names_require_explicit_choice_and_scan_failure_loses_connection() {
        let p = port("20:0", "Piano:Piano");
        let mut m = MidiService {
            active: Some(p.clone()),
            desired: Some(p),
            ..Default::default()
        };
        assert_eq!(
            m.observe(Err("ALSA unavailable".into())),
            vec![Change::Lost]
        );
        assert!(
            m.observe(Ok(vec![
                port("21:0", "Piano:Piano"),
                port("22:0", "Piano:Piano")
            ]))
            .is_empty()
        );
        assert!(m.message.contains("несколько"));
        assert!(!m.connected());
        assert!(m.observe(Ok(vec![port("20:0", "Other:Other")])).is_empty());
        assert!(m.waiting());
    }
    #[test]
    fn reordering_does_not_disconnect_and_address_is_not_identity() {
        let p = port("20:0", "Piano:Piano");
        assert_eq!(p.identity(), port("99:1", "Piano:Piano").identity());
        let mut m = MidiService {
            active: Some(p.clone()),
            desired: Some(p.clone()),
            ..Default::default()
        };
        assert!(
            m.observe(Ok(vec![port("19:0", "Other:Other"), p]))
                .is_empty()
        );
        assert!(m.connected());
    }
}
