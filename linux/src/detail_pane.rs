// SPDX-License-Identifier: GPL-3.0-or-later

//! Seed pane: scout one seed and browse its item manifest by floor.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
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

use crate::level_map_view::{FloorMapView, MapProfile};
use crate::sprites::ItemSprite;
use crate::state::{AppState, quest_rows, region, source_label};
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
    info_button: gtk::Button,
    stack: gtk::Stack,
    summary_items: gtk::Label,
    summary_matches: gtk::Label,
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
    open_map: RefCell<Option<(u8, Rc<FloorMapView>)>>,
    updating: Cell<bool>,
    toasts: adw::ToastOverlay,
    on_scout: RefCell<Option<Box<dyn Fn()>>>,
}

impl DetailPane {
    #[allow(clippy::too_many_lines)] // Widget assembly is declarative and linear.
    pub fn new(toasts: &adw::ToastOverlay) -> Rc<Self> {
        let entry = gtk::Entry::builder()
            .placeholder_text("Seed or YYYY-MM-DD")
            .css_classes(["seed-entry"])
            .input_hints(gtk::InputHints::UPPERCASE_CHARS)
            .max_length(11)
            .width_chars(11)
            .height_request(40)
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
        let info_button = gtk::Button::builder()
            .icon_name("dialog-information-symbolic")
            .css_classes(["flat"])
            .tooltip_text("Seed information")
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
        let calendar = gtk::Calendar::new();
        let today = glib::DateTime::now_utc().expect("UTC clock available");
        calendar.set_date(&today);
        let daily_popover = gtk::Popover::new();
        let daily_content = gtk::Box::new(gtk::Orientation::Vertical, 6);
        daily_content.append(&calendar);
        let use_date = gtk::Button::with_label("Use date");
        daily_content.append(&use_date);
        daily_popover.set_child(Some(&daily_content));
        let daily_picker = gtk::MenuButton::builder()
            .icon_name("x-office-calendar-symbolic")
            .popover(&daily_popover)
            .tooltip_text("Choose a daily run date (UTC), then Scout")
            .build();
        daily_picker.update_property(&[gtk::accessible::Property::Label("Choose daily run date")]);
        let daily_today = gtk::Button::with_label("Today");
        daily_today.set_tooltip_text(Some("Scout today's daily run (UTC)"));
        entry_area.append(&entry);
        entry_area.append(&daily_picker);
        entry_area.append(&daily_today);
        entry_area.append(&scout_button);
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
        header_bar.pack_end(&copy_button);
        header_bar.pack_end(&info_button);
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
            info_button,
            stack,
            summary_items,
            summary_matches,
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
            open_map: RefCell::new(None),
            updating: Cell::new(false),
            toasts: toasts.clone(),
            on_scout: RefCell::new(None),
        });

        use_date.connect_clicked({
            let pane = Rc::clone(&pane);
            move |_| {
                let date = calendar.date();
                let text = format!(
                    "{:04}-{:02}-{:02}",
                    date.year(),
                    date.month(),
                    date.day_of_month()
                );
                if DungeonSeed::from_daily_date(&text).is_ok() {
                    pane.entry.set_text(&text);
                    daily_popover.popdown();
                } else {
                    pane.toasts
                        .add_toast(adw::Toast::new("Choose a date between 1970 and 9999"));
                }
            }
        });
        daily_today.connect_clicked({
            let pane = Rc::clone(&pane);
            move |_| {
                if let Ok(today) = glib::DateTime::now_utc() {
                    pane.entry.set_text(&format!(
                        "{:04}-{:02}-{:02}",
                        today.year(),
                        today.month(),
                        today.day_of_month()
                    ));
                    pane.request_scout();
                }
            }
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
                    .set_sensitive(DungeonSeed::from_scout_input(&formatted).is_ok());
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
        pane.info_button.connect_clicked({
            let weak = Rc::downgrade(&pane);
            move |button| {
                if let Some(pane) = weak.upgrade()
                    && let Some(world) = pane.world.borrow().as_ref()
                {
                    crate::item_mappings::present(button, world.seed);
                }
            }
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
        DungeonSeed::from_scout_input(self.entry.text().as_str())
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
        let Ok(seed) = DungeonSeed::from_scout_input(text.trim()) else {
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
            self.info_button.set_visible(false);
            return;
        };
        self.stack.set_visible_child_name("manifest");
        self.copy_button.set_visible(true);
        self.info_button.set_visible(true);

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
        for floor in &world.feelings {
            by_depth.entry(floor.depth).or_default();
        }
        let last_depth = by_depth.keys().next_back().copied().unwrap_or(0);
        for depth in shpd_seedfinder_core::level_map::SUPPORTED_DEPTHS {
            if depth <= last_depth {
                by_depth.entry(depth).or_default();
            }
        }
        let floors = by_depth
            .keys()
            .copied()
            .filter(|d| shpd_seedfinder_core::level_map::SUPPORTED_DEPTHS.contains(d))
            .collect::<Vec<_>>();
        let profile = MapProfile {
            seed: world.seed,
            challenges: self.world_challenges.get(),
            trinket: self.selected_trinket.get(),
        };
        let discard_map = self.open_map.borrow().as_ref().is_some_and(|(_, view)| {
            view.profile().seed != profile.seed || view.profile().challenges != profile.challenges
        });
        if discard_map && let Some((_, view)) = self.open_map.borrow_mut().take() {
            view.close();
        }
        if let Some((_, view)) = self.open_map.borrow().as_ref() {
            view.update_profile(profile);
            let weak = Rc::downgrade(self);
            let trinket_state = state.clone();
            view.set_trinket_handler(move |id| {
                if let Some(pane) = weak.upgrade() {
                    pane.trinket_override.set(TrinketOverride::Manual(
                        (pane.selected_trinket.get() != Some(id)).then_some(id),
                    ));
                    pane.scout_with_override(Some(&profile.seed.to_code()), &trinket_state);
                }
            });
            let weak = Rc::downgrade(self);
            let state = state.clone();
            view.set_close_handler(move || {
                if let Some(pane) = weak.upgrade() {
                    let previous = pane.open_map.borrow_mut().take();
                    pane.render(&state);
                    if let Some((depth, _)) = previous {
                        pane.focus_floor(depth);
                    }
                }
            });
            if let Some(parent) = view.widget.parent().and_downcast::<gtk::Box>() {
                parent.remove(&view.widget);
            }
        }
        let matched_choices = world
            .items
            .iter()
            .zip(&marks.matched)
            .filter_map(|(item, matched)| {
                if *matched && let Accessibility::Choice { group, option } = item.accessibility {
                    Some((group, option))
                } else {
                    None
                }
            })
            .collect::<BTreeMap<_, _>>();

        self.summary_items.set_label(&format!(
            "{} items across {} floors",
            world.items.len(),
            by_depth.len()
        ));
        if state.requirements.is_empty() && !state.needs_resin() {
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
        if !world.artifact_decks.is_empty() {
            self.manifest_box.append(&artifact_deck_view(
                world,
                &marks,
                manifest_query(state).max_depth,
            ));
        }
        let quests = quest_rows(world.quests);
        for (depth, indices) in &by_depth {
            let section = gtk::Box::new(gtk::Orientation::Vertical, 8);
            let heading = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let labels = adw::WrapBox::builder()
                .child_spacing(6)
                .line_spacing(4)
                .hexpand(true)
                .build();
            let identity = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            identity.append(
                &gtk::Label::builder()
                    .label(format!("Floor {depth}"))
                    .css_classes(["heading"])
                    .accessible_role(gtk::AccessibleRole::Heading)
                    .build(),
            );
            if let Some(floor) = world.feelings.iter().find(|floor| floor.depth == *depth)
                && let Some(icon) = sprites::feeling_image(floor.feeling)
            {
                identity.append(&icon);
            }
            labels.append(&identity);
            labels.append(
                &gtk::Label::builder()
                    .label(region(*depth))
                    .css_classes(["dim-label"])
                    .build(),
            );
            if let Some(quest) = quests.iter().find(|quest| quest.depth == *depth) {
                labels.append(
                    &gtk::Label::builder()
                        .label(format!("· {}", quest.variant))
                        .css_classes(["dim-label"])
                        .build(),
                );
            }
            if shpd_seedfinder_core::floor_filters::is_farming_floor(world, *depth) {
                labels.append(
                    &gtk::Label::builder()
                        .label("· Garden")
                        .css_classes(["success"])
                        .tooltip_text("Dark floor with a garden.")
                        .build(),
                );
            }
            heading.append(&labels);
            if floors.contains(depth) {
                let map_button = gtk::ToggleButton::builder()
                    .child(&heading)
                    .active(
                        self.open_map
                            .borrow()
                            .as_ref()
                            .is_some_and(|(d, _)| d == depth),
                    )
                    .css_classes(["flat"])
                    .tooltip_text(format!("Show or hide floor {depth} map"))
                    .build();
                heading.set_hexpand(true);
                let map_label = gtk::Label::builder()
                    .label("Map")
                    .valign(gtk::Align::Center)
                    .halign(gtk::Align::End)
                    .css_classes(["caption", "dim-label"])
                    .build();
                heading.append(&map_label);
                section.append(&map_button);
                let map_slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
                if let Some((d, view)) = self.open_map.borrow().as_ref()
                    && d == depth
                {
                    map_slot.append(&view.widget);
                }
                section.append(&map_slot);
                let weak = Rc::downgrade(self);
                let state = state.clone();
                let depth = *depth;
                let floors = floors.clone();
                map_button.connect_clicked(move |_| {
                    let Some(pane) = weak.upgrade() else {
                        return;
                    };
                    let previous = pane.open_map.borrow_mut().take();
                    let closing = previous.as_ref().is_some_and(|(d, _)| *d == depth);
                    if let Some((_, view)) = previous {
                        view.close();
                    }
                    if !closing {
                        let weak = Rc::downgrade(&pane);
                        let state = state.clone();
                        let profile = MapProfile {
                            seed: profile.seed,
                            challenges: profile.challenges,
                            trinket: pane.selected_trinket.get(),
                        };
                        let view = FloorMapView::new(profile, depth, floors.clone(), move |id| {
                            if let Some(pane) = weak.upgrade() {
                                pane.trinket_override.set(TrinketOverride::Manual(
                                    (pane.selected_trinket.get() != Some(id)).then_some(id),
                                ));
                                pane.scout_with_override(Some(&profile.seed.to_code()), &state);
                            }
                        });
                        pane.open_map.replace(Some((depth, view)));
                    }
                    pane.render(&state);
                    if closing {
                        pane.focus_floor(depth);
                    } else if let Some((_, view)) = pane.open_map.borrow().as_ref() {
                        view.focus();
                    }
                });
            } else {
                section.append(&heading);
            }
            let group = adw::PreferencesGroup::new();
            if indices.is_empty() {
                group.add(
                    &gtk::Label::builder()
                        .label("No notable items on this floor.")
                        .css_classes(["caption", "dim-label"])
                        .xalign(0.0)
                        .build(),
                );
            }
            let mut catalyst_shown = false;
            for index in indices {
                if item(world.items[*index].item).kind == ItemKind::Trinket {
                    if !catalyst_shown {
                        let (catalyst, offers) = trinket_choices(
                            world,
                            &world.items[*index],
                            &marks.matched,
                            &marks.transmuted_trinkets,
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
                    let world_item = &world.items[*index];
                    let row = item_row(world_item, gems, marks.matched[*index]);
                    if choice_is_dimmed(
                        world_item.accessibility,
                        marks.matched[*index],
                        &matched_choices,
                    ) {
                        row.set_opacity(0.45);
                    }
                    group.add(&row);
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

    fn focus_floor(&self, depth: u8) {
        if let Some((_, section)) = self.sections.borrow().iter().find(|(d, _)| *d == depth)
            && let Some(heading) = section.first_child()
        {
            heading.grab_focus();
        }
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

/// The catalyst keeps the source and accessibility of its generated location.
fn trinket_choices(
    world: &GeneratedWorld,
    location: &WorldItem,
    matched: &[bool],
    transmuted: &[bool; 13],
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
            .label("Transmutation order · 1–13")
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
    for (index, id) in order[4..].iter().enumerate() {
        let tile = sprites::trinket_tile(item(*id), transmuted[index], false);
        let label = format!(
            "Transmutation #{}: {}{}",
            index + 1,
            item(*id).name,
            if transmuted[index] {
                ", matches requirement"
            } else {
                ""
            }
        );
        tile.set_tooltip_text(Some(&label));
        tile.update_property(&[gtk::accessible::Property::Label(&label)]);
        remaining.append(&tile);
    }
    content.append(&remaining);
    (content, choices)
}

fn item_row(world_item: &WorldItem, gems: RingGems, matched: bool) -> adw::ActionRow {
    let mut subtitle = source_label(world_item.source).to_owned();
    match world_item.accessibility {
        Accessibility::Independent | Accessibility::Choice { .. } => {}
        Accessibility::Scenarios { group, .. } => {
            let _ = write!(
                subtitle,
                "\nOnly in some outcomes of scenario group {}",
                choice_letter(group)
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
        glow::item(definition.kind, world_item.cursed, world_item.effect),
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
    if let Accessibility::Choice { group, option } = world_item.accessibility {
        let badge = tag(&format!("⑂ {}", choice_letter(group)), "dim-label");
        let description = format!(
            "One reward of choice group {} (option {})",
            choice_letter(group),
            option + 1
        );
        badge.set_tooltip_text(Some(&description));
        badge.update_property(&[gtk::accessible::Property::Label(&description)]);
        row.add_suffix(&badge);
    }
    row
}

fn choice_letter(group: u16) -> char {
    char::from(b'A' + u8::try_from(group % 26).unwrap_or(0))
}

fn choice_is_dimmed(
    accessibility: Accessibility,
    matched: bool,
    choices: &BTreeMap<u16, u8>,
) -> bool {
    !matched
        && matches!(accessibility,Accessibility::Choice{group,option} if choices.get(&group).is_some_and(|selected|*selected!=option))
}

fn tag(label: &str, color: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(label)
        .css_classes(["tag", color])
        .valign(gtk::Align::Center)
        .build()
}

fn artifact_deck_view(
    world: &shpd_seedfinder_core::model::GeneratedWorld,
    marks: &shpd_seedfinder_core::query::ScoutMatches,
    maximum_depth: u8,
) -> gtk::Expander {
    let depth = marks
        .transmuted_artifacts
        .iter()
        .map(|&(depth, _)| depth)
        .min()
        .unwrap_or(maximum_depth);
    let order = shpd_seedfinder_core::artifacts::deck_at(world, depth);
    let deck = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .homogeneous(true)
        .spacing(2)
        .margin_top(8)
        .build();
    if order.is_empty() {
        deck.append(&gtk::Label::new(Some("Deck exhausted.")));
    }
    for (index, &id) in order.iter().enumerate() {
        let matched = marks.transmuted_artifacts.contains(&(depth, index));
        let tile = gtk::Box::new(gtk::Orientation::Vertical, 4);
        tile.append(&sprites::trinket_tile(item(id), matched, false));
        tile.append(
            &gtk::Label::builder()
                .label((index + 1).to_string())
                .css_classes(["caption"])
                .build(),
        );
        let label = format!(
            "Transmutation #{}: {}{}",
            index + 1,
            item(id).name,
            if matched { ", matches requirement" } else { "" }
        );
        tile.set_tooltip_text(Some(&label));
        tile.update_property(&[gtk::accessible::Property::Label(&label)]);
        deck.append(&tile);
    }
    gtk::Expander::builder()
        .label("Artifact transmutation order")
        .expanded(true)
        .child(&deck)
        .build()
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_conflicting_choices_dim_after_a_match() {
        use super::{Accessibility, BTreeMap, choice_is_dimmed, choice_letter};
        let selected = BTreeMap::from([(2, 1)]);
        assert!(choice_is_dimmed(
            Accessibility::Choice {
                group: 2,
                option: 0
            },
            false,
            &selected
        ));
        assert!(!choice_is_dimmed(
            Accessibility::Choice {
                group: 2,
                option: 1
            },
            false,
            &selected
        ));
        assert!(!choice_is_dimmed(
            Accessibility::Choice {
                group: 3,
                option: 0
            },
            false,
            &selected
        ));
        assert!(!choice_is_dimmed(
            Accessibility::Choice {
                group: 2,
                option: 0
            },
            true,
            &selected
        ));
        assert!(!choice_is_dimmed(
            Accessibility::Independent,
            false,
            &selected
        ));
        assert_eq!(choice_letter(0), 'A');
        assert_eq!(choice_letter(25), 'Z');
        assert_eq!(choice_letter(26), 'A');
    }

    /// Run under Xvfb with --ignored; normal unit tests need no display.
    #[test]
    #[ignore = "requires a GTK display"]
    #[allow(clippy::too_many_lines)] // End-to-end scrolling, trinket and map continuity.
    fn scout_dock_tracks_scroll_and_trinket_switches() {
        use super::{AppState, DetailPane, Rc};
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
        // Opening a map survives the manifest rebuild used by trinket changes.
        adjustment.set_value(0.0);
        let first_heading = pane.sections.borrow()[0]
            .1
            .first_child()
            .unwrap()
            .downcast::<gtk::ToggleButton>()
            .unwrap();
        first_heading.emit_clicked();
        settle();
        let map = Rc::clone(&pane.open_map.borrow().as_ref().unwrap().1);
        choice(1).emit_clicked();
        settle();
        assert!(Rc::ptr_eq(
            &map,
            &pane.open_map.borrow().as_ref().unwrap().1
        ));
        assert_eq!(map.profile().trinket, pane.selected_trinket.get());
        let first_heading = pane.sections.borrow()[0]
            .1
            .first_child()
            .unwrap()
            .downcast::<gtk::ToggleButton>()
            .unwrap();
        first_heading.emit_clicked();
        settle();
        assert!(pane.open_map.borrow().is_none());
        window.close();
    }

    use super::match_summary;

    #[test]
    fn match_summary_counts_slots() {
        assert_eq!(match_summary(0, 1), "· 0 of 1 requirement matched");
        assert_eq!(match_summary(2, 3), "· 2 of 3 requirements matched");
    }
}
