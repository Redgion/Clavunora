// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
use anyhow::{Result, ensure};

/// All time is musical seconds. Caller supplies elapsed monotonic seconds, never frame counts.
#[derive(Debug, Clone)]
pub struct Transport {
    position: f64,
    anchor_time: f64,
    anchor_position: f64,
    pub speed: f64,
    pub playing: bool,
    pub duration: f64,
}
impl Transport {
    pub fn new(duration: f64) -> Result<Self> {
        ensure!(
            duration.is_finite() && duration >= 0.0,
            "Некорректная длительность"
        );
        Ok(Self {
            position: -3.0,
            anchor_time: 0.0,
            anchor_position: -3.0,
            speed: 1.0,
            playing: false,
            duration,
        })
    }
    pub fn current(&self, now: f64) -> f64 {
        if self.playing {
            (self.anchor_position + (now - self.anchor_time).max(0.0) * self.speed)
                .min(self.duration)
        } else {
            self.position
        }
    }
    pub fn seek(&mut self, position: f64, now: f64) -> Result<()> {
        ensure!(
            position.is_finite() && now.is_finite(),
            "Некорректная позиция"
        );
        self.position = position.clamp(-3.0, self.duration);
        self.anchor_position = self.position;
        self.anchor_time = now;
        Ok(())
    }
    pub fn play(&mut self, now: f64) -> Result<()> {
        self.seek(self.current(now), now)?;
        self.playing = true;
        Ok(())
    }
    pub fn pause(&mut self, now: f64) -> Result<()> {
        self.seek(self.current(now), now)?;
        self.playing = false;
        Ok(())
    }
    pub fn set_speed(&mut self, speed: f64, now: f64) -> Result<()> {
        ensure!(
            speed.is_finite() && (0.25..=2.0).contains(&speed),
            "Скорость вне 25–200%"
        );
        self.seek(self.current(now), now)?;
        self.speed = speed;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn time_is_continuous_through_speed_and_pause() {
        let mut t = Transport::new(10.0).unwrap();
        t.play(100.0).unwrap();
        assert_eq!(t.current(103.0), 0.0);
        t.set_speed(0.5, 104.0).unwrap();
        assert_eq!(t.current(106.0), 2.0);
        t.pause(106.0).unwrap();
        assert_eq!(t.current(200.0), 2.0);
        t.play(200.0).unwrap();
        assert_eq!(t.current(202.0), 3.0);
        assert_eq!(t.current(300.0), 10.0);
    }
    #[test]
    fn clamps_and_rejects_bad_values() {
        let mut t = Transport::new(10.0).unwrap();
        t.seek(-20.0, 0.0).unwrap();
        assert_eq!(t.current(0.0), -3.0);
        assert!(t.set_speed(f64::NAN, 0.0).is_err());
        assert!(t.set_speed(0.1, 0.0).is_err());
        assert!(t.seek(f64::INFINITY, 0.0).is_err());
    }
}
