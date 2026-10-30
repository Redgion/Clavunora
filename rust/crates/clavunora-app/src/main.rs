#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]
// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
mod aero;
mod app;
mod audio;
mod backups;
mod dialogs;
mod keyboard_view;
mod midi_import;
mod midi_input;
mod platform;
mod score_editor;
mod staff_view;
use anyhow::{Context, Result, ensure};
use app::PreviewApp;
use clavunora_core::store::{Library, import_legacy};
use std::{
    env,
    path::{Path, PathBuf},
    process::ExitCode,
};

#[derive(Default)]
struct Options {
    data_dir: Option<PathBuf>,
    legacy: Option<PathBuf>,
    self_test: bool,
    audio_test: bool,
    audio_device: Option<String>,
    soundfont: Option<String>,
    smoke: bool,
    list_midi: bool,
    inspect: bool,
    capture: Option<PathBuf>,
    capture_page: Option<String>,
    capture_dark: bool,
}
fn parse() -> Result<Options> {
    let mut args = env::args().skip(1);
    let mut options = Options::default();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--data-dir" => {
                options.data_dir = Some(args.next().context("После --data-dir нужен путь")?.into());
            }
            "--import-legacy" => {
                options.legacy = Some(
                    args.next()
                        .context("После --import-legacy нужен путь к library.sqlite3")?
                        .into(),
                );
            }
            "--capture-page" => {
                options.capture_page = Some(args.next().context("Нужен экран для снимка")?)
            }
            "--capture-dark" => options.capture_dark = true,
            "--capture" => {
                options.capture = Some(
                    args.next()
                        .context("После --capture нужен PNG-путь")?
                        .into(),
                );
                options.smoke = true;
            }
            "--audio-test" => options.audio_test = true,
            "--audio-device" => {
                options.audio_device = Some(
                    args.next()
                        .context("После --audio-device нужно имя устройства")?,
                )
            }
            "--soundfont" => {
                options.soundfont = Some(args.next().context("После --soundfont нужен путь")?)
            }
            "--self-test" => options.self_test = true,
            "--inspect" => options.inspect = true,
            "--smoke-test" => options.smoke = true,
            "--list-midi" => options.list_midi = true,
            "--help" | "-h" => {
                println!(
                    "Clavunora\n\n--data-dir PATH        Separate data directory\n--import-legacy PATH   Copy Python library.sqlite3 into a NEW preview database\n--inspect              Check profile data and print counts without opening a window\n--self-test            Headless checks using temporary data\n--capture PATH        Save screenshot and close; temporary profile unless --data-dir is set\n--smoke-test           Open native UI for three seconds, then close\n--list-midi            List input ports\n--audio-test           Play a short tone and metronome click; no user data\n--audio-device NAME    Audio device for --audio-test (default)\n--soundfont PATH       Optional .sf2/.sf3 for --audio-test\n\nLocal piano practice. Works offline; profiles are separate from the legacy Python application."
                );
                std::process::exit(0);
            }
            _ => anyhow::bail!("Неизвестный аргумент: {arg}"),
        }
    }
    ensure!(
        !(options.self_test && options.legacy.is_some()),
        "Самопроверка не импортирует пользовательские данные"
    );
    ensure!(
        (options.capture_page.is_none() && !options.capture_dark) || options.capture.is_some(),
        "Выбор экрана и темы для снимка требует --capture"
    );
    Ok(options)
}
fn config_root() -> Result<PathBuf> {
    platform::config_root()
}

fn normalized(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return Ok(path.canonicalize()?);
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()?.join(path)
    };
    let mut result = PathBuf::new();
    for part in absolute.components() {
        match part {
            std::path::Component::ParentDir => {
                result.pop();
            }
            std::path::Component::CurDir => {}
            _ => result.push(part),
        }
    }
    Ok(result)
}
fn prepare(options: &Options) -> Result<PreviewApp> {
    let data = options
        .data_dir
        .clone()
        .unwrap_or(platform::profile_dir(&config_root()?));
    ensure!(
        normalized(&data)? != normalized(&config_root()?.join("PianoFlow"))?,
        "Укажите отдельную папку профиля; рабочая папка Python защищена"
    );
    let target = data.join("library.sqlite3");
    if let Some(source) = &options.legacy {
        import_legacy(source, &target)?;
        println!("Скопирована библиотека: {}", target.display());
    }
    PreviewApp::new(Library::open(&target)?, data, options.smoke)
}
fn self_test() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let mut app = prepare(&Options {
        data_dir: Some(temp.path().join("data")),
        ..Default::default()
    })?;
    let (scoring_report, scoring_score) = app.verify_scoring()?;
    let rhythm_report = app.verify_rhythm()?;
    let musicxml_import_score = clavunora_core::musicxml::parse(
        include_bytes!("../../../../tests/fixtures/musicxml-import.musicxml"),
        "MusicXML",
    )?
    .song;
    let midi_import_score = clavunora_core::midi_file::parse(
        include_bytes!("../../../../assets/happy_birthday.mid"),
        "happy_birthday",
    )?
    .song;
    let (lesson_report, lesson_score) = app.verify_selected_lesson()?;
    let ctx = eframe::egui::Context::default();
    aero::install_fonts(&ctx);
    let mut frames = 0;
    for dark in [false, true] {
        app.dark = dark;
        for page in app::Page::ALL {
            app.page = page;
            if page == app::Page::Practice {
                app.open_for_test(0);
            }
            let output = ctx.run_ui(
                eframe::egui::RawInput {
                    screen_rect: Some(eframe::egui::Rect::from_min_size(
                        eframe::egui::Pos2::ZERO,
                        eframe::egui::vec2(1200.0, 800.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    eframe::egui::CentralPanel::default().show_inside(ui, |ui| app.draw(ui));
                },
            );
            ensure!(!output.shapes.is_empty(), "Пустой экран");
            let meshes = ctx.tessellate(output.shapes, output.pixels_per_point);
            ensure!(!meshes.is_empty(), "Нет графических примитивов");
            frames += 1;
        }
    }
    ensure!(app.records.len() == 124, "Неполный каталог");
    println!(
        "{}",
        serde_json::json!({"ok":true,"runtime":"Rust","library_items":app.records.len(),"ui_frames":frames,"user_data":"untouched","scoring_report":scoring_report,"rhythm_report":rhythm_report,"scoring_score":scoring_score,"lesson_report":lesson_report,"lesson_score":lesson_score,"midi_import_score":midi_import_score,"musicxml_import_score":musicxml_import_score})
    );
    Ok(())
}
fn isolate_capture(options: &mut Options) -> Result<Option<tempfile::TempDir>> {
    if options.capture.is_some() && options.data_dir.is_none() {
        let temp = tempfile::tempdir()?;
        options.data_dir = Some(temp.path().join("data"));
        Ok(Some(temp))
    } else {
        Ok(None)
    }
}
fn run() -> Result<()> {
    let mut options = parse()?;
    let _capture_profile = isolate_capture(&mut options)?;
    if options.audio_test {
        ensure!(
            options.legacy.is_none(),
            "Проверка звука не импортирует данные"
        );
        return audio::check_output(&audio::Config {
            device: options.audio_device.unwrap_or_else(|| "default".into()),
            soundfont: options.soundfont.unwrap_or_default(),
            ..Default::default()
        });
    }
    if options.self_test {
        return self_test();
    }
    if options.list_midi {
        for (i, name) in midi_input::MidiService::ports()?.iter().enumerate() {
            println!("{i}: {name}");
        }
        return Ok(());
    }
    let mut app = prepare(&options)?;
    let capture_requested = options.capture.is_some();
    let capture_succeeded = app.capture_succeeded.clone();
    if options.capture.is_some() {
        app.capture_page(
            options.capture_page.as_deref().unwrap_or("home"),
            options.capture_dark,
        )?;
    }
    if let Some(path) = options.capture {
        app.set_capture(path);
    }
    if options.inspect {
        println!("{}", app.inspect()?);
        return Ok(());
    }
    let icon = image::load_from_memory(include_bytes!(
        "../../../../pianoflow/resources/icons/pianoflow.png"
    ))?
    .into_rgba8();
    let native = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Clavunora")
            .with_icon(eframe::egui::IconData {
                width: icon.width(),
                height: icon.height(),
                rgba: icon.into_raw(),
            })
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([760.0, 540.0])
            .with_app_id("org.redgion.clavunora"),
        renderer: eframe::Renderer::Glow,
        // Covered Wayland windows must not block MIDI/event processing in a buffer swap.
        // The transport requests repaint at 16 ms while active; musical time is independent.
        vsync: false,
        ..Default::default()
    };
    eframe::run_native(
        "Clavunora",
        native,
        Box::new(move |cc| {
            aero::install_fonts(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    ensure!(
        !capture_requested || capture_succeeded.load(std::sync::atomic::Ordering::Relaxed),
        "Окно закрылось до получения снимка"
    );
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Clavunora: {e:#}");
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            if env::args().len() == 1 {
                rfd::MessageDialog::new()
                    .set_title("Clavunora")
                    .set_level(rfd::MessageLevel::Error)
                    .set_description(format!("Не удалось запустить приложение: {e:#}"))
                    .show();
            }
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn captures_default_to_private_temporary_profile() {
        let mut options = Options {
            capture: Some("unused.png".into()),
            ..Default::default()
        };
        let temp = isolate_capture(&mut options).unwrap().unwrap();
        assert!(options.data_dir.as_ref().unwrap().starts_with(temp.path()));
        let mut app = prepare(&options).unwrap();
        app.capture_page("error-review", false).unwrap();
        assert_eq!(app.inspect().unwrap()["attempts"], 1);
        assert_eq!(app.data_dir, options.data_dir.clone().unwrap());
        let supplied = options.data_dir.clone();
        assert!(isolate_capture(&mut options).unwrap().is_none());
        assert_eq!(options.data_dir, supplied);
        drop(app);
        let path = temp.path().to_path_buf();
        drop(temp);
        assert!(!path.exists());
    }
    #[test]
    fn all_pages_render_without_device_or_user_data() {
        self_test().unwrap();
    }
}
