// SPDX-License-Identifier: GPL-3.0-or-later

//! Main window: an adaptive triple-pane layout built from two nested
//! navigation split views (query → results → seed), following GNOME's
//! multi-pane navigation pattern.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, gio, glib};

use shpd_seedfinder_core::deep_link;
use shpd_seedfinder_core::editor::{Draft, ResinOutcome, SaveResult};
use shpd_seedfinder_core::{json_query, results_export};
use shpd_seedfinder_session::MAX_RESULTS;

use crate::config::APP_NAME;
use crate::query_pane::BoardAction;
use crate::state::AppState;
use crate::{
    challenges_dialog, detail_pane, persist, presets_dialog, query_pane, requirement_editor,
    resin_editor, results_pane, update,
};

#[allow(clippy::too_many_lines)] // Linear assembly of panes, actions, and wiring.
pub fn present(app: &adw::Application) {
    if let Some(window) = app.active_window() {
        window.present();
        return;
    }

    let state = Rc::new(RefCell::new(persist::load()));
    let user_presets = Rc::new(RefCell::new(persist::load_presets()));
    // A device-local preference rather than part of the query: presets, share
    // links and imported results all replace the state above without ever
    // touching how many threads this machine wants to run.
    let workers = Rc::new(Cell::new(persist::load_workers()));
    // The query that produced the current results list, snapshotted at search
    // start (or import) so an export never reflects later editor changes.
    let exported_query: Rc<RefCell<Option<shpd_seedfinder_core::query::SearchQuery>>> =
        Rc::new(RefCell::new(None));
    let toasts = adw::ToastOverlay::new();

    let query = query_pane::QueryPane::new(build_menu().upcast_ref());
    let results = results_pane::ResultsPane::new(&toasts);
    let detail = detail_pane::DetailPane::new(&toasts);
    query.set_worker_count(workers.get());
    results.set_worker_count(workers.get());

    // Results and seed detail form the inner split view; the query sidebar
    // wraps both in the outer one. Nesting the two split views is the
    // libadwaita pattern for adaptive triple-pane layouts.
    let inner_split = adw::NavigationSplitView::builder()
        .sidebar(&results.page)
        .content(&detail.page)
        .min_sidebar_width(270.0)
        .max_sidebar_width(380.0)
        .sidebar_width_fraction(0.36)
        .build();
    let inner_page = adw::NavigationPage::builder()
        .title("Results")
        .child(&inner_split)
        .build();
    let outer_split = adw::NavigationSplitView::builder()
        .sidebar(&query.page)
        .content(&inner_page)
        .min_sidebar_width(300.0)
        .max_sidebar_width(420.0)
        .sidebar_width_fraction(0.3)
        .build();
    toasts.set_child(Some(&outer_split));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .content(&toasts)
        .default_width(1240)
        .default_height(760)
        .width_request(360)
        .height_request(360)
        .title(APP_NAME)
        .build();

    let medium = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        1000.0,
        adw::LengthUnit::Sp,
    ));
    medium.add_setter(&inner_split, "collapsed", Some(&true.to_value()));
    window.add_breakpoint(medium);
    let narrow = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        620.0,
        adw::LengthUnit::Sp,
    ));
    narrow.add_setter(&inner_split, "collapsed", Some(&true.to_value()));
    narrow.add_setter(&outer_split, "collapsed", Some(&true.to_value()));
    window.add_breakpoint(narrow);

    // Actions and cross-pane wiring.

    let start_action = gio::SimpleAction::new("start-search", None);
    let clear_action = gio::SimpleAction::new("clear-results", None);
    let refresh_all: Rc<dyn Fn()> = Rc::new({
        let state = Rc::clone(&state);
        let query = Rc::clone(&query);
        let detail = Rc::clone(&detail);
        let results = Rc::clone(&results);
        let start_action = start_action.clone();
        let clear_action = clear_action.clone();
        move || {
            let snapshot = state.borrow();
            persist::save(&snapshot);
            query.refresh(&snapshot);
            detail.render(&snapshot);
            start_action.set_enabled(
                !snapshot.requirements.is_empty()
                    || !snapshot.floor_requirements.is_empty()
                    || snapshot.needs_resin()
                    || results.is_running(),
            );
            clear_action.set_enabled(results.can_clear());
        }
    });

    // The sheets save through the shared editor onto the rows as they are
    // when they close; a refused save keeps its sheet open with why. A
    // closing requirement sheet returns to the chip it saved.
    let save_sheet: Rc<dyn Fn(&Draft) -> SaveResult> = Rc::new({
        let state = Rc::clone(&state);
        let query = Rc::clone(&query);
        let refresh_all = Rc::clone(&refresh_all);
        move |draft| {
            let saved = state.borrow_mut().save(draft);
            if let SaveResult::Saved { result, resin } = &saved {
                query.follow_rekeyed(&result.rekeyed);
                if result.changed || *resin != ResinOutcome::Unchanged {
                    refresh_all();
                }
            }
            saved
        }
    });
    let edit_resin: Rc<dyn Fn(Draft)> = Rc::new({
        let save_sheet = Rc::clone(&save_sheet);
        let window = window.clone();
        move |draft| {
            let save_sheet = Rc::clone(&save_sheet);
            resin_editor::present(&window, draft, move |draft| save_sheet(draft));
        }
    });
    let edit_requirement: Rc<dyn Fn(Draft)> = Rc::new({
        let state = Rc::clone(&state);
        let query = Rc::clone(&query);
        let save_sheet = Rc::clone(&save_sheet);
        let edit_resin = Rc::clone(&edit_resin);
        let window = window.clone();
        move |draft| {
            let focus = Rc::new(Cell::new(None));
            let dialog = requirement_editor::present(
                &window,
                draft,
                {
                    let save_sheet = Rc::clone(&save_sheet);
                    let focus = Rc::clone(&focus);
                    move |draft| {
                        let saved = save_sheet(draft);
                        if let SaveResult::Saved { result, .. } = &saved {
                            focus.set(result.focus);
                        }
                        saved
                    }
                },
                // The sheet's Arcane Resin row leads to the query's resin as
                // the resin chip does, so the dialog edits (and offers to
                // remove) a resin the query already asks for.
                {
                    let state = Rc::clone(&state);
                    let edit_resin = Rc::clone(&edit_resin);
                    move || {
                        let draft = state.borrow().open_resin();
                        edit_resin(draft);
                    }
                },
            );
            let query = Rc::clone(&query);
            dialog.connect_closed(move |_| {
                if let Some(key) = focus.take() {
                    query.focus_row(key);
                }
            });
        }
    });

    // Every board gesture — activating a chip, dropping one on another,
    // the context menu — arrives here as one of these. Board edits go to the
    // shared editor, which may refuse one and say why; the board redraws
    // only when the rows changed.
    query.connect_board({
        let state = Rc::clone(&state);
        let query = Rc::clone(&query);
        let edit_requirement = Rc::clone(&edit_requirement);
        let refresh_all = Rc::clone(&refresh_all);
        let toasts = toasts.clone();
        move |action| {
            let edit = match action {
                BoardAction::Open(key) => {
                    let draft = state.borrow_mut().open_sheet(Some(key), false);
                    edit_requirement(draft);
                    return;
                }
                BoardAction::ToggleFarmingFloor(depth) => {
                    state.borrow_mut().toggle_farming_floor(depth);
                    refresh_all();
                    return;
                }
                BoardAction::RemoveFloorRequirement(depth) => {
                    state
                        .borrow_mut()
                        .floor_requirements
                        .retain(|floor| floor.depth != depth);
                    refresh_all();
                    return;
                }
                BoardAction::Edit(edit) => edit,
            };
            let result = state.borrow_mut().apply(&[edit]);
            query.follow_rekeyed(&result.rekeyed);
            if let Some(refusal) = result.refused {
                toasts.add_toast(adw::Toast::new(&refusal.to_string()));
            }
            if result.changed {
                refresh_all();
                if let Some(key) = result.focus {
                    query.focus_row(key);
                }
            }
        }
    });
    query.connect_changed({
        let state = Rc::clone(&state);
        let query = Rc::clone(&query);
        let results = Rc::clone(&results);
        let workers = Rc::clone(&workers);
        let refresh_all = Rc::clone(&refresh_all);
        move || {
            query.read_scope(&mut state.borrow_mut());
            // The worker count belongs to the machine, so it is saved on its
            // own rather than with the query the pane just wrote.
            let chosen = query.worker_count();
            if workers.replace(chosen) != chosen {
                results.set_worker_count(chosen);
                persist::save_workers(chosen);
            }
            refresh_all();
        }
    });

    results.connect_select({
        let state = Rc::clone(&state);
        let detail = Rc::clone(&detail);
        let results = Rc::clone(&results);
        let inner_split = inner_split.clone();
        let outer_split = outer_split.clone();
        let exported_query = Rc::clone(&exported_query);
        move |seed| {
            if let (Some(recipe), Some(query)) =
                (results.recipe(seed), exported_query.borrow().as_ref())
            {
                detail.scout_recipe(recipe, &AppState::from_query(query));
            } else {
                detail.scout(Some(seed), &state.borrow());
            }
            detail.set_result_position(results.position_of(seed));
            outer_split.set_show_content(true);
            inner_split.set_show_content(true);
        }
    });
    results.connect_finished({
        let query = Rc::clone(&query);
        let refresh_all = Rc::clone(&refresh_all);
        move || {
            query.set_running(false);
            // Re-derives the enabled actions, including whether the finished
            // search left anything to clear.
            refresh_all();
        }
    });
    detail.connect_navigate({
        let results = Rc::clone(&results);
        let detail = Rc::clone(&detail);
        move |delta| {
            if let Some(seed) = detail.current_seed() {
                results.select_step(&seed, delta);
            }
        }
    });
    detail.connect_scout({
        let exported_query = Rc::clone(&exported_query);
        let state = Rc::clone(&state);
        let detail = Rc::clone(&detail);
        let results = Rc::clone(&results);
        move || {
            let recipe = detail.entered_seed().and_then(|seed| results.recipe(&seed));
            if let (Some(recipe), Some(query)) = (recipe, exported_query.borrow().as_ref()) {
                detail.scout_recipe(recipe, &AppState::from_query(query));
            } else {
                detail.scout(None, &state.borrow());
            }
            // A hand-entered seed may still be one of the results; keep the
            // position indicator honest either way.
            let position = detail
                .current_seed()
                .and_then(|seed| results.position_of(&seed));
            detail.set_result_position(position);
        }
    });

    // Keeps the seed pane's "Result N of M" indicator honest while a new
    // search clears the list or a running one streams matches in.
    results.connect_results_changed({
        let detail = Rc::clone(&detail);
        let results = Rc::clone(&results);
        move || {
            let position = detail
                .current_seed()
                .and_then(|seed| results.position_of(&seed));
            detail.set_result_position(position);
        }
    });

    // J/K step the scouted seed through the search results; row selection
    // then drives the scout above. The keys stay inert while a dialog is
    // presented, while an editable widget has focus, or while a collapsed
    // split view is showing another page (navigating would flip pages the
    // user is not on).
    let navigate_keys = gtk::EventControllerKey::new();
    navigate_keys.set_propagation_phase(gtk::PropagationPhase::Bubble);
    navigate_keys.connect_key_pressed({
        let window = window.clone();
        let results = Rc::clone(&results);
        let detail = Rc::clone(&detail);
        let inner_split = inner_split.clone();
        let outer_split = outer_split.clone();
        move |_, key, _, modifiers| {
            let delta = match key {
                gdk::Key::j | gdk::Key::J => 1,
                gdk::Key::k | gdk::Key::K => -1,
                _ => return glib::Propagation::Proceed,
            };
            if modifiers.intersects(
                gdk::ModifierType::CONTROL_MASK
                    | gdk::ModifierType::ALT_MASK
                    | gdk::ModifierType::SUPER_MASK,
            ) {
                return glib::Propagation::Proceed;
            }
            // Never navigate behind a modal (requirement editor, presets,
            // challenges, shortcuts are all in-window adw::Dialogs).
            if window.visible_dialog().is_some() {
                return glib::Propagation::Proceed;
            }
            // Bubble-phase keys rarely escape an editable widget, but a
            // focused entry must never lose typed letters to navigation.
            if GtkWindowExt::focus(&window)
                .is_some_and(|focus| focus.is::<gtk::Text>() || focus.is::<gtk::TextView>())
            {
                return glib::Propagation::Proceed;
            }
            // On collapsed layouts, act only while the seed page is visible.
            let seed_page_visible = (!outer_split.is_collapsed() || outer_split.shows_content())
                && (!inner_split.is_collapsed() || inner_split.shows_content());
            if !seed_page_visible {
                return glib::Propagation::Proceed;
            }
            let Some(seed) = detail.current_seed() else {
                return glib::Propagation::Proceed;
            };
            if results.select_step(&seed, delta) {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        }
    });
    window.add_controller(navigate_keys);

    start_action.connect_activate({
        let state = Rc::clone(&state);
        let query = Rc::clone(&query);
        let results = Rc::clone(&results);
        let exported_query = Rc::clone(&exported_query);
        let toasts = toasts.clone();
        let inner_split = inner_split.clone();
        let outer_split = outer_split.clone();
        let refresh_all = Rc::clone(&refresh_all);
        move |_, _| {
            if results.is_running() {
                results.cancel();
                return;
            }
            match state.borrow().to_query() {
                Ok(search_query) => {
                    // Every search checks the retained pool and then scans for more matches.
                    results.start_search(search_query.clone());
                    if results.is_running() {
                        exported_query.replace(Some(search_query));
                        query.set_running(true);
                        outer_split.set_show_content(true);
                        inner_split.set_show_content(false);
                        refresh_all();
                    }
                }
                Err(message) => toasts.add_toast(adw::Toast::new(&message)),
            }
        }
    });
    window.add_action(&start_action);

    clear_action.connect_activate({
        let results = Rc::clone(&results);
        let exported_query = Rc::clone(&exported_query);
        let refresh_all = Rc::clone(&refresh_all);
        move |_, _| {
            if results.is_running() {
                return;
            }
            results.clear();
            // Nothing is listed any more, so no query describes an export.
            exported_query.replace(None);
            refresh_all();
        }
    });
    window.add_action(&clear_action);

    let resin_action = gio::SimpleAction::new("edit-resin", None);
    resin_action.connect_activate({
        let state = Rc::clone(&state);
        let edit_resin = Rc::clone(&edit_resin);
        move |_, _| {
            let draft = state.borrow().open_resin();
            edit_resin(draft);
        }
    });
    window.add_action(&resin_action);
    let remove_resin = gio::SimpleAction::new("remove-resin", None);
    remove_resin.connect_activate({
        let state = Rc::clone(&state);
        let refresh_all = Rc::clone(&refresh_all);
        move |_, _| {
            state.borrow_mut().set_resin(None);
            refresh_all();
        }
    });
    window.add_action(&remove_resin);

    let add_action = gio::SimpleAction::new("add-requirement", None);
    add_action.connect_activate({
        let state = Rc::clone(&state);
        let edit_requirement = Rc::clone(&edit_requirement);
        move |_, _| {
            let draft = state.borrow_mut().open_sheet(None, false);
            edit_requirement(draft);
        }
    });
    window.add_action(&add_action);

    let blanket_action = gio::SimpleAction::new("add-blanket", None);
    blanket_action.connect_activate({
        let state = Rc::clone(&state);
        let edit_requirement = Rc::clone(&edit_requirement);
        move |_, _| {
            let draft = state.borrow_mut().open_sheet(None, true);
            edit_requirement(draft);
        }
    });
    window.add_action(&blanket_action);

    let challenges_action = gio::SimpleAction::new("challenges", None);
    challenges_action.connect_activate({
        let state = Rc::clone(&state);
        let refresh_all = Rc::clone(&refresh_all);
        let window = window.clone();
        move |_, _| challenges_dialog::present(&window, &state, &refresh_all)
    });
    window.add_action(&challenges_action);

    let presets_action = gio::SimpleAction::new("presets", None);
    presets_action.connect_activate({
        let state = Rc::clone(&state);
        let user_presets = Rc::clone(&user_presets);
        let refresh_all = Rc::clone(&refresh_all);
        let results = Rc::clone(&results);
        let toasts = toasts.clone();
        let window = window.clone();
        move |_, _| {
            if results.is_running() {
                toasts.add_toast(adw::Toast::new("Stop the search before loading a preset"));
                return;
            }
            presets_dialog::present(&window, &toasts, &state, &user_presets, &refresh_all);
        }
    });
    window.add_action(&presets_action);

    let copy_link_action = gio::SimpleAction::new("copy-link", None);
    copy_link_action.connect_activate({
        let state = Rc::clone(&state);
        let toasts = toasts.clone();
        let window = window.clone();
        move |_, _| {
            let link = state
                .borrow()
                .to_query()
                .and_then(|query| deep_link::encode_link(&query));
            match link {
                Ok(link) => {
                    window.clipboard().set_text(&link);
                    toasts.add_toast(adw::Toast::new("Link copied"));
                }
                Err(message) => toasts.add_toast(adw::Toast::new(&message)),
            }
        }
    });
    window.add_action(&copy_link_action);

    // The query as its JSON document, which the CLI reads and Import from
    // Clipboard accepts back.
    let copy_search_action = gio::SimpleAction::new("copy-search", None);
    copy_search_action.connect_activate({
        let state = Rc::clone(&state);
        let toasts = toasts.clone();
        let window = window.clone();
        move |_, _| {
            let document = state.borrow().to_query().and_then(|query| {
                results_export::encode_query_document(&json_query::encode(&query).to_string())
            });
            match document {
                Ok(document) => {
                    window.clipboard().set_text(&document);
                    toasts.add_toast(adw::Toast::new("Search copied"));
                }
                Err(message) => toasts.add_toast(adw::Toast::new(&message)),
            }
        }
    });
    window.add_action(&copy_search_action);

    // Receives seedseeker:// URIs from the application's `open` handler and
    // loads the carried query into the editor.
    let open_link_action = gio::SimpleAction::new("open-share-link", Some(glib::VariantTy::STRING));
    open_link_action.connect_activate({
        let state = Rc::clone(&state);
        let results = Rc::clone(&results);
        let refresh_all = Rc::clone(&refresh_all);
        let toasts = toasts.clone();
        move |_, parameter| {
            let Some(text) = parameter.and_then(glib::Variant::str) else {
                return;
            };
            if results.is_running() {
                toasts.add_toast(adw::Toast::new("Stop the search before opening a link"));
                return;
            }
            match deep_link::decode_text(text) {
                Ok(query) => {
                    *state.borrow_mut() = AppState::load(&query);
                    refresh_all();
                    toasts.add_toast(adw::Toast::new("Search loaded from link"));
                }
                Err(message) => {
                    toasts.add_toast(adw::Toast::new(&format!("Cannot open link: {message}")));
                }
            }
        }
    });
    window.add_action(&open_link_action);

    let export_action = gio::SimpleAction::new("export-results", None);
    export_action.connect_activate({
        let results = Rc::clone(&results);
        let exported_query = Rc::clone(&exported_query);
        let toasts = toasts.clone();
        let window = window.clone();
        move |_, _| {
            if results.is_running() {
                toasts.add_toast(adw::Toast::new("Stop the search before exporting results"));
                return;
            }
            let codes = results.seed_codes();
            // Export the query snapshot captured when these results were
            // produced, never the live editor state.
            let query = exported_query.borrow().clone();
            let (Some(query), false) = (query, codes.is_empty()) else {
                toasts.add_toast(adw::Toast::new(
                    "Run a search first — there are no results to export yet",
                ));
                return;
            };
            let recipes = results.seed_recipes();
            let contents =
                results_export::encode_recipes(&query, &recipes, env!("CARGO_PKG_VERSION"));
            let dialog = gtk::FileDialog::builder()
                .title("Export Results")
                .initial_name("seed-seeker-results.json")
                .build();
            let toasts = toasts.clone();
            dialog.save(Some(&window), gio::Cancellable::NONE, move |chosen| {
                // Cancelling the dialog is not an error worth reporting.
                let Ok(file) = chosen else { return };
                match file.replace_contents(
                    contents.as_bytes(),
                    None,
                    false,
                    gio::FileCreateFlags::REPLACE_DESTINATION,
                    gio::Cancellable::NONE,
                ) {
                    Ok(_) => toasts.add_toast(adw::Toast::new("Results exported")),
                    Err(error) => {
                        toasts.add_toast(adw::Toast::new(&format!("Export failed: {error}")));
                    }
                }
            });
        }
    });
    window.add_action(&export_action);

    // File and clipboard imports apply exactly the same validated document.
    let import_text: Rc<dyn Fn(&str)> = Rc::new({
        let state = Rc::clone(&state);
        let results = Rc::clone(&results);
        let refresh_all = Rc::clone(&refresh_all);
        let exported_query = Rc::clone(&exported_query);
        let toasts = toasts.clone();
        move |text| {
            // A search may have started while the clipboard or file read was pending.
            if results.is_running() {
                toasts.add_toast(adw::Toast::new("Stop the search before importing results"));
                return;
            }
            match results_export::decode(text) {
                // A copied search restores the query alone, as a link does.
                Ok(imported) if imported.bare_query => {
                    *state.borrow_mut() = AppState::load(&imported.query);
                    refresh_all();
                    toasts.add_toast(adw::Toast::new("Search loaded"));
                }
                Ok(imported) => {
                    let (kept, dropped) =
                        results_export::dedupe_and_cap(&imported.seeds, MAX_RESULTS);
                    *state.borrow_mut() = AppState::load(&imported.query);
                    let codes: Vec<String> = kept.iter().map(|seed| seed.to_code()).collect();
                    // The import becomes the session's Target: the
                    // imported query plus seeds, with no coverage.
                    results.load_imported(&codes, &imported.query, &imported.recipes);
                    exported_query.replace(Some(imported.query));
                    refresh_all();
                    let mut message = format!(
                        "Imported {} seed{}",
                        codes.len(),
                        if codes.len() == 1 { "" } else { "s" },
                    );
                    if dropped > 0 {
                        let _ = std::fmt::Write::write_fmt(
                            &mut message,
                            format_args!(" · {dropped} duplicate or over-limit entries dropped"),
                        );
                    }
                    toasts.add_toast(adw::Toast::new(&message));
                    if let Some(file_version) = imported.shpd_version
                        && file_version != shpd_seedfinder_core::SHPD_VERSION
                    {
                        toasts.add_toast(adw::Toast::new(&format!(
                            "Note: this JSON targets Shattered Pixel Dungeon \
                         v{file_version}; this app targets v{} — seeds may \
                         generate differently",
                            shpd_seedfinder_core::SHPD_VERSION,
                        )));
                    }
                }
                Err(message) => {
                    toasts.add_toast(adw::Toast::new(&format!("Import failed: {message}")));
                }
            }
        }
    });

    let import_action = gio::SimpleAction::new("import-results", None);
    import_action.connect_activate({
        let import_text = Rc::clone(&import_text);
        let results = Rc::clone(&results);
        let toasts = toasts.clone();
        let window = window.clone();
        move |_, _| {
            if results.is_running() {
                toasts.add_toast(adw::Toast::new("Stop the search before importing results"));
                return;
            }
            let dialog = gtk::FileDialog::builder().title("Import Results").build();
            let import_text = Rc::clone(&import_text);
            let toasts = toasts.clone();
            dialog.open(Some(&window), gio::Cancellable::NONE, move |chosen| {
                let Ok(file) = chosen else { return };
                file.load_contents_async(gio::Cancellable::NONE, move |loaded| match loaded {
                    Ok((bytes, _)) => import_text(&String::from_utf8_lossy(&bytes)),
                    Err(error) => {
                        toasts.add_toast(adw::Toast::new(&format!("Import failed: {error}")));
                    }
                });
            });
        }
    });
    window.add_action(&import_action);

    let clipboard_action = gio::SimpleAction::new("import-clipboard", None);
    clipboard_action.connect_activate({
        let results = Rc::clone(&results);
        let toasts = toasts.clone();
        let window = window.clone();
        move |_, _| {
            if results.is_running() {
                toasts.add_toast(adw::Toast::new("Stop the search before importing results"));
                return;
            }
            let import_text = Rc::clone(&import_text);
            let toasts = toasts.clone();
            window
                .clipboard()
                .read_text_async(gio::Cancellable::NONE, move |read| match read {
                    Ok(Some(text)) if !text.trim().is_empty() => import_text(&text),
                    Ok(_) => toasts.add_toast(adw::Toast::new(
                        "The clipboard has no text. Copy results JSON and try again.",
                    )),
                    Err(error) => toasts.add_toast(adw::Toast::new(&format!(
                        "Could not read the clipboard: {error}"
                    ))),
                });
        }
    });
    window.add_action(&clipboard_action);

    let focus_seed_action = gio::SimpleAction::new("focus-seed", None);
    focus_seed_action.connect_activate({
        let detail = Rc::clone(&detail);
        let inner_split = inner_split.clone();
        let outer_split = outer_split.clone();
        move |_, _| {
            outer_split.set_show_content(true);
            inner_split.set_show_content(true);
            detail.focus_entry();
        }
    });
    window.add_action(&focus_seed_action);

    let show_results_action = gio::SimpleAction::new("show-results", None);
    show_results_action.connect_activate({
        let inner_split = inner_split.clone();
        let outer_split = outer_split.clone();
        move |_, _| {
            outer_split.set_show_content(true);
            inner_split.set_show_content(false);
        }
    });
    window.add_action(&show_results_action);
    // Each way on appears only while its split view hides the page it leads to.
    outer_split
        .bind_property("collapsed", &query.results_button, "visible")
        .sync_create()
        .build();
    inner_split
        .bind_property("collapsed", &results.seed_button, "visible")
        .sync_create()
        .build();

    let shortcuts_action = gio::SimpleAction::new("shortcuts", None);
    shortcuts_action.connect_activate({
        let window = window.clone();
        move |_, _| present_shortcuts(&window)
    });
    window.add_action(&shortcuts_action);

    refresh_all();
    window.present();
    update::check_on_startup(&window);
}

fn build_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    let query_section = gio::Menu::new();
    query_section.append(Some("_Presets…"), Some("win.presets"));
    query_section.append(Some("_Challenges…"), Some("win.challenges"));
    query_section.append(Some("Copy _Link"), Some("win.copy-link"));
    query_section.append(Some("Copy _Search"), Some("win.copy-search"));
    menu.append_section(None, &query_section);
    let results_section = gio::Menu::new();
    results_section.append(Some("_Import Results…"), Some("win.import-results"));
    results_section.append(Some("Import from _Clipboard"), Some("win.import-clipboard"));
    results_section.append(Some("_Export Results…"), Some("win.export-results"));
    menu.append_section(None, &results_section);
    let app_section = gio::Menu::new();
    app_section.append(Some("_Keyboard Shortcuts"), Some("win.shortcuts"));
    app_section.append(Some("_About Seed Seeker"), Some("app.about"));
    menu.append_section(None, &app_section);
    menu
}

fn present_shortcuts(window: &adw::ApplicationWindow) {
    let section = adw::ShortcutsSection::new(None);
    for (title, accelerator) in [
        ("Add requirement", "<primary>n"),
        ("Start or stop the search", "<primary>Return"),
        ("Enter a seed code", "<primary>l"),
        ("Scout the next search result", "j"),
        ("Scout the previous search result", "k"),
        ("Challenges", "<primary>comma"),
        ("Keyboard shortcuts", "<primary>question"),
        ("Quit", "<primary>q"),
    ] {
        section.add(adw::ShortcutsItem::new(title, accelerator));
    }
    let dialog = adw::ShortcutsDialog::new();
    dialog.add(section);
    dialog.present(Some(window));
}
