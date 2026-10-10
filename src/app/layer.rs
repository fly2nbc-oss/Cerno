//! What lies over the window and takes the keyboard: the help page, the list beside a menu bar
//! row or a card – at most one at a time. Not layers: the faces grid (it covers the photo
//! only, the bars stay usable) and a straighten or crop session (a mode of the keys).

use crate::ui::help::Page;

use super::menu::{ConfirmAction, RowList};
use super::{camera_time, name_list};

#[derive(Default)]
pub(super) enum Layer {
    #[default]
    None,
    /// The help page (`H`, `F1`, `?`) on one of its tabs (`←`/`→`).
    Help(Page),
    /// The list beside a row of the menu bar: *Edit elsewhere*'s programs or the languages.
    List(RowList),
    /// Models & data.
    Models,
    /// A question waiting for Enter or Esc; `back_to_models` opens the models card again
    /// afterwards.
    Confirm {
        action: ConfirmAction,
        back_to_models: bool,
    },
    /// The file-list card (*Visible photos ▸ By file list …*).
    NameList(name_list::Card),
    /// The camera time card (*Visible photos ▸ Camera time …*).
    CameraTime(camera_time::Card),
}

impl Layer {
    pub(super) fn is_open(&self) -> bool {
        !matches!(self, Self::None)
    }

    pub(super) fn is_help(&self) -> bool {
        matches!(self, Self::Help(_))
    }

    pub(super) fn is_list(&self) -> bool {
        matches!(self, Self::List(_))
    }

    /// A card over the dimmed window – modal like the faces grid (`CernoApp::modal_open`);
    /// help and the list are not.
    pub(super) fn is_card(&self) -> bool {
        matches!(
            self,
            Self::Models | Self::Confirm { .. } | Self::NameList(_) | Self::CameraTime(_)
        )
    }

    pub(super) fn is_name_list(&self) -> bool {
        matches!(self, Self::NameList(_))
    }

    pub(super) fn is_camera_time(&self) -> bool {
        matches!(self, Self::CameraTime(_))
    }

    /// Closes the layer when `is` holds for it (`Layer::is_list` …); any other stays open.
    pub(super) fn close_if(&mut self, is: fn(&Self) -> bool) {
        if is(self) {
            *self = Self::None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_if_closes_only_its_own_kind() {
        let mut layer = Layer::Models;
        layer.close_if(Layer::is_list);
        assert!(layer.is_card());
        layer.close_if(Layer::is_card);
        assert!(!layer.is_open());
        let mut layer = Layer::Help(Page::Keys);
        layer.close_if(Layer::is_name_list);
        assert!(layer.is_help());
        assert!(!layer.is_card(), "help is not modal like a card");
    }
}
