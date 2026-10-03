//! A video playing in the photo area (`playback`). `Space` starts it and pauses it, `Alt+←`/
//! `Alt+→` jump 5 s, `,`/`.` step a frame, `↑`/`↓` set the volume; the bar under it does the
//! same with the mouse. It plays only in the single view: another photo, the grid or compare
//! mode stop it, and so does anything that moves or deletes the file – Windows refuses to
//! rename a file that is open, so a file operation waits until the player has let it go.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, Id, Rect, pos2};

use crate::i18n;
use crate::library;
use crate::playback::{self, Audio, MediaInfo, Player};
use crate::ui::video_controls::{self, Controls};
use crate::ui::viewer;

use super::CernoApp;
use super::notice::Notice;

/// How far `Alt+←`/`Alt+→` jump.
const JUMP: Duration = Duration::from_secs(5);
/// How much `↑`/`↓` change the volume.
const VOLUME_STEP: f32 = 0.1;
/// The bar fades this long after the pointer last moved while the video plays.
const CONTROLS_SHOW: Duration = Duration::from_secs(2);
const CONTROLS_FADE: Duration = Duration::from_millis(300);
/// While the timeline is dragged, a new keyframe seek at most this often.
const SCRUB_EVERY: Duration = Duration::from_millis(120);

/// Saved settings.
pub(super) const VOLUME_KEY: &str = "video_volume";
pub(super) const MUTED_KEY: &str = "video_muted";

pub(super) struct Session {
    player: Player,
    /// The pointer last moved over the photo area: the bar shows for a while after it.
    pointer_moved: Instant,
    /// The last keyframe seek of a drag on the timeline.
    scrubbed: Option<Instant>,
}

/// What `playback::probe` found for one video, read in the background for the details panel.
pub(super) struct Probe {
    path: PathBuf,
    result: Option<Option<MediaInfo>>,
    rx: Option<mpsc::Receiver<Option<MediaInfo>>>,
}

/// What the video keys of one frame ask for (read with the others in `keys`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct VideoKeys {
    /// `Space`, pressed now (not a repeat).
    pub toggle: bool,
    /// `Alt+←` −1, `Alt+→` +1.
    pub jump: i8,
    /// `,` −1, `.` +1.
    pub step: i8,
    /// `↓` −1, `↑` +1.
    pub volume: i8,
}

impl CernoApp {
    /// The current photo, if it is a video shown alone (not in the grid, not compared).
    pub(super) fn current_video(&self) -> Option<&Path> {
        if self.grid || self.pinned.is_some() {
            return None;
        }
        self.view
            .get(self.current)
            .map(|p| p.as_path())
            .filter(|p| library::format_of(p) == Some(library::Format::Video))
    }

    /// `Space` or the play button on the current video: start it, or play / pause. A
    /// build without the feature `video` says it plays no videos.
    pub(super) fn toggle_video(&mut self, ctx: &egui::Context) {
        let Some(path) = self.current_video().map(Path::to_path_buf) else {
            return;
        };
        if let Some(session) = &self.video
            && session.player.path() == path
        {
            session.player.toggle();
            return;
        }
        self.stop_video();
        let target = self.target.unwrap_or(super::START_TARGET);
        match Player::start(
            ctx,
            &path,
            target,
            self.video_volume,
            self.video_muted,
            Audio::Device,
        ) {
            Ok(player) => {
                self.video = Some(Session {
                    player,
                    pointer_moved: Instant::now(),
                    scrubbed: None,
                });
            }
            Err(err) => self.notice = Some(Notice::error((i18n::t().video_play_failed)(&err))),
        }
    }

    /// The streams of the video `path` for the details panel: read in the background the
    /// first time (a few hundred milliseconds), then kept until another video is asked for.
    /// `None` while it is read, or when it can't be.
    pub(super) fn media_info(&mut self, ctx: &egui::Context, path: &Path) -> Option<MediaInfo> {
        if self
            .media_probe
            .as_ref()
            .is_none_or(|probe| probe.path != path)
        {
            let (tx, rx) = mpsc::channel();
            let (file, ctx) = (path.to_path_buf(), ctx.clone());
            let spawned = std::thread::Builder::new()
                .name("cerno-video-probe".into())
                .spawn(move || {
                    let info = playback::probe(&file)
                        .inspect_err(|err| log::warn!("video {}: {err}", file.display()))
                        .ok();
                    let _ = tx.send(info);
                    ctx.request_repaint();
                });
            self.media_probe = Some(Probe {
                path: path.to_path_buf(),
                result: spawned.is_err().then_some(None),
                rx: spawned.is_ok().then_some(rx),
            });
        }
        let probe = self.media_probe.as_mut()?;
        if let Some(rx) = &probe.rx
            && let Ok(info) = rx.try_recv()
        {
            probe.result = Some(info);
            probe.rx = None;
        }
        probe.result.clone().flatten()
    }

    /// Stops the video (in the background); its file counts as open until it is closed.
    pub(super) fn stop_video(&mut self) {
        if let Some(session) = self.video.take() {
            self.video_releases.push(session.player.stop());
        }
    }

    /// Every stopped player has closed its file: copy, move and delete may touch it.
    pub(super) fn videos_released(&mut self) -> bool {
        self.video_releases.retain(|release| !release.done());
        self.video_releases.is_empty()
    }

    /// Waits up to `limit` for the stopped players (on exit, before deletions are carried out).
    pub(super) fn wait_for_videos(&mut self, limit: Duration) {
        let until = Instant::now() + limit;
        while !self.videos_released() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Once per frame: the player follows the view – another photo, the grid or compare mode
    /// stop it – takes the photo area's size, and reports an error. GStreamer is loaded in the
    /// background as soon as a video is shown.
    pub(super) fn update_video(&mut self, ctx: &egui::Context) {
        let current = self.current_video().map(Path::to_path_buf);
        if current.is_some() {
            playback::warm_up();
        }
        let keep = self
            .video
            .as_ref()
            .is_some_and(|session| Some(session.player.path()) == current.as_deref());
        if !keep {
            self.stop_video();
        }
        let Some(session) = &self.video else {
            return;
        };
        if let Some(target) = self.target {
            session.player.set_target(target);
        }
        let status = session.player.status();
        if let Some(err) = status.error {
            self.stop_video();
            self.notice = Some(Notice::error((i18n::t().video_play_failed)(&err)));
            return;
        }
        // No sound device: it plays without sound – said once per run, not for every video.
        if status.silent && !self.video_silent_told {
            self.video_silent_told = true;
            self.notice = Some(Notice::hint(i18n::t().video_no_sound));
        }
        // The time and the timeline move between frames that arrive on their own.
        if status.playing || status.starting {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        if !self.video_releases.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }

    /// The video keys, on the current video only.
    pub(super) fn video_keys(&mut self, ctx: &egui::Context, keys: VideoKeys) {
        if keys.toggle {
            self.toggle_video(ctx);
        }
        if keys.volume != 0 {
            let volume = (self.video_volume + VOLUME_STEP * f32::from(keys.volume)).clamp(0.0, 1.0);
            self.set_video_volume(Some(volume), Some(false));
        }
        let Some(session) = &mut self.video else {
            return;
        };
        session.pointer_moved = Instant::now();
        let status = session.player.status();
        if keys.jump != 0 {
            let to = if keys.jump > 0 {
                status.position + JUMP
            } else {
                status.position.saturating_sub(JUMP)
            };
            let to = status.duration.map_or(to, |d| to.min(d));
            session.player.seek(to, false);
        }
        if keys.step != 0 {
            session.player.step(keys.step > 0);
        }
    }

    /// New volume and/or sound on/off, for the player and saved.
    fn set_video_volume(&mut self, volume: Option<f32>, muted: Option<bool>) {
        if let Some(volume) = volume {
            self.video_volume = volume;
            self.db.put_setting(VOLUME_KEY, &format!("{volume:.2}"));
        }
        if let Some(muted) = muted {
            self.video_muted = muted;
            self.db
                .put_setting(MUTED_KEY, if muted { "1" } else { "0" });
        }
        if let Some(session) = &self.video {
            session
                .player
                .set_volume(self.video_volume, self.video_muted);
        }
    }

    /// The playing video's frame over the photo slot `area`, and its bar. `false` when this
    /// slot shows no playing video (the poster and the play pill stay).
    pub(super) fn draw_video(&mut self, ui: &egui::Ui, area: Rect, path: &Path) -> bool {
        let Some(session) = &mut self.video else {
            return false;
        };
        if session.player.path() != path {
            return false;
        }
        let status = session.player.status();
        if let Some(texture) = session.player.texture() {
            let [w, h] = texture.size();
            let frame = viewer::Frame {
                area,
                image_size: [w as u32, h as u32],
                display_size: [w as u32, h as u32],
                pixels_per_point: ui.ctx().pixels_per_point(),
            };
            let rect = viewer::Zoom::default().image_rect(&frame);
            let painter = ui.painter().with_clip_rect(area);
            // The poster underneath may be framed a hair differently: covered.
            painter.rect_filled(rect.expand(1.0), 0.0, crate::theme::tokens::CANVAS);
            painter.image(
                texture.id(),
                rect,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        let now = Instant::now();
        if ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO)
            && ui
                .input(|i| i.pointer.hover_pos())
                .is_some_and(|p| area.contains(p))
        {
            session.pointer_moved = now;
        }
        let since = now.duration_since(session.pointer_moved);
        let opacity = if status.playing && since > CONTROLS_SHOW {
            1.0 - ((since - CONTROLS_SHOW).as_secs_f32() / CONTROLS_FADE.as_secs_f32())
        } else {
            1.0
        }
        .clamp(0.0, 1.0);
        if opacity > 0.0 && opacity < 1.0 {
            ui.ctx().request_repaint();
        } else if opacity == 1.0 && status.playing {
            ui.ctx()
                .request_repaint_after(CONTROLS_SHOW.saturating_sub(since));
        }
        let out = video_controls::show(
            ui,
            area,
            &Controls {
                playing: status.playing,
                position: status.position,
                duration: status.duration,
                volume: self.video_volume,
                muted: self.video_muted,
                opacity,
            },
            Id::new("video-controls"),
        );
        let Some(session) = &mut self.video else {
            return true;
        };
        if out.hovered {
            session.pointer_moved = now;
        }
        if out.toggle {
            session.player.toggle();
        }
        if let Some((to, accurate)) = out.seek {
            let due = session
                .scrubbed
                .is_none_or(|last| now.duration_since(last) >= SCRUB_EVERY);
            if accurate || due {
                session.player.seek(to, accurate);
                session.scrubbed = (!accurate).then_some(now);
            }
        }
        if out.mute {
            let muted = !self.video_muted;
            self.set_video_volume(None, Some(muted));
        }
        if let Some(volume) = out.volume {
            self.set_video_volume(Some(volume), Some(false));
        }
        true
    }
}

/// The volume saved last time (0..=1), and whether the sound was off.
pub(super) fn saved_volume(db: &crate::db::Db) -> (f32, bool) {
    let volume = db
        .setting(VOLUME_KEY)
        .and_then(|v| v.parse::<f32>().ok())
        .map_or(0.8, |v| v.clamp(0.0, 1.0));
    (volume, db.setting(MUTED_KEY).as_deref() == Some("1"))
}
