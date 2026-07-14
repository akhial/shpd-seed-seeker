// SPDX-License-Identifier: GPL-3.0-or-later

//! Seed-search page: JSON query editor, streaming session, and live results.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use adw::prelude::*;
use gtk::glib;
use shpd_seedfinder_core::json_query;
use shpd_seedfinder_session::{
    NativeSession, STATE_CANCELLED, STATE_COMPLETED, STATE_FAILED, STATE_RUNNING,
};

use crate::format::group_digits;

const SAMPLE_QUERY: &str = r#"{
  "max_depth": 24,
  "requirements": [
    { "item": "ring_tenacity", "upgrade": 4 }
  ]
}
"#;

const POLL_INTERVAL: Duration = Duration::from_millis(100);
const DRAIN_BATCH: usize = 256;

struct ActiveSearch {
    session: Rc<NativeSession>,
    matches: u64,
    last_tested: u64,
    last_tick: Instant,
    seeds_per_second: f64,
}

#[derive(Default)]
struct SearchState {
    active: RefCell<Option<ActiveSearch>>,
}

struct SearchWidgets {
    query_buffer: gtk::TextBuffer,
    start_button: gtk::Button,
    stop_button: gtk::Button,
    status_label: gtk::Label,
    progress_bar: gtk::ProgressBar,
    results_list: gtk::ListBox,
    results_heading: gtk::Label,
    toasts: adw::ToastOverlay,
}

#[allow(clippy::too_many_lines)] // Widget assembly is declarative and linear.
pub fn build(toasts: &adw::ToastOverlay) -> gtk::Widget {
    let query_view = gtk::TextView::builder()
        .accepts_tab(false)
        .bottom_margin(8)
        .left_margin(8)
        .monospace(true)
        .right_margin(8)
        .top_margin(8)
        .build();
    query_view.buffer().set_text(SAMPLE_QUERY);

    let query_scroller = gtk::ScrolledWindow::builder()
        .child(&query_view)
        .has_frame(true)
        .min_content_height(160)
        .build();

    let start_button = gtk::Button::builder()
        .css_classes(["suggested-action"])
        .label("Start Search")
        .build();
    let stop_button = gtk::Button::builder()
        .css_classes(["destructive-action"])
        .label("Stop")
        .sensitive(false)
        .build();

    let status_label = gtk::Label::builder()
        .css_classes(["dim-label"])
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .hexpand(true)
        .label("Idle")
        .xalign(1.0)
        .build();

    let controls = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build();
    controls.append(&start_button);
    controls.append(&stop_button);
    controls.append(&status_label);

    let progress_bar = gtk::ProgressBar::builder().build();

    let results_heading = gtk::Label::builder()
        .css_classes(["heading"])
        .label("Matches")
        .xalign(0.0)
        .build();

    let results_list = gtk::ListBox::builder()
        .css_classes(["boxed-list"])
        .selection_mode(gtk::SelectionMode::None)
        .build();
    let results_placeholder = gtk::Label::builder()
        .css_classes(["dim-label"])
        .label("Matching seeds appear here while the search runs")
        .margin_bottom(24)
        .margin_top(24)
        .build();
    results_list.set_placeholder(Some(&results_placeholder));

    let results_scroller = gtk::ScrolledWindow::builder()
        .child(&results_list)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();

    let content = gtk::Box::builder()
        .margin_bottom(12)
        .margin_end(12)
        .margin_start(12)
        .margin_top(12)
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .build();
    let query_heading = gtk::Label::builder()
        .css_classes(["heading"])
        .label("Query")
        .xalign(0.0)
        .build();
    content.append(&query_heading);
    content.append(&query_scroller);
    content.append(&controls);
    content.append(&progress_bar);
    content.append(&results_heading);
    content.append(&results_scroller);

    let widgets = Rc::new(SearchWidgets {
        query_buffer: query_view.buffer(),
        start_button,
        stop_button,
        status_label,
        progress_bar,
        results_list,
        results_heading,
        toasts: toasts.clone(),
    });
    let state = Rc::new(SearchState::default());

    widgets.start_button.connect_clicked({
        let widgets = Rc::clone(&widgets);
        let state = Rc::clone(&state);
        move |_| start_search(&widgets, &state)
    });
    widgets.stop_button.connect_clicked({
        let state = Rc::clone(&state);
        move |button| {
            if let Some(active) = state.active.borrow().as_ref() {
                active.session.cancel();
                button.set_sensitive(false);
            }
        }
    });

    adw::Clamp::builder()
        .child(&content)
        .maximum_size(860)
        .build()
        .upcast()
}

fn start_search(widgets: &Rc<SearchWidgets>, state: &Rc<SearchState>) {
    if state.active.borrow().is_some() {
        return;
    }

    let buffer = &widgets.query_buffer;
    let text = buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), false)
        .to_string();
    let query = match json_query::decode(&text) {
        Ok(query) => query,
        Err(message) => {
            widgets.toasts.add_toast(adw::Toast::new(&message));
            return;
        }
    };
    let session = match NativeSession::production(query) {
        Ok(session) => Rc::new(session),
        Err(error) => {
            widgets.toasts.add_toast(adw::Toast::new(&format!(
                "Could not start search: {error:?}"
            )));
            return;
        }
    };

    widgets.results_list.remove_all();
    widgets.results_heading.set_label("Matches");
    widgets.start_button.set_sensitive(false);
    widgets.stop_button.set_sensitive(true);
    widgets.status_label.set_label("Starting…");
    widgets.progress_bar.set_fraction(0.0);
    state.active.replace(Some(ActiveSearch {
        session,
        matches: 0,
        last_tested: 0,
        last_tick: Instant::now(),
        seeds_per_second: 0.0,
    }));

    let widgets = Rc::clone(widgets);
    let state = Rc::clone(state);
    glib::timeout_add_local(POLL_INTERVAL, move || tick(&widgets, &state));
}

fn tick(widgets: &Rc<SearchWidgets>, state: &Rc<SearchState>) -> glib::ControlFlow {
    let mut active_slot = state.active.borrow_mut();
    let Some(active) = active_slot.as_mut() else {
        return glib::ControlFlow::Break;
    };

    drain_matches(widgets, active);

    let status = active.session.status();
    let search_state = status[0];
    let tested = status[1].max(0).unsigned_abs();
    let total = status[2].max(1).unsigned_abs();

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

    widgets
        .progress_bar
        .set_fraction((precise(tested) / precise(total)).clamp(0.0, 1.0));
    widgets
        .results_heading
        .set_label(&format!("Matches ({})", group_digits(active.matches)));

    if search_state == STATE_RUNNING {
        widgets.status_label.set_label(&format!(
            "Tested {} of {} · {} seeds/s",
            group_digits(tested),
            group_digits(total),
            group_digits(round_rate(active.seeds_per_second)),
        ));
        return glib::ControlFlow::Continue;
    }

    // Catch matches that raced the terminal state transition.
    drain_matches(widgets, active);
    widgets
        .results_heading
        .set_label(&format!("Matches ({})", group_digits(active.matches)));

    let summary = match search_state {
        STATE_COMPLETED => format!(
            "Completed · tested {} · {} matches",
            group_digits(tested),
            group_digits(active.matches)
        ),
        STATE_CANCELLED => format!(
            "Stopped · tested {} · {} matches",
            group_digits(tested),
            group_digits(active.matches)
        ),
        STATE_FAILED => {
            let diagnostic = active
                .session
                .take_failure_diagnostic()
                .unwrap_or_else(|| "unknown worker failure".to_owned());
            format!("Search failed: {diagnostic}")
        }
        _ => "Search ended in an unknown state".to_owned(),
    };
    widgets.status_label.set_label(&summary);
    if search_state == STATE_FAILED {
        widgets.toasts.add_toast(adw::Toast::new(&summary));
    }
    widgets.start_button.set_sensitive(true);
    widgets.stop_button.set_sensitive(false);
    *active_slot = None;
    glib::ControlFlow::Break
}

fn drain_matches(widgets: &Rc<SearchWidgets>, active: &mut ActiveSearch) {
    loop {
        let worlds = active.session.drain_worlds(DRAIN_BATCH);
        if worlds.is_empty() {
            break;
        }
        for world in &worlds {
            append_result_row(widgets, &world.seed.to_code());
        }
        active.matches += worlds.len() as u64;
    }
}

fn append_result_row(widgets: &Rc<SearchWidgets>, seed_code: &str) {
    let row = adw::ActionRow::builder()
        .activatable(true)
        .subtitle("Activate to copy")
        .title(seed_code)
        .build();
    row.add_css_class("monospace");
    let toasts = widgets.toasts.clone();
    row.connect_activated(move |row| {
        let seed = row.title();
        row.clipboard().set_text(&seed);
        toasts.add_toast(adw::Toast::new(&format!("Copied {seed}")));
    });
    widgets.results_list.append(&row);
}

// Seed counts stay far below 2^53, so the f64 progress math is exact enough
// for display purposes.
#[allow(clippy::cast_precision_loss)]
const fn precise(value: u64) -> f64 {
    value as f64
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn round_rate(rate: f64) -> u64 {
    if rate.is_finite() && rate >= 0.0 {
        rate.round() as u64
    } else {
        0
    }
}
