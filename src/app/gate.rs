//! One action at a time on a photo: whether a mark, deletion, edit or copy/move may
//! happen right now.

use std::path::Path;

use crate::i18n;
use crate::library;
use crate::transfer::Mode as TransferMode;

use super::CernoApp;
use super::notice::Notice;

/// What an action wants to do to a photo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Change {
    /// Stars, rejection, colour label.
    Mark,
    /// Into `.originals` beside the photo (with the countdown).
    Delete,
    /// Open straighten or crop.
    Edit,
    /// Rewrite the file at once: quarter turn, `Ctrl+Z`.
    Rewrite,
    /// Open it in another program, which may save over it (its first original is kept first).
    External,
    /// Copy or move the photos on screen.
    Transfer,
    /// Put a deleted photo back into its folder.
    Restore,
}

/// Why a photo can't be changed right now: one action at a time on a photo. Checked against
/// in-memory state only – the UI never waits for a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Blocked {
    /// A straighten, crop, quarter turn or `Ctrl+Z` is being written.
    Writing,
    /// Straighten or crop is open: `Enter` or `Esc` first.
    Editing,
    /// The photo is part of a copy that is waiting or running.
    Copying,
    /// The photo is being moved to another folder.
    Moving,
    /// The index could not be opened: an edit's kept original would have no row to find it
    /// by (`Ctrl+Z`).
    NoIndex,
    /// Straighten, crop, quarter turns and `Ctrl+Z` exist for JPEG only.
    NotJpeg,
    /// The photo is deleted (in `.originals`): it takes nothing until it is put back.
    Deleted,
    /// Marks and edits are written by ExifTool, and there is none.
    NoExifTool,
    /// The ExifTool found is older than 12.24 (`exiftool::MIN_VERSION`).
    ExifToolOld,
    /// ExifTool is being downloaded.
    ExifToolLoading,
}

impl Blocked {
    pub(super) fn hint(self) -> &'static str {
        let t = i18n::t();
        match self {
            Self::Writing => t.edit_writing,
            Self::Editing => t.busy_editing,
            Self::Copying => t.busy_copying,
            Self::Moving => t.busy_moving,
            Self::NoIndex => t.edit_needs_index,
            Self::NotJpeg => t.edit_not_jpeg,
            Self::Deleted => t.busy_deleted,
            Self::NoExifTool => t.exiftool_missing,
            Self::ExifToolOld => t.exiftool_too_old,
            Self::ExifToolLoading => t.exiftool_loading,
        }
    }

    /// The fix is ExifTool: the hint offers it.
    pub(super) fn needs_exiftool(self) -> bool {
        matches!(
            self,
            Self::NoExifTool | Self::ExifToolOld | Self::ExifToolLoading
        )
    }
}

/// Whether ExifTool can write now.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tool {
    #[default]
    Ready,
    Missing,
    TooOld,
    Loading,
}

/// What is going on right now, as far as [`blocked`] is concerned.
#[derive(Debug, Default, Clone, Copy)]
struct Activity {
    /// An edit is in the writer.
    writing: bool,
    /// A straighten or crop session is open.
    editing: bool,
    /// How the photo (or, for several photos, any of them) takes part in a copy or move.
    transfer: Option<TransferMode>,
    /// The index is the in-memory fallback.
    no_index: bool,
    /// The photo is deleted, lying in `.originals`.
    deleted: bool,
    /// ExifTool, which writes marks and edits.
    tool: Tool,
}

fn blocked(change: Change, now: Activity) -> Option<Blocked> {
    let Activity {
        writing,
        editing,
        transfer,
        no_index,
        deleted,
        tool,
    } = now;
    // A deleted photo is only looked at – and put back, which is all that may happen to it.
    match (change, deleted) {
        (Change::Restore, _) => return None,
        (_, true) => return Some(Blocked::Deleted),
        _ => {}
    }
    // Marks and edits go through ExifTool (`E` too: its save may drop the marks, which come
    // back through ExifTool). Without it they are greyed out instead of failing.
    if matches!(
        change,
        Change::Mark | Change::Edit | Change::Rewrite | Change::External
    ) {
        match tool {
            Tool::Ready => {}
            Tool::Missing => return Some(Blocked::NoExifTool),
            Tool::TooOld => return Some(Blocked::ExifToolOld),
            Tool::Loading => return Some(Blocked::ExifToolLoading),
        }
    }
    if no_index && matches!(change, Change::Edit | Change::Rewrite | Change::External) {
        return Some(Blocked::NoIndex);
    }
    match (change, transfer) {
        // A moved file is gone from here: nothing may queue up for its old path.
        (_, Some(TransferMode::Move)) => return Some(Blocked::Moving),
        // Marks may wait: the file lock keeps a rating write and the copy apart.
        (
            Change::Delete | Change::Edit | Change::Rewrite | Change::External,
            Some(TransferMode::Copy),
        ) => {
            return Some(Blocked::Copying);
        }
        _ => {}
    }
    match change {
        Change::Mark => None,
        Change::Delete | Change::Edit | Change::Rewrite | Change::External | Change::Transfer
            if writing =>
        {
            Some(Blocked::Writing)
        }
        Change::Rewrite | Change::External | Change::Transfer if editing => Some(Blocked::Editing),
        _ => None,
    }
}

impl CernoApp {
    /// Why `change` can't happen to `path` right now – or, without a path, to the photos on
    /// screen (they may take part in a copy or move).
    fn blocked_for(&self, change: Change, path: Option<&Path>) -> Option<Blocked> {
        let transfer = match path {
            Some(path) => self.transfers.involves(path),
            None => self.transfers.mode(),
        };
        blocked(
            change,
            Activity {
                writing: self.edits.busy,
                editing: self.edits.session.is_some(),
                transfer,
                no_index: self.db.is_in_memory(),
                deleted: path.is_some_and(|p| self.is_deleted(p)),
                tool: self.exiftool_tool(),
            },
        )
        .or_else(|| {
            let edit = matches!(change, Change::Edit | Change::Rewrite);
            let jpeg = path.is_none_or(|p| library::format_of(p) == Some(library::Format::Jpeg));
            (edit && !jpeg).then_some(Blocked::NotJpeg)
        })
    }

    /// `true` when `change` may go ahead; otherwise a hint says why not. ExifTool is looked
    /// for once more first (installed meanwhile); without it the hint offers it.
    pub(super) fn allowed(&mut self, change: Change, path: Option<&Path>) -> bool {
        let mut reason = self.blocked_for(change, path);
        if reason.is_some_and(Blocked::needs_exiftool) && self.recheck_exiftool() {
            reason = self.blocked_for(change, path);
        }
        match reason {
            Some(reason) if reason.needs_exiftool() => {
                self.offer_exiftool(reason);
                false
            }
            Some(reason) => {
                self.notice = Some(Notice::hint(reason.hint()));
                false
            }
            None => true,
        }
    }

    /// The reason as a menu row shows it (greyed out, the reason as its tooltip).
    pub(super) fn menu_block(&self, change: Change, path: Option<&Path>) -> Option<&'static str> {
        self.blocked_for(change, path).map(Blocked::hint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_action_at_a_time_on_a_photo() {
        use Change::*;
        let idle = Activity::default();
        let moving = Activity {
            transfer: Some(TransferMode::Move),
            ..idle
        };
        let copying = Activity {
            transfer: Some(TransferMode::Copy),
            ..idle
        };
        let writing = Activity {
            writing: true,
            ..idle
        };
        let editing = Activity {
            editing: true,
            ..idle
        };
        let no_index = Activity {
            no_index: true,
            ..idle
        };
        // Nothing going on: everything may happen.
        for change in [Mark, Delete, Edit, Rewrite, External, Transfer] {
            assert_eq!(blocked(change, idle), None, "{change:?}");
        }
        // A moved photo takes nothing, a copied one still takes marks.
        for change in [Mark, Delete, Edit, Rewrite, External] {
            assert_eq!(blocked(change, moving), Some(Blocked::Moving));
        }
        assert_eq!(blocked(Mark, copying), None);
        for change in [Delete, Edit, Rewrite, External] {
            assert_eq!(blocked(change, copying), Some(Blocked::Copying));
        }
        // An edit being written holds everything but marks.
        assert_eq!(blocked(Mark, writing), None);
        for change in [Delete, Edit, Rewrite, External, Transfer] {
            assert_eq!(blocked(change, writing), Some(Blocked::Writing));
        }
        // An open session: no quarter turn, no Ctrl+Z, no other program, no copy or move
        // until Enter or Esc.
        assert_eq!(blocked(Rewrite, editing), Some(Blocked::Editing));
        assert_eq!(blocked(External, editing), Some(Blocked::Editing));
        assert_eq!(blocked(Transfer, editing), Some(Blocked::Editing));
        assert_eq!(blocked(Mark, editing), None);
        assert_eq!(blocked(Edit, editing), None, "S/R switch the session");
        // Without the index file no edit at all – its kept original would be lost track of.
        for change in [Edit, Rewrite, External] {
            assert_eq!(blocked(change, no_index), Some(Blocked::NoIndex));
        }
        for change in [Mark, Delete, Transfer] {
            assert_eq!(blocked(change, no_index), None);
        }
        // A deleted photo takes nothing but going back – not even while another one is
        // being written.
        let deleted = Activity {
            deleted: true,
            ..idle
        };
        for change in [Mark, Delete, Edit, Rewrite, External, Transfer] {
            assert_eq!(
                blocked(change, deleted),
                Some(Blocked::Deleted),
                "{change:?}"
            );
        }
        for now in [
            deleted,
            Activity {
                writing: true,
                ..deleted
            },
        ] {
            assert_eq!(blocked(Restore, now), None);
        }
        // Without ExifTool nothing is written into a photo; deleting, putting back, copying
        // and moving don't need it.
        for (tool, reason) in [
            (Tool::Missing, Blocked::NoExifTool),
            (Tool::TooOld, Blocked::ExifToolOld),
            (Tool::Loading, Blocked::ExifToolLoading),
        ] {
            let now = Activity { tool, ..idle };
            for change in [Mark, Edit, Rewrite, External] {
                assert_eq!(blocked(change, now), Some(reason), "{change:?}");
                assert!(reason.needs_exiftool());
            }
            for change in [Delete, Transfer, Restore] {
                assert_eq!(blocked(change, now), None, "{change:?}");
            }
            // A deleted photo says so first: putting it back is what it needs.
            let gone = Activity {
                deleted: true,
                ..now
            };
            assert_eq!(blocked(Mark, gone), Some(Blocked::Deleted));
        }
        assert!(!Blocked::Writing.needs_exiftool());
    }
}
