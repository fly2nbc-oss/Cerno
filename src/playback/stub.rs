//! The player of a build without the feature `video`: it never starts.

use super::*;

/// Without the feature there is no player; [`Player::start`] says so.
pub enum Player {}

impl Player {
    pub fn start(
        _ctx: &egui::Context,
        _path: &Path,
        _target: [u32; 2],
        _volume: f32,
        _muted: bool,
        _audio: Audio,
    ) -> Result<Self, String> {
        Err("this build plays no videos (feature `video`)".to_owned())
    }

    pub fn path(&self) -> &Path {
        match *self {}
    }

    pub fn texture(&self) -> Option<egui::TextureHandle> {
        match *self {}
    }

    pub fn status(&self) -> Status {
        match *self {}
    }

    pub fn toggle(&self) {
        match *self {}
    }

    pub fn seek(&self, _to: Duration, _accurate: bool) {
        match *self {}
    }

    pub fn step(&self, _forward: bool) {
        match *self {}
    }

    pub fn set_volume(&self, _volume: f32, _muted: bool) {
        match *self {}
    }

    pub fn set_target(&self, _target: [u32; 2]) {
        match *self {}
    }

    pub fn stop(self) -> Release {
        match self {}
    }
}
