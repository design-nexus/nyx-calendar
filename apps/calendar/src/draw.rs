//! Drawing helpers shared by the views: theme colors, shapes, text, times.

use crate::prefs;
use chrono::{NaiveDateTime, NaiveTime, Timelike};
use gtk::prelude::*;
use nyx_calendar_core::{Event, Span};
use nyx_theme::Rgb;
use std::collections::HashMap;

/// A calendar's name and color, by uid.
pub type Calendars = HashMap<String, (String, Option<String>)>;

/// Where a click on empty space landed.
#[derive(Clone, Copy, Debug)]
pub enum Slot {
    At(NaiveDateTime),
    Day(chrono::NaiveDate),
}

/// A callback given the widget and rectangle to point a popover at.
pub type At<T> = Box<dyn Fn(&gtk::Widget, gtk::gdk::Rectangle, T)>;

/// What views report to the window.
pub struct Hooks {
    /// An event was clicked.
    pub event: At<Event>,
    /// Empty space was double-clicked.
    pub empty: At<Slot>,
    /// A day's number (or "+N more") was clicked.
    pub day: Box<dyn Fn(chrono::NaiveDate)>,
    /// An event was right-clicked, at a point in the widget.
    pub event_menu: Menu<Event>,
    /// Empty space was right-clicked.
    pub empty_menu: Menu<Slot>,
}

/// A callback given the widget and the point a menu opens at.
pub type Menu<T> = Box<dyn Fn(&gtk::Widget, f64, f64, T)>;

pub struct Colors {
    pub bg: Rgb,
    pub text: Rgb,
    pub dim: Rgb,
    /// Hints and labels, quieter than `dim`.
    pub tertiary: Rgb,
    pub accent: Rgb,
    pub on_accent: Rgb,
}

pub fn colors() -> Colors {
    let p = nyx_theme::palette();
    let bg = Rgb::hex(&p.bg);
    Colors {
        bg,
        text: Rgb::hex(&p.text),
        dim: Rgb::hex(&p.dim_text),
        tertiary: Rgb::hex(&nyx_theme::look().text_tertiary),
        accent: Rgb::hex(&p.accent),
        on_accent: if p.light { Rgb(1.0, 1.0, 1.0) } else { bg },
    }
}

pub fn set(cr: &gtk::cairo::Context, c: Rgb) {
    cr.set_source_rgb(c.0, c.1, c.2);
}

pub fn set_a(cr: &gtk::cairo::Context, c: Rgb, a: f64) {
    cr.set_source_rgba(c.0, c.1, c.2, a);
}

pub fn rounded(cr: &gtk::cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    cr.arc(x + r, y + h - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
    cr.arc(x + r, y + r, r, std::f64::consts::PI, 3.0 * std::f64::consts::FRAC_PI_2);
    cr.close_path();
}

/// Text options for [`text`].
#[derive(Clone, Copy, Default)]
pub struct Style {
    pub scale: f64,
    pub bold: bool,
    pub strike: bool,
    /// Center within the width.
    pub center: bool,
}

pub const NORMAL: Style = Style { scale: 1.0, bold: false, strike: false, center: false };

/// Draws ellipsized text in a box of width `w`; returns its height.
pub fn text(area: &impl IsA<gtk::Widget>, cr: &gtk::cairo::Context, s: &str, x: f64, y: f64, w: f64, style: Style) -> f64 {
    let layout = layout(area, s, style);
    layout.set_width((w.max(1.0) * gtk::pango::SCALE as f64) as i32);
    layout.set_ellipsize(gtk::pango::EllipsizeMode::End);
    if style.center {
        layout.set_alignment(gtk::pango::Alignment::Center);
    }
    cr.move_to(x, y);
    pangocairo::functions::show_layout(cr, &layout);
    layout.pixel_size().1 as f64
}

/// The width and height `text` would take without a width limit.
pub fn text_size(area: &impl IsA<gtk::Widget>, s: &str, style: Style) -> (f64, f64) {
    let (w, h) = layout(area, s, style).pixel_size();
    (w as f64, h as f64)
}

fn layout(area: &impl IsA<gtk::Widget>, s: &str, style: Style) -> gtk::pango::Layout {
    let layout = area.create_pango_layout(Some(s));
    let attrs = gtk::pango::AttrList::new();
    attrs.insert(gtk::pango::AttrFloat::new_scale(if style.scale == 0.0 { 1.0 } else { style.scale }));
    if style.bold {
        attrs.insert(gtk::pango::AttrInt::new_weight(gtk::pango::Weight::Semibold));
    }
    if style.strike {
        attrs.insert(gtk::pango::AttrInt::new_strikethrough(true));
    }
    layout.set_attributes(Some(&attrs));
    layout
}

/// Draws one ellipsized line of Pango markup.
#[allow(clippy::too_many_arguments)]
pub fn markup(
    area: &impl IsA<gtk::Widget>,
    cr: &gtk::cairo::Context,
    markup: &str,
    x: f64,
    y: f64,
    w: f64,
    scale: f64,
    strike: bool,
) {
    let layout = area.create_pango_layout(None);
    layout.set_markup(markup);
    let attrs = layout.attributes().unwrap_or_default();
    attrs.insert(gtk::pango::AttrFloat::new_scale(scale));
    if strike {
        attrs.insert(gtk::pango::AttrInt::new_strikethrough(true));
    }
    layout.set_attributes(Some(&attrs));
    layout.set_width((w.max(1.0) * gtk::pango::SCALE as f64) as i32);
    layout.set_ellipsize(gtk::pango::EllipsizeMode::End);
    cr.move_to(x, y);
    pangocairo::functions::show_layout(cr, &layout);
}

/// An event as Nyx 2.0 draws it: the calendar's colour at a fifth over the background,
/// a faint inner edge in the colour, rounded corners. Returns the colours for its text:
/// the title tinted toward the calendar's colour, and the time a little quieter.
#[allow(clippy::too_many_arguments)]
pub fn event_block(cr: &gtk::cairo::Context, c: &Colors, color: Rgb, cancelled: bool, x: f64, y: f64, w: f64, h: f64, r: f64) -> (Rgb, Rgb) {
    set(cr, c.bg.mix(color, if cancelled { 0.08 } else { 0.20 }));
    rounded(cr, x, y, w, h, r);
    let _ = cr.fill();
    set_a(cr, color, if cancelled { 0.16 } else { 0.28 });
    cr.set_line_width(1.0);
    rounded(cr, x + 0.5, y + 0.5, w - 1.0, h - 1.0, (r - 0.5).max(0.0));
    let _ = cr.stroke();
    let title = if cancelled { c.dim } else { c.text.mix(color, 0.45) };
    (title, title.mix(c.bg, 0.18))
}

/// A timed event in the day and week grid: the calendar's colour at a fifth over the
/// background with a 3 px stripe of it down the left; the open event solid. Past events
/// fade toward the background. Returns the colours for its title and its details.
#[allow(clippy::too_many_arguments)]
pub fn timed_block(cr: &gtk::cairo::Context, c: &Colors, color: Rgb, event: &Event, past: bool, x: f64, y: f64, w: f64, h: f64) -> (Rgb, Rgb) {
    const R: f64 = 7.0;
    if is_selected(event) {
        set(cr, color);
        rounded(cr, x, y, w, h, R);
        let _ = cr.fill();
        let white = Rgb(1.0, 1.0, 1.0);
        return (white, white.mix(color, 0.15));
    }
    let fade = |rgb: Rgb| if past { rgb.mix(c.bg, 0.38) } else { rgb };
    set(cr, fade(c.bg.mix(color, if event.cancelled { 0.08 } else { 0.20 })));
    rounded(cr, x, y, w, h, R);
    let _ = cr.fill();
    cr.save().ok();
    rounded(cr, x, y, w, h, R);
    cr.clip();
    set(cr, fade(if event.cancelled { color.mix(c.bg, 0.5) } else { color }));
    cr.rectangle(x, y, 3.0, h);
    let _ = cr.fill();
    cr.restore().ok();
    let title = if event.cancelled { c.dim } else { c.text.mix(color, 0.30) };
    (fade(title), fade(c.dim))
}

/// An all-day bar: the calendar's colour at a fifth, its text toward the colour. A
/// reminder from the overlay is an outline instead, with a box before its title.
/// Returns the text colour and where the text starts.
#[allow(clippy::too_many_arguments)]
pub fn all_day_bar(cr: &gtk::cairo::Context, c: &Colors, color: Rgb, event: &Event, x: f64, y: f64, w: f64, h: f64) -> (Rgb, f64) {
    const R: f64 = 6.0;
    let text = if event.cancelled { c.dim } else { color.mix(c.text, 0.25) };
    if is_selected(event) {
        set(cr, color);
        rounded(cr, x, y, w, h, R);
        let _ = cr.fill();
        return (Rgb(1.0, 1.0, 1.0), x + 8.0);
    }
    if event.calendar.starts_with("task:") {
        set_a(cr, color, 0.45);
        cr.set_line_width(1.0);
        rounded(cr, x + 0.5, y + 0.5, w - 1.0, h - 1.0, R - 0.5);
        let _ = cr.stroke();
        set(cr, color);
        cr.set_line_width(1.5);
        rounded(cr, x + 8.75, y + (h - 10.0) / 2.0 + 0.75, 8.5, 8.5, 2.5);
        let _ = cr.stroke();
        return (text, x + 24.0);
    }
    set(cr, c.bg.mix(color, if event.cancelled { 0.08 } else { 0.20 }));
    rounded(cr, x, y, w, h, R);
    let _ = cr.fill();
    (text, x + 8.0)
}

/// One occurrence: its calendar, uid, recurrence id and when.
type Occurrence = (String, String, Option<String>, Span);

thread_local! {
    /// The event whose popover is open.
    static SELECTED: std::cell::RefCell<Option<Occurrence>> = const { std::cell::RefCell::new(None) };
}

/// Marks `event` as the one open in a popover (`None` when it closes).
pub fn select(event: Option<&Event>) {
    SELECTED.with(|s| *s.borrow_mut() = event.map(|e| (e.calendar.clone(), e.uid.clone(), e.rid.clone(), e.span)));
}

pub fn is_selected(event: &Event) -> bool {
    SELECTED.with(|s| {
        s.borrow().as_ref().is_some_and(|(cal, uid, rid, span)| {
            *cal == event.calendar && *uid == event.uid && *rid == event.rid && *span == event.span
        })
    })
}

/// The grid's lines: the text colour, barely there.
pub fn grid_line(cr: &gtk::cairo::Context, c: &Colors) {
    set_a(cr, c.text, 0.07);
    cr.set_line_width(1.0);
}

pub fn event_color(calendars: &Calendars, event: &Event, fallback: Rgb) -> Rgb {
    calendars.get(&event.calendar).and_then(|(_, c)| c.as_deref()).map(Rgb::hex).unwrap_or(fallback)
}

pub fn rect(x: f64, y: f64, w: f64, h: f64) -> gtk::gdk::Rectangle {
    gtk::gdk::Rectangle::new(x as i32, y as i32, w.max(1.0) as i32, h.max(1.0) as i32)
}

pub fn time_text(t: NaiveTime) -> String {
    if prefs::twelve_hour() {
        let h = t.hour() % 12;
        let h = if h == 0 { 12 } else { h };
        let m = if t.minute() == 0 { String::new() } else { format!(":{:02}", t.minute()) };
        format!("{h}{m} {}", if t.hour() < 12 { "AM" } else { "PM" })
    } else {
        t.format("%H:%M").to_string()
    }
}

pub fn hour_label(h: u32) -> String {
    if !prefs::twelve_hour() {
        return format!("{h:02}:00");
    }
    match h {
        0 => "12 AM".into(),
        12 => "Noon".into(),
        h if h < 12 => format!("{h} AM"),
        h => format!("{} PM", h - 12),
    }
}

pub fn time_range(start: NaiveDateTime, end: NaiveDateTime) -> String {
    if start == end {
        return time_text(start.time());
    }
    format!("{} – {}", time_text(start.time()), time_text(end.time()))
}

/// A time without its AM or PM: "9", "1:30"; 24-hour clocks keep "09:00".
pub fn bare_time(t: NaiveTime) -> String {
    if !prefs::twelve_hour() {
        return t.format("%H:%M").to_string();
    }
    let h = t.hour() % 12;
    let h = if h == 0 { 12 } else { h };
    if t.minute() == 0 { h.to_string() } else { format!("{h}:{:02}", t.minute()) }
}

/// A range as a block in the grid says it: "10–11:30 AM", "11:30 AM–1 PM".
pub fn short_range(start: NaiveDateTime, end: NaiveDateTime) -> String {
    if start == end {
        return time_text(start.time());
    }
    if !prefs::twelve_hour() {
        return format!("{}–{}", bare_time(start.time()), bare_time(end.time()));
    }
    let half = |t: NaiveTime| if t.hour() < 12 { "AM" } else { "PM" };
    if half(start.time()) == half(end.time()) && start.date() == end.date() {
        format!("{}–{} {}", bare_time(start.time()), bare_time(end.time()), half(end.time()))
    } else {
        format!("{}–{}", time_text(start.time()), time_text(end.time()))
    }
}

/// "Wed, Oct 21 · 2 – 3 PM", "Dec 24 – 25", ...
pub fn when_text(span: Span) -> String {
    match span {
        Span::AllDay { first, last } if first == last => first.format("%A, %B %-d").to_string(),
        Span::AllDay { first, last } => format!("{} – {}", first.format("%a, %b %-d"), last.format("%a, %b %-d")),
        Span::Timed { start, end } if start.date() == end.date() => {
            format!("{} · {}", start.format("%A, %B %-d"), time_range(start, end))
        }
        Span::Timed { start, end } => format!(
            "{} {} – {} {}",
            start.format("%a, %b %-d"),
            time_text(start.time()),
            end.format("%a, %b %-d"),
            time_text(end.time())
        ),
    }
}
