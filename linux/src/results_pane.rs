// SPDX-License-Identifier: GPL-3.0-or-later

//! Results pane: streaming search session, live statistics, and seed list.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use adw::prelude::*;
use gtk::{gio, glib};
use shpd_seedfinder_core::query::SearchQuery;
use shpd_seedfinder_core::seed::DungeonSeed;
use shpd_seedfinder_session::{
    MAX_ACCEPTED_RESULTS, NativeSession, STATE_CANCELLED, STATE_COMPLETED, STATE_FAILED,
    STATE_RUNNING, production_filter_seeds,
};

use crate::format::{duration, estimate_duration, group_digits, probability_percent, seed_rate};
use crate::seed_list::matches_search;

const POLL_INTERVAL: Duration = Duration::from_millis(100);
const DRAIN_BATCH: usize = 256;

struct ActiveSearch {
    session: Rc<NativeSession>,
    matches: u64,
    last_tested: u64,
    last_tick: Instant,
    started: Instant,
    seeds_per_second: f64,
}

pub struct ResultsPane {
    pub page: adw::NavigationPage,
    title: adw::WindowTitle,
    stack: gtk::Stack,
    message_page: adw::StatusPage,
    stats_line: gtk::Label,
    progress_line: gtk::Label,
    progress_bar: gtk::ProgressBar,
    list: gtk::ListBox,
    search_entry: gtk::SearchEntry,
    import_button: gtk::Button,
    export_button: gtk::Button,
    filter_button: gtk::Button,
    clear_filter_button: gtk::Button,
    base_seeds: RefCell<Vec<DungeonSeed>>,
    filtered_seeds: RefCell<Option<Vec<DungeonSeed>>>,
    visible_seeds: RefCell<Vec<DungeonSeed>>,
    active: RefCell<Option<ActiveSearch>>,
    filtering: Cell<bool>,
    toasts: adw::ToastOverlay,
    on_select: RefCell<Option<SelectHandler>>,
    on_finished: RefCell<Option<Box<dyn Fn()>>>,
    on_import: RefCell<Option<Box<dyn Fn()>>>,
    on_export: RefCell<Option<Box<dyn Fn()>>>,
    on_filter: RefCell<Option<Box<dyn Fn()>>>,
}

type SelectHandler = Box<dyn Fn(&str)>;

impl ResultsPane {
    pub fn new(toasts: &adw::ToastOverlay) -> Rc<Self> {
        let empty_page = adw::StatusPage::builder()
            .icon_name("system-search-symbolic")
            .title("Find Seeds")
            .description(
                "Add requirements, then start a search. \
                 Matching seeds appear here as they are found.",
            )
            .build();
        let message_page = adw::StatusPage::new();

        let stats_line = caption_label();
        let progress_line = caption_label();
        let status_area = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(3)
            .margin_top(9)
            .margin_bottom(9)
            .margin_start(12)
            .margin_end(12)
            .build();
        status_area.append(&stats_line);
        status_area.append(&progress_line);

        let search_entry = gtk::SearchEntry::builder()
            .placeholder_text("Search seed codes")
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(12)
            .margin_end(12)
            .build();

        let list = gtk::ListBox::builder()
            .css_classes(["navigation-sidebar"])
            .build();
        let scroller = gtk::ScrolledWindow::builder()
            .child(&list)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .build();
        let results_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        results_box.append(&status_area);
        results_box.append(&search_entry);
        results_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        results_box.append(&scroller);

        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .build();
        stack.add_named(&empty_page, Some("empty"));
        stack.add_named(&results_box, Some("results"));
        stack.add_named(&message_page, Some("message"));

        let progress_bar = gtk::ProgressBar::builder()
            .css_classes(["osd"])
            .valign(gtk::Align::Start)
            .visible(false)
            .build();
        let overlay = gtk::Overlay::builder().child(&stack).build();
        overlay.add_overlay(&progress_bar);

        let title = adw::WindowTitle::new("Results", "");
        let header_bar = adw::HeaderBar::builder().title_widget(&title).build();
        let import_button = header_button("document-open-symbolic", "Import Seed List");
        let export_button = header_button("document-save-symbolic", "Export Visible Seeds");
        let filter_button = header_button("view-filter-symbolic", "Filter with Current Query");
        let clear_filter_button = header_button("edit-clear-symbolic", "Clear Item Filter");
        header_bar.pack_start(&import_button);
        header_bar.pack_start(&filter_button);
        header_bar.pack_end(&export_button);
        header_bar.pack_end(&clear_filter_button);
        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header_bar);
        toolbar_view.set_content(Some(&overlay));

        let nav_page = adw::NavigationPage::builder()
            .title("Results")
            .tag("results")
            .child(&toolbar_view)
            .build();

        let pane = Rc::new(Self {
            page: nav_page,
            title,
            stack,
            message_page,
            stats_line,
            progress_line,
            progress_bar,
            list,
            search_entry,
            import_button,
            export_button,
            filter_button,
            clear_filter_button,
            base_seeds: RefCell::new(Vec::new()),
            filtered_seeds: RefCell::new(None),
            visible_seeds: RefCell::new(Vec::new()),
            active: RefCell::new(None),
            filtering: Cell::new(false),
            toasts: toasts.clone(),
            on_select: RefCell::new(None),
            on_finished: RefCell::new(None),
            on_import: RefCell::new(None),
            on_export: RefCell::new(None),
            on_filter: RefCell::new(None),
        });
        pane.initialize();
        pane
    }

    fn initialize(self: &Rc<Self>) {
        self.connect_explorer_signals();
        self.refresh_controls();
    }

    fn connect_explorer_signals(self: &Rc<Self>) {
        self.list.connect_row_selected({
            let pane = Rc::clone(self);
            move |_, row| {
                let Some(row) = row else { return };
                let seeds = pane.visible_seeds.borrow();
                let Ok(index) = usize::try_from(row.index().unsigned_abs()) else {
                    return;
                };
                let Some(seed) = seeds.get(index) else {
                    return;
                };
                if let Some(handler) = pane.on_select.borrow().as_ref() {
                    handler(&seed.to_code());
                }
            }
        });
        self.search_entry.connect_search_changed({
            let pane = Rc::clone(self);
            move |_| pane.rebuild_list()
        });
        self.import_button.connect_clicked({
            let pane = Rc::clone(self);
            move |_| {
                if let Some(handler) = pane.on_import.borrow().as_ref() {
                    handler();
                }
            }
        });
        self.export_button.connect_clicked({
            let pane = Rc::clone(self);
            move |_| {
                if let Some(handler) = pane.on_export.borrow().as_ref() {
                    handler();
                }
            }
        });
        self.filter_button.connect_clicked({
            let pane = Rc::clone(self);
            move |_| {
                if let Some(handler) = pane.on_filter.borrow().as_ref() {
                    handler();
                }
            }
        });
        self.clear_filter_button.connect_clicked({
            let pane = Rc::clone(self);
            move |_| pane.clear_filter()
        });
    }

    /// Runs when the user selects a found seed.
    pub fn connect_select(&self, handler: impl Fn(&str) + 'static) {
        self.on_select.replace(Some(Box::new(handler)));
    }

    /// Runs once whenever a search reaches a terminal state.
    pub fn connect_finished(&self, handler: impl Fn() + 'static) {
        self.on_finished.replace(Some(Box::new(handler)));
    }

    pub fn connect_import(&self, handler: impl Fn() + 'static) {
        self.on_import.replace(Some(Box::new(handler)));
    }

    pub fn connect_export(&self, handler: impl Fn() + 'static) {
        self.on_export.replace(Some(Box::new(handler)));
    }

    pub fn connect_filter(&self, handler: impl Fn() + 'static) {
        self.on_filter.replace(Some(Box::new(handler)));
    }

    pub fn is_running(&self) -> bool {
        self.active.borrow().is_some()
    }

    pub fn is_filtering(&self) -> bool {
        self.filtering.get()
    }

    pub fn is_busy(&self) -> bool {
        self.is_running() || self.is_filtering()
    }

    pub fn cancel(&self) {
        if let Some(active) = self.active.borrow().as_ref() {
            active.session.cancel();
        }
    }

    /// Replaces the explorer's base list after a successful import. Existing
    /// data stays untouched when parsing or reading fails in the caller.
    pub fn replace_seeds(&self, seeds: Vec<DungeonSeed>) {
        if self.is_busy() {
            return;
        }
        let count = seeds.len();
        self.base_seeds.replace(seeds);
        self.filtered_seeds.replace(None);
        self.search_entry.set_text("");
        self.stack.set_visible_child_name("results");
        self.progress_bar.set_visible(false);
        self.progress_line.set_visible(false);
        self.stats_line.set_label(&format!(
            "Imported {} seed{}",
            group_digits(count_u64(count)),
            if count == 1 { "" } else { "s" }
        ));
        self.rebuild_list();
    }

    /// Returns exactly the rows visible after item and seed-code filtering.
    pub fn visible_seeds(&self) -> Vec<DungeonSeed> {
        self.visible_seeds.borrow().clone()
    }

    pub fn show_toast(&self, message: &str) {
        self.toasts.add_toast(adw::Toast::new(message));
    }

    /// Exhaustively filters the original base list with the current query. A
    /// second filter replaces the first one; clearing restores the base list.
    pub fn filter(self: &Rc<Self>, query: SearchQuery) {
        if self.is_busy() {
            return;
        }
        let seeds = self.base_seeds.borrow().clone();
        if seeds.is_empty() {
            self.show_toast("Import or find seeds before filtering");
            return;
        }

        let input_count = seeds.len();
        self.filtering.set(true);
        self.stack.set_visible_child_name("results");
        self.title.set_subtitle("Filtering…");
        self.stats_line.set_label(&format!(
            "Checking {} seed{} against the current query…",
            group_digits(count_u64(input_count)),
            if input_count == 1 { "" } else { "s" }
        ));
        self.progress_line.set_visible(false);
        self.progress_bar.set_fraction(0.0);
        self.progress_bar.pulse();
        self.progress_bar.set_visible(true);
        self.refresh_controls();

        let pane = Rc::clone(self);
        glib::spawn_future_local(async move {
            let outcome =
                gio::spawn_blocking(move || production_filter_seeds(&query, &seeds)).await;
            pane.filtering.set(false);
            pane.progress_bar.set_visible(false);
            match outcome {
                Ok(Ok(matches)) => {
                    let match_count = matches.len();
                    pane.filtered_seeds.replace(Some(matches));
                    pane.stats_line.set_label(&format!(
                        "Filtered {} seed{} · {} match{}",
                        group_digits(count_u64(input_count)),
                        if input_count == 1 { "" } else { "s" },
                        group_digits(count_u64(match_count)),
                        if match_count == 1 { "" } else { "es" }
                    ));
                    pane.rebuild_list();
                }
                Ok(Err(error)) => {
                    pane.show_toast(&format!("Could not filter seeds: {error:?}"));
                    pane.restore_count_subtitle();
                    pane.refresh_controls();
                }
                Err(_) => {
                    pane.show_toast("Seed filtering stopped unexpectedly");
                    pane.restore_count_subtitle();
                    pane.refresh_controls();
                }
            }
            pane.finish();
        });
    }

    pub fn clear_filter(&self) {
        if self.is_busy() || self.filtered_seeds.borrow().is_none() {
            return;
        }
        self.filtered_seeds.replace(None);
        self.stats_line.set_label("Item filter cleared");
        self.rebuild_list();
    }

    /// Starts a full-range production search; a failure to spawn is reported
    /// as a toast and leaves the pane idle.
    pub fn start(self: &Rc<Self>, query: SearchQuery) {
        if self.is_busy() {
            return;
        }
        let session = match NativeSession::production(query) {
            Ok(session) => Rc::new(session),
            Err(error) => {
                self.toasts.add_toast(adw::Toast::new(&format!(
                    "Could not start search: {error:?}"
                )));
                self.finish();
                return;
            }
        };
        self.base_seeds.borrow_mut().clear();
        self.filtered_seeds.replace(None);
        self.visible_seeds.borrow_mut().clear();
        self.search_entry.set_text("");
        self.list.remove_all();
        self.stack.set_visible_child_name("results");
        self.title.set_subtitle("Searching…");
        self.stats_line.set_label("Measuring search speed…");
        self.progress_line.set_label("Starting…");
        self.progress_line.set_visible(true);
        self.progress_bar.set_fraction(0.0);
        self.progress_bar.set_visible(true);
        let now = Instant::now();
        self.active.replace(Some(ActiveSearch {
            session,
            matches: 0,
            last_tested: 0,
            last_tick: now,
            started: now,
            seeds_per_second: 0.0,
        }));
        self.refresh_controls();

        let pane = Rc::clone(self);
        glib::timeout_add_local(POLL_INTERVAL, move || pane.tick());
    }

    fn tick(self: &Rc<Self>) -> glib::ControlFlow {
        let mut active_slot = self.active.borrow_mut();
        let Some(active) = active_slot.as_mut() else {
            return glib::ControlFlow::Break;
        };

        Self::drain_matches(self, active);

        let status = active.session.status();
        let search_state = status[0];
        let tested = status[1].max(0).unsigned_abs();
        let total = status[2].max(1).unsigned_abs();
        let probability = f64::from_bits(u64::from_ne_bytes(status[4].to_ne_bytes()));
        let probability = (probability > 0.0 && probability.is_finite()).then_some(probability);

        let now = Instant::now();
        let elapsed = now.duration_since(active.last_tick).as_secs_f64();
        if elapsed > 0.0 && tested >= active.last_tested {
            let instantaneous = precise(tested - active.last_tested) / elapsed;
            active.seeds_per_second = if active.seeds_per_second > 0.0 {
                0.7 * active.seeds_per_second + 0.3 * instantaneous
            } else {
                instantaneous
            };
        }
        active.last_tested = tested;
        active.last_tick = now;

        self.progress_bar
            .set_fraction((precise(tested) / precise(total)).clamp(0.0, 1.0));
        self.title.set_subtitle(&match active.matches {
            0 => "Searching…".to_owned(),
            1 => "1 seed".to_owned(),
            count => format!("{} seeds", group_digits(count)),
        });

        if search_state == STATE_RUNNING {
            let time_to_seed = probability
                .filter(|_| active.seeds_per_second > 0.0)
                .map(|probability| 1.0 / probability / active.seeds_per_second);
            self.stats_line.set_label(&format!(
                "Match probability {} · {} seeds/s · ~{} to a match",
                probability_percent(probability),
                seed_rate(active.seeds_per_second),
                estimate_duration(time_to_seed),
            ));
            self.progress_line.set_label(&format!(
                "Tested {} of {} · elapsed {}",
                group_digits(tested),
                group_digits(total),
                duration(active.started.elapsed().as_secs_f64()),
            ));
            return glib::ControlFlow::Continue;
        }

        // Catch matches that raced the terminal state transition.
        Self::drain_matches(self, active);
        let matches = active.matches;
        let diagnostic = if search_state == STATE_FAILED {
            active
                .session
                .take_failure_diagnostic()
                .unwrap_or_else(|| "unknown worker failure".to_owned())
        } else {
            String::new()
        };
        *active_slot = None;
        drop(active_slot);

        self.conclude(search_state, tested, matches, &diagnostic);
        glib::ControlFlow::Break
    }

    fn conclude(self: &Rc<Self>, search_state: i64, tested: u64, matches: u64, diagnostic: &str) {
        self.progress_bar.set_visible(false);
        self.title.set_subtitle(&match matches {
            0 => String::new(),
            1 => "1 seed".to_owned(),
            count => format!("{} seeds", group_digits(count)),
        });
        match search_state {
            STATE_FAILED => {
                self.show_message(
                    "computer-fail-symbolic",
                    "Search Failed",
                    &format!("The search stopped unexpectedly: {diagnostic}"),
                );
                self.toasts
                    .add_toast(adw::Toast::new("The search failed unexpectedly"));
            }
            STATE_COMPLETED if tested == 0 && matches == 0 => {
                // The engine proves some queries unsatisfiable before testing
                // a single seed; surface that instead of a zero-result search.
                self.show_message(
                    "action-unavailable-symbolic",
                    "Impossible Query",
                    "No seed can satisfy these requirements within the current floor \
                     limit. Quest-reward-only items need their quest floors in range: \
                     +3 wands floor 9, +3/+4 rings floor 19.",
                );
            }
            STATE_COMPLETED if matches == 0 => {
                self.show_message(
                    "edit-find-symbolic",
                    "No Seeds Found",
                    &format!(
                        "All {} seeds were tested without a match.",
                        group_digits(tested)
                    ),
                );
            }
            STATE_CANCELLED if matches == 0 => {
                self.show_message(
                    "media-playback-stop-symbolic",
                    "Search Stopped",
                    &format!(
                        "Tested {} seeds before stopping, without a match.",
                        group_digits(tested)
                    ),
                );
            }
            state => {
                let summary = if state == STATE_COMPLETED {
                    "Completed"
                } else {
                    "Stopped"
                };
                let cap_notice = if matches >= count_u64(MAX_ACCEPTED_RESULTS) {
                    " · result limit reached"
                } else {
                    ""
                };
                self.stats_line.set_label(&format!(
                    "{summary} · tested {} · {} match{}{cap_notice}",
                    group_digits(tested),
                    group_digits(matches),
                    if matches == 1 { "" } else { "es" },
                ));
                self.progress_line.set_visible(false);
            }
        }
        self.refresh_controls();
        self.finish();
    }

    fn show_message(&self, icon: &str, title: &str, description: &str) {
        self.message_page.set_icon_name(Some(icon));
        self.message_page.set_title(title);
        self.message_page.set_description(Some(description));
        self.stack.set_visible_child_name("message");
    }

    fn finish(&self) {
        if let Some(handler) = self.on_finished.borrow().as_ref() {
            handler();
        }
    }

    fn drain_matches(self: &Rc<Self>, active: &mut ActiveSearch) {
        loop {
            let worlds = active.session.drain_worlds(DRAIN_BATCH);
            if worlds.is_empty() {
                break;
            }
            let search = self.search_entry.text();
            let mut seeds = self.base_seeds.borrow_mut();
            let mut visible = self.visible_seeds.borrow_mut();
            for world in &worlds {
                seeds.push(world.seed);
                if matches_search(world.seed, &search) {
                    self.append_row(&world.seed.to_code(), visible.len() + 1);
                    visible.push(world.seed);
                }
            }
            active.matches += count_u64(worlds.len());
            self.export_button.set_sensitive(!visible.is_empty());
            drop(visible);
            drop(seeds);
        }
    }

    fn rebuild_list(&self) {
        self.list.unselect_all();
        self.list.remove_all();
        let search = self.search_entry.text();
        let source = self
            .filtered_seeds
            .borrow()
            .clone()
            .unwrap_or_else(|| self.base_seeds.borrow().clone());
        let visible = source
            .into_iter()
            .filter(|seed| matches_search(*seed, &search))
            .collect::<Vec<_>>();
        for (index, seed) in visible.iter().enumerate() {
            self.append_row(&seed.to_code(), index + 1);
        }
        self.visible_seeds.replace(visible);
        if !self.is_running() && !self.is_filtering() {
            self.restore_count_subtitle();
        }
        self.refresh_controls();
    }

    fn restore_count_subtitle(&self) {
        let visible = self.visible_seeds.borrow().len();
        let available = self
            .filtered_seeds
            .borrow()
            .as_ref()
            .map_or_else(|| self.base_seeds.borrow().len(), Vec::len);
        self.title.set_subtitle(&if visible == available {
            match visible {
                0 => String::new(),
                1 => "1 seed".to_owned(),
                count => format!("{} seeds", group_digits(count_u64(count))),
            }
        } else {
            format!(
                "{} of {} seeds",
                group_digits(count_u64(visible)),
                group_digits(count_u64(available))
            )
        });
    }

    fn refresh_controls(&self) {
        let busy = self.is_busy();
        let has_base = !self.base_seeds.borrow().is_empty();
        self.import_button.set_sensitive(!busy);
        self.filter_button.set_sensitive(!busy && has_base);
        self.clear_filter_button
            .set_sensitive(!busy && self.filtered_seeds.borrow().is_some());
        self.export_button
            .set_sensitive(!self.visible_seeds.borrow().is_empty());
        self.search_entry
            .set_sensitive(has_base || self.is_running());
    }

    fn append_row(&self, seed_code: &str, position: usize) {
        let index_label = gtk::Label::builder()
            .label(position.to_string())
            .css_classes(["dim-label", "caption", "numeric"])
            .width_chars(4)
            .xalign(1.0)
            .build();
        let copy_button = gtk::Button::builder()
            .icon_name("edit-copy-symbolic")
            .css_classes(["flat"])
            .valign(gtk::Align::Center)
            .tooltip_text("Copy Seed Code")
            .build();
        let row = adw::ActionRow::builder()
            .title(seed_code)
            .css_classes(["seed-row"])
            .build();
        row.add_prefix(&index_label);
        row.add_suffix(&copy_button);

        let toasts = self.toasts.clone();
        let seed = seed_code.to_owned();
        copy_button.connect_clicked(move |button| {
            button.clipboard().set_text(&seed);
            toasts.add_toast(adw::Toast::new(&format!("Copied {seed}")));
        });
        self.list.append(&row);
    }
}

fn caption_label() -> gtk::Label {
    gtk::Label::builder()
        .css_classes(["caption", "dim-label", "numeric"])
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .xalign(0.0)
        .build()
}

fn header_button(icon: &str, tooltip: &str) -> gtk::Button {
    gtk::Button::builder()
        .icon_name(icon)
        .css_classes(["flat"])
        .tooltip_text(tooltip)
        .build()
}

fn count_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

// Seed counts stay far below 2^53, so the f64 progress math is exact enough
// for display purposes.
#[allow(clippy::cast_precision_loss)]
const fn precise(value: u64) -> f64 {
    value as f64
}
