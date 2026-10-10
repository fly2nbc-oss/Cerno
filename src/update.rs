//! The update check: at most once a day, one GET on the repository's "latest release" page.
//! GitHub answers with a redirect to `…/releases/tag/v1.10.0`, and only that `Location`
//! header is read – no API (and its rate limit), no JSON, no body. Nothing is sent but the
//! request itself: the User-Agent is just `Cerno`, without version or system, and no path or
//! photo ever goes anywhere. Nothing is downloaded or installed; a newer version is only named,
//! with a link to its page.

use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Context as _, Result};
use eframe::egui;

const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
/// One day between two automatic checks.
pub const DAY_MS: i64 = 24 * 60 * 60 * 1000;
/// The whole request, connecting included.
const TIMEOUT: Duration = Duration::from_secs(10);

/// A release version, `major.minor.patch`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(u32, u32, u32);

impl Version {
    /// `1.10.0` or `v1.10.0`; anything else – a pre-release suffix, a fourth number – is none.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim().trim_start_matches('v');
        let mut parts = text.split('.').map(|part| {
            (!part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
                .then(|| part.parse::<u32>().ok())
                .flatten()
        });
        let version = Self(parts.next()??, parts.next()??, parts.next()??);
        parts.next().is_none().then_some(version)
    }

    /// The running Cerno.
    pub fn current() -> Self {
        Self::parse(env!("CARGO_PKG_VERSION")).unwrap_or(Self(0, 0, 0))
    }

    /// The release's page on GitHub, for the links.
    pub fn page(self) -> String {
        format!("{REPOSITORY}/releases/tag/v{self}")
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// The version a redirect points at: `https://github.com/o/r/releases/tag/v1.10.0`.
pub fn version_in(location: &str) -> Option<Version> {
    let (_, tag) = location.rsplit_once("/releases/tag/")?;
    Version::parse(tag.split(['?', '#']).next()?)
}

/// Whether the automatic check is due: switched on, and none for a day – a clock set back
/// counts as due, or it would wait until it caught up.
pub fn due(enabled: bool, last_ms: Option<i64>, now_ms: i64) -> bool {
    enabled && last_ms.is_none_or(|last| now_ms < last || now_ms - last >= DAY_MS)
}

/// Asks GitHub which release is the latest. HTTPS only, no redirect followed.
pub fn latest() -> Result<Version> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .user_agent("Cerno")
        .max_redirects(0)
        .timeout_global(Some(TIMEOUT))
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .into();
    let response = agent.get(format!("{REPOSITORY}/releases/latest")).call()?;
    let location = response
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .with_context(|| format!("no redirect ({})", response.status()))?;
    version_in(location).with_context(|| format!("not a release: {location}"))
}

/// One check at a time, on its own thread.
#[derive(Default)]
pub struct Checker {
    rx: Option<mpsc::Receiver<Result<Version, String>>>,
}

impl Checker {
    /// Starts a check unless one runs; the window is woken when it is done.
    pub fn start(&mut self, ctx: &egui::Context) {
        if self.rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        let spawned = std::thread::Builder::new()
            .name("cerno-update".into())
            .spawn(move || {
                let _ = tx.send(latest().map_err(|err| format!("{err:#}")));
                ctx.request_repaint();
            });
        match spawned {
            Ok(_) => self.rx = Some(rx),
            Err(err) => log::warn!("update check: {err}"),
        }
    }

    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }

    /// The finished check, once.
    pub fn poll(&mut self) -> Option<Result<Version, String>> {
        let result = match self.rx.as_ref()?.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => Err("the check ended".to_owned()),
        };
        self.rx = None;
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_their_numbers() {
        assert_eq!(Version::parse("v1.10.0"), Some(Version(1, 10, 0)));
        assert!(Version::parse("1.10.0") > Version::parse("1.9.1"));
        assert!(Version::parse("2.0.0") > Version::parse("1.99.99"));
        for odd in [
            "1.10",
            "1.10.0.1",
            "1.10.0-rc1",
            "models-1",
            "",
            "v",
            "1..0",
            "1.x.0",
        ] {
            assert_eq!(Version::parse(odd), None, "{odd}");
        }
        assert_eq!(Version(1, 10, 0).to_string(), "1.10.0");
        assert!(Version::current() > Version(1, 9, 0));
        assert_eq!(
            Version(1, 10, 0).page(),
            "https://github.com/fly2nbc-oss/Cerno/releases/tag/v1.10.0"
        );
    }

    #[test]
    fn the_redirect_names_the_release() {
        let at = "https://github.com/fly2nbc-oss/Cerno/releases/tag/v1.10.0";
        assert_eq!(version_in(at), Some(Version(1, 10, 0)));
        assert_eq!(version_in(&format!("{at}?x=1")), Some(Version(1, 10, 0)));
        assert_eq!(
            version_in("https://github.com/fly2nbc-oss/Cerno/releases"),
            None
        );
        assert_eq!(
            version_in("https://github.com/fly2nbc-oss/Cerno/releases/tag/models-1"),
            None
        );
    }

    #[test]
    fn a_check_is_due_once_a_day_and_only_when_on() {
        let now = 1_000 * DAY_MS;
        assert!(due(true, None, now));
        assert!(!due(false, None, now));
        assert!(!due(true, Some(now - DAY_MS + 1), now));
        assert!(due(true, Some(now - DAY_MS), now));
        assert!(due(true, Some(now + 5_000), now), "a clock set back");
    }

    /// The real page (network): `CERNO_TEST_DOWNLOAD=update cargo test -- --ignored latest`.
    #[test]
    #[ignore]
    fn the_latest_release_is_found() {
        if std::env::var("CERNO_TEST_DOWNLOAD").as_deref() != Ok("update") {
            return;
        }
        let latest = latest().expect("the latest release");
        eprintln!("latest: {latest}");
        assert!(latest >= Version(1, 9, 1));
    }
}
