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
        }
    }
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
}

fn blocked(change: Change, now: Activity) -> Option<Blocked> {
    let Activity {
        writing,
        editing,
        transfer,
        no_index,
    } = now;
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
                writing: self.edit_busy,
                editing: self.edit.is_some(),
                transfer,
                no_index: self.db.is_in_memory(),
            },
        )
        .or_else(|| {
            let edit = matches!(change, Change::Edit | Change::Rewrite);
            let jpeg = path.is_none_or(|p| library::format_of(p) == Some(library::Format::Jpeg));
            (edit && !jpeg).then_some(Blocked::NotJpeg)
        })
    }

    /// `true` when `change` may go ahead; otherwise a hint says why not.
    pub(super) fn allowed(&mut self, change: Change, path: Option<&Path>) -> bool {
        match self.blocked_for(change, path) {
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
    }
}
