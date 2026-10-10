//! The update check from the app's side (`crate::update` asks GitHub): the switch (*Settings ▸
//! Check for updates*, off until the user says yes – the user's decision of 2026-10-10), the
//! question at the first start, when the check is due (10 s after the first photo, so the start
//! never waits for it, and at most once a day), the hint once per newer version, and what About
//! Cerno says about it.

use std::time::Duration;

use eframe::egui;

use crate::i18n;
use crate::update::{self, Checker, Version};

use super::CernoApp;
use super::menu::ConfirmAction;
use super::notice::Notice;

/// The saved switch; off unless the user turned it on (the question or the settings).
pub(super) const SETTING: &str = "update_check";
/// The first start's question was answered (yes or no): it doesn't come again.
const ASKED: &str = "update_asked";
/// When the last check got an answer, and which version was the latest then.
const CHECKED: &str = "update_checked_ms";
const LATEST: &str = "update_latest";
/// The newer version the hint was shown for.
const TOLD: &str = "update_told";
/// The automatic check waits this long after the first photo.
const AFTER_FIRST_PHOTO: Duration = Duration::from_secs(10);

/// What is known about updates, for About Cerno.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum State {
    Never,
    Checking,
    Current,
    Newer(Version),
    Failed,
}

pub(super) struct Updates {
    pub(super) enabled: bool,
    /// The user has decided – answered the question or used the switch.
    asked: bool,
    checker: Checker,
    state: State,
    /// This session's automatic check was looked at (it runs once a session at most).
    looked: bool,
    /// A newer version whose hint waits for the notice to be free.
    to_tell: Option<Version>,
}

impl Updates {
    /// From the settings: the switch and what the last check found.
    pub(super) fn new(db: &crate::db::Db) -> Self {
        let switch = db.setting(SETTING);
        let enabled = switch.as_deref() == Some("1");
        let asked = switch.is_some() || db.setting(ASKED).is_some();
        let latest = db.setting(LATEST).and_then(|v| Version::parse(&v));
        let state = match latest {
            Some(latest) if latest > Version::current() => State::Newer(latest),
            Some(_) => State::Current,
            None => State::Never,
        };
        Self {
            enabled,
            asked,
            checker: Checker::default(),
            state,
            looked: false,
            to_tell: None,
        }
    }

    /// A newer release than this Cerno, once known.
    pub(super) fn newer(&self) -> Option<Version> {
        match self.state {
            State::Newer(version) => Some(version),
            _ => None,
        }
    }
}

impl CernoApp {
    /// Each frame: the first start's question, a finished check, a hint that waits, the
    /// automatic check once it is due.
    pub(super) fn poll_updates(&mut self, ctx: &egui::Context) {
        self.ask_about_updates();
        if let Some(result) = self.updates.checker.poll() {
            self.finish_update_check(result);
        }
        if let Some(version) = self.updates.to_tell
            && self.notice.is_none()
        {
            self.updates.to_tell = None;
            self.db.put_setting(TOLD, &version.to_string());
            self.notice = Some(Notice::hint((i18n::t().update_available)(
                &version.to_string(),
            )));
        }
        if self.updates.looked || !self.updates.enabled {
            return;
        }
        let Some(first) = self.startup.first_photo else {
            return;
        };
        let waited = first.elapsed();
        if waited < AFTER_FIRST_PHOTO {
            ctx.request_repaint_after(AFTER_FIRST_PHOTO - waited);
            return;
        }
        let last = self.db.setting(CHECKED).and_then(|ms| ms.parse().ok());
        if !update::due(true, last, crate::db::now_ms()) {
            self.updates.looked = true;
            return;
        }
        self.updates.looked = true;
        self.start_update_check(ctx);
    }

    /// Until the user has decided: once the window shows, a card asks whether Cerno may check
    /// – over nothing else, so it never closes a menu or card that is open. Closed without an
    /// answer (Cerno ended), it asks again at the next start.
    fn ask_about_updates(&mut self) {
        if self.updates.asked
            || !self.startup.logged_first_frame
            || self.layer.is_open()
            || self.modal_open()
            || self.menu_bar.state.focus
        {
            return;
        }
        // Asked once: the card is up, and its answer is saved.
        self.updates.asked = true;
        self.ask(ConfirmAction::UpdateCheck, false);
    }

    /// The question's answer: the switch as the user said, and no question again.
    pub(super) fn answer_update_question(&mut self, yes: bool) {
        self.db.put_flag(ASKED, true);
        self.updates.enabled = yes;
        self.db.put_flag(SETTING, yes);
    }

    /// About Cerno's *Check now* (also while the switch is off: the user asked).
    pub(super) fn start_update_check(&mut self, ctx: &egui::Context) {
        self.updates.checker.start(ctx);
        if self.updates.checker.busy() {
            self.updates.state = State::Checking;
        }
    }

    fn finish_update_check(&mut self, result: Result<Version, String>) {
        match result {
            Ok(latest) => {
                self.db
                    .put_setting(CHECKED, &crate::db::now_ms().to_string());
                self.db.put_setting(LATEST, &latest.to_string());
                if latest > Version::current() {
                    self.updates.state = State::Newer(latest);
                    let told = self.db.setting(TOLD).and_then(|v| Version::parse(&v));
                    if told != Some(latest) {
                        self.updates.to_tell = Some(latest);
                    }
                } else {
                    self.updates.state = State::Current;
                }
                log::info!("update check: latest release {latest}");
            }
            Err(err) => {
                // No network, a proxy: only the log and About Cerno say so.
                log::info!("update check: {err}");
                self.updates.state = State::Failed;
            }
        }
    }

    /// *Settings ▸ Check for updates*.
    pub(super) fn toggle_update_check(&mut self) {
        self.updates.enabled = !self.updates.enabled;
        self.updates.asked = true;
        self.db.put_flag(SETTING, self.updates.enabled);
    }

    /// What About Cerno says, and the newer version's page when there is one.
    pub(super) fn update_line(&self) -> crate::ui::help::UpdateLine {
        let t = i18n::t();
        let status = match self.updates.state {
            State::Checking => t.update_checking.to_owned(),
            State::Newer(version) => (t.update_newer)(&version.to_string()),
            State::Current => t.update_current.to_owned(),
            State::Failed => t.update_failed.to_owned(),
            State::Never if !self.updates.enabled => t.update_off.to_owned(),
            State::Never => t.update_never.to_owned(),
        };
        let status = match self.updates.state {
            State::Current | State::Failed | State::Newer(_) if !self.updates.enabled => {
                format!("{status} {}", t.update_off)
            }
            _ => status,
        };
        crate::ui::help::UpdateLine {
            status,
            release: self.updates.newer().map(|version| {
                (
                    (t.cmd_update_download)(&version.to_string()),
                    version.page(),
                )
            }),
            checking: self.updates.checker.busy(),
        }
    }

    /// The newer version's page in the browser (the menu's *Download Cerno … *).
    pub(super) fn open_release_page(&self, ctx: &egui::Context) {
        if let Some(version) = self.updates.newer() {
            ctx.open_url(egui::OpenUrl::new_tab(version.page()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    fn updates(settings: &[(&str, &str)]) -> Updates {
        let db = Db::open_in_memory().unwrap();
        for (key, value) in settings {
            db.put_setting(key, value);
        }
        Updates::new(&db)
    }

    #[test]
    fn the_check_is_off_and_asked_for_until_the_user_decides() {
        let fresh = updates(&[]);
        assert!(!fresh.enabled && !fresh.asked);

        let yes = updates(&[(ASKED, "1"), (SETTING, "1")]);
        assert!(yes.enabled && yes.asked);
        let no = updates(&[(ASKED, "1"), (SETTING, "0")]);
        assert!(!no.enabled && no.asked);
        // The switch used without the question is an answer too.
        let switched = updates(&[(SETTING, "0")]);
        assert!(!switched.enabled && switched.asked);
    }
}
