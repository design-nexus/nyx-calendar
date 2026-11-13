//! The day and week views: day headers and all-day lanes on top, a scrolling
//! grid of hours below, one column per day.

use crate::draw::{self, Calendars, Hooks, Slot, Style};
use chrono::{Datelike, Local, NaiveDate, NaiveTime, Timelike};
use gtk::prelude::*;
use nyx_calendar_core::{Event, Span, WeekLayout};
use std::cell::RefCell;
use std::rc::Rc;

const GUTTER: f64 = 56.0;
const HOUR: f64 = 48.0;
const DAY_HEADER: f64 = 52.0;
/// An all-day bar and the gap under it.
const LANE: f64 = 25.0;
const BAR: f64 = 22.0;

type Hit = (f64, f64, f64, f64, usize);

#[derive(Default)]
struct Data {
    days: Vec<NaiveDate>,
    events: Vec<Event>,
    layout: WeekLayout,
    calendars: Calendars,
    grid_hits: Vec<Hit>,
    header_hits: Vec<Hit>,
}

#[derive(Clone)]
pub struct DaysView {
    pub root: gtk::Box,
    header: gtk::DrawingArea,
    grid: gtk::DrawingArea,
    scroll: gtk::ScrolledWindow,
    data: Rc<RefCell<Data>>,
    hooks: Rc<Hooks>,
}

impl DaysView {
    pub fn new(hooks: Rc<Hooks>) -> Self {
        let data = Rc::new(RefCell::new(Data::default()));
        let header = gtk::DrawingArea::new();
        header.add_css_class("week-header");
        header.set_content_height((DAY_HEADER + LANE + 6.0) as i32);
        let grid = gtk::DrawingArea::new();
        grid.set_content_height((24.0 * HOUR) as i32);
        grid.set_vexpand(true);
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::External)
            .vexpand(true)
            .child(&grid)
            .build();
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&header);
        root.append(&scroll);

        let d = data.clone();
        header.set_draw_func(move |area, cr, w, _| draw_header(area, cr, w as f64, &mut d.borrow_mut()));
        let d = data.clone();
        grid.set_draw_func(move |area, cr, w, _| draw_grid(area, cr, w as f64, &mut d.borrow_mut()));

        for (area, top) in [(&header, true), (&grid, false)] {
            let menu_hooks = hooks.clone();
            let click = gtk::GestureClick::new();
            let (d, hooks, target) = (data.clone(), hooks.clone(), area.clone());
            click.connect_pressed(move |gesture, n, x, y| {
                let d = d.borrow();
                let hits = if top { &d.header_hits } else { &d.grid_hits };
                let hit =
                    hits.iter().rev().find(|(hx, hy, hw, hh, _)| x >= *hx && x <= hx + hw && y >= *hy && y <= hy + hh).copied();
                let col = column_width(target.width() as f64, d.days.len());
                let day = ((x - GUTTER) / col).floor();
                match hit {
                    Some((hx, hy, hw, hh, index)) if n == 1 => {
                        gesture.set_state(gtk::EventSequenceState::Claimed);
                        let event = d.events[index].clone();
                        drop(d);
                        (hooks.event)(target.upcast_ref(), draw::rect(hx, hy, hw, hh), event);
                    }
                    None if n == 2 && x > GUTTER && day >= 0.0 && (day as usize) < d.days.len() => {
                        let date = d.days[day as usize];
                        let (slot, rect) = if top {
                            (Slot::Day(date), draw::rect(GUTTER + day * col, DAY_HEADER, col, BAR))
                        } else {
                            // Half-hour steps.
                            let minutes = ((y / HOUR * 2.0).floor() * 30.0).clamp(0.0, 23.5 * 60.0) as u32;
                            let at = date.and_time(NaiveTime::from_hms_opt(minutes / 60, minutes % 60, 0).unwrap_or_default());
                            (Slot::At(at), draw::rect(GUTTER + day * col, minutes as f64 / 60.0 * HOUR, col, HOUR))
                        };
                        drop(d);
                        (hooks.empty)(target.upcast_ref(), rect, slot);
                    }
                    None if n == 1 && top && y < DAY_HEADER && day >= 0.0 && (day as usize) < d.days.len() => {
                        let date = d.days[day as usize];
                        drop(d);
                        (hooks.day)(date);
                    }
                    _ => {}
                }
            });
            area.add_controller(click);

            let menu = gtk::GestureClick::new();
            menu.set_button(gtk::gdk::BUTTON_SECONDARY);
            let (d, hooks, target) = (data.clone(), menu_hooks, area.clone());
            menu.connect_pressed(move |gesture, _, x, y| {
                let d = d.borrow();
                let hits = if top { &d.header_hits } else { &d.grid_hits };
                let hit = hits.iter().rev().find(|(hx, hy, hw, hh, _)| x >= *hx && x <= hx + hw && y >= *hy && y <= hy + hh);
                if let Some(&(.., index)) = hit {
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                    let event = d.events[index].clone();
                    drop(d);
                    (hooks.event_menu)(target.upcast_ref(), x, y, event);
                    return;
                }
                let col = column_width(target.width() as f64, d.days.len());
                let day = ((x - GUTTER) / col).floor();
                if x <= GUTTER || day < 0.0 || day as usize >= d.days.len() {
                    return;
                }
                let date = d.days[day as usize];
                let slot = if top {
                    Slot::Day(date)
                } else {
                    let minutes = ((y / HOUR * 2.0).floor() * 30.0).clamp(0.0, 23.5 * 60.0) as u32;
                    Slot::At(date.and_time(NaiveTime::from_hms_opt(minutes / 60, minutes % 60, 0).unwrap_or_default()))
                };
                drop(d);
                gesture.set_state(gtk::EventSequenceState::Claimed);
                (hooks.empty_menu)(target.upcast_ref(), x, y, slot);
            });
            area.add_controller(menu);
        }

        let view = Self { root, header, grid, scroll, data, hooks };
        let v = view.clone();
        nyx_theme::subscribe(&view.root, move || v.redraw());
        let grid = view.grid.downgrade();
        glib::timeout_add_seconds_local(60, move || match grid.upgrade() {
            Some(g) => {
                g.queue_draw();
                glib::ControlFlow::Continue
            }
            None => glib::ControlFlow::Break,
        });
        view
    }

    pub fn set(&self, days: Vec<NaiveDate>, events: &[Event], calendars: Calendars) {
        {
            let mut d = self.data.borrow_mut();
            let shown: Vec<Event> =
                events.iter().filter(|e| days.iter().any(|day| (e.days().0..=e.days().1).contains(day))).cloned().collect();
            d.layout = WeekLayout::new(&days, &shown);
            d.days = days;
            d.events = shown;
            d.calendars = calendars;
            let lanes = d.layout.lanes.max(1) as f64;
            self.header.set_content_height((DAY_HEADER + lanes * LANE + 4.0) as i32);
        }
        self.redraw();
    }

    pub fn scroll_to_hour(&self, hour: u32) {
        let adj = self.scroll.vadjustment();
        // A little above the hour, so its label is not cut in half.
        let target = (hour as f64 * HOUR - 12.0).max(0.0);
        glib::idle_add_local_once(move || adj.set_value(target.min((adj.upper() - adj.page_size()).max(0.0))));
    }

    /// Opens the first event `pick` likes as if it were clicked; false if none is drawn.
    pub fn open_where(&self, pick: impl Fn(&Event) -> bool) -> bool {
        let found = {
            let d = self.data.borrow();
            [(&self.grid, &d.grid_hits), (&self.header, &d.header_hits)].into_iter().find_map(|(area, hits)| {
                let &(x, y, w, h, index) = hits.iter().find(|hit| pick(&d.events[hit.4]))?;
                Some((area.clone(), draw::rect(x, y, w, h), d.events[index].clone()))
            })
        };
        let Some((area, rect, event)) = found else { return false };
        (self.hooks.event)(area.upcast_ref(), rect, event);
        true
    }

    pub fn redraw(&self) {
        self.header.queue_draw();
        self.grid.queue_draw();
    }
}

fn column_width(width: f64, days: usize) -> f64 {
    ((width - GUTTER) / days.max(1) as f64).max(10.0)
}

fn draw_header(area: &gtk::DrawingArea, cr: &gtk::cairo::Context, width: f64, d: &mut Data) {
    let c = draw::colors();
    let col = column_width(width, d.days.len());
    let today = Local::now().date_naive();
    let single = d.days.len() == 1;
    d.header_hits.clear();
    // "SUN 8" on one baseline; today's name in the accent and its date on an accent tab.
    let name_style = Style { scale: 0.82, bold: true, ..draw::NORMAL };
    let date_style = Style { scale: 1.36, bold: true, ..draw::NORMAL };
    let tertiary = c.tertiary;
    let mid = DAY_HEADER / 2.0 + 2.0;
    for (i, day) in d.days.iter().enumerate() {
        let x = GUTTER + i as f64 * col + 10.0;
        let is_today = *day == today;
        let name = if single { day.format("%A") } else { day.format("%a") }.to_string().to_uppercase();
        let (name_w, name_h) = draw::text_size(area, &name, name_style);
        let date = day.day().to_string();
        let (date_w, date_h) = draw::text_size(area, &date, date_style);
        // Both on the date's baseline, which sits a little under the middle.
        let base = mid + date_h * 0.3;
        draw::set(cr, if is_today { c.accent } else { tertiary });
        draw::text(area, cr, &name, x, base - name_h * 0.78, col - 20.0, name_style);
        let dx = x + name_w + 6.0;
        if is_today {
            draw::set(cr, c.accent);
            draw::rounded(cr, dx, base - date_h * 0.78 - 1.0, date_w + 16.0, date_h + 2.0, 8.0);
            let _ = cr.fill();
            draw::set(cr, c.on_accent);
            draw::text(area, cr, &date, dx + 8.0, base - date_h * 0.78, date_w + 2.0, date_style);
        } else {
            draw::set(cr, c.text);
            draw::text(area, cr, &date, dx, base - date_h * 0.78, (col - 20.0 - name_w - 6.0).max(1.0), date_style);
        }
    }
    let lanes = d.layout.lanes.max(1) as f64;
    let label_style = Style { scale: 0.77, ..draw::NORMAL };
    let (label_w, label_h) = draw::text_size(area, "All day", label_style);
    draw::set(cr, tertiary);
    draw::text(
        area,
        cr,
        "All day",
        (GUTTER - 10.0 - label_w).max(2.0),
        DAY_HEADER + (lanes * LANE - 3.0 - label_h) / 2.0,
        GUTTER - 10.0,
        label_style,
    );

    for bar in d.layout.all_day.clone() {
        let event = &d.events[bar.event];
        let color = draw::event_color(&d.calendars, event, c.accent);
        let x = GUTTER + bar.first_day as f64 * col + 3.0;
        let w = (bar.last_day - bar.first_day + 1) as f64 * col - 6.0;
        let y = DAY_HEADER + bar.lane as f64 * LANE;
        let (text, tx) = draw::all_day_bar(cr, &c, color, event, x, y, w, BAR);
        draw::set(cr, text);
        let style = Style { scale: 0.85, bold: true, strike: event.cancelled, ..draw::NORMAL };
        let title = event.title.trim_start_matches("☐ ");
        let (_, th) = draw::text_size(area, title, style);
        draw::text(area, cr, title, tx, y + (BAR - th) / 2.0, x + w - tx - 6.0, style);
        d.header_hits.push((x, y, w, BAR, bar.event));
    }
}

fn draw_grid(area: &gtk::DrawingArea, cr: &gtk::cairo::Context, width: f64, d: &mut Data) {
    let c = draw::colors();
    let col = column_width(width, d.days.len());
    let now = Local::now().naive_local();
    let tertiary = c.tertiary;
    draw::grid_line(cr, &c);
    for i in 0..d.days.len() {
        let x = (GUTTER + i as f64 * col).floor() + 0.5;
        cr.move_to(x, 0.0);
        cr.line_to(x, 24.0 * HOUR);
    }
    for h in 0..24 {
        let y = (h as f64 * HOUR).floor() + 0.5;
        cr.move_to(GUTTER, y);
        cr.line_to(width, y);
    }
    let _ = cr.stroke();
    let hour_style = Style { scale: 0.77, ..draw::NORMAL };
    draw::set(cr, tertiary);
    for h in 1..24 {
        let label = draw::hour_label(h);
        let (w, th) = draw::text_size(area, &label, hour_style);
        draw::text(area, cr, &label, (GUTTER - 10.0 - w).max(2.0), h as f64 * HOUR - th / 2.0, GUTTER - 10.0, hour_style);
    }
    d.grid_hits.clear();
    let title_style = |strike| Style { scale: 0.88, bold: true, strike, ..draw::NORMAL };
    let detail_style = Style { scale: 0.82, ..draw::NORMAL };
    for p in d.layout.timed.clone() {
        let event = &d.events[p.event];
        let color = draw::event_color(&d.calendars, event, c.accent);
        let slot = col / p.columns as f64;
        let x = GUTTER + p.day as f64 * col + p.column as f64 * slot + 3.0;
        let w = slot - 7.0;
        let y = p.top as f64 / 60.0 * HOUR + 1.0;
        let h = (p.bottom - p.top) as f64 / 60.0 * HOUR - 2.0;
        let (start, end) = match event.span {
            Span::Timed { start, end } => (start, end),
            Span::AllDay { first, .. } => (first.and_time(NaiveTime::MIN), first.and_time(NaiveTime::MIN)),
        };
        let (title_color, detail_color) = draw::timed_block(cr, &c, color, event, end <= now, x, y, w, h);
        cr.save().ok();
        cr.rectangle(x, y, w, h);
        cr.clip();
        let (tx, tw) = (x + 9.0, w - 14.0);
        if end - start <= chrono::Duration::minutes(30) {
            // One line: the title, then when it starts.
            let style = title_style(event.cancelled);
            let (_, th) = draw::text_size(area, &event.title, style);
            let top = y + ((h - th) / 2.0).max(0.0);
            let when = draw::bare_time(start.time());
            let (when_w, _) = draw::text_size(area, &when, detail_style);
            let (title_w, _) = draw::text_size(area, &event.title, style);
            let room = (tw - when_w - 6.0).max(tw * 0.5);
            draw::set(cr, title_color);
            draw::text(area, cr, &event.title, tx, top, room, style);
            if title_w + 6.0 + when_w <= tw {
                draw::set(cr, detail_color);
                draw::text(area, cr, &when, tx + title_w + 6.0, top + 1.0, when_w + 2.0, detail_style);
            }
        } else {
            let top = y + 4.0;
            draw::set(cr, title_color);
            let mut used = draw::text(area, cr, &event.title, tx, top, tw, title_style(event.cancelled));
            draw::set(cr, detail_color);
            for line in [draw::short_range(start, end), event.location.clone()] {
                if line.is_empty() || top + used + 14.0 > y + h {
                    break;
                }
                used += draw::text(area, cr, &line, tx, top + used, tw, detail_style);
            }
        }
        cr.restore().ok();
        d.grid_hits.push((x, y, w, h, p.event));
    }

    if let Some(i) = d.days.iter().position(|day| *day == now.date()) {
        let y = (now.hour() as f64 + now.minute() as f64 / 60.0) * HOUR;
        let x = GUTTER + i as f64 * col;
        draw::set(cr, c.accent);
        cr.set_line_width(2.0);
        cr.move_to(x, y);
        cr.line_to(x + col, y);
        let _ = cr.stroke();
        cr.arc(x, y, 4.0, 0.0, std::f64::consts::TAU);
        let _ = cr.fill();
    }
}
