// SPDX-License-Identifier: GPL-3.0-or-later

//! On-demand map disclosure with native GTK gestures and an adaptive dialog.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant};

use adw::prelude::*;
use gtk::{gdk, glib};
use shpd_seedfinder_core::catalog::{ItemId, item};
use shpd_seedfinder_core::challenges::Challenges;
use shpd_seedfinder_core::level_map::{self, LevelMap};
use shpd_seedfinder_core::seed::DungeonSeed;
use shpd_seedfinder_core::trinkets::trinket_order;

use crate::level_map_render::Renderer;
use crate::sprites::{self, ItemSprite};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MapProfile {
    pub seed: DungeonSeed,
    pub challenges: Challenges,
    pub trinket: Option<ItemId>,
}

type MapKey = (MapProfile, u8, u8);
type MapCache = Mutex<VecDeque<(MapKey, Arc<LevelMap>)>>;
static CACHE: OnceLock<MapCache> = OnceLock::new();

fn load(key: MapKey) -> Result<Arc<LevelMap>, String> {
    let cache = CACHE.get_or_init(Mutex::default);
    {
        let mut cache = cache.lock().map_err(|e| e.to_string())?;
        if let Some(index) = cache.iter().position(|(cached, _)| *cached == key) {
            let entry = cache.remove(index).unwrap();
            let map = Arc::clone(&entry.1);
            cache.push_back(entry);
            return Ok(map);
        }
    }
    let (profile, depth, branch) = key;
    let map = std::panic::catch_unwind(|| {
        level_map::generate_level_map_in_branch(
            profile.seed,
            depth,
            branch,
            profile.challenges,
            profile.trinket,
        )
    })
    .map_err(|_| "Map generation failed. Please retry.".to_owned())?
    .map_err(|e| e.to_string())?;
    let map = Arc::new(map);
    let mut cache = cache.lock().map_err(|e| e.to_string())?;
    cache.push_back((key, Arc::clone(&map)));
    while cache.len() > 6 {
        cache.pop_front();
    }
    Ok(map)
}

pub struct FloorMapView {
    pub widget: gtk::Box,
    content: gtk::Box,
    area: gtk::DrawingArea,
    stack: gtk::Stack,
    status: gtk::Label,
    retry: gtk::Button,
    title: gtk::Label,
    branches: gtk::Box,
    secrets_button: gtk::ToggleButton,
    previous: gtk::Button,
    next: gtk::Button,
    expand: gtk::Button,
    trinkets: gtk::Box,
    profile: Cell<MapProfile>,
    depth: Cell<u8>,
    initial_depth: u8,
    branch: Cell<u8>,
    floors: Vec<u8>,
    map: RefCell<Option<Arc<LevelMap>>>,
    renderer: RefCell<Option<Renderer>>,
    generation: Cell<u64>,
    viewport_location: Cell<Option<(DungeonSeed, Challenges, u8, u8)>>,
    secrets: Cell<bool>,
    zoom: Cell<f64>,
    pan: Cell<(f64, f64)>,
    pointer: Cell<Option<(f64, f64)>>,
    dragging: Cell<bool>,
    start: Cell<Instant>,
    elapsed: Cell<u64>,
    dialog: RefCell<Option<adw::Dialog>>,
    on_trinket: RefCell<Rc<dyn Fn(ItemId)>>,
    on_close: RefCell<Option<Rc<dyn Fn()>>>,
}

impl FloorMapView {
    #[allow(clippy::too_many_lines)] // Declarative native widget and gesture assembly.
    pub fn new(
        profile: MapProfile,
        depth: u8,
        floors: Vec<u8>,
        on_trinket: impl Fn(ItemId) + 'static,
    ) -> Rc<Self> {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
        widget.append(&content);
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let title = gtk::Label::builder()
            .hexpand(true)
            .xalign(0.0)
            .css_classes(["heading"])
            .build();
        let previous = button("go-previous-symbolic", "Previous floor (K)");
        let next = button("go-next-symbolic", "Next floor (J)");
        previous.set_visible(false);
        next.set_visible(false);
        let expand = button("view-fullscreen-symbolic", "Expand map");
        toolbar.append(&title);
        toolbar.append(&previous);
        toolbar.append(&next);
        toolbar.append(&expand);
        content.append(&toolbar);
        let trinkets = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .halign(gtk::Align::Center)
            .visible(false)
            .build();
        content.append(&trinkets);
        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let branches = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .css_classes(["linked"])
            .hexpand(true)
            .build();
        let secrets_button = gtk::ToggleButton::builder()
            .label("Secrets")
            .tooltip_text("Reveal secret rooms, doors and traps")
            .build();
        let minus = button("zoom-out-symbolic", "Zoom out (−)");
        let plus = button("zoom-in-symbolic", "Zoom in (+)");
        let fit = button("zoom-fit-best-symbolic", "Fit map (0)");
        content.append(&branches);
        secrets_button.set_hexpand(true);
        secrets_button.set_halign(gtk::Align::Start);
        controls.append(&secrets_button);
        controls.append(&minus);
        controls.append(&plus);
        controls.append(&fit);
        content.append(&controls);
        let area = gtk::DrawingArea::builder()
            .content_height(350)
            .hexpand(true)
            .vexpand(true)
            .focusable(true)
            .overflow(gtk::Overflow::Hidden)
            .build();
        area.update_property(&[gtk::accessible::Property::Label(
            "Floor map. Drag to pan, scroll or pinch to zoom; zero to fit.",
        )]);
        let status_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .build();
        let status = gtk::Label::builder()
            .label("Loading map…")
            .wrap(true)
            .build();
        let retry = gtk::Button::with_label("Retry");
        status_box.append(&status);
        status_box.append(&retry);
        let stack = gtk::Stack::builder()
            .vexpand(true)
            .height_request(350)
            .build();
        stack.add_named(&area, Some("map"));
        stack.add_named(&status_box, Some("status"));
        content.append(&stack);
        let view = Rc::new(Self {
            widget,
            content,
            area,
            stack,
            status,
            retry,
            title,
            branches,
            secrets_button,
            previous,
            next,
            expand,
            trinkets,
            profile: Cell::new(profile),
            depth: Cell::new(depth),
            initial_depth: depth,
            branch: Cell::new(0),
            floors,
            map: RefCell::new(None),
            renderer: RefCell::new(None),
            generation: Cell::new(0),
            viewport_location: Cell::new(None),
            secrets: Cell::new(false),
            zoom: Cell::new(1.0),
            pan: Cell::new((0.0, 0.0)),
            pointer: Cell::new(None),
            dragging: Cell::new(false),
            start: Cell::new(Instant::now()),
            elapsed: Cell::new(0),
            dialog: RefCell::new(None),
            on_trinket: RefCell::new(Rc::new(on_trinket)),
            on_close: RefCell::new(None),
        });
        view.area.set_draw_func({
            let weak = Rc::downgrade(&view);
            move |_, context, w, h| {
                context.set_source_rgb(0.0, 0.0, 0.0);
                let _ = context.paint();
                let Some(view) = weak.upgrade() else {
                    return;
                };
                let map = view.map.borrow();
                let Some(map) = map.as_ref() else {
                    return;
                };
                let fit = (f64::from(w) / f64::from(map.width * 16))
                    .min(f64::from(h) / f64::from(map.height * 16));
                let scale = fit * view.zoom.get();
                let (px, py) = view.constrain_pan(w, h, scale, map);
                context.translate(
                    (f64::from(w) - f64::from(map.width * 16) * scale) / 2.0 + px,
                    (f64::from(h) - f64::from(map.height * 16) * scale) / 2.0 + py,
                );
                context.scale(scale, scale);
                if let Some(renderer) = view.renderer.borrow_mut().as_mut()
                    && let Err(error) =
                        renderer.render(map, view.secrets.get(), view.elapsed.get(), context)
                {
                    view.status.set_label(&error);
                    view.retry.set_visible(true);
                    view.stack.set_visible_child_name("status");
                }
            }
        });
        view.area.set_has_tooltip(true);
        view.area.connect_query_tooltip({
            let weak = Rc::downgrade(&view);
            move |_, x, y, keyboard, tooltip| {
                let Some(view) = weak.upgrade() else {
                    return false;
                };
                if view.dragging.get() {
                    return false;
                }
                let map = view.map.borrow();
                let Some(map) = map.as_ref() else {
                    return false;
                };
                let w = f64::from(view.area.width());
                let h = f64::from(view.area.height());
                let scale = (w / f64::from(map.width * 16)).min(h / f64::from(map.height * 16))
                    * view.zoom.get();
                if scale <= 0.0 {
                    return false;
                }
                let (pan_x, pan_y) =
                    view.constrain_pan(view.area.width(), view.area.height(), scale, map);
                let (x, y) = if keyboard {
                    view.pointer
                        .get()
                        .map_or((w / 2.0, h / 2.0), |(x, y)| (x + w / 2.0, y + h / 2.0))
                } else {
                    (f64::from(x), f64::from(y))
                };
                let col = ((x - w / 2.0 - pan_x) / scale / 16.0 + f64::from(map.width) / 2.0)
                    .floor() as i32;
                let row = ((y - h / 2.0 - pan_y) / scale / 16.0 + f64::from(map.height) / 2.0)
                    .floor() as i32;
                if col < 0 || row < 0 || col >= map.width || row >= map.height {
                    return false;
                }
                let cell = (row * map.width + col) as usize;
                let Some(tip) = map
                    .item_tooltips()
                    .into_iter()
                    .find(|tip| tip.cell == cell && (view.secrets.get() || !tip.hidden))
                else {
                    return false;
                };
                let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
                if !tip.label.is_empty() {
                    body.append(
                        &gtk::Label::builder()
                            .label(&tip.label)
                            .xalign(0.0)
                            .css_classes(["dim-label", "caption"])
                            .build(),
                    );
                }
                for item in tip.items {
                    let title = if item.quantity > 1 {
                        format!("{}  ×{}", item.name, item.quantity)
                    } else {
                        item.name
                    };
                    let heading = gtk::Box::new(gtk::Orientation::Horizontal, 10);
                    heading.append(&sprites::map_item_image(item.image, 32));
                    heading.append(
                        &gtk::Label::builder()
                            .label(&title)
                            .xalign(0.0)
                            .wrap(true)
                            .max_width_chars(42)
                            .css_classes(["heading"])
                            .build(),
                    );
                    body.append(&heading);
                    if !item.deterministic {
                        body.append(
                            &gtk::Label::builder()
                                .label("Varies with play")
                                .xalign(0.0)
                                .css_classes(["dim-label", "caption"])
                                .build(),
                        );
                    }
                    if !item.description.is_empty() {
                        body.append(
                            &gtk::Label::builder()
                                .label(item.description)
                                .xalign(0.0)
                                .wrap(true)
                                .max_width_chars(42)
                                .build(),
                        );
                    }
                }
                tooltip.set_custom(Some(&body));
                tooltip.set_tip_area(&gdk::Rectangle::new(
                    (w / 2.0 + pan_x + f64::from(col * 16 - map.width * 8) * scale).floor() as i32,
                    (h / 2.0 + pan_y + f64::from(row * 16 - map.height * 8) * scale).floor() as i32,
                    (16.0 * scale).ceil() as i32,
                    (16.0 * scale).ceil() as i32,
                ));
                true
            }
        });
        view.area.add_tick_callback({
            let weak = Rc::downgrade(&view);
            move |area, _| {
                let Some(view) = weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                // GTK stops frame callbacks while unmapped; also skip scrolled-away maps.
                let visible = area
                    .root()
                    .and_then(|root| root.dynamic_cast::<gtk::Widget>().ok())
                    .and_then(|root| {
                        area.compute_bounds(&root)
                            .map(|b| b.y() + b.height() > 0.0 && b.y() < root.height() as f32)
                    })
                    .unwrap_or(false);
                let animations = gtk::Settings::default()
                    .is_none_or(|settings| settings.is_gtk_enable_animations());
                let elapsed = if animations {
                    view.start.get().elapsed().as_millis() as u64
                } else {
                    0
                };
                if visible && view.elapsed.replace(elapsed) != elapsed {
                    area.queue_draw();
                }
                glib::ControlFlow::Continue
            }
        });
        for (button, delta) in [(&view.previous, -1), (&view.next, 1)] {
            button.connect_clicked({
                let weak = Rc::downgrade(&view);
                move |_| {
                    if let Some(view) = weak.upgrade() {
                        view.navigate(delta);
                    }
                }
            });
        }
        view.retry.connect_clicked({
            let weak = Rc::downgrade(&view);
            move |_| {
                if let Some(view) = weak.upgrade() {
                    view.request();
                }
            }
        });
        view.expand.connect_clicked({
            let weak = Rc::downgrade(&view);
            move |_| {
                if let Some(view) = weak.upgrade() {
                    view.expand();
                }
            }
        });
        view.secrets_button.connect_toggled({
            let weak = Rc::downgrade(&view);
            move |button| {
                if let Some(view) = weak.upgrade() {
                    view.hide_item();
                    view.secrets.set(button.is_active());
                    button.set_tooltip_text(Some(if button.is_active() {
                        "Hide secrets"
                    } else {
                        "Reveal secret rooms, doors and traps"
                    }));
                    view.area.queue_draw();
                }
            }
        });
        for (button, factor) in [(minus, 0.8), (plus, 1.25), (fit, 0.0)] {
            button.connect_clicked({
                let weak = Rc::downgrade(&view);
                move |_| {
                    if let Some(view) = weak.upgrade() {
                        view.zoom_by(factor);
                    }
                }
            });
        }
        let drag = gtk::GestureDrag::new();
        let drag_origin = Rc::new(Cell::new((0.0, 0.0)));
        drag.connect_drag_begin({
            let weak = Rc::downgrade(&view);
            let origin = Rc::clone(&drag_origin);
            move |_, _, _| {
                if let Some(view) = weak.upgrade() {
                    view.hide_item();
                    view.dragging.set(true);
                    origin.set(view.pan.get());
                    view.area.grab_focus();
                }
            }
        });
        drag.connect_drag_update({
            let weak = Rc::downgrade(&view);
            move |_, x, y| {
                if let Some(view) = weak.upgrade() {
                    let (ox, oy) = drag_origin.get();
                    view.pan.set((ox + x, oy + y));
                    view.area.queue_draw();
                }
            }
        });
        drag.connect_drag_end({
            let weak = Rc::downgrade(&view);
            move |_, _, _| {
                if let Some(view) = weak.upgrade() {
                    view.dragging.set(false);
                }
            }
        });
        view.area.add_controller(drag);
        let motion = gtk::EventControllerMotion::new();
        motion.connect_motion({
            let weak = Rc::downgrade(&view);
            move |_, x, y| {
                if let Some(view) = weak.upgrade() {
                    view.pointer.set(Some((
                        x - f64::from(view.area.width()) / 2.0,
                        y - f64::from(view.area.height()) / 2.0,
                    )));
                }
            }
        });
        motion.connect_leave({
            let weak = Rc::downgrade(&view);
            move |_| {
                if let Some(view) = weak.upgrade() {
                    view.pointer.set(None);
                }
            }
        });
        view.area.add_controller(motion);
        let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        scroll.connect_scroll({
            let weak = Rc::downgrade(&view);
            move |_, _, dy| {
                if let Some(view) = weak.upgrade() {
                    view.zoom_at((-dy * 0.12).exp(), view.pointer.get().unwrap_or((0.0, 0.0)));
                }
                glib::Propagation::Stop
            }
        });
        view.area.add_controller(scroll);
        let pinch = gtk::GestureZoom::new();
        let initial_zoom = Rc::new(Cell::new(1.0));
        pinch.connect_begin({
            let weak = Rc::downgrade(&view);
            let initial = Rc::clone(&initial_zoom);
            move |_, _| {
                if let Some(view) = weak.upgrade() {
                    initial.set(view.zoom.get());
                }
            }
        });
        pinch.connect_scale_changed({
            let weak = Rc::downgrade(&view);
            move |gesture, scale| {
                if let Some(view) = weak.upgrade() {
                    let anchor = gesture.bounding_box_center().map_or((0.0, 0.0), |(x, y)| {
                        (
                            x - f64::from(view.area.width()) / 2.0,
                            y - f64::from(view.area.height()) / 2.0,
                        )
                    });
                    view.zoom_at(initial_zoom.get() * scale / view.zoom.get(), anchor);
                }
            }
        });
        view.area.add_controller(pinch);
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed({
            let weak = Rc::downgrade(&view);
            move |_, key, _, modifiers| {
                if modifiers.intersects(
                    gdk::ModifierType::CONTROL_MASK
                        | gdk::ModifierType::ALT_MASK
                        | gdk::ModifierType::SUPER_MASK
                        | gdk::ModifierType::META_MASK,
                ) {
                    return glib::Propagation::Proceed;
                }
                let Some(view) = weak.upgrade() else {
                    return glib::Propagation::Proceed;
                };
                match key {
                    gdk::Key::plus | gdk::Key::equal => view.zoom_by(1.25),
                    gdk::Key::minus => view.zoom_by(0.8),
                    gdk::Key::_0 => view.zoom_by(0.0),
                    gdk::Key::j | gdk::Key::J if view.dialog.borrow().is_some() => view.navigate(1),
                    gdk::Key::k | gdk::Key::K if view.dialog.borrow().is_some() => {
                        view.navigate(-1);
                    }
                    gdk::Key::Escape if view.dialog.borrow().is_none() => {
                        let handler = view.on_close.borrow().clone();
                        if let Some(handler) = handler {
                            handler();
                        }
                    }
                    gdk::Key::Left | gdk::Key::Right | gdk::Key::Up | gdk::Key::Down => {
                        let (x, y) = view.pan.get();
                        view.pan.set((
                            x + if key == gdk::Key::Left {
                                32.0
                            } else if key == gdk::Key::Right {
                                -32.0
                            } else {
                                0.0
                            },
                            y + if key == gdk::Key::Up {
                                32.0
                            } else if key == gdk::Key::Down {
                                -32.0
                            } else {
                                0.0
                            },
                        ));
                        view.area.queue_draw();
                    }
                    _ => return glib::Propagation::Proceed,
                }
                glib::Propagation::Stop
            }
        });
        view.content.add_controller(keys);
        view.update_trinkets();
        view.request();
        view
    }

    fn constrain_pan(&self, w: i32, h: i32, scale: f64, map: &LevelMap) -> (f64, f64) {
        let (x, y) = self.pan.get();
        let max_x = ((f64::from(map.width * 16) * scale - f64::from(w)) / 2.0).max(0.0);
        let max_y = ((f64::from(map.height * 16) * scale - f64::from(h)) / 2.0).max(0.0);
        let pan = (x.clamp(-max_x, max_x), y.clamp(-max_y, max_y));
        self.pan.set(pan);
        pan
    }

    fn zoom_by(&self, factor: f64) {
        self.zoom_at(factor, (0.0, 0.0));
    }

    fn hide_item(&self) {
        self.area.set_has_tooltip(false);
        self.area.set_has_tooltip(true);
    }

    fn zoom_at(&self, factor: f64, anchor: (f64, f64)) {
        self.hide_item();
        let previous = self.zoom.get();
        self.zoom.set(if factor == 0.0 {
            1.0
        } else {
            (self.zoom.get() * factor).clamp(1.0, 12.0)
        });
        if factor == 0.0 {
            self.pan.set((0.0, 0.0));
        } else {
            let ratio = self.zoom.get() / previous;
            let pan = self.pan.get();
            self.pan.set((
                anchor.0 - (anchor.0 - pan.0) * ratio,
                anchor.1 - (anchor.1 - pan.1) * ratio,
            ));
        }
        self.area.queue_draw();
    }

    pub fn profile(&self) -> MapProfile {
        self.profile.get()
    }

    pub fn set_trinket_handler(&self, handler: impl Fn(ItemId) + 'static) {
        self.on_trinket.replace(Rc::new(handler));
    }

    pub fn set_close_handler(&self, handler: impl Fn() + 'static) {
        self.on_close.replace(Some(Rc::new(handler)));
    }

    pub fn focus(&self) {
        self.area.grab_focus();
    }

    pub fn update_profile(self: &Rc<Self>, profile: MapProfile) {
        let previous = self.profile.replace(profile);
        if previous != profile {
            if previous.seed != profile.seed || previous.challenges != profile.challenges {
                self.branch.set(0);
            }
            self.update_trinkets();
            self.request();
        }
    }

    fn update_trinkets(self: &Rc<Self>) {
        while let Some(child) = self.trinkets.first_child() {
            self.trinkets.remove(&child);
        }
        for id in &trinket_order(self.profile.get().seed)[..4] {
            let button = gtk::ToggleButton::builder()
                .active(self.profile.get().trinket == Some(*id))
                .child(&sprites::item_image_sized(
                    ItemSprite::from_catalog(item(*id)),
                    None,
                    24,
                ))
                .tooltip_text(item(*id).name)
                .css_classes(["flat"])
                .build();
            button.update_property(&[gtk::accessible::Property::Label(item(*id).name)]);
            button.connect_clicked({
                let weak = Rc::downgrade(self);
                let id = *id;
                move |_| {
                    if let Some(view) = weak.upgrade() {
                        let handler = Rc::clone(&view.on_trinket.borrow());
                        handler(id);
                    }
                }
            });
            self.trinkets.append(&button);
        }
    }

    fn navigate(self: &Rc<Self>, delta: isize) {
        let Some(index) = self
            .floors
            .iter()
            .position(|depth| *depth == self.depth.get())
        else {
            return;
        };
        if let Some(depth) = index
            .checked_add_signed(delta)
            .and_then(|i| self.floors.get(i))
        {
            self.depth.set(*depth);
            self.branch.set(0);
            self.request();
        }
    }

    #[allow(clippy::too_many_lines)] // One request's asynchronous state and native controls.
    fn request(self: &Rc<Self>) {
        self.hide_item();
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        self.map.replace(None);
        self.renderer.replace(None);
        let profile = self.profile.get();
        let location = (
            profile.seed,
            profile.challenges,
            self.depth.get(),
            self.branch.get(),
        );
        if self.viewport_location.replace(Some(location)) != Some(location) {
            self.zoom_by(0.0);
        }
        self.status.set_label("Loading map…");
        self.retry.set_visible(false);
        self.stack.set_visible_child_name("status");
        self.secrets_button.set_sensitive(false);
        if self.branch.get() == 0 {
            while let Some(child) = self.branches.first_child() {
                self.branches.remove(&child);
            }
        }
        self.title.set_label(&format!(
            "Floor {} · {}",
            self.depth.get(),
            crate::state::region(self.depth.get())
        ));
        let index = self
            .floors
            .iter()
            .position(|d| *d == self.depth.get())
            .unwrap_or(0);
        self.previous.set_sensitive(index > 0);
        self.next.set_sensitive(index + 1 < self.floors.len());
        let key = (self.profile.get(), self.depth.get(), self.branch.get());
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let _ = sender.send(load((key.0, key.1, 0)).and_then(|main| {
                let map = if key.2 != 0 && main.branches.iter().any(|area| area.branch == key.2) {
                    load(key)?
                } else {
                    Arc::clone(&main)
                };
                Ok((main, map))
            }));
        });
        let weak = Rc::downgrade(self);
        glib::timeout_add_local(Duration::from_millis(20), move || {
            let Some(view) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            if generation != view.generation.get() {
                return glib::ControlFlow::Break;
            }
            let result = match receiver.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Err("Map worker stopped. Please retry.".into())
                }
            };
            match result
                .and_then(|(main, map)| Renderer::new(&map).map(|renderer| (main, map, renderer)))
            {
                Ok((main, map, renderer)) => {
                    if view.branch.replace(map.kind.branch()) != map.kind.branch() {
                        view.viewport_location.set(Some((
                            key.0.seed,
                            key.0.challenges,
                            key.1,
                            map.kind.branch(),
                        )));
                        view.zoom_by(0.0);
                    }
                    view.secrets_button.set_sensitive(
                        !map.secret_rooms.is_empty()
                            || map.traps.iter().any(|t| t.hidden)
                            || map
                                .terrain
                                .contains(&shpd_seedfinder_core::geometry::terrain::SECRET_DOOR),
                    );
                    while let Some(child) = view.branches.first_child() {
                        view.branches.remove(&child);
                    }
                    {
                        for (branch, label) in
                            std::iter::once((0, "Main")).chain(main.branches.iter().map(|b| {
                                (
                                    b.branch,
                                    if b.kind == level_map::MapKind::ImpVault {
                                        "Imp Vault"
                                    } else {
                                        "Blacksmith Mine"
                                    },
                                )
                            }))
                        {
                            if main.branches.is_empty() {
                                break;
                            }
                            let button = gtk::ToggleButton::builder()
                                .label(label)
                                .active(branch == 0)
                                .build();
                            button.connect_clicked({
                                let weak = Rc::downgrade(&view);
                                move |_| {
                                    if let Some(view) = weak.upgrade() {
                                        view.branch.set(branch);
                                        view.request();
                                    }
                                }
                            });
                            view.branches.append(&button);
                        }
                    }
                    // Keep branch toggles coherent after loading either side.
                    let mut child = view.branches.first_child();
                    let mut index = 0;
                    while let Some(widget) = child {
                        child = widget.next_sibling();
                        if let Ok(button) = widget.downcast::<gtk::ToggleButton>() {
                            button.set_active(index == view.branch.get());
                        }
                        index += 1;
                    }
                    view.map.replace(Some(map));
                    view.renderer.replace(Some(renderer));
                    view.start.set(Instant::now());
                    view.elapsed.set(0);
                    view.stack.set_visible_child_name("map");
                    view.area.queue_draw();
                }
                Err(error) => {
                    view.status.set_label(&error);
                    view.retry.set_visible(true);
                }
            }
            glib::ControlFlow::Break
        });
    }

    fn expand(self: &Rc<Self>) {
        if self.dialog.borrow().is_some() {
            return;
        }
        let dialog = adw::Dialog::builder()
            .title("Floor map")
            .content_width(1100)
            .content_height(800)
            .build();
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&adw::HeaderBar::new());
        self.widget.remove(&self.content);
        toolbar.set_content(Some(&self.content));
        dialog.set_child(Some(&toolbar));
        self.expand.set_visible(false);
        self.previous.set_visible(true);
        self.next.set_visible(true);
        self.trinkets.set_visible(true);
        dialog.connect_closed({
            let weak = Rc::downgrade(self);
            move |_| {
                if let Some(view) = weak.upgrade() {
                    toolbar.set_content(gtk::Widget::NONE);
                    view.widget.append(&view.content);
                    view.expand.set_visible(true);
                    view.previous.set_visible(false);
                    view.next.set_visible(false);
                    view.trinkets.set_visible(false);
                    view.dialog.replace(None);
                    if view.depth.get() != view.initial_depth {
                        view.depth.set(view.initial_depth);
                        view.branch.set(0);
                        view.request();
                    }
                    view.expand.grab_focus();
                }
            }
        });
        dialog.present(Some(&self.widget));
        self.dialog.replace(Some(dialog));
        self.area.grab_focus();
    }

    pub fn close(&self) {
        let dialog = self.dialog.borrow().clone();
        if let Some(dialog) = dialog {
            dialog.close();
        }
    }
}

fn button(icon: &str, label: &str) -> gtk::Button {
    let button = gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(label)
        .css_classes(["flat"])
        .build();
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    button
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a GTK display"]
    #[allow(clippy::float_cmp)] // Exact viewport state must survive scene reloads unchanged.
    fn maps_load_expand_navigate_and_keep_the_floor_after_a_trinket_change() {
        adw::init().unwrap();
        gtk::gio::resources_register_include!("dev.seedseeker.SeedSeeker.gresource").unwrap();
        crate::load_stylesheet();
        let seed = DungeonSeed::from_code("AAA-AAA-AAA").unwrap();
        let profile = MapProfile {
            seed,
            challenges: Challenges::NONE,
            trinket: None,
        };
        let view = FloorMapView::new(profile, 9, level_map::SUPPORTED_DEPTHS.to_vec(), |_| {});
        let window = gtk::Window::builder()
            .default_width(480)
            .default_height(650)
            .child(&view.widget)
            .build();
        window.present();
        let settle = || {
            let context = glib::MainContext::default();
            let deadline = Instant::now() + Duration::from_secs(15);
            while view.map.borrow().is_none() && Instant::now() < deadline {
                while context.pending() {
                    context.iteration(false);
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            for _ in 0..25 {
                while context.pending() {
                    context.iteration(false);
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            assert_eq!(
                view.stack.visible_child_name().as_deref(),
                Some("map"),
                "{}",
                view.status.text()
            );
        };
        settle();
        assert!(!view.previous.is_visible());
        assert!(!view.next.is_visible());
        view.expand();
        assert!(view.previous.is_visible());
        view.navigate(1);
        settle();
        assert_eq!(view.depth.get(), 11); // Unsupported floor 10 is skipped.
        assert!(view.dialog.borrow().is_some());
        view.zoom.set(2.0);
        view.pan.set((10.0, -10.0));
        view.update_profile(MapProfile {
            trinket: Some(trinket_order(seed)[0]),
            ..profile
        });
        settle();
        assert_eq!(view.depth.get(), 11);
        assert_eq!(
            view.map.borrow().as_ref().unwrap().selected_trinket,
            Some(trinket_order(seed)[0])
        );
        assert!(view.dialog.borrow().is_some());
        assert_eq!(view.zoom.get(), 2.0);
        assert_eq!(view.pan.get(), (10.0, -10.0));
        view.request(); // Retrying the same location also retains the viewport.
        settle();
        assert_eq!(view.zoom.get(), 2.0);
        assert_eq!(view.pan.get(), (10.0, -10.0));
        view.secrets_button.set_active(true);
        view.zoom.set(2.0);
        view.pan.set((10.0, -10.0));
        view.zoom_at(2.0, (40.0, 20.0));
        assert_eq!(view.pan.get(), (-20.0, -40.0)); // Keep the anchor's map point fixed.
        view.zoom_by(2.0);
        view.pan.set((10_000.0, -10_000.0));
        settle();
        assert!(view.pan.get().0 < 10_000.0);
        view.close();
        settle();
        assert_eq!(view.depth.get(), 9);
        assert_eq!(view.zoom.get(), 1.0);
        assert_eq!(view.pan.get(), (0.0, 0.0));
        assert!(view.dialog.borrow().is_none());
        assert_eq!(
            view.content.parent().as_ref(),
            Some(view.widget.upcast_ref())
        );
        // Exercise complete v3 art, emitters and native controls in a saved review image.
        let snapshot = gtk::Snapshot::new();
        gtk::WidgetPaintable::new(Some(&window)).snapshot(
            &snapshot,
            f64::from(window.width()),
            f64::from(window.height()),
        );
        if let Some(node) = snapshot.to_node() {
            let texture = window.renderer().unwrap().render_texture(&node, None);
            let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../target/gtk-native/map-previews");
            std::fs::create_dir_all(&output).unwrap();
            texture.save_to_png(output.join("inline-map.png")).unwrap();
        }
        window.close();
    }
}
