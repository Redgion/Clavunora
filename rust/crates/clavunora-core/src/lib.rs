// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
//! Music and storage without UI dependencies. Compatible with the Python score format.
pub mod backup;
pub mod catalog;
pub mod midi;
pub mod practice;
pub mod score;
pub mod store;
pub mod transport;

pub mod performance;

pub mod session;

pub mod version;

pub mod achievements;

pub mod lesson;

pub mod midi_file;

pub mod musicxml;

mod course_extension;

pub mod error_practice;
pub mod notation;
pub mod playback;
