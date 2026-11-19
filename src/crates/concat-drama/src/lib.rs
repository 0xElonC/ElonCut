// SPDX-License-Identifier: AGPL-3.0-or-later

//! Short-drama creation behind the window's Story screen.
//!
//! A project's story is scenes of shots and the cast that appears in them.
//! Each shot has three things generated for it - a first frame, a video
//! from that frame, and the spoken line - by cloud services the person
//! brings keys for. This crate holds the story and keeps it in the project
//! folder, makes the words and pictures each request carries, and runs the
//! queue that has a [`Backend`] make them; the window applies what comes
//! back and draws it.

pub mod jobs;
pub mod model;
pub mod prompt;
pub mod script;
pub mod store;

pub use jobs::{Backend, Event, Made, Queue, Rehearsal, Target};
pub use model::{Character, Kind, Line, Motion, Phase, Scene, Shot, ShotSize, Slot, Story, Take, Voice};
pub use prompt::{Missing, Request};
pub use script::{Plan, parse_line};
