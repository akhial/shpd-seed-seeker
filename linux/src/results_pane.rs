// SPDX-License-Identifier: GPL-3.0-or-later

//! Results pane: streaming search session, live statistics, and seed list.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use adw::prelude::*;
use gtk::{gio, glib};
use shpd_seedfinder_core::auto_trinkets::{SeedRecipe, TrinketSearchMatch};
use shpd_seedfinder_core::feasibility::QueryPlan;
use shpd_seedfinder_core::query::SearchQuery;
use shpd_seedfinder_core::search::SearchError;
use shpd_seedfinder_session::{
    MAX_RESULTS, NativeSession, STATE_CANCELLED, STATE_COMPLETED, STATE_FAILED, STATE_RUNNING,
    filter_matching_recipes,
};

use crate::format::{duration, group_digits, search_statistics};
use crate::result_navigation;

const POLL_INTERVAL: Duration = Duration::from_millis(100);
const DRAIN_BATCH: usize = 256;

/// Most rows the seed list ever holds (docs/search-semantics.md). The
/// collection behind the list is uncapped — every find still reaches the
/// Target Set, refine filters, and export — but appending thousands of GTK
/// rows stalls the main loop, so only the first `DISPLAY_CAP` seeds are
/// listed. Deliberately equal to the engine's per-session accept cap.
const DISPLAY_CAP: usize = MAX_RESULTS;

struct ActiveSearch {
    replay_failed: bool,
    session: Rc<NativeSession>,
    query: SearchQuery,
    /// How this run relates to the session's Target, fixed at start; the
    /// conclusion folds the run into the Target accordingly.
    matches: u64,
    last_tested: u64,
    last_tick: Instant,
    started: Instant,
    seeds_per_second: f64,
    /// Every seed already listed, so a resumed traversal's small re-scan
    /// overlap never produces a duplicate row.
    seen: HashSet<String>,
    /// `(kept, previous)` when this search refines an earlier one.
    refined: Option<(u64, u64)>,
}

impl ActiveSearch {
    fn result_goal(&self) -> u64 {
        let kept = self.refined.map_or(0, |(kept, _)| kept);
        if kept >= MAX_RESULTS as u64 {
            kept + MAX_RESULTS as u64
        } else {
            MAX_RESULTS as u64
        }
    }
}

/// Everything needed to continue the last concluded run: the query it ran and
/// where its traversal stopped. The engine guarantees every match in the
/// region before `resume_from` was delivered, so combining the filtered
/// result list with a scan of the `remaining` seeds loses nothing.
struct BaseRun {
    query: SearchQuery,
    resume_from: u64,
    remaining: u64,
}

/// Every saved seed and its original recipe and query, retained until Clear.
struct Target {
    recipes: HashMap<String, SeedRecipe>,
    sources: HashMap<String, SearchQuery>,
    query: SearchQuery,
    seeds: Vec<String>,
}

/// An in-flight re-verification of previously found seeds on a worker thread.
struct PendingRefine {
    fresh_scan: bool,
    receiver: mpsc::Receiver<Result<Vec<TrinketSearchMatch>, SearchError>>,
    query: SearchQuery,
    resume_from: u64,
    remaining: u64,
    previous_matches: u64,
    started: Instant,
}

pub struct ResultsPane {
    recipes: RefCell<HashMap<String, SeedRecipe>>,
    pub page: adw::NavigationPage,
    title: adw::WindowTitle,
    stack: gtk::Stack,
    message_page: adw::StatusPage,
    stats_line: gtk::Label,
    progress_line: gtk::Label,
    list: gtk::ListBox,
    /// Every accepted seed of the current display run, uncapped and in
    /// traversal order. This collection — not the row widgets — feeds the
    /// Target, refine filters, export, and scout navigation; `list` holds
    /// rows for its first `DISPLAY_CAP` entries only.
    seeds: RefCell<Vec<String>>,
    active: RefCell<Option<ActiveSearch>>,
    pending_refine: RefCell<Option<PendingRefine>>,
    base: RefCell<Option<BaseRun>>,
    target: RefCell<Option<Target>>,
    /// Threads every search spawns, `None` meaning every core. The session
    /// clamps whatever it is given to the host's parallelism.
    workers: Cell<Option<NonZeroUsize>>,
    toasts: adw::ToastOverlay,
    on_select: RefCell<Option<SelectHandler>>,
    on_finished: RefCell<Option<Box<dyn Fn()>>>,
    on_results_changed: RefCell<Option<Box<dyn Fn()>>>,
}

type SelectHandler = Box<dyn Fn(&str)>;

impl ResultsPane {
    #[allow(clippy::too_many_lines)] // Widget assembly is declarative and linear.
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
        results_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        results_box.append(&scroller);

        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .build();
        stack.add_named(&empty_page, Some("empty"));
        stack.add_named(&results_box, Some("results"));
        stack.add_named(&message_page, Some("message"));

        let title = adw::WindowTitle::new("Results", "");
        let header_bar = adw::HeaderBar::builder().title_widget(&title).build();
        let export_button = gtk::Button::builder()
            .icon_name("results-export-symbolic")
            .tooltip_text("Export Results…")
            .action_name("win.export-results")
            .build();
        let import_menu = gio::Menu::new();
        import_menu.append(Some("From File…"), Some("win.import-results"));
        import_menu.append(Some("From Clipboard"), Some("win.import-clipboard"));
        let import_button = gtk::MenuButton::builder()
            .icon_name("results-import-symbolic")
            .tooltip_text("Import Results")
            .menu_model(&import_menu)
            .build();
        let clear_button = gtk::Button::builder()
            .icon_name("clear-results-symbolic")
            .tooltip_text("Clear Results")
            .action_name("win.clear-results")
            .build();
        header_bar.pack_end(&export_button);
        header_bar.pack_end(&import_button);
        header_bar.pack_end(&clear_button);
        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header_bar);
        toolbar_view.set_content(Some(&stack));

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
            list,
            seeds: RefCell::new(Vec::new()),
            recipes: RefCell::new(HashMap::new()),
            active: RefCell::new(None),
            pending_refine: RefCell::new(None),
            base: RefCell::new(None),
            target: RefCell::new(None),
            workers: Cell::new(None),
            toasts: toasts.clone(),
            on_select: RefCell::new(None),
            on_finished: RefCell::new(None),
            on_results_changed: RefCell::new(None),
        });
        pane.list.connect_row_selected({
            let pane = Rc::clone(&pane);
            move |_, row| {
                let Some(row) = row else { return };
                let seeds = pane.seeds.borrow();
                let Some(seed) = seeds.get(row.index().unsigned_abs() as usize) else {
                    return;
                };
                if let Some(handler) = pane.on_select.borrow().as_ref() {
                    handler(seed);
                }
            }
        });
        pane
    }

    /// Runs when the user selects a found seed.
    pub fn connect_select(&self, handler: impl Fn(&str) + 'static) {
        self.on_select.replace(Some(Box::new(handler)));
    }

    /// Runs once whenever a search reaches a terminal state.
    pub fn connect_finished(&self, handler: impl Fn() + 'static) {
        self.on_finished.replace(Some(Box::new(handler)));
    }

    /// Runs whenever the seed list changes (cleared by a new search, or grown
    /// by streamed matches), so dependent views can refresh positions.
    pub fn connect_results_changed(&self, handler: impl Fn() + 'static) {
        self.on_results_changed.replace(Some(Box::new(handler)));
    }

    fn notify_results_changed(&self) {
        if let Some(handler) = self.on_results_changed.borrow().as_ref() {
            handler();
        }
    }

    pub fn is_running(&self) -> bool {
        self.active.borrow().is_some() || self.pending_refine.borrow().is_some()
    }

    /// Sets how many threads the next search spawns. A running search keeps
    /// the count it started with; the new one takes effect at the next start,
    /// resumed continuations included.
    pub fn set_worker_count(&self, workers: usize) {
        self.workers.set(NonZeroUsize::new(workers));
    }

    /// Whether there is anything for "Clear Results" to discard: listed seeds,
    /// a finished-run message, or the Target a later search could refine.
    pub fn can_clear(&self) -> bool {
        !self.is_running()
            && (!self.seeds.borrow().is_empty()
                || self.base.borrow().is_some()
                || self.target.borrow().is_some()
                || self
                    .stack
                    .visible_child_name()
                    .is_some_and(|name| name != "empty"))
    }

    /// Empties the seed list along with the Target behind it — the Target
    /// Query, the Target Set, and the coverage a later start would otherwise
    /// refine or resume — so the next search anchors a new session from
    /// scratch. This is the only action that discards the Target. Callers
    /// must ensure no search is running.
    pub fn clear(&self) {
        self.base.replace(None);
        self.target.replace(None);
        self.seeds.borrow_mut().clear();
        self.recipes.borrow_mut().clear();
        self.list.remove_all();
        self.progress_line.set_visible(false);
        self.stats_line.set_label("");
        self.title.set_subtitle("");
        self.stack.set_visible_child_name("empty");
        self.notify_results_changed();
    }

    /// 0-based position of `seed` among the found seeds with the total count,
    /// or `None` when it is not a search result.
    pub fn position_of(&self, seed: &str) -> Option<(usize, usize)> {
        result_navigation::position(&self.seeds.borrow(), seed)
    }

    /// Moves the selection `delta` rows from the row holding `seed`, clamped
    /// to the list; selecting a row scouts it through the select handler.
    /// Returns whether the selection moved.
    pub fn select_step(&self, seed: &str, delta: i64) -> bool {
        let Some(target) = result_navigation::step(&self.seeds.borrow(), seed, delta) else {
            return false;
        };
        // Rows exist only for the first DISPLAY_CAP seeds of the collection;
        // a step landing past the listed prefix leaves the selection alone.
        let Some(row) = self
            .list
            .row_at_index(i32::try_from(target).unwrap_or(i32::MAX))
        else {
            return false;
        };
        self.list.select_row(Some(&row));
        // Keeps the selected result in view without disturbing an entry's
        // focus: J/K only fire while no editable widget is focused.
        row.grab_focus();
        true
    }

    /// The currently listed seed codes, in display order.
    #[must_use]
    pub fn recipe(&self, code: &str) -> Option<SeedRecipe> {
        self.recipes.borrow().get(code).copied()
    }

    pub fn seed_recipes(&self) -> Vec<SeedRecipe> {
        self.seeds
            .borrow()
            .iter()
            .filter_map(|code| self.recipe(code))
            .collect()
    }

    pub fn seed_codes(&self) -> Vec<String> {
        self.seeds.borrow().clone()
    }

    /// Replaces the list with seeds restored from an imported results file.
    /// Entries join the persistent pool. Callers must ensure no search is running.
    pub fn load_imported(&self, imported: &[String], query: &SearchQuery, recipes: &[SeedRecipe]) {
        self.recipes.borrow_mut().clear();
        for recipe in recipes {
            self.recipes
                .borrow_mut()
                .entry(recipe.seed.to_code())
                .or_insert(*recipe);
        }
        // Imported results carry no traversal state, so the previous
        // search's refine base no longer describes the listed seeds.
        self.base.replace(None);
        self.remember_pool(query, recipes.iter().copied());
        self.list.remove_all();
        {
            let mut seeds = self.seeds.borrow_mut();
            seeds.clear();
            seeds.extend_from_slice(imported);
        }
        for (index, seed) in imported.iter().take(DISPLAY_CAP).enumerate() {
            self.append_row(seed, index + 1);
        }
        self.progress_line.set_visible(false);
        let count = imported.len() as u64;
        self.title.set_subtitle(&match count {
            0 => String::new(),
            1 => "1 seed".to_owned(),
            count => format!("{} seeds", group_digits(count)),
        });
        self.stats_line.set_label(&format!(
            "Imported · {} seed{}",
            group_digits(count),
            if count == 1 { "" } else { "s" },
        ));
        self.stack.set_visible_child_name("results");
        self.notify_results_changed();
    }

    pub fn cancel(self: &Rc<Self>) {
        if let Some(active) = self.active.borrow().as_ref() {
            active.session.cancel();
            return;
        }
        // Abandoning the re-verification phase of a refine: its worker thread
        // result is discarded by the poll loop finding the slot empty, and
        // the previous results stay listed untouched.
        if self.pending_refine.borrow_mut().take().is_some() {
            self.progress_line.set_visible(false);
            self.restore_count_subtitle();
            self.stats_line
                .set_label("Refine stopped · previous results unchanged");
            self.finish();
        }
    }

    fn restore_count_subtitle(&self) {
        let count = self.seeds.borrow().len() as u64;
        self.title.set_subtitle(&match count {
            0 => String::new(),
            1 => "1 seed".to_owned(),
            count => format!("{} seeds", group_digits(count)),
        });
    }

    /// Every search checks the entire pool; only an identical query resumes coverage.
    pub fn start_search(self: &Rc<Self>, query: SearchQuery) {
        if self.is_running() {
            return;
        }
        if let Some(reason) = QueryPlan::analyze(&query).unsatisfiable_reason() {
            self.show_message("action-unavailable-symbolic", "Impossible Query", reason);
            return;
        }
        let seeds = self.target.borrow().as_ref().map(|pool| pool.seeds.clone());
        if let Some(seeds) = seeds {
            let window = self
                .base
                .borrow()
                .as_ref()
                .filter(|base| base.query == query)
                .map(|base| (base.resume_from, base.remaining));
            self.begin_filter(query, &seeds, window);
        } else {
            self.start_scan(query);
        }
    }

    fn new_session(
        &self,
        query: &SearchQuery,
        window: Option<(u64, u64)>,
    ) -> Result<NativeSession, SearchError> {
        if let Some((position, remaining)) = window {
            NativeSession::production_resumed(
                query.clone(),
                position,
                remaining,
                self.workers.get(),
            )
        } else {
            NativeSession::production(query.clone(), self.workers.get())
        }
    }

    /// Start a full traversal when there are no saved seeds to check.
    fn start_scan(self: &Rc<Self>, query: SearchQuery) {
        self.base.replace(None);
        let session = match NativeSession::production(query.clone(), self.workers.get()) {
            Ok(session) => Rc::new(session),
            Err(error) => {
                self.toasts.add_toast(adw::Toast::new(&format!(
                    "Could not start search: {error:?}"
                )));
                self.finish();
                return;
            }
        };
        self.seeds.borrow_mut().clear();
        self.recipes.borrow_mut().clear();
        self.list.remove_all();
        self.notify_results_changed();
        self.stack.set_visible_child_name("results");
        self.title.set_subtitle("Searching…");
        self.stats_line.set_label("Measuring search speed…");
        self.progress_line.set_label("Starting…");
        self.progress_line.set_visible(true);
        let now = Instant::now();
        self.active.replace(Some(ActiveSearch {
            replay_failed: false,
            session,
            query,
            matches: 0,
            last_tested: 0,
            last_tick: now,
            started: now,
            seeds_per_second: 0.0,
            seen: HashSet::new(),
            refined: None,
        }));

        let pane = Rc::clone(self);
        glib::timeout_add_local(POLL_INTERVAL, move || pane.tick());
    }

    /// Recheck every saved recipe, grouped by the query that chose its trinket.
    fn begin_filter(
        self: &Rc<Self>,
        query: SearchQuery,
        seed_codes: &[String],
        window: Option<(u64, u64)>,
    ) {
        let mut groups: Vec<(SearchQuery, Vec<SeedRecipe>)> = Vec::new();
        if let Some(pool) = self.target.borrow().as_ref() {
            for code in seed_codes {
                let Some(recipe) = pool.recipes.get(code) else {
                    continue;
                };
                let source = pool.sources.get(code).unwrap_or(&pool.query);
                if let Some((_, entries)) = groups.iter_mut().find(|(q, _)| q == source) {
                    entries.push(*recipe);
                } else {
                    groups.push((source.clone(), vec![*recipe]));
                }
            }
        }
        let previous_matches = seed_codes.len() as u64;
        let (sender, receiver) = mpsc::channel();
        let filter_query = query.clone();
        std::thread::spawn(move || {
            let result = groups
                .into_iter()
                .try_fold(Vec::new(), |mut matches, (base, recipes)| {
                    matches.extend(filter_matching_recipes(&filter_query, &base, &recipes)?);
                    Ok(matches)
                });
            let _ = sender.send(result);
        });
        let (resume_from, remaining) = window.unwrap_or((0, 0));
        self.pending_refine.replace(Some(PendingRefine {
            fresh_scan: window.is_none(),
            receiver,
            query,
            resume_from,
            remaining,
            previous_matches,
            started: Instant::now(),
        }));

        self.stack.set_visible_child_name("results");
        self.title.set_subtitle("Refining…");
        self.stats_line.set_label(&format!(
            "Re-checking {} found seed{} against the current requirements…",
            group_digits(previous_matches),
            if previous_matches == 1 { "" } else { "s" },
        ));
        self.progress_line.set_visible(false);
        let pane = Rc::clone(self);
        glib::timeout_add_local(POLL_INTERVAL, move || pane.refine_tick());
    }

    fn refine_tick(self: &Rc<Self>) -> glib::ControlFlow {
        let outcome = {
            let pending_slot = self.pending_refine.borrow();
            let Some(pending) = pending_slot.as_ref() else {
                // Cancelled while filtering; the thread result is discarded.
                return glib::ControlFlow::Break;
            };
            match pending.receiver.try_recv() {
                Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Ok(result) => result.map_err(|error| format!("{error:?}")),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Err("the verification thread stopped unexpectedly".to_owned())
                }
            }
        };
        let Some(pending) = self.pending_refine.borrow_mut().take() else {
            return glib::ControlFlow::Break;
        };

        let kept_worlds = match outcome {
            Ok(worlds) => worlds,
            Err(message) => {
                self.restore_count_subtitle();
                self.stats_line
                    .set_label("Refine failed · previous results unchanged");
                self.toasts.add_toast(adw::Toast::new(&format!(
                    "Could not re-verify the results: {message}"
                )));
                self.finish();
                return glib::ControlFlow::Break;
            }
        };

        // Replace the list with the surviving subset, in their original order.
        self.seeds.borrow_mut().clear();
        self.recipes.borrow_mut().clear();
        self.list.remove_all();
        let mut seen = HashSet::new();
        {
            let mut seeds = self.seeds.borrow_mut();
            for world in &kept_worlds {
                let code = world.recipe.seed.to_code();
                self.recipes.borrow_mut().insert(code.clone(), world.recipe);
                // Only the first DISPLAY_CAP survivors get a row; the rest
                // stay in the collection for the Target and later refines.
                if seeds.len() < DISPLAY_CAP {
                    self.append_row(&code, seeds.len() + 1);
                }
                seen.insert(code.clone());
                seeds.push(code);
            }
        }
        let kept = kept_worlds.len() as u64;

        if pending.remaining == 0 && !pending.fresh_scan {
            // The unchanged query already exhausted its traversal.
            self.base.replace(Some(BaseRun {
                query: pending.query,
                resume_from: pending.resume_from,
                remaining: 0,
            }));
            self.conclude_refined_filter_only(kept, pending.previous_matches);
            return glib::ControlFlow::Break;
        }

        let window = (!pending.fresh_scan).then_some((pending.resume_from, pending.remaining));
        let session = match self.new_session(&pending.query, window) {
            Ok(session) => Rc::new(session),
            Err(error) => {
                self.restore_count_subtitle();
                self.stats_line
                    .set_label("Refine failed · kept seeds are listed");
                self.toasts.add_toast(adw::Toast::new(&format!(
                    "Could not resume the search: {error:?}"
                )));
                self.finish();
                return glib::ControlFlow::Break;
            }
        };
        self.title.set_subtitle("Searching…");
        self.stats_line.set_label("Measuring search speed…");
        self.progress_line.set_label("Resuming…");
        self.progress_line.set_visible(true);
        let now = Instant::now();
        self.active.replace(Some(ActiveSearch {
            replay_failed: false,
            session,
            query: pending.query,
            matches: kept,
            last_tested: 0,
            last_tick: now,
            started: pending.started,
            seeds_per_second: 0.0,
            seen,
            refined: Some((kept, pending.previous_matches)),
        }));
        let pane = Rc::clone(self);
        glib::timeout_add_local(POLL_INTERVAL, move || pane.tick());
        glib::ControlFlow::Break
    }

    fn conclude_refined_filter_only(&self, kept: u64, previous: u64) {
        self.progress_line.set_visible(false);
        self.restore_count_subtitle();
        self.stats_line
            .set_label("Completed · every seed was already scanned");
        if let Some(notice) = conclusion_toast(
            Some((kept, previous)),
            usize::try_from(kept).unwrap_or(usize::MAX),
            0,
        ) {
            self.toasts.add_toast(adw::Toast::new(&notice));
        }
        self.finish();
    }

    fn remember_pool(&self, query: &SearchQuery, recipes: impl Iterator<Item = SeedRecipe>) {
        let mut slot = self.target.borrow_mut();
        let pool = slot.get_or_insert_with(|| Target {
            query: query.clone(),
            recipes: HashMap::new(),
            sources: HashMap::new(),
            seeds: Vec::new(),
        });
        for recipe in recipes {
            let code = recipe.seed.to_code();
            if pool.recipes.contains_key(&code) {
                continue;
            }
            pool.seeds.push(code.clone());
            pool.sources.insert(code.clone(), query.clone());
            pool.recipes.insert(code, recipe);
        }
    }

    /// Commit only a concluded scan's query, recipes, and exact coverage.
    fn remember_scan(&self, active: &ActiveSearch, search_state: i64) {
        let base =
            (search_state == STATE_COMPLETED || search_state == STATE_CANCELLED).then(|| {
                let [resume_from, remaining] = active.session.resume_hint();
                BaseRun {
                    query: active.query.clone(),
                    resume_from: resume_from.max(0).unsigned_abs(),
                    remaining: remaining.max(0).unsigned_abs(),
                }
            });
        self.base.replace(base);
    }

    fn tick(self: &Rc<Self>) -> glib::ControlFlow {
        let mut active_slot = self.active.borrow_mut();
        let Some(active) = active_slot.as_mut() else {
            return glib::ControlFlow::Break;
        };

        Self::drain_matches(self, active);

        let status = active.session.status();
        let search_state = if active.replay_failed {
            STATE_FAILED
        } else {
            status[0]
        };
        let tested = status[1].max(0).unsigned_abs();
        let probability = f64::from_bits(u64::from_ne_bytes(status[4].to_ne_bytes()));

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

        self.title.set_subtitle(&match active.matches {
            0 => "Searching…".to_owned(),
            1 => "1 seed".to_owned(),
            count => format!("{} seeds", group_digits(count)),
        });

        let goal = active.result_goal();
        if search_state == STATE_RUNNING {
            if active.matches >= goal {
                active.session.cancel();
            }
            self.stats_line
                .set_label(&search_statistics(probability, active.seeds_per_second));
            self.progress_line.set_label(&format!(
                "Tested {} · elapsed {}",
                group_digits(tested),
                duration(active.started.elapsed().as_secs_f64()),
            ));
            return glib::ControlFlow::Continue;
        }

        // Catch matches that raced the terminal state transition.
        Self::drain_matches(self, active);
        let mut search_state = if active.replay_failed {
            STATE_FAILED
        } else if search_state == STATE_CANCELLED && active.matches >= goal {
            STATE_COMPLETED
        } else {
            search_state
        };
        let mut resume_error = None;
        let [position, remaining] = active.session.resume_hint();
        if search_state == STATE_COMPLETED && tested > 0 && remaining > 0 && active.matches < goal {
            let next = self.new_session(
                &active.query,
                Some((
                    position.max(0).unsigned_abs(),
                    remaining.max(0).unsigned_abs(),
                )),
            );
            match next {
                Ok(session) => {
                    active.session = Rc::new(session);
                    active.last_tested = 0;
                    return glib::ControlFlow::Continue;
                }
                Err(error) => {
                    search_state = STATE_FAILED;
                    resume_error = Some(format!("Could not resume the search: {error:?}"));
                }
            }
        }
        let matches = active.matches;
        let refined = active.refined;
        let diagnostic = if search_state == STATE_FAILED {
            resume_error
                .or_else(|| active.session.take_failure_diagnostic())
                .unwrap_or_else(|| "unknown worker failure".to_owned())
        } else {
            String::new()
        };
        let plan = QueryPlan::analyze(&active.query);
        self.remember_scan(active, search_state);
        *active_slot = None;
        drop(active_slot);

        self.conclude(
            search_state,
            tested,
            matches,
            refined,
            plan.unsatisfiable_reason(),
            &diagnostic,
        );
        glib::ControlFlow::Break
    }

    fn conclude(
        self: &Rc<Self>,
        search_state: i64,
        tested: u64,
        matches: u64,
        refined: Option<(u64, u64)>,
        impossible_reason: Option<&str>,
        diagnostic: &str,
    ) {
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
            STATE_COMPLETED if matches == 0 && impossible_reason.is_some() => {
                self.show_message(
                    "action-unavailable-symbolic",
                    "Impossible Query",
                    impossible_reason.expect("impossible query"),
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
                self.stats_line.set_label(&format!(
                    "{summary} · tested {} · {} match{}",
                    group_digits(tested),
                    group_digits(matches),
                    if matches == 1 { "" } else { "es" },
                ));
                self.progress_line.set_visible(false);
                // The engine's cap counts this session's accepts only, so a
                // resumed refine subtracts the survivors it started from.
                let new_finds = matches.saturating_sub(refined.map_or(0, |(kept, _)| kept));
                let collected = self.seeds.borrow().len();
                if let Some(notice) = conclusion_toast(refined, collected, new_finds) {
                    self.toasts.add_toast(adw::Toast::new(&notice));
                }
            }
        }
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
        let mut appended = false;
        loop {
            let worlds = match active.session.drain_matches(DRAIN_BATCH) {
                Ok(worlds) => worlds,
                Err(error) => {
                    active.session.cancel();
                    active.replay_failed = true;
                    self.toasts.add_toast(adw::Toast::new(&format!(
                        "Match verification failed: {error}"
                    )));
                    break;
                }
            };
            if worlds.is_empty() {
                break;
            }
            self.remember_pool(&active.query, worlds.iter().map(|world| world.recipe));
            let mut seeds = self.seeds.borrow_mut();
            for world in &worlds {
                let code = world.recipe.seed.to_code();
                self.recipes.borrow_mut().insert(code.clone(), world.recipe);
                // A resumed traversal may re-test a small overlap around the
                // previous stop position; keep each seed listed once.
                if !active.seen.insert(code.clone()) {
                    continue;
                }
                // Live finds past the display cap join the collection (and
                // through it the Target) without a row.
                if seeds.len() < DISPLAY_CAP {
                    self.append_row(&code, seeds.len() + 1);
                }
                seeds.push(code);
                active.matches += 1;
                appended = true;
            }
        }
        if appended {
            self.notify_results_changed();
        }
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
        row.set_title("");
        let code = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        code.set_hexpand(true);
        code.append(
            &gtk::Label::builder()
                .label(seed_code)
                .css_classes(["monospace"])
                .build(),
        );
        if let Some(id) = self.recipe(seed_code).and_then(|r| r.trinket) {
            let sprite = crate::sprites::item_image_sized(
                crate::sprites::ItemSprite::from_catalog(shpd_seedfinder_core::catalog::item(id)),
                None,
                16,
            );
            sprite.set_opacity(0.6);
            sprite.set_tooltip_text(Some(shpd_seedfinder_core::catalog::item(id).name));
            code.append(&sprite);
        }
        row.add_prefix(&code);
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

/// The single status toast for a concluded run — a refine outcome, a list
/// notice, or both joined into one message (stacked toasts would hide one
/// behind the other) — or `None` when nothing is worth announcing. `refined`
/// is `(kept, previous)` when the run re-verified earlier seeds, `collected`
/// the full uncapped collection size, and `new_finds` how many seeds this
/// session accepted beyond the refine survivors. The list notice reports
/// truncation — the collection outgrew the `DISPLAY_CAP` listed rows — or,
/// for an untruncated run, that the engine's accept cap ended the session.
fn conclusion_toast(
    refined: Option<(u64, u64)>,
    collected: usize,
    new_finds: u64,
) -> Option<String> {
    let refined_notice = refined.map(|(kept, previous)| {
        format!(
            "Refined: kept {} of {} previous seed{}",
            group_digits(kept),
            group_digits(previous),
            if previous == 1 { "" } else { "s" },
        )
    });
    let limit_notice = if collected > DISPLAY_CAP {
        Some(format!(
            "listing the first {} of {} seeds",
            group_digits(DISPLAY_CAP as u64),
            group_digits(collected as u64),
        ))
    } else if new_finds >= MAX_RESULTS as u64 {
        Some(format!(
            "result limit reached ({} seeds)",
            group_digits(MAX_RESULTS as u64),
        ))
    } else {
        None
    };
    match (refined_notice, limit_notice) {
        (Some(refine), Some(limit)) => Some(format!("{refine} · {limit}")),
        (Some(refine), None) => Some(refine),
        (None, Some(limit)) => {
            let mut chars = limit.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect())
        }
        (None, None) => None,
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

// Seed counts stay far below 2^53, so the f64 progress math is exact enough
// for display purposes.
#[allow(clippy::cast_precision_loss)]
const fn precise(value: u64) -> f64 {
    value as f64
}

#[cfg(test)]
mod tests {
    use super::{DISPLAY_CAP, conclusion_toast};

    const CAP: u64 = DISPLAY_CAP as u64;

    #[test]
    fn quiet_run_shows_no_toast() {
        assert_eq!(conclusion_toast(None, 12, 12), None);
    }

    #[test]
    fn plain_refine_reports_the_kept_count() {
        assert_eq!(
            conclusion_toast(Some((3, 10)), 3, 0),
            Some("Refined: kept 3 of 10 previous seeds".to_owned()),
        );
    }

    #[test]
    fn refine_of_a_single_seed_stays_singular() {
        assert_eq!(
            conclusion_toast(Some((1, 1)), 1, 0),
            Some("Refined: kept 1 of 1 previous seed".to_owned()),
        );
    }

    #[test]
    fn fresh_scan_hitting_the_engine_cap_reports_the_limit() {
        assert_eq!(
            conclusion_toast(None, DISPLAY_CAP, CAP),
            Some("Result limit reached (1\u{202f}024 seeds)".to_owned()),
        );
    }

    #[test]
    fn survivors_alone_filling_the_display_are_not_truncation() {
        // Exactly DISPLAY_CAP collected seeds all have rows, and a session
        // that accepted nothing new never hit the engine cap.
        assert_eq!(
            conclusion_toast(Some((CAP, 2_000)), DISPLAY_CAP, 0),
            Some("Refined: kept 1\u{202f}024 of 2\u{202f}000 previous seeds".to_owned()),
        );
    }

    #[test]
    fn accumulated_refine_reports_truncation_in_one_toast() {
        assert_eq!(
            conclusion_toast(Some((900, 1_500)), 1_900, 1_000),
            Some(
                "Refined: kept 900 of 1\u{202f}500 previous seeds · \
                 listing the first 1\u{202f}024 of 1\u{202f}900 seeds"
                    .to_owned()
            ),
        );
    }

    #[test]
    fn truncation_outranks_the_engine_cap_notice() {
        assert_eq!(
            conclusion_toast(Some((500, 500)), 1_524, CAP),
            Some(
                "Refined: kept 500 of 500 previous seeds · \
                 listing the first 1\u{202f}024 of 1\u{202f}524 seeds"
                    .to_owned()
            ),
        );
    }

    #[test]
    fn capped_resume_without_truncation_reports_the_limit() {
        assert_eq!(
            conclusion_toast(Some((0, 500)), DISPLAY_CAP, CAP),
            Some(
                "Refined: kept 0 of 500 previous seeds · \
                 result limit reached (1\u{202f}024 seeds)"
                    .to_owned()
            ),
        );
    }

    #[test]
    fn oversized_filter_survivor_set_reports_truncation_alone() {
        // A target filter of a grown Target Set: no scan, no engine cap,
        // but more survivors than rows.
        assert_eq!(
            conclusion_toast(Some((3_000, 5_116)), 3_000, 0),
            Some(
                "Refined: kept 3\u{202f}000 of 5\u{202f}116 previous seeds · \
                 listing the first 1\u{202f}024 of 3\u{202f}000 seeds"
                    .to_owned()
            ),
        );
    }
}
