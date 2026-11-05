//! Right-click menus: events, empty time in the views, calendars and sets in
//! the sidebar. Each item does what the event popover, its keys or the
//! sidebar already do.

use crate::draw::Slot;
use crate::editor::{self, Start};
use crate::widgets::{self, MenuEntry, danger, item, keyed, separator, sub};
use crate::{actions, details, prefs, quick, window, worker};
use chrono::Local;
use gtk::glib;
use gtk::prelude::*;
use nyx_calendar_core::{CalendarInfo, Event};
use nyx_eds::Response;
use std::cell::RefCell;
use std::rc::Rc;

fn point(x: f64, y: f64) -> gtk::gdk::Rectangle {
    gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)
}

/// An event in any view.
pub fn event(widget: &gtk::Widget, x: f64, y: f64, event: Event) {
    // A reminder in the overlay belongs to Reminders.
    if event.calendar.starts_with("task:") {
        let (w, e) = (widget.clone(), event.clone());
        widgets::popup_menu(widget, x, y, vec![item("Open in Reminders", move || details::show(&w, point(x, y), e.clone()))]);
        return;
    }
    let calendars = window::calendars();
    let calendar = calendars.iter().find(|c| c.uid == event.calendar);
    let read_only = calendar.is_none_or(|c| c.read_only);
    let mut out: Vec<MenuEntry> = Vec::new();
    let (w, e) = (widget.clone(), event.clone());
    out.push(item("Open", move || details::show(&w, point(x, y), e.clone())));
    if !read_only {
        let e = event.clone();
        out.push(keyed("Edit…", "E", move || editor::open(Some(&e), None)));
        let e = event.clone();
        out.push(item("Duplicate…", move || {
            let start = Start { title: e.title.clone(), span: Some(e.span), calendar: Some(e.calendar.clone()), repeat: None };
            editor::open(None, Some(start));
        }));
    }
    if let Some(link) = nyx_calendar_core::plan::meeting_link(&event) {
        out.push(keyed("Join the video call", "J", move || window::open_link(&link)));
    }
    if let Some(link) = nyx_calendar_core::plan::map_link(&event.location) {
        out.push(item("Show on a map", move || window::open_link(&link)));
    }
    if !read_only && event.invitation(&window::me()).is_some() {
        let answers = [("Going", Response::Accepted), ("Maybe", Response::Tentative), ("Not going", Response::Declined)];
        out.push(sub(
            "Answer",
            answers
                .into_iter()
                .map(|(label, response)| {
                    let e = event.clone();
                    item(label, move || actions::respond(&e, response))
                })
                .collect(),
        ));
    }
    if !read_only {
        let targets: Vec<MenuEntry> = calendars
            .iter()
            .filter(|c| !c.read_only && c.uid != event.calendar)
            .map(|c| {
                let (e, uid) = (event.clone(), c.uid.clone());
                item(&format!("{} · {}", c.name, c.group_label()), move || actions::move_to(&e, uid.clone()))
            })
            .collect();
        if !targets.is_empty() {
            out.push(sub("Move to", targets));
        }
    }
    out.push(separator());
    let e = event.clone();
    out.push(item("Export as .ics…", move || actions::export(&e)));
    if let Some(cal) = calendar {
        let uid = cal.uid.clone();
        out.push(item(&format!("Hide {}", cal.name), move || worker::set_selected(uid.clone(), false)));
    }
    if !read_only {
        out.push(separator());
        let e = event.clone();
        out.push(danger("Delete", Some("Delete"), move || actions::delete(&e)));
    }
    widgets::popup_menu(widget, x, y, out);
}

/// Empty time (or a day) in a view.
pub fn empty(widget: &gtk::Widget, x: f64, y: f64, slot: Slot) {
    let day = match slot {
        Slot::At(at) => at.date(),
        Slot::Day(day) => day,
    };
    let w = widget.clone();
    let mut out = vec![
        item("New event here…", move || editor::open(None, Some(editor::start_at(slot)))),
        item("Quick add here…", move || quick::show(&w, point(x, y), Some(slot))),
    ];
    out.push(item(&format!("Show {}", day.format("%A, %B %-d")), move || {
        window::set_view(window::ViewKind::Day);
        window::go_to(day);
    }));
    if day != Local::now().date_naive() {
        out.push(separator());
        out.push(keyed("Go to today", "T", window::go_to_today));
    }
    widgets::popup_menu(widget, x, y, out);
}

/// Colors offered for a calendar (EDS saves them; providers that keep colors sync them).
const COLORS: &[(&str, &str)] = &[
    ("Red", "#e5484d"),
    ("Orange", "#f76b15"),
    ("Yellow", "#ffc53d"),
    ("Green", "#30a46c"),
    ("Teal", "#12a594"),
    ("Blue", "#3e63dd"),
    ("Purple", "#8e4ec6"),
    ("Pink", "#d6409f"),
    ("Gray", "#8b8d98"),
];

/// A card for the sidebar, at `(x, y)` in `anchor`. It hangs off the sidebar,
/// not the row, because the rows are rebuilt while it's open (a new color).
fn card(anchor: &gtk::Widget, x: f64, y: f64) -> Option<(gtk::Popover, gtk::Box)> {
    let nav = window::nav()?;
    let at = anchor.compute_point(&nav, &gtk::graphene::Point::new(x as f32, y as f32))?;
    let popover = gtk::Popover::new();
    popover.add_css_class("menu-popover");
    popover.add_css_class("sidebar-card");
    popover.set_has_arrow(false);
    popover.set_position(gtk::PositionType::Right);
    popover.set_parent(&nav);
    popover.set_pointing_to(Some(&point(at.x() as f64, at.y() as f64)));
    popover.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    let body = gtk::Box::new(gtk::Orientation::Vertical, 2);
    body.add_css_class("calendar-card");
    popover.set_child(Some(&body));
    Some((popover, body))
}

/// The card's title, with a dim line under it.
fn card_title(body: &gtk::Box, title: &str, subtitle: &str) {
    let head = gtk::Box::new(gtk::Orientation::Vertical, 1);
    head.add_css_class("card-head");
    let t = widgets::label(title, "card-title");
    t.set_xalign(0.0);
    t.set_ellipsize(gtk::pango::EllipsizeMode::End);
    t.set_max_width_chars(28);
    head.append(&t);
    if !subtitle.is_empty() {
        let s = widgets::label(subtitle, "card-subtitle");
        s.set_xalign(0.0);
        s.set_ellipsize(gtk::pango::EllipsizeMode::End);
        s.set_max_width_chars(32);
        head.append(&s);
    }
    body.append(&head);
}

fn card_divider(body: &gtk::Box) {
    let line = gtk::Separator::new(gtk::Orientation::Horizontal);
    line.add_css_class("card-divider");
    body.append(&line);
}

/// One of the card's actions; it closes the card.
fn card_action(body: &gtk::Box, popover: &gtk::Popover, icon: &str, label: &str, key: Option<&str>, danger: bool, run: impl Fn() + 'static) {
    let button = gtk::Button::new();
    button.add_css_class("flat");
    button.add_css_class("card-action");
    if danger {
        button.add_css_class("card-danger");
    }
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    content.append(&gtk::Image::from_icon_name(icon));
    let text = widgets::label(label, "");
    text.set_xalign(0.0);
    text.set_hexpand(true);
    content.append(&text);
    if let Some(key) = key {
        content.append(&widgets::label(key, "card-key"));
    }
    button.set_child(Some(&content));
    let p = popover.clone();
    button.connect_clicked(move |_| {
        p.popdown();
        run();
    });
    body.append(&button);
}

/// The colors as round swatches; the calendar's own has a ring. Picking one
/// keeps the card open so the change shows.
fn swatch_row(cal: &CalendarInfo) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    row.add_css_class("swatch-row");
    let current = Rc::new(RefCell::new(cal.color.clone().unwrap_or_default().to_lowercase()));
    let areas: Rc<RefCell<Vec<gtk::DrawingArea>>> = Rc::default();
    for (name, hex) in COLORS {
        let area = gtk::DrawingArea::new();
        area.set_content_width(22);
        area.set_content_height(22);
        let (c, ring, mine) = (nyx_theme::Rgb::hex(hex), current.clone(), hex.to_lowercase());
        area.set_draw_func(move |_, cr, w, h| {
            let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0);
            let picked = *ring.borrow() == mine;
            cr.set_source_rgb(c.0, c.1, c.2);
            cr.arc(cx, cy, if picked { 7.0 } else { 9.0 }, 0.0, std::f64::consts::TAU);
            let _ = cr.fill();
            if picked {
                cr.set_line_width(2.0);
                cr.arc(cx, cy, 10.0, 0.0, std::f64::consts::TAU);
                let _ = cr.stroke();
            }
        });
        let button = gtk::Button::new();
        button.add_css_class("flat");
        button.add_css_class("swatch");
        button.set_tooltip_text(Some(name));
        button.update_property(&[gtk::accessible::Property::Label(name)]);
        button.set_child(Some(&area));
        let (uid, hex, current, all) = (cal.uid.clone(), hex.to_string(), current.clone(), areas.clone());
        button.connect_clicked(move |_| {
            *current.borrow_mut() = hex.to_lowercase();
            for a in all.borrow().iter() {
                a.queue_draw();
            }
            worker::set_color(uid.clone(), hex.clone(), |r| {
                if let Err(e) = r {
                    window::toast(&e);
                }
            });
        });
        areas.borrow_mut().push(area);
        row.append(&button);
    }
    row
}

/// A calendar in the sidebar: its color and what you can do with it.
pub fn attach_calendar(button: &gtk::Button, cal: &CalendarInfo) {
    let cal = cal.clone();
    widgets::on_right_click(button, move |w, x, y| {
        let Some((popover, body)) = card(w, x, y) else { return };
        let group = if window::is_imported(&cal) { "Imported (not syncing)" } else { cal.group_label() };
        let mut about = vec![group.to_string()];
        if cal.read_only {
            about.push("Read only".into());
        }
        if cal.offline {
            about.push("Offline".into());
        }
        card_title(&body, &cal.name, &about.join(" · "));
        body.append(&swatch_row(&cal));
        card_divider(&body);
        let (uid, selected) = (cal.uid.clone(), cal.selected);
        let (icon, verb) = if selected { ("view-conceal-symbolic", "Hide") } else { ("view-reveal-symbolic", "Show") };
        card_action(&body, &popover, icon, verb, None, false, move || worker::set_selected(uid.clone(), !selected));
        let only = cal.uid.clone();
        card_action(&body, &popover, "zoom-fit-best-symbolic", "Show only this calendar", None, false, move || {
            window::show_set(Some(&prefs::CalendarSet { name: String::new(), calendars: vec![only.clone()] }));
        });
        card_action(&body, &popover, "view-app-grid-symbolic", "Show all calendars", None, false, || window::show_set(None));
        if !cal.read_only {
            let uid = cal.uid.clone();
            card_action(&body, &popover, "list-add-symbolic", "New event in this calendar…", Some("N"), false, move || {
                editor::open(None, Some(Start { calendar: Some(uid.clone()), ..Start::default() }));
            });
        }
        card_action(&body, &popover, "view-list-symbolic", "Save shown calendars as a set…", None, false, window::save_set);
        card_divider(&body);
        let (uid, name) = (cal.uid.clone(), cal.name.clone());
        card_action(&body, &popover, "system-shutdown-symbolic", "Turn off", None, true, move || turn_off(&uid, &name));
        popover.popup();
    });
}

/// An account's heading in the sidebar: its calendars together.
pub fn attach_group(row: &gtk::Box, title: &str, items: &[CalendarInfo]) {
    let (title, items) = (title.to_string(), items.to_vec());
    widgets::on_right_click(row, move |w, x, y| {
        let Some((popover, body)) = card(w, x, y) else { return };
        let count = if items.len() == 1 { "1 calendar".to_string() } else { format!("{} calendars", items.len()) };
        card_title(&body, &title, &count);
        let uids: Vec<String> = items.iter().map(|c| c.uid.clone()).collect();
        let u = uids.clone();
        card_action(&body, &popover, "view-reveal-symbolic", "Show all", None, false, move || {
            for uid in &u {
                worker::set_selected(uid.clone(), true);
            }
        });
        let u = uids.clone();
        card_action(&body, &popover, "view-conceal-symbolic", "Hide all", None, false, move || {
            for uid in &u {
                worker::set_selected(uid.clone(), false);
            }
        });
        card_action(&body, &popover, "view-app-grid-symbolic", "Show all calendars", None, false, || window::show_set(None));
        card_action(&body, &popover, "view-list-symbolic", "Save shown calendars as a set…", None, false, window::save_set);
        card_divider(&body);
        let (u, name) = (uids.clone(), title.clone());
        card_action(&body, &popover, "system-shutdown-symbolic", "Turn all off", None, true, move || turn_off_all(&u, &name));
        popover.popup();
    });
}

/// Switches several calendars off (not loaded or synced, in every app), with Undo.
fn turn_off_all(uids: &[String], name: &str) {
    for uid in uids {
        worker::set_enabled(uid.clone(), false, |r| {
            if let Err(e) = r {
                window::toast(&e);
            }
        });
    }
    let uids = uids.to_vec();
    window::toast_undo(&format!("{name}'s calendars are off. Turn them back on in Settings → Calendars."), move || {
        for uid in &uids {
            worker::set_enabled(uid.clone(), true, |_| {});
        }
    });
}

/// Switches a calendar off (not loaded or synced, in every app), with Undo.
pub fn turn_off(uid: &str, name: &str) {
    let (uid, name) = (uid.to_string(), name.to_string());
    worker::set_enabled(uid.clone(), false, move |r| match r {
        Ok(()) => window::toast_undo(&format!("{name} is off. Turn it back on in Settings → Calendars."), move || {
            worker::set_enabled(uid.clone(), true, |_| {})
        }),
        Err(e) => window::toast(&e),
    });
}

/// A saved set in the sidebar.
pub fn attach_set(button: &gtk::Button, set: &prefs::CalendarSet) {
    let set = set.clone();
    widgets::on_right_click(button, move |w, x, y| {
        let (show, name) = (set.clone(), set.name.clone());
        widgets::popup_menu(
            w,
            x,
            y,
            vec![
                item("Show", move || window::show_set(Some(&show))),
                separator(),
                danger("Delete set", None, move || {
                    prefs::update(|p| p.sets.retain(|s| s.name != name));
                    window::refresh_nav();
                    window::toast(&format!("Deleted the set {name}."));
                }),
            ],
        );
    });
}
