// SPDX-License-Identifier: GPL-3.0-or-later

//! A bin that is always as tall as it is wide.
//!
//! `gtk::AspectFrame` only fits its child into whatever height the parent
//! hands it; it never asks for that height. A row of trinket choices therefore
//! collapses into thin strips. This bin answers height-for-width with its own
//! width, so a row of them grows into squares as the pane widens.

use gtk::glib;
use gtk::prelude::*;

/// The narrowest a square gets before its row stops shrinking.
const MINIMUM: i32 = 48;
/// The side a square asks for when nothing constrains it.
const NATURAL: i32 = 96;

mod imp {
    use super::{MINIMUM, NATURAL, glib};
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;

    #[derive(Default)]
    pub struct SquareBin;

    #[glib::object_subclass]
    impl ObjectSubclass for SquareBin {
        const NAME: &'static str = "SeedSeekerSquareBin";
        type Type = super::SquareBin;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for SquareBin {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for SquareBin {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let child_minimum = self
                .obj()
                .first_child()
                .map_or(0, |child| child.measure(orientation, -1).0);
            let minimum = child_minimum.max(MINIMUM);
            if orientation == gtk::Orientation::Horizontal {
                return (minimum, NATURAL.max(minimum), -1, -1);
            }
            let side = if for_size < 0 { NATURAL } else { for_size }.max(minimum);
            (side, side, -1, -1)
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            if let Some(child) = self.obj().first_child() {
                child.allocate(width, height, baseline, None);
            }
        }
    }
}

glib::wrapper! {
    pub struct SquareBin(ObjectSubclass<imp::SquareBin>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl SquareBin {
    /// Squares `child`, filling the square with it.
    pub fn new(child: &impl IsA<gtk::Widget>) -> Self {
        let bin: Self = glib::Object::new();
        child.set_parent(&bin);
        bin
    }
}
