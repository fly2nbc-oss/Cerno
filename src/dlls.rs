//! The DLLs Windows loads only at their first call (`build.rs`'s `/DELAYLOAD`): when one is
//! missing, that call ends Cerno with an exception (0xC06D007E) – no panic, no `crash.log`.
//! So before the first call each part looks for its DLLs here and, without them, switches
//! itself off with a message: videos without GStreamer, HEIC without libheif, the models on
//! the CPU without DirectML. The packages carry every one of them; a development build started
//! without GStreamer on `PATH` does not.

/// GStreamer and GLib, as `build.rs` delay-loads them with the feature `video`.
#[cfg(feature = "video")]
pub const GSTREAMER: &[&str] = &[
    "gstreamer-1.0-0.dll",
    "gstvideo-1.0-0.dll",
    "gstpbutils-1.0-0.dll",
    "gstapp-1.0-0.dll",
    "gobject-2.0-0.dll",
    "glib-2.0-0.dll",
    "gio-2.0-0.dll",
];

/// libheif (feature `heic`); libde265 is its own import, found beside it.
#[cfg(feature = "heic")]
pub const LIBHEIF: &[&str] = &["heif.dll"];

/// The models' GPU backend.
#[cfg(windows)]
pub const DIRECTML: &[&str] = &["DirectML.dll"];

/// Loads every one of `names` the way the delay-load helper would – beside the exe, the
/// system folders, `PATH`; never the working directory (`SetDllDirectoryW("")` at start) –
/// and keeps them loaded. The first missing one is named.
#[cfg(windows)]
pub fn load(names: &[&str]) -> Result<(), String> {
    use windows::Win32::System::LibraryLoader::LoadLibraryW;
    use windows::core::HSTRING;
    for name in names {
        // SAFETY: a plain file name; the module stays loaded for the process, as the
        // delay-load helper would keep it.
        if let Err(err) = unsafe { LoadLibraryW(&HSTRING::from(*name)) } {
            return Err(format!("{name} not found ({err})"));
        }
    }
    Ok(())
}

/// Other systems link these libraries the usual way: the program doesn't start without them.
#[cfg(not(windows))]
#[cfg_attr(not(any(feature = "video", feature = "heic")), allow(dead_code))]
pub fn load(_names: &[&str]) -> Result<(), String> {
    Ok(())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// A DLL that isn't there is named, one that is (System32's) loads.
    #[test]
    fn a_missing_dll_is_named() {
        assert_eq!(load(&["kernel32.dll"]), Ok(()));
        let err = load(&["kernel32.dll", "cerno-no-such-library.dll"]).unwrap_err();
        assert!(err.contains("cerno-no-such-library.dll"), "{err}");
    }
}
