// SPDX-License-Identifier: GPL-3.0-or-later

//! Query-builder sidebar: the requirement board, search scope, and the search
//! action.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::{cairo, gdk, gio, glib, pango};

use shpd_seedfinder_core::editor::{
    self, BoardView, ChipView, Edit, EffectBadge, ItemView, ResinChip, Tag,
};
use shpd_seedfinder_core::feasibility::Quest;
use shpd_seedfinder_core::main_world::normalize_floor_limit;
use shpd_seedfinder_core::query::MAX_SEARCH_DEPTH;
use shpd_seedfinder_core::quests::WandmakerQuestType;
use shpd_seedfinder_session::available_workers;

use crate::board::{self, BoardCache, Dragged, DropAnswer, Landing, StackField};
use crate::state::{AppState, FARMING_FLOORS, is_farming_requirement, wandmaker_quest_label};
use crate::{glow, sprites};

/// Makes a floor-limit spin row skip the empty boss floors (5, 10, 15):
/// spinning up from 4 lands on 6, spinning down from 6 lands on 4, and typed
/// values snap down (10 means the first 10 floors, ≡ 9), since those floors
/// add no searchable items and are useless as limits.
fn skip_empty_boss_floors(row: &adw::SpinRow) {
    let previous = Cell::new(row.value());
    row.connect_value_notify(move |row| {
        let value = row.value();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let requested = value.round().clamp(0.0, f64::from(MAX_SEARCH_DEPTH)) as u8;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let anchor = previous
            .get()
            .round()
            .clamp(0.0, f64::from(MAX_SEARCH_DEPTH)) as u8;
        let target = editor::skip_boss_floor(anchor, requested);
        if target != requested {
            // The corrected value re-enters this handler and, being a real
            // floor, records itself as the new anchor.
            row.set_value(f64::from(target));
            return;
        }
        previous.set(value);
    });
}

/// What the board asks the window to do with a requirement. Every variant
/// names rows by their session key, which survives the list moving under it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoardAction {
    ToggleFarmingFloor(u8),
    RemoveFloorRequirement(u8),
    /// Open the requirement sheet on the row.
    Open(u64),
    /// A board edit — a drop, a menu choice, a badge stepper — for the
    /// shared editor to apply or refuse.
    Edit(Edit),
}

/// What the window does with one board gesture.
type BoardHandler = Box<dyn Fn(BoardAction)>;

pub struct QueryPane {
    pub page: adw::NavigationPage,
    requirements_group: adw::PreferencesGroup,
    /// Holds the board and the drop-to-remove zone, and parents the popovers
    /// so a rebuild of the chips cannot pull them out from under the pointer.
    board_root: gtk::Box,
    /// One chip or cluster capsule per board entry, wrapping as they fill.
    board: adw::WrapBox,
    blanket_board: adw::WrapBox,
    blanket_expander: adw::ExpanderRow,
    remove_revealer: gtk::Revealer,
    menu: gtk::PopoverMenu,
    /// The "How many" radio action, whose state is set to the chip's own
    /// count just before its menu opens so the right item is ticked.
    count_action: gio::SimpleAction,
    stack_popover: gtk::Popover,
    stack_title: gtk::Label,
    stack_spin: gtk::SpinButton,
    /// The row and number the stack popover is editing, and the value it
    /// opened on — the change lands when the popover closes, so spinning
    /// never rebuilds the board out from under the pointer.
    stack_target: Cell<Option<(u64, StackField)>>,
    stack_opened_on: Cell<f64>,
    /// The board view the chips were drawn from, folded once per list.
    board_view: RefCell<BoardCache>,
    /// The chip in flight, as the drop targets under it read it.
    dragging: RefCell<Option<Dragged>>,
    /// Every chip on the board by the row it shows, for following the row
    /// an edit or a closing sheet lands on.
    chips: RefCell<Vec<(u64, gtk::Widget)>>,
    farming_buttons: Vec<(u8, gtk::ToggleButton)>,
    other_floors: gtk::Box,
    rooms_expander: adw::ExpanderRow,
    depth_row: adw::SpinRow,
    auto_trinket_row: adw::SwitchRow,
    blacksmith_row: adw::SwitchRow,
    exclude_row: adw::SwitchRow,
    wandmaker_row: adw::ComboRow,
    /// The search worker count, a device-local preference the window saves
    /// beside the query rather than inside it.
    workers_row: adw::SpinRow,
    start_content: adw::ButtonContent,
    start_button: gtk::Button,
    challenges_button: gtk::Button,
    updating: Cell<bool>,
    on_board: RefCell<Option<BoardHandler>>,
    on_changed: RefCell<Option<Box<dyn Fn()>>>,
}

impl QueryPane {
    #[allow(clippy::too_many_lines)] // Widget assembly is declarative and linear.
    pub fn new(menu_model: &gio::MenuModel) -> Rc<Self> {
        let presets_group = adw::PreferencesGroup::builder().title("Presets").build();
        let presets_row = adw::ActionRow::builder()
            .title("Manage presets")
            .subtitle("Load an included query or save the current one")
            .action_name("win.presets")
            .activatable(true)
            .build();
        presets_row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        presets_group.add(&presets_row);

        let requirements_group = adw::PreferencesGroup::builder()
            .title("Requirements")
            .description(
                "Every requirement must be satisfiable in the same run. \
                 Drop one chip on another for an either/or.",
            )
            .build();
        let board = adw::WrapBox::builder()
            .child_spacing(6)
            .line_spacing(6)
            .build();
        let remove_zone = gtk::Box::builder()
            .halign(gtk::Align::Fill)
            .spacing(6)
            .css_classes(["remove-zone"])
            .build();
        let remove_icon = gtk::Image::from_icon_name("user-trash-symbolic");
        let remove_label = gtk::Label::builder()
            .label("Drop to remove")
            .hexpand(true)
            .build();
        remove_zone.append(&remove_icon);
        remove_zone.append(&remove_label);
        let remove_revealer = gtk::Revealer::builder()
            .transition_type(gtk::RevealerTransitionType::SlideDown)
            .child(&remove_zone)
            .build();
        let board_root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .build();
        board_root.append(&board);
        board_root.append(&remove_revealer);
        requirements_group.add(&board_root);
        let blanket_board = adw::WrapBox::builder()
            .child_spacing(6)
            .line_spacing(6)
            .margin_start(12)
            .margin_end(12)
            .margin_bottom(12)
            .build();
        let blanket_expander = adw::ExpanderRow::builder()
            .title("Blanket Requirements")
            .build();
        let help = gtk::Label::builder().label(
            "Each blanket must match at least one item fulfilling your ordinary requirements or contributing Arcane Resin, without asking for an additional item. All filters in one blanket apply to the same item; separate blankets can match the same or different chosen items.\n\nFor example, require Lightning, Disintegration, and Frost at +2 or higher, then add an Any wand blanket at exactly +3 from the Wandmaker.")
            .wrap(true).max_width_chars(42).xalign(0.0).margin_start(12).margin_end(12).margin_top(12).margin_bottom(12).build();
        let help_popover = gtk::Popover::builder().child(&help).build();
        let help_button = gtk::MenuButton::builder()
            .icon_name("dialog-information-symbolic")
            .popover(&help_popover)
            .valign(gtk::Align::Center)
            .css_classes(["flat"])
            .tooltip_text("About blanket requirements")
            .build();
        blanket_expander.add_suffix(&help_button);
        blanket_expander.add_row(&blanket_board);
        let blankets_group = adw::PreferencesGroup::new();
        blankets_group.add(&blanket_expander);

        let stack_title = gtk::Label::builder()
            .css_classes(["caption-heading"])
            .halign(gtk::Align::Start)
            .build();
        let stack_spin = gtk::SpinButton::builder()
            .adjustment(&gtk::Adjustment::new(1.0, 1.0, 3.0, 1.0, 1.0, 0.0))
            .numeric(true)
            .build();
        let stack_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .build();
        stack_box.append(&stack_title);
        stack_box.append(&stack_spin);
        let stack_popover = gtk::Popover::builder().child(&stack_box).build();

        let depth_row = adw::SpinRow::builder()
            .title("Floor limit")
            .subtitle("Search only the first floors")
            .adjustment(&gtk::Adjustment::new(
                f64::from(MAX_SEARCH_DEPTH),
                1.0,
                f64::from(MAX_SEARCH_DEPTH),
                1.0,
                5.0,
                0.0,
            ))
            .build();
        let auto_trinket_row = adw::SwitchRow::builder().title("AutoTrinket")
            .subtitle("Applies a helpful trinket at +3 at the first brewing opportunity. Keeps it only when the match needs it.").margin_top(12).build();
        let blacksmith_row = adw::SwitchRow::builder()
            .title("Require accessible blacksmith")
            .subtitle("Always in range when searching 14 floors or more")
            .build();
        let exclude_row = adw::SwitchRow::builder()
            .title("Exclude Smith rewards")
            .subtitle(
                "Required items cannot come from the 2,000-favor Smith choice, \
                 leaving favor available for reforging",
            )
            .build();
        // Index zero is "Any"; the rest follow WandmakerQuestType::ALL.
        let wandmaker_labels = std::iter::once("Any")
            .chain(
                WandmakerQuestType::ALL
                    .into_iter()
                    .map(wandmaker_quest_label),
            )
            .collect::<Vec<_>>();
        let wandmaker_row = adw::ComboRow::builder()
            .title("Quest")
            .model(&gtk::StringList::new(&wandmaker_labels))
            .build();
        let wandmaker_group = adw::PreferencesGroup::builder().title("Wandmaker").build();
        wandmaker_group.add(&wandmaker_row);

        let scope_group = adw::PreferencesGroup::builder()
            .title("Search Scope")
            .build();
        scope_group.add(&depth_row);
        scope_group.add(&auto_trinket_row);

        let blacksmith_group = adw::PreferencesGroup::builder().title("Blacksmith").build();
        blacksmith_group.add(&blacksmith_row);
        blacksmith_group.add(&exclude_row);

        // How many threads a search spawns. This is a preference about the
        // machine rather than about the query, so it never reaches the search
        // document; a single-core host has nothing to choose and sees no row.
        let ceiling = available_workers();
        let workers_row = adw::SpinRow::builder()
            .title("Workers")
            .subtitle(worker_subtitle(ceiling, ceiling))
            .adjustment(&gtk::Adjustment::new(
                usize_to_f64(ceiling),
                1.0,
                usize_to_f64(ceiling),
                1.0,
                1.0,
                0.0,
            ))
            .build();
        let performance_group = adw::PreferencesGroup::builder()
            .title("Performance")
            .description("Number of search threads to spawn.")
            .visible(ceiling > 1)
            .build();
        performance_group.add(&workers_row);

        let rooms_expander = adw::ExpanderRow::builder()
            .title("Rooms and feelings")
            .build();
        let farming_row = adw::ActionRow::builder()
            .title("Ring of Wealth farming floors")
            .subtitle("Dark floor with a garden.")
            .build();
        rooms_expander.add_row(&farming_row);
        let choices = gtk::Box::builder()
            .spacing(8)
            .margin_start(12)
            .margin_end(12)
            .margin_bottom(12)
            .build();
        let farming_buttons = FARMING_FLOORS
            .into_iter()
            .map(|depth| {
                let button = gtk::ToggleButton::with_label(&format!("Floor {depth}"));
                choices.append(&button);
                (depth, button)
            })
            .collect::<Vec<_>>();
        rooms_expander.add_row(&choices);
        let other_floors = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .build();
        rooms_expander.add_row(&other_floors);
        let rooms_group = adw::PreferencesGroup::new();
        rooms_group.add(&rooms_expander);

        let preferences_page = adw::PreferencesPage::new();
        preferences_page.add(&presets_group);
        preferences_page.add(&requirements_group);
        preferences_page.add(&blankets_group);
        preferences_page.add(&scope_group);
        preferences_page.add(&wandmaker_group);
        preferences_page.add(&blacksmith_group);
        preferences_page.add(&rooms_group);
        preferences_page.add(&performance_group);

        let challenges_button = gtk::Button::builder()
            .css_classes(["flat", "caption"])
            .action_name("win.challenges")
            .halign(gtk::Align::Center)
            .visible(false)
            .build();
        let start_content = adw::ButtonContent::builder()
            .icon_name("media-playback-start-symbolic")
            .label("Start Search")
            .build();
        let start_button = gtk::Button::builder()
            .child(&start_content)
            .css_classes(["pill", "suggested-action"])
            .action_name("win.start-search")
            .build();
        let action_area = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .margin_top(6)
            .margin_bottom(12)
            .margin_start(18)
            .margin_end(18)
            .build();
        action_area.append(&challenges_button);
        action_area.append(&start_button);

        let menu_button = gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .menu_model(menu_model)
            .primary(true)
            .tooltip_text("Main Menu")
            .build();
        let header_bar = adw::HeaderBar::new();
        header_bar.pack_end(&menu_button);

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header_bar);
        toolbar_view.add_bottom_bar(&action_area);
        toolbar_view.set_content(Some(&preferences_page));

        let nav_page = adw::NavigationPage::builder()
            .title("Seed Seeker")
            .tag("query")
            .child(&toolbar_view)
            .build();

        let pane = Rc::new(Self {
            page: nav_page,
            requirements_group,
            board_root,
            board,
            blanket_board,
            blanket_expander,
            remove_revealer,
            menu: gtk::PopoverMenu::from_model(None::<&gio::Menu>),
            count_action: gio::SimpleAction::new_stateful(
                "count",
                Some(count_variant_type()),
                &(0_u64, 0_u64).to_variant(),
            ),
            stack_popover,
            stack_title,
            stack_spin,
            stack_target: Cell::new(None),
            stack_opened_on: Cell::new(1.0),
            board_view: RefCell::new(BoardCache::default()),
            dragging: RefCell::new(None),
            chips: RefCell::new(Vec::new()),
            farming_buttons,
            other_floors,
            rooms_expander,
            depth_row,
            auto_trinket_row,
            blacksmith_row,
            exclude_row,
            wandmaker_row,
            workers_row,
            start_content,
            start_button,
            challenges_button,
            updating: Cell::new(false),
            on_board: RefCell::new(None),
            on_changed: RefCell::new(None),
        });

        pane.menu.set_parent(&pane.board_root);
        pane.menu.set_position(gtk::PositionType::Bottom);
        pane.menu.set_has_arrow(true);
        pane.stack_popover.set_parent(&pane.board_root);
        pane.stack_popover.set_position(gtk::PositionType::Bottom);
        pane.board_root
            .insert_action_group("board", Some(&pane.board_actions()));
        pane.stack_popover.connect_closed({
            let pane = Rc::clone(&pane);
            move |_| pane.commit_stack()
        });
        // Dropping a cluster member on its section's own background —
        // anywhere no chip sits — pulls it out of its cluster.
        for (blanket, board) in [(false, &pane.board), (true, &pane.blanket_board)] {
            board.add_controller(pane.drop_target(Landing::Board { blanket }));
        }
        remove_zone.add_controller(pane.drop_target(Landing::Remove));

        remove_zone.add_controller(
            pane.typed_drop_target(String::static_type(), |pane, value| {
                if value.get::<String>().as_deref() != Ok("arcane_resin") {
                    return false;
                }
                pane.remove_chip(None);
                true
            }),
        );

        for (depth, button) in &pane.farming_buttons {
            let pane = Rc::clone(&pane);
            let depth = *depth;
            button.connect_toggled(move |_| {
                if !pane.updating.get() {
                    pane.emit(BoardAction::ToggleFarmingFloor(depth));
                }
            });
        }
        skip_empty_boss_floors(&pane.depth_row);
        pane.depth_row.connect_value_notify({
            let pane = Rc::clone(&pane);
            move |_| pane.notify_changed()
        });
        for row in [
            &pane.auto_trinket_row,
            &pane.blacksmith_row,
            &pane.exclude_row,
        ] {
            row.connect_active_notify({
                let pane = Rc::clone(&pane);
                move |_| pane.notify_changed()
            });
        }
        pane.wandmaker_row.connect_selected_notify({
            let pane = Rc::clone(&pane);
            move |_| pane.notify_changed()
        });
        pane.workers_row.connect_value_notify({
            let pane = Rc::clone(&pane);
            move |row| {
                row.set_subtitle(&worker_subtitle(
                    round_to_usize(row.value()),
                    available_workers(),
                ));
                pane.notify_changed();
            }
        });
        pane
    }

    fn notify_changed(&self) {
        if self.updating.get() {
            return;
        }
        if let Some(handler) = self.on_changed.borrow().as_ref() {
            handler();
        }
    }

    /// Runs when the board asks for a change to the requirements.
    pub fn connect_board(&self, handler: impl Fn(BoardAction) + 'static) {
        self.on_board.replace(Some(Box::new(handler)));
    }

    /// Runs after the user changes any scope or performance control.
    pub fn connect_changed(&self, handler: impl Fn() + 'static) {
        self.on_changed.replace(Some(Box::new(handler)));
    }

    fn emit(&self, action: BoardAction) {
        if let Some(handler) = self.on_board.borrow().as_ref() {
            handler(action);
        }
    }

    /// Copies the scope controls into `state`.
    pub fn read_scope(&self, state: &mut AppState) {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let depth = self.depth_row.value().round() as u8;
        state.max_depth = normalize_floor_limit(depth.clamp(1, MAX_SEARCH_DEPTH));
        state.auto_apply_trinket = self.auto_trinket_row.is_active();
        state.require_blacksmith = self.blacksmith_row.is_active();
        state.exclude_blacksmith_rewards = self.exclude_row.is_active();
        state.wandmaker_quest = usize::try_from(self.wandmaker_row.selected())
            .ok()
            .and_then(|index| index.checked_sub(1))
            .and_then(|index| WandmakerQuestType::ALL.get(index).copied());
    }

    /// The chosen number of search threads, always at least one.
    #[must_use]
    pub fn worker_count(&self) -> usize {
        round_to_usize(self.workers_row.value()).max(1)
    }

    /// Sets the saved worker count on the row, without echoing the change
    /// back as an edit. The caller has already clamped it to this machine.
    pub fn set_worker_count(&self, workers: usize) {
        self.updating.set(true);
        self.workers_row.set_value(usize_to_f64(workers));
        self.workers_row
            .set_subtitle(&worker_subtitle(workers, available_workers()));
        self.updating.set(false);
    }

    /// Rebuilds every control from `state` without echoing change signals.
    pub fn refresh(self: &Rc<Self>, state: &AppState) {
        self.updating.set(true);
        self.auto_trinket_row.set_active(state.auto_apply_trinket);
        self.depth_row
            .set_value(f64::from(normalize_floor_limit(state.max_depth)));
        self.blacksmith_row.set_active(state.require_blacksmith);
        // Past the Blacksmith's last possible floor every seed has the
        // quest, so the filter has nothing left to exclude.
        self.blacksmith_row
            .set_sensitive(state.max_depth < Quest::Blacksmith.window().1);
        self.exclude_row
            .set_active(state.exclude_blacksmith_rewards);
        self.wandmaker_row.set_selected(
            state
                .wandmaker_quest
                .map_or(0, |variant| u32::from(variant.wire_id())),
        );
        for (depth, button) in &self.farming_buttons {
            button.set_active(
                state
                    .floor_requirements
                    .iter()
                    .any(|floor| floor.depth == *depth && is_farming_requirement(floor)),
            );
        }
        self.rooms_expander
            .set_title(&if state.floor_requirements.is_empty() {
                "Rooms and feelings".to_owned()
            } else {
                format!("Rooms and feelings ({})", state.floor_requirements.len())
            });
        while let Some(child) = self.other_floors.first_child() {
            self.other_floors.remove(&child);
        }
        for floor in state
            .floor_requirements
            .iter()
            .filter(|floor| !is_farming_requirement(floor))
        {
            let mut details = Vec::new();
            if let Some(feeling) = floor.feeling {
                details.push(format!("{feeling:?}"));
            }
            details.extend(
                floor
                    .rooms
                    .iter()
                    .map(|room| room.stable_id().replace('_', " ")),
            );
            if !floor.any_rooms.is_empty() {
                details.push(
                    floor
                        .any_rooms
                        .iter()
                        .map(|room| room.stable_id().replace('_', " "))
                        .collect::<Vec<_>>()
                        .join(" / "),
                );
            }
            let row = adw::ActionRow::builder()
                .title(format!("Floor {}", floor.depth))
                .subtitle(details.join(" · "))
                .build();
            let remove = gtk::Button::builder()
                .icon_name("edit-delete-symbolic")
                .valign(gtk::Align::Center)
                .tooltip_text(format!("Remove floor {} requirement", floor.depth))
                .build();
            let pane = Rc::clone(self);
            let depth = floor.depth;
            remove.connect_clicked(move |_| pane.emit(BoardAction::RemoveFloorRequirement(depth)));
            row.add_suffix(&remove);
            self.other_floors.append(&row);
        }
        self.rebuild_board(state);
        let enabled = state.challenges.bits().count_ones();
        self.challenges_button.set_visible(enabled > 0);
        self.challenges_button.set_label(&format!(
            "{enabled} challenge{} enabled",
            if enabled == 1 { "" } else { "s" }
        ));
        self.updating.set(false);
    }

    fn rebuild_board(self: &Rc<Self>, state: &AppState) {
        // A drop rebuilds the board, and the chip that was in flight — along
        // with the drag it was carrying — goes with it, so the bin is put
        // away here rather than waiting for a drag that may never end.
        self.remove_revealer.set_reveal_child(false);
        self.dragging.replace(None);
        self.chips.borrow_mut().clear();
        let view = self
            .board_view
            .borrow_mut()
            .view(&state.requirements, state.resin());
        for (blanket, board) in [(false, &self.board), (true, &self.blanket_board)] {
            board.remove_all();
            if blanket {
                self.blanket_expander
                    .set_title(&format!("Blanket Requirements ({})", view.counts.blanket));
            } else {
                self.requirements_group.set_title(&requirements_title(
                    view.counts.ordinary + usize::from(view.resin.is_some()),
                ));
            }
            if !blanket && view.items.is_empty() && view.resin.is_none() {
                board.append(
                    &gtk::Label::builder()
                        .label("Nothing yet — add the item you are hunting for")
                        .css_classes(["dim-label"])
                        .build(),
                );
            }
            for item in view.items.iter().filter(|item| item.blanket == blanket) {
                if item.cluster.is_some() {
                    board.append(&self.cluster(item));
                } else {
                    board.append(&self.chip(&item.chips[0]));
                }
            }
            if !blanket && let Some(resin) = &view.resin {
                board.append(&self.resin_chip(resin));
            }
            let add = gtk::Button::builder()
                .child(
                    &adw::ButtonContent::builder()
                        .icon_name("list-add-symbolic")
                        .label("Add")
                        .build(),
                )
                .css_classes(["chip", "chip-add"])
                .action_name(if blanket {
                    "win.add-blanket"
                } else {
                    "win.add-requirement"
                })
                .tooltip_text(if blanket {
                    "Add Blanket Requirement"
                } else {
                    "Add Requirement"
                })
                .build();
            board.append(&add);
        }
    }

    fn resin_chip(self: &Rc<Self>, resin: &ResinChip) -> gtk::Widget {
        let chip = gtk::Box::builder()
            .spacing(6)
            .css_classes(["chip"])
            .focusable(true)
            .accessible_role(gtk::AccessibleRole::Button)
            .tooltip_text(board::resin_tooltip(resin))
            .build();
        chip.update_property(&[gtk::accessible::Property::Label(&resin.description)]);
        chip.append(&sprites::arcane_resin_image());
        chip.append(&gtk::Label::new(Some(&resin.name)));
        for tag in &resin.tags {
            chip.append(&chip_tag(tag));
        }
        if resin.uncursed {
            chip.append(&uncursed_mark());
        }
        self.wire_chip(&chip, None);
        chip.upcast()
    }

    /// One either/or cluster: its members share a dashed capsule. Each member
    /// wears its own stack's badges; the capsule wears none.
    fn cluster(self: &Rc<Self>, item: &ItemView) -> gtk::Widget {
        let capsule = gtk::Box::builder()
            .spacing(2)
            .css_classes(["cluster"])
            .accessible_role(gtk::AccessibleRole::Group)
            .build();
        if let Some(label) = &item.label {
            capsule.update_property(&[gtk::accessible::Property::Label(label)]);
        }
        for (position, chip) in item.chips.iter().enumerate() {
            if position > 0 {
                capsule.append(
                    &gtk::Label::builder()
                        .label("or")
                        .css_classes(["cluster-or"])
                        .build(),
                );
            }
            capsule.append(&self.chip(chip));
        }
        // The capsule around the chips stands for the whole cluster.
        capsule.add_controller(self.drop_target(Landing::Row(item.members[0])));
        capsule.upcast()
    }

    /// One requirement as a chip: its face, then the badges of its own stack,
    /// alone or as a cluster member.
    fn chip(self: &Rc<Self>, chip: &ChipView) -> gtk::Widget {
        let widget = chip_face(chip);
        for badge in self.badges(chip) {
            widget.append(&badge);
        }
        self.wire_chip(&widget, Some(chip.key));
        widget.add_controller(self.drop_target(Landing::Row(chip.key)));
        self.chips
            .borrow_mut()
            .push((chip.key, widget.clone().upcast()));
        widget.upcast()
    }

    /// The gestures every chip answers to: activate to edit, drag onto another
    /// chip for an either/or, the context menu for the same in words.
    fn wire_chip(self: &Rc<Self>, chip: &gtk::Box, key: Option<u64>) {
        let click = gtk::GestureClick::builder()
            .button(gdk::BUTTON_PRIMARY)
            .build();
        click.connect_released({
            let pane = Rc::clone(self);
            move |gesture, _, _, _| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                pane.edit_chip(key);
            }
        });
        chip.add_controller(click);

        let secondary = gtk::GestureClick::builder()
            .button(gdk::BUTTON_SECONDARY)
            .build();
        secondary.connect_pressed({
            let pane = Rc::clone(self);
            move |gesture, _, _, _| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                if let Some(chip) = gesture.widget() {
                    pane.show_menu(&chip, key);
                }
            }
        });
        chip.add_controller(secondary);

        let long_press = gtk::GestureLongPress::new();
        long_press.connect_pressed({
            let pane = Rc::clone(self);
            move |gesture, _, _| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                if let Some(chip) = gesture.widget() {
                    pane.show_menu(&chip, key);
                }
            }
        });
        chip.add_controller(long_press);

        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed({
            let pane = Rc::clone(self);
            move |controller, keyval, _, modifiers| {
                let context_key = keyval == gdk::Key::Menu
                    || (keyval == gdk::Key::F10
                        && modifiers.contains(gdk::ModifierType::SHIFT_MASK));
                if context_key {
                    if let Some(chip) = controller.widget() {
                        pane.show_menu(&chip, key);
                    }
                } else if matches!(
                    keyval,
                    gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::space
                ) {
                    pane.edit_chip(key);
                } else if matches!(keyval, gdk::Key::Delete | gdk::Key::BackSpace) {
                    pane.remove_chip(key);
                } else {
                    return glib::Propagation::Proceed;
                }
                glib::Propagation::Stop
            }
        });
        chip.add_controller(keys);

        chip.add_controller(self.drag_source(key));
    }

    /// Picking a chip up: the ghost it flies as, where it may land, and the
    /// bin shown while it is in flight.
    fn drag_source(self: &Rc<Self>, key: Option<u64>) -> gtk::DragSource {
        let drag = gtk::DragSource::builder()
            .actions(gdk::DragAction::MOVE)
            .build();
        drag.connect_prepare(move |source, _, _| {
            let widget = source.widget()?;
            // The resin chip flies as itself; a requirement chip's ghost is
            // its face alone, set once the drag begins.
            if key.is_none() {
                source.set_icon(Some(&gtk::WidgetPaintable::new(Some(&widget))), 0, 0);
            }
            Some(gdk::ContentProvider::for_value(&key.map_or_else(
                || "arcane_resin".to_value(),
                |key| key.to_value(),
            )))
        });
        drag.connect_drag_begin({
            let pane = Rc::clone(self);
            move |source, drag| {
                if let Some(widget) = source.widget() {
                    widget.add_css_class("chip-dragging");
                }
                // Where the chip may land is read once, off the board it was
                // drawn on, so hovering asks the editor nothing.
                let dragged = key.and_then(|key| pane.board_view.borrow().pick_up(key));
                // A drag moves one item: the chip in flight is its face
                // alone, without the badges of the stack it leaves behind.
                if let Some(key) = key
                    && let Some(view) = pane.board_view.borrow().current()
                    && let Some((_, chip)) = board::find_chip(&view, key)
                {
                    gtk::DragIcon::for_drag(drag).set_child(Some(&chip_face(chip)));
                }
                pane.dragging.replace(dragged);
                pane.remove_revealer.set_reveal_child(true);
            }
        });
        drag.connect_drag_end({
            let pane = Rc::clone(self);
            move |source, _, _| {
                if let Some(widget) = source.widget() {
                    widget.remove_css_class("chip-dragging");
                }
                pane.dragging.replace(None);
                pane.remove_revealer.set_reveal_child(false);
            }
        });
        drag
    }

    fn edit_chip(&self, key: Option<u64>) {
        if let Some(key) = key {
            self.emit(BoardAction::Open(key));
        } else {
            let _ = WidgetExt::activate_action(&self.page, "win.edit-resin", None);
        }
    }

    /// The menu's Remove, and Delete on a chip: the chip with its whole
    /// stack, where the bin takes one item.
    fn remove_chip(&self, key: Option<u64>) {
        if let Some(key) = key {
            self.emit(BoardAction::Edit(Edit::Remove { key }));
        } else {
            let _ = WidgetExt::activate_action(&self.page, "win.remove-resin", None);
        }
    }

    /// What releasing the chip in flight on `landing` would do.
    fn drop_answer(&self, landing: Landing) -> DropAnswer {
        self.dragging
            .borrow()
            .as_ref()
            .map_or(DropAnswer::Ignore, |dragged| dragged.drop_answer(landing))
    }

    /// A drop zone for a chip in flight. It lights up while a release there
    /// would change the board, is marked as refusing where the editor turns
    /// the join or the removal down — a release then says why — and stays
    /// dark elsewhere.
    ///
    /// Drag events bubble from the widget under the pointer to its ancestors
    /// until one takes the drop. A chip or a cluster's capsule therefore
    /// keeps even a drop it ignores, so releasing a chip on itself or on its
    /// own cluster never falls through to the board behind, where a cluster
    /// member would leave its cluster.
    fn drop_target(self: &Rc<Self>, landing: Landing) -> gtk::DropTarget {
        let target = gtk::DropTarget::new(u64::static_type(), gdk::DragAction::MOVE);
        target.connect_enter({
            let pane = Rc::clone(self);
            move |target, _, _| pane.hover(target, landing)
        });
        target.connect_motion({
            let pane = Rc::clone(self);
            move |target, _, _| pane.hover(target, landing)
        });
        target.connect_leave(unmark);
        let pane = Rc::clone(self);
        target.connect_drop(move |target, value, _, _| {
            unmark(target);
            let answer = match value.get::<u64>() {
                Ok(source)
                    if pane
                        .dragging
                        .borrow()
                        .as_ref()
                        .is_some_and(|dragged| dragged.key == source) =>
                {
                    pane.drop_answer(landing)
                }
                _ => DropAnswer::Ignore,
            };
            match answer.edit() {
                Some(edit) => {
                    pane.emit(BoardAction::Edit(edit));
                    true
                }
                None => keeps_ignored_drops(landing),
            }
        });
        target
    }

    /// Marks a drop target by what a release on it would do.
    fn hover(&self, target: &gtk::DropTarget, landing: Landing) -> gdk::DragAction {
        let answer = self.drop_answer(landing);
        unmark(target);
        let mark = match answer {
            DropAnswer::Accept(_) => "drop-target",
            DropAnswer::Refuse(_) => "drop-refused",
            DropAnswer::Ignore if keeps_ignored_drops(landing) => return gdk::DragAction::MOVE,
            DropAnswer::Ignore => return gdk::DragAction::empty(),
        };
        if let Some(widget) = target.widget() {
            widget.add_css_class(mark);
        }
        gdk::DragAction::MOVE
    }

    fn typed_drop_target(
        self: &Rc<Self>,
        payload_type: glib::Type,
        dropped: impl Fn(&Rc<Self>, &glib::Value) -> bool + 'static,
    ) -> gtk::DropTarget {
        let target = gtk::DropTarget::new(payload_type, gdk::DragAction::MOVE);
        target.connect_enter(|target, _, _| {
            if let Some(widget) = target.widget() {
                widget.add_css_class("drop-target");
            }
            gdk::DragAction::MOVE
        });
        target.connect_leave(|target| {
            if let Some(widget) = target.widget() {
                widget.remove_css_class("drop-target");
            }
        });
        let pane = Rc::clone(self);
        target.connect_drop(move |target, value, _, _| {
            if let Some(widget) = target.widget() {
                widget.remove_css_class("drop-target");
            }
            dropped(&pane, value)
        });
        target
    }

    /// The badges of one chip's own stack: how many items it asks for, and
    /// the combined level they reach together. Each opens its stepper.
    fn badges(self: &Rc<Self>, chip: &ChipView) -> Vec<gtk::Widget> {
        let key = chip.key;
        board::stack_badges(chip)
            .into_iter()
            .map(|shown| {
                let classes: &[&str] = match shown.field {
                    StackField::Count => &["stack-badge"],
                    StackField::Total => &["stack-badge", "stack-badge-total"],
                };
                let button = gtk::Button::builder()
                    .label(&shown.badge.text)
                    .css_classes(classes)
                    .valign(gtk::Align::Center)
                    .tooltip_text(&shown.badge.tooltip)
                    .build();
                button.connect_clicked({
                    let pane = Rc::clone(self);
                    let (field, value, maximum) = (shown.field, shown.value, shown.maximum);
                    move |button| {
                        let title = match field {
                            StackField::Count => "How many",
                            StackField::Total => "Combined level",
                        };
                        pane.open_stack_popover(
                            button,
                            key,
                            field,
                            title,
                            f64::from(value),
                            f64::from(maximum),
                        );
                    }
                });
                button.upcast()
            })
            .collect()
    }

    fn open_stack_popover(
        self: &Rc<Self>,
        badge: &gtk::Button,
        key: u64,
        field: StackField,
        title: &str,
        value: f64,
        maximum: f64,
    ) {
        self.stack_target.set(None);
        self.stack_title.set_label(title);
        let adjustment = self.stack_spin.adjustment();
        adjustment.set_lower(1.0);
        adjustment.set_upper(maximum.max(1.0));
        self.stack_spin.set_value(value);
        self.stack_opened_on.set(value);
        self.stack_target.set(Some((key, field)));
        self.point_at(&self.stack_popover, badge.upcast_ref());
        self.stack_popover.popup();
    }

    /// Applies what the stack popover was left showing. Doing this on close
    /// rather than on every step keeps the board still while the user spins.
    fn commit_stack(&self) {
        let Some((key, field)) = self.stack_target.take() else {
            return;
        };
        let value = self.stack_spin.value().round();
        if (value - self.stack_opened_on.get()).abs() < f64::EPSILON {
            return;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let value = value.clamp(1.0, f64::from(u8::MAX)) as u8;
        self.emit(BoardAction::Edit(match field {
            StackField::Count => Edit::SetCount { key, count: value },
            StackField::Total => Edit::SetTotal {
                key,
                total: Some(value),
            },
        }));
    }

    /// Follows the editor's key repairs in the one piece of board state that
    /// outlives a rebuild: the row the stack popover is editing.
    pub fn follow_rekeyed(&self, rekeyed: &[(u64, u64)]) {
        if let Some((key, field)) = self.stack_target.get() {
            self.stack_target
                .set(Some((board::follow_key(key, rekeyed), field)));
        }
    }

    /// Moves the focus to the chip showing row `key` — where an edit or a
    /// closing sheet landed — once the gesture that led there has settled,
    /// so a closing menu or dialog does not take the focus back.
    pub fn focus_row(self: &Rc<Self>, key: u64) {
        let pane = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            let Some(pane) = pane.upgrade() else {
                return;
            };
            let chip = pane
                .chips
                .borrow()
                .iter()
                .find(|(shown, _)| *shown == key)
                .map(|(_, chip)| chip.clone());
            if let Some(chip) = chip {
                chip.grab_focus();
            }
        });
    }

    fn show_menu(self: &Rc<Self>, chip: &gtk::Widget, key: Option<u64>) {
        let menu = match key {
            None => resin_menu(),
            Some(key) => {
                let Some(view) = self.board_view.borrow().current() else {
                    return;
                };
                let Some((_, entry)) = board::find_chip(&view, key) else {
                    return;
                };
                self.count_action
                    .set_state(&(key, u64::from(entry.stack.count)).to_variant());
                Self::chip_menu(&view, entry)
            }
        };
        self.menu.set_menu_model(Some(&menu));
        self.point_at(self.menu.upcast_ref(), chip);
        self.menu.popup();
    }

    /// Aims a popover at `widget`, in the board's own coordinates.
    fn point_at(&self, popover: &gtk::Popover, widget: &gtk::Widget) {
        if let Some(bounds) = widget.compute_bounds(&self.board_root) {
            #[allow(clippy::cast_possible_truncation)]
            popover.set_pointing_to(Some(&gdk::Rectangle::new(
                bounds.x() as i32,
                bounds.y() as i32,
                bounds.width() as i32,
                bounds.height() as i32,
            )));
        }
    }

    /// The chip's context menu: every gesture of the board said in words, for
    /// the keyboard, for touch, and for anyone who would rather not drag. What
    /// it offers is what the editor says the chip and its own stack can do —
    /// a cluster member's "How many" counts that member.
    fn chip_menu(view: &BoardView, chip: &ChipView) -> gio::Menu {
        let key = chip.key;
        let menu = gio::Menu::new();
        let first = gio::Menu::new();
        first.append_item(&menu_item("_Edit…", "board.edit", &key.to_variant()));
        // Either/or with every other entry of the section, named as it reads.
        let peers = gio::Menu::new();
        for choice in board::join_choices(view, chip) {
            peers.append_item(&menu_item(
                &choice.label,
                "board.join",
                &(key, choice.target).to_variant(),
            ));
        }
        if peers.n_items() > 0 {
            first.append_submenu(Some("_Either/or with…"), &peers);
        }
        menu.append_section(None, &first);

        if chip.stack.can_change_count {
            let counts = gio::Menu::new();
            for count in 1..=chip.stack.count_max {
                counts.append_item(&menu_item(
                    &count.to_string(),
                    "board.count",
                    &(key, u64::from(count)).to_variant(),
                ));
            }
            menu.append_section(Some("How many"), &counts);
        }
        if chip.stack.can_count_levels {
            let levels = gio::Menu::new();
            levels.append_item(&menu_item(
                if chip.stack.total.is_some() {
                    "Stop counting _levels"
                } else {
                    "Count _levels together"
                },
                "board.total",
                &key.to_variant(),
            ));
            menu.append_section(None, &levels);
        }
        if chip.can_detach {
            let alone = gio::Menu::new();
            alone.append_item(&menu_item("On its _own", "board.detach", &key.to_variant()));
            menu.append_section(None, &alone);
        }
        let last = gio::Menu::new();
        last.append_item(&menu_item("_Remove", "board.remove", &key.to_variant()));
        menu.append_section(None, &last);
        menu
    }

    /// The actions the chip menus fire, all of them naming their row by key.
    fn board_actions(self: &Rc<Self>) -> gio::SimpleActionGroup {
        let group = gio::SimpleActionGroup::new();
        for (name, action) in [
            ("edit", BoardAction::Open as fn(u64) -> BoardAction),
            ("detach", |key| BoardAction::Edit(Edit::Detach { key })),
            ("remove", |key| BoardAction::Edit(Edit::Remove { key })),
            ("total", |key| BoardAction::Edit(Edit::ToggleLevels { key })),
        ] {
            let entry = gio::SimpleAction::new(name, Some(glib::VariantTy::UINT64));
            entry.connect_activate({
                let pane = Rc::clone(self);
                move |_, target| {
                    if let Some(key) = target.and_then(glib::Variant::get::<u64>) {
                        pane.emit(action(key));
                    }
                }
            });
            group.add_action(&entry);
        }
        let join = gio::SimpleAction::new("join", Some(count_variant_type()));
        join.connect_activate({
            let pane = Rc::clone(self);
            move |_, target| {
                if let Some((source, target)) = target.and_then(glib::Variant::get::<(u64, u64)>) {
                    pane.emit(BoardAction::Edit(Edit::Join { source, target }));
                }
            }
        });
        group.add_action(&join);
        // A stateful action with a target renders as a row of radio items; the
        // state is set to the chip's own count just before the menu opens.
        let count = self.count_action.clone();
        count.connect_activate({
            let pane = Rc::clone(self);
            move |action, target| {
                let Some((key, count)) = target.and_then(glib::Variant::get::<(u64, u64)>) else {
                    return;
                };
                action.set_state(&(key, count).to_variant());
                pane.emit(BoardAction::Edit(Edit::SetCount {
                    key,
                    count: u8::try_from(count).unwrap_or(u8::MAX),
                }));
            }
        });
        group.add_action(&count);
        group
    }

    /// Flips the search action between its start and stop presentation.
    pub fn set_running(&self, running: bool) {
        if running {
            self.start_content
                .set_icon_name("media-playback-stop-symbolic");
            self.start_content.set_label("Stop Search");
            self.start_button.remove_css_class("suggested-action");
            self.start_button.add_css_class("destructive-action");
        } else {
            self.start_content
                .set_icon_name("media-playback-start-symbolic");
            self.start_content.set_label("Start Search");
            self.start_button.remove_css_class("destructive-action");
            self.start_button.add_css_class("suggested-action");
        }
    }
}

/// Whether a drop target keeps a drop it ignores rather than letting it fall
/// through to the board behind (see [`QueryPane::drop_target`]): chips and
/// capsules do; the empty board and the bin have nothing behind them.
const fn keeps_ignored_drops(landing: Landing) -> bool {
    matches!(landing, Landing::Row(_))
}

/// Clears what [`QueryPane::hover`] marked a drop target with.
fn unmark(target: &gtk::DropTarget) {
    if let Some(widget) = target.widget() {
        widget.remove_css_class("drop-target");
        widget.remove_css_class("drop-refused");
    }
}

/// The variant type of the actions that name a row and a number together.
fn count_variant_type() -> &'static glib::VariantTy {
    glib::VariantTy::new("(tt)").expect("(tt) is a valid variant type")
}

/// A menu item bound to a board action with the row it acts on as its target.
/// A u64 target cannot be spelled in a detailed action name, so it is set as a
/// value instead.
fn menu_item(label: &str, action: &str, target: &glib::Variant) -> gio::MenuItem {
    let entry = gio::MenuItem::new(Some(label), None);
    entry.set_action_and_target_value(Some(action), Some(target));
    entry
}

/// The resin chip's context menu.
fn resin_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(Some("Edit…"), Some("win.edit-resin"));
    let removal = gio::Menu::new();
    removal.append(Some("Remove"), Some("win.remove-resin"));
    menu.append_section(None, &removal);
    menu
}

/// One qualifier beside a chip's name, tinted as the editor styles it, with
/// its own hover text where the editor explains it ("Auto", "Mage +2").
/// A requirement chip's face: its sprite, its name, and the tiny tags that
/// qualify it, all as the shared editor words them — everything but the
/// badges of its stack. The board adds those; a chip in flight is its face
/// alone, the one item a drag moves.
fn chip_face(chip: &ChipView) -> gtk::Box {
    let widget = gtk::Box::builder()
        .spacing(6)
        .css_classes(["chip"])
        .focusable(true)
        .accessible_role(gtk::AccessibleRole::Button)
        .tooltip_text(board::chip_tooltip(chip))
        .build();
    widget.update_property(&[gtk::accessible::Property::Label(&chip.description)]);
    if let Some(problem) = &chip.problem {
        widget.add_css_class("chip-error");
        widget.update_property(&[gtk::accessible::Property::Description(problem)]);
    }
    widget.append(&chip_prefix(chip));
    widget.append(
        &gtk::Label::builder()
            .label(&chip.name)
            .ellipsize(pango::EllipsizeMode::End)
            .max_width_chars(18)
            .build(),
    );
    for tag in &chip.tags {
        widget.append(&chip_tag(tag));
    }
    if let Some(badge) = chip
        .effect
        .as_ref()
        .and_then(|effect| effect_badge(chip, effect))
    {
        widget.append(&badge);
    }
    for tag in &chip.trailing_tags {
        widget.append(&chip_tag(tag));
    }
    if chip.uncursed {
        widget.append(&uncursed_mark());
    }
    widget
}

fn chip_tag(tag: &Tag) -> gtk::Label {
    let label = gtk::Label::builder()
        .label(&tag.text)
        .css_classes(["chip-tag", board::tag_class(tag.style)])
        .build();
    label.set_tooltip_text(tag.tooltip.as_deref());
    label
}

/// The check mark of a chip that rules out cursed items.
fn uncursed_mark() -> gtk::Label {
    gtk::Label::builder()
        .label("\u{2713}")
        .tooltip_text("Uncursed")
        .css_classes(["chip-tag", "chip-tag-soft"])
        .build()
}

/// The effect badge, for what a pulsing sprite cannot say on its own: several
/// effects at once, "any enchantment", which settles on no colour, or an
/// effect on a wildcard chip, whose category silhouette stays grayscale.
fn effect_badge(chip: &ChipView, effect: &EffectBadge) -> Option<gtk::Widget> {
    let label = &effect.label;
    // "Any enchantment" settles on no colour of its own, so it wears them all.
    if effect.any_enchantment {
        return Some(effect_dot(None, label));
    }
    if effect.effects.len() > 1 {
        let count = gtk::Label::builder()
            .label(effect.effects.len().to_string())
            .css_classes(["effect-count"])
            .valign(gtk::Align::Center)
            .halign(gtk::Align::Center)
            .build();
        let ring = gtk::DrawingArea::builder()
            .content_width(22)
            .content_height(22)
            .build();
        let colors = effect
            .effects
            .iter()
            .filter_map(|effect| glow::effect(Some(*effect)))
            .map(glow::Glow::rgb)
            .collect::<Vec<_>>();
        ring.set_draw_func(move |_, context, width, height| {
            // Equal stationary stops, interpolated around a closed ring. The
            // item sprite still pulses; the count always shows all effects.
            let radius = f64::from(width.min(height)) / 2.0 - 1.5;
            context.set_line_width(2.0);
            for segment in 0_u32..180 {
                let position = f64::from(segment)
                    * f64::from(u32::try_from(colors.len()).unwrap_or(1))
                    / 180.0;
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let index = position.floor() as usize;
                let fraction = position.fract();
                let (r0, g0, b0) = colors[index];
                let (r1, g1, b1) = colors[(index + 1) % colors.len()];
                context.set_source_rgb(
                    r0 + (r1 - r0) * fraction,
                    g0 + (g1 - g0) * fraction,
                    b0 + (b1 - b0) * fraction,
                );
                let start = f64::from(segment) * std::f64::consts::TAU / 180.0
                    - std::f64::consts::FRAC_PI_2;
                context.arc(
                    f64::from(width) / 2.0,
                    f64::from(height) / 2.0,
                    radius,
                    start,
                    start + std::f64::consts::TAU / 180.0 + 0.01,
                );
                let _ = context.stroke();
            }
        });
        let badge = gtk::Overlay::builder()
            .child(&ring)
            .tooltip_text(label)
            .valign(gtk::Align::Center)
            .build();
        badge.add_overlay(&count);
        badge.update_property(&[gtk::accessible::Property::Label(label)]);
        return Some(badge.upcast());
    }
    // A single effect — enchantment or curse — already pulses on a real
    // sprite, and the tooltip names it; a badge would only say it twice.
    // A wildcard keeps its grayscale silhouette and green question mark, so
    // the dot carries the effect's colour.
    if chip.item.is_some() {
        return None;
    }
    Some(effect_dot(
        glow::effect(effect.effects.first().copied()),
        label,
    ))
}

/// The dot standing in for an effect: its glow colour, or the rainbow of "any
/// enchantment", which is every colour and so none.
fn effect_dot(glow: Option<glow::Glow>, label: &str) -> gtk::Widget {
    const SIZE: i32 = 12;
    let area = gtk::DrawingArea::builder()
        .content_width(SIZE)
        .content_height(SIZE)
        .valign(gtk::Align::Center)
        .tooltip_text(label)
        .accessible_role(gtk::AccessibleRole::Img)
        .build();
    area.set_draw_func(move |_, context, width, height| {
        let radius = f64::from(width.min(height)) / 2.0;
        context.arc(
            f64::from(width) / 2.0,
            f64::from(height) / 2.0,
            radius,
            0.0,
            std::f64::consts::TAU,
        );
        if let Some(glow) = glow {
            let (red, green, blue) = glow.rgb();
            context.set_source_rgb(red, green, blue);
        } else {
            let rainbow = cairo::LinearGradient::new(0.0, 0.0, f64::from(width), 0.0);
            for (offset, red, green, blue) in [
                (0.0, 1.0, 0.33, 0.33),
                (0.2, 1.0, 1.0, 0.33),
                (0.4, 0.33, 1.0, 0.33),
                (0.6, 0.33, 1.0, 1.0),
                (0.8, 0.33, 0.33, 1.0),
                (1.0, 1.0, 0.33, 1.0),
            ] {
                rainbow.add_color_stop_rgb(offset, red, green, blue);
            }
            let _ = context.set_source(&rainbow);
        }
        let _ = context.fill();
    });
    area.upcast()
}

/// The chip icon for one requirement: the item's real sprite once a concrete
/// item is pinned, pulsing the enchantment or curse the requirement asks for,
/// and otherwise a grayscale category sprite beneath a green question mark.
fn chip_prefix(chip: &ChipView) -> gtk::Widget {
    match chip.item {
        // No seed is in sight here, so rings keep the catalog's own cell for
        // their class rather than any run's gem.
        Some(item_id) => {
            let pinned = chip
                .effect
                .as_ref()
                .and_then(|effect| match effect.effects[..] {
                    [effect] => Some(effect),
                    _ => None,
                });
            sprites::item_image(
                sprites::ItemSprite::from_catalog(shpd_seedfinder_core::catalog::item(item_id)),
                glow::effect(pinned),
            )
        }
        None => sprites::wildcard_image(chip.family, chip.kind.weapon_category()),
    }
}

/// The requirements header, counting board entries: a cluster is one
/// requirement however many alternatives it lists, and a stack is one however
/// many items it asks for.
fn requirements_title(entries: usize) -> String {
    if entries == 0 {
        "Requirements".to_owned()
    } else {
        format!("Requirements ({entries})")
    }
}

/// The worker row's subtitle: how much of the machine the search will use.
fn worker_subtitle(workers: usize, ceiling: usize) -> String {
    format!("{workers} of {ceiling} cores")
}

/// A spin row's value as a count. Values come from an adjustment that is
/// already bounded to `[1, ceiling]`, so this only has to round.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn round_to_usize(value: f64) -> usize {
    value.round().max(0.0) as usize
}

/// A count as a spin-row value. Worker counts are small enough to convert
/// exactly; an implausibly huge one saturates rather than losing precision
/// silently.
#[allow(clippy::cast_precision_loss)]
fn usize_to_f64(value: usize) -> f64 {
    u32::try_from(value).map_or(f64::from(u32::MAX), f64::from)
}

#[cfg(test)]
mod tests {
    use super::{requirements_title, round_to_usize, usize_to_f64, worker_subtitle};

    #[test]
    fn requirements_header_counts_board_entries() {
        assert_eq!(requirements_title(0), "Requirements");
        assert_eq!(requirements_title(3), "Requirements (3)");
    }

    #[test]
    fn the_worker_row_says_how_much_of_the_machine_it_uses() {
        assert_eq!(worker_subtitle(1, 8), "1 of 8 cores");
        assert_eq!(worker_subtitle(8, 8), "8 of 8 cores");
    }

    #[test]
    fn worker_counts_survive_the_trip_through_the_spin_row() {
        for workers in [1_usize, 3, 8, 64] {
            assert_eq!(round_to_usize(usize_to_f64(workers)), workers);
        }
        assert_eq!(round_to_usize(2.6), 3);
        assert_eq!(round_to_usize(-1.0), 0);
    }
}
