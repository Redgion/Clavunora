// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
//! CPAL output on Windows (WASAPI) and macOS (CoreAudio).
#![cfg_attr(target_os = "linux", allow(dead_code))]
use super::*;
use cpal::{
    FromSample, SampleFormat, SizedSample, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};

pub(super) fn run(config: &Config, shared: &Arc<Mutex<Shared>>, stop: &AtomicBool) -> Result<()> {
    let host = cpal::default_host();
    let device = if config.device == "default" {
        host.default_output_device()
            .context("Нет системного аудиовыхода")?
    } else {
        host.output_devices()?
            .find(|d| {
                d.description().is_ok_and(|s| s.name() == config.device)
                    || d.id().is_ok_and(|id| id.to_string() == config.device)
            })
            .context("Аудиоустройство не найдено. Укажите default и подключите звук заново")?
    };
    let supported = device.default_output_config()?;
    let format = supported.sample_format();
    let stream_config: StreamConfig = supported.into();
    ensure!(
        stream_config.channels > 0 && stream_config.sample_rate > 0,
        "Некорректный формат аудиоустройства"
    );
    let failed = Arc::new(AtomicBool::new(false));
    macro_rules! build {
        ($sample:ty) => {
            stream::<$sample>(
                &device,
                stream_config.clone(),
                config,
                shared.clone(),
                failed.clone(),
            )?
        };
    }
    let stream = match format {
        SampleFormat::F32 => build!(f32),
        SampleFormat::F64 => build!(f64),
        SampleFormat::I8 => build!(i8),
        SampleFormat::I16 => build!(i16),
        SampleFormat::I24 => build!(cpal::I24),
        SampleFormat::I32 => build!(i32),
        SampleFormat::I64 => build!(i64),
        SampleFormat::U8 => build!(u8),
        SampleFormat::U16 => build!(u16),
        SampleFormat::U32 => build!(u32),
        SampleFormat::U64 => build!(u64),
        _ => anyhow::bail!("Неподдерживаемый формат звука: {format}"),
    };
    stream.play()?;
    shared.lock().unwrap_or_else(|e| e.into_inner()).status = format!(
        "Подключено: {} · {} Гц",
        device.description()?.name(),
        stream_config.sample_rate
    );
    while !stop.load(Ordering::Relaxed) && !failed.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(10));
    }
    drop(stream);
    ensure!(
        !failed.load(Ordering::Relaxed),
        "Аудиовыход потерян. Подключите звук заново в настройках"
    );
    Ok(())
}
fn stream<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: StreamConfig,
    sound: &Config,
    shared: Arc<Mutex<Shared>>,
    failed: Arc<AtomicBool>,
) -> Result<cpal::Stream> {
    let channels = usize::from(config.channels);
    let rate = config.sample_rate;
    let mut renderer = Renderer::new(&sound.soundfont, rate)?;
    let failure = failed.clone();
    let error_shared = shared.clone();
    let mut buffer = [0.0; FRAMES * 2];
    let stream = device.build_output_stream(
        config,
        move |out: &mut [T], info: &cpal::OutputCallbackInfo| {
            if failed.load(Ordering::Relaxed) {
                out.fill(T::from_sample(0.0));
                return;
            }
            let snapshot = shared.lock().unwrap_or_else(|e| e.into_inner()).clone();
            let timestamp = info.timestamp();
            let lead = timestamp
                .playback
                .duration_since(timestamp.callback)
                .min(Duration::from_millis(500));
            let start = Instant::now() + lead;
            for (block, chunk) in out.chunks_mut(FRAMES * channels).enumerate() {
                let frames = chunk.len() / channels;
                let at =
                    start + Duration::from_secs_f64(block as f64 * FRAMES as f64 / f64::from(rate));
                if renderer
                    .render_at(&snapshot, &mut buffer[..frames * 2], at)
                    .is_err()
                {
                    failed.store(true, Ordering::Relaxed);
                    chunk.fill(T::from_sample(0.0));
                    continue;
                }
                convert(&buffer[..frames * 2], chunk, channels);
                let peak = buffer[..frames * 2]
                    .iter()
                    .map(|x| x.abs())
                    .fold(0.0_f32, f32::max);
                let mut state = shared.lock().unwrap_or_else(|e| e.into_inner());
                state.peak = state.peak.max(peak);
            }
        },
        move |error| report_error(error, &error_shared, &failure),
        None,
    )?;
    Ok(stream)
}
fn report_error(error: cpal::Error, shared: &Arc<Mutex<Shared>>, failed: &AtomicBool) {
    if matches!(
        error.kind(),
        cpal::ErrorKind::DeviceChanged | cpal::ErrorKind::Xrun | cpal::ErrorKind::RealtimeDenied
    ) {
        shared.lock().unwrap_or_else(|e| e.into_inner()).status = format!("Подключено: {error}");
    } else {
        shared.lock().unwrap_or_else(|e| e.into_inner()).status =
            format!("Звук недоступен: {error}. Подключите звук заново");
        failed.store(true, Ordering::Relaxed);
    }
}
fn convert<T: SizedSample + FromSample<f32>>(stereo: &[f32], output: &mut [T], channels: usize) {
    output.fill(T::from_sample(0.0));
    for (input, frame) in stereo
        .as_chunks::<2>()
        .0
        .iter()
        .zip(output.chunks_exact_mut(channels))
    {
        if channels == 1 {
            frame[0] = T::from_sample((input[0] + input[1]) * 0.5);
        } else {
            frame[0] = T::from_sample(input[0]);
            frame[1] = T::from_sample(input[1]);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn routine_device_notifications_do_not_stop_the_stream_but_disconnection_does() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let failed = AtomicBool::new(false);
        for kind in [
            cpal::ErrorKind::RealtimeDenied,
            cpal::ErrorKind::Xrun,
            cpal::ErrorKind::DeviceChanged,
        ] {
            report_error(kind.into(), &shared, &failed);
            assert!(!failed.load(Ordering::Relaxed));
        }
        report_error(cpal::ErrorKind::DeviceNotAvailable.into(), &shared, &failed);
        assert!(failed.load(Ordering::Relaxed));
        assert!(shared.lock().unwrap().status.starts_with("Звук недоступен"));
    }
    #[test]
    fn device_sample_conversion_preserves_stereo_downmixes_mono_and_silences_extra_channels() {
        let input = [0.5, -0.5, 1.0, 0.0];
        let mut mono = [0_i16; 2];
        convert(&input, &mut mono, 1);
        assert_eq!(mono[0], 0);
        assert!(mono[1] > 16000);
        let mut surround = [1.0_f32; 12];
        convert(&input, &mut surround, 6);
        assert_eq!(&surround[..2], &input[..2]);
        assert_eq!(&surround[2..6], &[0.0; 4]);
        assert_eq!(&surround[6..8], &input[2..]);
        let mut unsigned = [0_u16; 4];
        convert(&[0.0; 4], &mut unsigned, 2);
        assert_eq!(unsigned, [32768; 4]);
    }
}
