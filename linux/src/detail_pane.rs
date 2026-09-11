// SPDX-License-Identifier: GPL-3.0-or-later

//! Seed pane: scout one seed and browse its item manifest by floor.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::rc::Rc;

use adw::prelude::*;
use shpd_seedfinder_core::auto_trinkets::SeedRecipe;
use shpd_seedfinder_core::catalog::{Effect, ItemId, ItemKind, item};
use shpd_seedfinder_core::challenges::Challenges;
use shpd_seedfinder_core::feasibility::QueryPlan;
use shpd_seedfinder_core::model::{Accessibility, GeneratedWorld, WorldItem};
use shpd_seedfinder_core::query::{SearchQuery, scout_matches};
use shpd_seedfinder_core::run::RingGems;
use shpd_seedfinder_core::search::FloorGate;
use shpd_seedfinder_core::seed::{DungeonSeed, format_input};
use shpd_seedfinder_core::trinkets::trinket_order;
use shpd_seedfinder_session::production_scout_world_selected;

use crate::sprites::ItemSprite;
use crate::state::{AppState, QuestRow, quest_rows, region, source_label};
use crate::{glow, sprites};

#[derive(Clone, Copy)]
enum TrinketOverride {
    Automatic,
    Manual(Option<ItemId>),
}

type NavigateHandler = Box<dyn Fn(i64)>;

pub struct DetailPane {
    pub page: adw::NavigationPage,
    title: adw::WindowTitle,
    entry: gtk::Entry,
    scout_button: gtk::Button,
    copy_button: gtk::Button,
    stack: gtk::Stack,
    summary_items: gtk::Label,
    summary_matches: gtk::Label,
    summary_quests: gtk::Label,
    manifest_box: gtk::Box,
    scroller: gtk::ScrolledWindow,
    dock: gtk::Fixed,
    shortcuts: gtk::Box,
    nav_hint: gtk::Label,
    nav_position: gtk::Label,
    offers: RefCell<Option<gtk::Box>>,
    sections: RefCell<Vec<(u8, gtk::Box)>>,
    saved_state: RefCell<Option<AppState>>,
    on_navigate: RefCell<Option<NavigateHandler>>,
    world: RefCell<Option<GeneratedWorld>>,
    selected_trinket: Cell<Option<ItemId>>,
    trinket_override: Cell<TrinketOverride>,
    world_challenges: Cell<Challenges>,
    updating: Cell<bool>,
    toasts: adw::ToastOverlay,
    on_scout: RefCell<Option<Box<dyn Fn()>>>,
}

impl DetailPane {
    #[allow(clippy::too_many_lines)] // Widget assembly is declarative and linear.
    pub fn new(toasts: &adw::ToastOverlay) -> Rc<Self> {
        let entry = gtk::Entry::builder()
            .placeholder_text("AAA-AAA-AAA")
            .css_classes(["seed-entry"])
            .input_hints(gtk::InputHints::UPPERCASE_CHARS)
            .max_length(11)
            .hexpand(true)
            .build();
        let scout_button = gtk::Button::builder()
            .label("Scout")
            .css_classes(["suggested-action"])
            .sensitive(false)
            .build();
        let copy_button = gtk::Button::builder()
            .icon_name("edit-copy-symbolic")
            .css_classes(["flat"])
            .tooltip_text("Copy Seed Code")
            .visible(false)
            .build();
        let entry_area = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(12)
            .margin_end(12)
            .build();
        entry_area.append(&entry);
        entry_area.append(&scout_button);
        entry_area.append(&copy_button);
        let entry_clamp = adw::Clamp::builder()
            .child(&entry_area)
            .maximum_size(500)
            .build();

        let placeholder = adw::StatusPage::builder()
            .icon_name("mark-location-symbolic")
            .title("No Seed Scouted")
            .description(
                "Enter a seed code, or select a search result, \
                 to inspect its item manifest.",
            )
            .build();

        let summary_items = gtk::Label::builder()
            .css_classes(["caption", "dim-label", "numeric"])
            .xalign(0.0)
            .build();
        let summary_matches = gtk::Label::builder()
            .css_classes(["caption", "dim-label"])
            .xalign(0.0)
            .build();
        let summary_area = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(4)
            .margin_start(12)
            .margin_end(12)
            .margin_top(9)
            .margin_bottom(3)
            .build();
        summary_area.append(&summary_items);
        summary_area.append(&summary_matches);

        // Every quest of the scouted seed on one line; each floor header
        // repeats its own quest, so this only has to name the givers.
        let summary_quests = gtk::Label::builder()
            .css_classes(["caption", "dim-label"])
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .margin_start(12)
            .margin_end(12)
            .margin_bottom(3)
            .xalign(0.0)
            .visible(false)
            .build();

        let manifest_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(24)
            .margin_top(12)
            .margin_bottom(24)
            .margin_start(12)
            .margin_end(12)
            .build();
        let manifest_clamp = adw::Clamp::builder()
            .child(&manifest_box)
            .maximum_size(600)
            .build();
        let manifest_scroller = gtk::ScrolledWindow::builder()
            .child(&manifest_clamp)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .build();
        let manifest_area = gtk::Box::new(gtk::Orientation::Vertical, 0);
        manifest_area.append(&summary_area);
        manifest_area.append(&summary_quests);
        manifest_area.append(&manifest_scroller);

        let nav = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .margin_start(12)
            .margin_end(12)
            .margin_top(6)
            .margin_bottom(6)
            .build();
        let previous = gtk::Button::builder()
            .icon_name("go-previous-symbolic")
            .tooltip_text("Previous result (K)")
            .css_classes(["flat"])
            .build();
        let next = gtk::Button::builder()
            .icon_name("go-next-symbolic")
            .tooltip_text("Next result (J)")
            .css_classes(["flat"])
            .build();
        let nav_position = gtk::Label::new(None);
        nav.append(&previous);
        nav.append(&nav_position);
        nav.append(&next);
        let spring = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spring.set_hexpand(true);
        nav.append(&spring);
        let nav_hint = gtk::Label::builder()
            .label("J / K to browse")
            .css_classes(["caption", "dim-label"])
            .halign(gtk::Align::End)
            .build();
        let dock = gtk::Fixed::builder()
            .width_request(140)
            .height_request(32)
            .overflow(gtk::Overflow::Hidden)
            .build();
        let shortcuts = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        shortcuts.set_opacity(0.0);
        shortcuts.set_sensitive(false);
        dock.put(&shortcuts, 0.0, 32.0);
        let nav_tools = gtk::Overlay::builder()
            .child(&nav_hint)
            .width_request(140)
            .height_request(32)
            .build();
        nav_tools.add_overlay(&dock);
        nav.append(&nav_tools);
        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .build();
        stack.add_named(&placeholder, Some("empty"));
        stack.add_named(&manifest_area, Some("manifest"));

        let title = adw::WindowTitle::new("Seed", "");
        let header_bar = adw::HeaderBar::builder().title_widget(&title).build();
        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header_bar);
        toolbar_view.add_top_bar(&entry_clamp);
        toolbar_view.add_top_bar(&nav);
        toolbar_view.set_content(Some(&stack));

        let nav_page = adw::NavigationPage::builder()
            .title("Seed")
            .tag("seed")
            .child(&toolbar_view)
            .build();

        let pane = Rc::new(Self {
            page: nav_page,
            title,
            entry,
            scout_button,
            copy_button,
            stack,
            summary_items,
            summary_matches,
            summary_quests,
            manifest_box,
            scroller: manifest_scroller,
            dock,
            shortcuts,
            nav_hint,
            nav_position,
            offers: RefCell::new(None),
            sections: RefCell::new(Vec::new()),
            saved_state: RefCell::new(None),
            on_navigate: RefCell::new(None),
            world: RefCell::new(None),
            selected_trinket: Cell::new(None),
            trinket_override: Cell::new(TrinketOverride::Automatic),
            world_challenges: Cell::new(Challenges::NONE),
            updating: Cell::new(false),
            toasts: toasts.clone(),
            on_scout: RefCell::new(None),
        });

        for (button, delta) in [(previous, -1), (next, 1)] {
            let weak = Rc::downgrade(&pane);
            button.connect_clicked(move |_| {
                if let Some(pane) = weak.upgrade()
                    && let Some(navigate) = pane.on_navigate.borrow().as_ref()
                {
                    navigate(delta);
                }
            });
        }
        pane.scroller.vadjustment().connect_value_changed({
            let weak = Rc::downgrade(&pane);
            move |_| {
                if let Some(pane) = weak.upgrade() {
                    pane.update_dock();
                }
            }
        });
        pane.entry.connect_changed({
            let pane = Rc::clone(&pane);
            move |entry| {
                if pane.updating.get() {
                    return;
                }
                pane.updating.set(true);
                let formatted = format_input(&entry.text());
                if formatted != entry.text() {
                    entry.set_text(&formatted);
                    entry.set_position(-1);
                }
                pane.scout_button
                    .set_sensitive(DungeonSeed::from_code(&formatted).is_ok());
                pane.updating.set(false);
            }
        });
        pane.entry.connect_activate({
            let pane = Rc::clone(&pane);
            move |_| pane.request_scout()
        });
        pane.scout_button.connect_clicked({
            let pane = Rc::clone(&pane);
            move |_| pane.request_scout()
        });
        pane.copy_button.connect_clicked({
            let pane = Rc::clone(&pane);
            move |button| {
                if let Some(world) = pane.world.borrow().as_ref() {
                    let code = world.seed.to_code();
                    button.clipboard().set_text(&code);
                    pane.toasts
                        .add_toast(adw::Toast::new(&format!("Copied {code}")));
                }
            }
        });
        pane
    }

    pub fn connect_navigate(&self, handler: impl Fn(i64) + 'static) {
        self.on_navigate.replace(Some(Box::new(handler)));
    }

    fn update_dock(&self) {
        let progress = self
            .offers
            .borrow()
            .as_ref()
            .and_then(|offers| offers.compute_bounds(&self.scroller.child()?))
            .filter(|bounds| bounds.height() > 0.0)
            .map_or(0.0, |bounds| {
                // value-changed precedes allocation of the viewport's new
                // transform, so use content coordinates and the live offset.
                ((self.scroller.vadjustment().value() - f64::from(bounds.y()))
                    / f64::from(bounds.height()))
                .clamp(0.0, 1.0)
            });
        self.shortcuts.set_opacity(progress);
        self.shortcuts.set_sensitive(progress > 0.0);
        self.shortcuts.set_can_target(progress > 0.0);
        self.nav_hint.set_opacity(1.0 - progress);
        self.dock
            .move_(&self.shortcuts, 0.0, (1.0 - progress) * 32.0);
    }

    pub fn scout_recipe(self: &Rc<Self>, recipe: SeedRecipe, state: &AppState) {
        self.saved_state.replace(Some(state.clone()));
        self.trinket_override
            .set(TrinketOverride::Manual(recipe.trinket));
        self.scout_with_override(Some(&recipe.seed.to_code()), state);
    }

    /// Runs when the user asks to scout the entered seed; the window owns the
    /// query state and calls back into [`Self::scout`].
    pub fn connect_scout(&self, handler: impl Fn() + 'static) {
        self.on_scout.replace(Some(Box::new(handler)));
    }

    fn request_scout(&self) {
        if let Some(handler) = self.on_scout.borrow().as_ref() {
            handler();
        }
    }

    pub fn focus_entry(&self) {
        self.entry.grab_focus();
    }

    /// The canonical code of the currently scouted seed, if any.
    pub fn entered_seed(&self) -> Option<String> {
        DungeonSeed::from_code(self.entry.text().as_str())
            .ok()
            .map(DungeonSeed::to_code)
    }

    pub fn current_seed(&self) -> Option<String> {
        self.world
            .borrow()
            .as_ref()
            .map(|world| world.seed.to_code())
    }

    /// Shows where the scouted seed sits in the search results (0-based
    /// index and total), or clears the indicator when it is not one of them.
    pub fn set_result_position(&self, position: Option<(usize, usize)>) {
        self.nav_position.set_label(&position.map_or_else(
            || "Trinkets".into(),
            |(index, total)| format!("{} of {total}", index + 1),
        ));
        self.nav_hint.set_visible(position.is_some());
        match position {
            Some((index, total)) => self
                .title
                .set_subtitle(&format!("Result {} of {total}", index + 1)),
            None => self.title.set_subtitle(""),
        }
    }

    /// Scouts the seed in the entry, or `code` when given (also filling the
    /// entry), and renders its manifest against the current requirements.
    pub fn scout(self: &Rc<Self>, code: Option<&str>, state: &AppState) {
        self.saved_state.replace(None);
        self.trinket_override.set(TrinketOverride::Automatic);
        self.scout_with_override(code, state);
    }

    fn scout_with_override(self: &Rc<Self>, code: Option<&str>, state: &AppState) {
        if let Some(code) = code {
            self.updating.set(true);
            self.entry.set_text(&format_input(code));
            self.scout_button.set_sensitive(true);
            self.updating.set(false);
        }
        let text = self.entry.text();
        let Ok(seed) = DungeonSeed::from_code(text.trim()) else {
            self.toasts
                .add_toast(adw::Toast::new("Seed codes use the AAA-AAA-AAA format"));
            return;
        };
        let query = manifest_query(state);
        let Ok((world, selected)) = production_scout_world_selected(
            seed,
            state.challenges,
            Some(&query),
            match self.trinket_override.get() {
                TrinketOverride::Automatic => None,
                TrinketOverride::Manual(selected) => Some(selected),
            },
        ) else {
            self.toasts.add_toast(adw::Toast::new(
                "World generation failed for this seed; please report it",
            ));
            return;
        };
        self.world.replace(Some(world));
        self.selected_trinket.set(selected);
        self.world_challenges.set(state.challenges);
        self.render(state);
    }

    /// Regenerates the world when the effective trinket or challenges changed.
    fn refresh_world(self: &Rc<Self>, state: &AppState) -> bool {
        let seed = self.world.borrow().as_ref().map(|world| world.seed);
        if let Some(seed) = seed {
            let selected = match self.trinket_override.get() {
                TrinketOverride::Automatic => {
                    QueryPlan::analyze(&manifest_query(state)).selected_trinket(seed)
                }
                TrinketOverride::Manual(selected) => selected,
            };
            if selected != self.selected_trinket.get()
                || state.challenges != self.world_challenges.get()
            {
                self.scout_with_override(Some(&seed.to_code()), state);
                return true;
            }
        }
        false
    }

    /// Re-renders the manifest, e.g. after the requirements changed.
    #[allow(clippy::too_many_lines)] // Native floor and item widget assembly.
    pub fn render(self: &Rc<Self>, state: &AppState) {
        let saved_state = self.saved_state.borrow().clone();
        let state = saved_state.as_ref().unwrap_or(state);
        if self.refresh_world(state) {
            return;
        }
        let world = self.world.borrow();
        let Some(world) = world.as_ref() else {
            self.stack.set_visible_child_name("empty");
            self.copy_button.set_visible(false);
            return;
        };
        self.stack.set_visible_child_name("manifest");
        self.copy_button.set_visible(true);

        // Which gem each ring class wears is shuffled once per run, so the
        // manifest has to draw this run's table rather than the catalog's.
        // Scouting rolls it with the rest of the run and hangs it on the world,
        // so the world being rendered already carries the answer.
        let gems = world.ring_gems;
        let marks = scout_matches(world, &manifest_query(state));
        let matched = marks.matched_requirements;
        let total = marks.total_requirements;
        let mut by_depth: BTreeMap<u8, Vec<usize>> = BTreeMap::new();
        for (index, world_item) in world.items.iter().enumerate() {
            by_depth.entry(world_item.depth).or_default().push(index);
        }

        self.summary_items.set_label(&format!(
            "{} items across {} floors",
            world.items.len(),
            by_depth.len()
        ));
        if state.requirements.is_empty() {
            self.summary_matches.set_label("");
        } else {
            self.summary_matches
                .set_label(&match_summary(matched, total));
        }
        if matched == 0 {
            self.summary_matches.remove_css_class("success");
            self.summary_matches.add_css_class("dim-label");
        } else {
            self.summary_matches.remove_css_class("dim-label");
            self.summary_matches.add_css_class("success");
        }

        let anchor = self.sections.borrow().iter().find_map(|(depth, section)| {
            let bounds = section.compute_bounds(&self.scroller)?;
            (bounds.y() + bounds.height() > 0.0).then_some((*depth, bounds.y()))
        });
        self.sections.borrow_mut().clear();
        self.offers.replace(None);
        while let Some(child) = self.shortcuts.first_child() {
            self.shortcuts.remove(&child);
        }
        for id in &trinket_order(world.seed)[..4] {
            let id = *id;
            let button = gtk::ToggleButton::builder()
                .active(self.selected_trinket.get() == Some(id))
                .child(&sprites::item_image_sized(
                    ItemSprite::from_catalog(item(id)),
                    None,
                    20,
                ))
                .tooltip_text(item(id).name)
                .css_classes(["flat", "trinket-shortcut"])
                .build();
            button.update_property(&[gtk::accessible::Property::Label(item(id).name)]);
            let weak = Rc::downgrade(self);
            let state = state.clone();
            let code = world.seed.to_code();
            button.connect_clicked(move |_| {
                if let Some(pane) = weak.upgrade() {
                    pane.trinket_override.set(TrinketOverride::Manual(
                        (pane.selected_trinket.get() != Some(id)).then_some(id),
                    ));
                    pane.scout_with_override(Some(&code), &state);
                }
            });
            self.shortcuts.append(&button);
        }
        while let Some(child) = self.manifest_box.first_child() {
            self.manifest_box.remove(&child);
        }
        let quests = quest_rows(world.quests);
        self.summary_quests.set_visible(!quests.is_empty());
        self.summary_quests.set_label(&quest_summary_line(&quests));
        for (depth, indices) in &by_depth {
            let mut description = region(*depth).to_owned();
            if let Some(quest) = quests.iter().find(|quest| quest.depth == *depth) {
                let _ = write!(description, " · {}", quest.variant);
            }
            let section = gtk::Box::new(gtk::Orientation::Vertical, 8);
            let heading = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            heading.append(
                &gtk::Label::builder()
                    .label(format!("Floor {depth}"))
                    .css_classes(["heading"])
                    .accessible_role(gtk::AccessibleRole::Heading)
                    .build(),
            );
            if let Some(floor) = world.feelings.iter().find(|floor| floor.depth == *depth)
                && let Some(icon) = sprites::feeling_image(floor.feeling)
            {
                heading.append(&icon);
            }
            section.append(&heading);
            let group = adw::PreferencesGroup::builder()
                .description(description)
                .build();
            let mut catalyst_shown = false;
            for index in indices {
                if item(world.items[*index].item).kind == ItemKind::Trinket {
                    if !catalyst_shown {
                        let (catalyst, offers) = trinket_choices(
                            world,
                            &world.items[*index],
                            &marks.matched,
                            self.selected_trinket.get(),
                            {
                                let pane = Rc::downgrade(self);
                                let state = state.clone();
                                let code = world.seed.to_code();
                                move |id| {
                                    if let Some(pane) = pane.upgrade() {
                                        let selected =
                                            (pane.selected_trinket.get() != Some(id)).then_some(id);
                                        pane.trinket_override
                                            .set(TrinketOverride::Manual(selected));
                                        pane.scout_with_override(Some(&code), &state);
                                    }
                                }
                            },
                        );
                        self.offers.replace(Some(offers));
                        group.add(&catalyst);
                        catalyst_shown = true;
                    }
                } else {
                    group.add(&item_row(&world.items[*index], gems, marks.matched[*index]));
                }
            }
            section.append(&group);
            self.manifest_box.append(&section);
            self.sections.borrow_mut().push((*depth, section));
        }
        let weak = Rc::downgrade(self);
        self.manifest_box.add_tick_callback(move |_, _| {
            let Some(pane) = weak.upgrade() else {
                return gtk::glib::ControlFlow::Break;
            };
            if pane
                .sections
                .borrow()
                .iter()
                .any(|(_, section)| section.height() == 0)
            {
                return gtk::glib::ControlFlow::Continue;
            }
            if let Some((depth, offset)) = anchor
                && let Some((_, section)) = pane.sections.borrow().iter().find(|(d, _)| *d == depth)
                && let Some(bounds) = section.compute_bounds(&pane.scroller)
            {
                let adjustment = pane.scroller.vadjustment();
                let offset = offset.max(-bounds.height() + 1.0);
                adjustment.set_value(adjustment.value() + f64::from(bounds.y() - offset));
            }
            pane.update_dock();
            gtk::glib::ControlFlow::Break
        });
    }
}

/// The editor's requirements as an engine query for
/// [`scout_matches`], which reads only the requirements, the floor limit and
/// the blacksmith-reward exclusion. Unlike [`AppState::to_query`] this never
/// rejects the state: a manifest is rendered while the query is still empty
/// or half-edited.
fn manifest_query(state: &AppState) -> SearchQuery {
    SearchQuery {
        require_blacksmith: false,
        wandmaker_quest: None,
        ..state.unvalidated_query()
    }
}

/// The header's match count, in slots: an "any of these" group is one
/// requirement however many alternatives it lists.
fn match_summary(matched: usize, total: usize) -> String {
    format!(
        "· {matched} of {total} requirement{} matched",
        if total == 1 { "" } else { "s" }
    )
}

/// The whole quest schedule on one line, e.g. "Sad ghost: Great crab ·
/// Wandmaker: Rotberry". The floors are left to the floor headers, which
/// already repeat each quest's variant.
fn quest_summary_line(quests: &[QuestRow]) -> String {
    let mut line = String::new();
    for quest in quests {
        if !line.is_empty() {
            line.push_str(" · ");
        }
        let _ = write!(line, "{}: {}", quest.giver, quest.variant);
    }
    line
}

/// The catalyst keeps the source and accessibility of its generated location.
fn trinket_choices(
    world: &GeneratedWorld,
    location: &WorldItem,
    matched: &[bool],
    selected: Option<ItemId>,
    on_select: impl Fn(ItemId) + 'static,
) -> (gtk::Box, gtk::Box) {
    let on_select = Rc::new(on_select);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let mut catalyst = location.clone();
    catalyst.item = ItemId::TrinketCatalyst;
    content.append(&item_row(&catalyst, world.ring_gems, false));
    let choices = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .homogeneous(true)
        .spacing(6)
        .margin_start(12)
        .margin_end(12)
        .build();
    let order = trinket_order(world.seed);
    for id in &order[..4] {
        let is_match = world
            .items
            .iter()
            .enumerate()
            .any(|(index, entry)| entry.item == *id && matched[index]);
        let tile = sprites::trinket_tile(item(*id), is_match, true);
        let applied = selected == Some(*id);
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&tile));
        if applied {
            overlay.add_overlay(
                &gtk::Label::builder()
                    .label("Applied +3")
                    .css_classes(["trinket-applied-badge"])
                    .halign(gtk::Align::Center)
                    .valign(gtk::Align::Start)
                    .build(),
            );
        }
        let button = gtk::ToggleButton::builder()
            .child(&overlay)
            .active(applied)
            .css_classes(["flat", "trinket-toggle"])
            .tooltip_text(item(*id).name)
            .build();
        button.update_property(&[gtk::accessible::Property::Label(item(*id).name)]);
        button.connect_clicked({
            let id = *id;
            let on_select = Rc::clone(&on_select);
            move |_| on_select(id)
        });
        choices.append(&button);
    }
    content.append(&choices);
    content.append(
        &gtk::Label::builder()
            .label("Remaining deck order")
            .css_classes(["caption", "dim-label"])
            .xalign(0.0)
            .margin_start(12)
            .margin_top(4)
            .build(),
    );
    let remaining = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .homogeneous(true)
        .spacing(2)
        .margin_start(12)
        .margin_end(12)
        .margin_bottom(12)
        .build();
    for id in &order[4..] {
        remaining.append(&sprites::trinket_tile(item(*id), false, false));
    }
    content.append(&remaining);
    (content, choices)
}

fn item_row(world_item: &WorldItem, gems: RingGems, matched: bool) -> adw::ActionRow {
    let mut subtitle = source_label(world_item.source).to_owned();
    match world_item.accessibility {
        Accessibility::Independent => {}
        Accessibility::Choice { group, option } => {
            let _ = write!(
                subtitle,
                "\nOne reward of choice group {group} (option {})",
                option + 1
            );
        }
        Accessibility::Scenarios { group, .. } => {
            let _ = write!(
                subtitle,
                "\nOnly in some outcomes of scenario group {group}"
            );
        }
    }

    let definition = item(world_item.item);
    let row = adw::ActionRow::builder()
        .title(gtk::glib::markup_escape_text(definition.name))
        .subtitle(gtk::glib::markup_escape_text(&subtitle))
        .build();
    row.add_prefix(&sprites::item_image(
        ItemSprite::in_run(definition, gems),
        glow::item(world_item.cursed, world_item.effect),
    ));

    if world_item.displayed_upgrade() > 0 {
        let upgrade = gtk::Label::builder()
            .label(format!("+{}", world_item.displayed_upgrade()))
            .css_classes(["caption-heading", "success"])
            .valign(gtk::Align::Center)
            .build();
        row.add_suffix(&upgrade);
    }
    if let Some(effect) = world_item.effect {
        let cursed_effect = match effect {
            Effect::Weapon(weapon_effect) => weapon_effect.is_curse(),
            Effect::Armor(armor_effect) => armor_effect.is_curse(),
        };
        row.add_suffix(&tag(
            effect.wire_name(),
            if cursed_effect { "error" } else { "accent" },
        ));
    }
    if world_item.cursed {
        row.add_suffix(&tag("Cursed", "error"));
    }
    if world_item.secret {
        let badge = tag("Secret", "warning");
        badge.set_tooltip_text(Some("Hidden in a secret room — search to reveal it"));
        row.add_suffix(&badge);
    }
    if matched {
        let badge = tag("Match", "success");
        badge.set_tooltip_text(Some(
            "Selected as part of a jointly obtainable requirement match",
        ));
        row.add_suffix(&badge);
    }
    row
}

fn tag(label: &str, color: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(label)
        .css_classes(["tag", color])
        .valign(gtk::Align::Center)
        .build()
}

#[cfg(test)]
mod tests {
    /// Run under Xvfb with --ignored; normal unit tests need no display.
    #[test]
    #[ignore = "requires a GTK display"]
    fn scout_dock_tracks_scroll_and_trinket_switches() {
        use super::{AppState, DetailPane};
        use adw::prelude::*;
        adw::init().unwrap();
        gtk::gio::resources_register_include!("dev.seedseeker.SeedSeeker.gresource").unwrap();
        crate::load_stylesheet();
        let toasts = adw::ToastOverlay::new();
        let pane = DetailPane::new(&toasts);
        let navigation = adw::NavigationView::new();
        navigation.add(&pane.page);
        toasts.set_child(Some(&navigation));
        let window = gtk::Window::builder()
            .default_width(500)
            .default_height(650)
            .child(&toasts)
            .build();
        window.present();
        let query =
            shpd_seedfinder_core::wire::decode_query(br#"{"requirements":[{"item":"whip"}]}"#)
                .unwrap();
        pane.scout(Some("AAA-AAA-AAA"), &AppState::from_query(&query));
        let settle = || {
            let context = gtk::glib::MainContext::default();
            for _ in 0..30 {
                while context.pending() {
                    context.iteration(false);
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        };
        settle();
        assert!(pane.shortcuts.opacity() < f64::EPSILON);
        let bounds = pane
            .offers
            .borrow()
            .as_ref()
            .unwrap()
            .compute_bounds(&pane.scroller)
            .unwrap();
        let adjustment = pane.scroller.vadjustment();
        adjustment.set_value(f64::from(bounds.y() + bounds.height() * 0.5));
        settle();
        assert!(
            (pane.shortcuts.opacity() - 0.5).abs() < 0.05,
            "progress={}, scroll={}, bounds={:?}",
            pane.shortcuts.opacity(),
            adjustment.value(),
            pane.offers
                .borrow()
                .as_ref()
                .unwrap()
                .compute_bounds(&pane.scroller)
        );
        assert!((pane.nav_hint.opacity() - 0.5).abs() < 0.05);
        adjustment.set_value(adjustment.upper() * 0.65);
        settle();
        assert!((pane.shortcuts.opacity() - 1.0).abs() < f64::EPSILON);
        let anchor = || {
            pane.sections
                .borrow()
                .iter()
                .find_map(|(depth, section)| {
                    section
                        .compute_bounds(&pane.scroller)
                        .filter(|b| b.y() + b.height() > 0.0)
                        .map(|b| (*depth, b.y()))
                })
                .unwrap()
        };
        let before = anchor();
        let choice = |index: usize| {
            let mut child = pane.shortcuts.first_child().unwrap();
            for _ in 0..index {
                child = child.next_sibling().unwrap();
            }
            child.downcast::<gtk::ToggleButton>().unwrap()
        };
        // Mimic Tooth changes the item layout; the neutral first offer would
        // not exercise restoration after floors change height.
        choice(1).emit_clicked();
        settle();
        assert_eq!(
            pane.selected_trinket.get(),
            Some(shpd_seedfinder_core::catalog::ItemId::MimicTooth)
        );
        let after = anchor();
        assert_eq!(before.0, after.0);
        assert!((before.1 - after.1).abs() < 2.0);
        choice(2).emit_clicked();
        settle();
        assert_eq!(
            pane.selected_trinket.get(),
            Some(shpd_seedfinder_core::catalog::ItemId::ParchmentScrap)
        );
        assert!(!choice(1).is_active());
        assert!(choice(2).is_active());
        choice(2).emit_clicked();
        settle();
        assert!(pane.selected_trinket.get().is_none());
        assert!((0..4).all(|index| !choice(index).is_active()));
        window.close();
    }

    use super::{QuestRow, match_summary, quest_summary_line};

    #[test]
    fn match_summary_counts_slots() {
        assert_eq!(match_summary(0, 1), "· 0 of 1 requirement matched");
        assert_eq!(match_summary(2, 3), "· 2 of 3 requirements matched");
    }

    #[test]
    fn quest_summary_names_every_giver_on_one_line() {
        assert_eq!(quest_summary_line(&[]), "");
        assert_eq!(
            quest_summary_line(&[
                QuestRow {
                    giver: "Sad ghost",
                    variant: "Great crab",
                    depth: 4,
                },
                QuestRow {
                    giver: "Blacksmith",
                    variant: "Crystal spire",
                    depth: 13,
                },
            ]),
            "Sad ghost: Great crab · Blacksmith: Crystal spire"
        );
    }
}
