//! The sidebar's month: pick a day, see where the views are and which days are busy.

use crate::draw::{self, Style};
use crate::prefs;
use crate::widgets;
use chrono::{Datelike, Local, Months, NaiveDate};
use gtk::prelude::*;
use nyx_calendar_core::month_weeks;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

const CELL_H: f64 = 26.0;
const HEAD_H: f64 = 20.0;

struct Data {
    month: NaiveDate,
    /// The days the main view shows.
    range: (NaiveDate, NaiveDate),
    /// Days with something on, marked with a dot.
    busy: HashSet<NaiveDate>,
}

#[derive(Clone)]
pub struct MiniCalendar {
    pub root: gtk::Box,
    title: gtk::Label,
    area: gtk::DrawingArea,
    data: Rc<RefCell<Data>>,
    /// Told when another month comes into view, to find its busy days.
    on_month: Rc<RefCell<Option<Hook>>>,
}

type Hook = Box<dyn Fn()>;

impl MiniCalendar {
    pub fn new(on_pick: impl Fn(NaiveDate) + 'static) -> Self {
        let today = Local::now().date_naive();
        let data = Rc::new(RefCell::new(Data {
            month: today.with_day(1).unwrap_or(today),
            range: (today, today),
            busy: HashSet::new(),
        }));
        let root = gtk::Box::new(gtk::Orientation::Vertical, 2);
        root.add_css_class("mini-calendar");
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let title = widgets::label("", "mini-title");
        title.set_hexpand(true);
        title.set_xalign(0.0);
        let prev = widgets::icon_button("go-previous-symbolic", "Previous month");
        let next = widgets::icon_button("go-next-symbolic", "Next month");
        for b in [&prev, &next] {
            b.add_css_class("flat");
            b.add_css_class("mini-nav");
        }
        head.append(&title);
        head.append(&prev);
        head.append(&next);
        root.append(&head);
        let area = gtk::DrawingArea::new();
        area.set_content_height((HEAD_H + 6.0 * CELL_H) as i32);
        root.append(&area);

        let d = data.clone();
        area.set_draw_func(move |area, cr, w, _| draw_mini(area, cr, w as f64, &d.borrow()));
        let click = gtk::GestureClick::new();
        let (d, target) = (data.clone(), area.clone());
        click.connect_released(move |_, _, x, y| {
            let rows = month_weeks(d.borrow().month, prefs::first_weekday());
            let cw = target.width() as f64 / 7.0;
            let (col, row) = ((x / cw).floor(), ((y - HEAD_H) / CELL_H).floor());
            if y > HEAD_H && (0.0..7.0).contains(&col) && row >= 0.0 && (row as usize) < rows.len() {
                on_pick(rows[row as usize][col as usize]);
            }
        });
        area.add_controller(click);

        let view = Self { root, title, area, data, on_month: Rc::default() };
        for (button, months) in [(&prev, -1i32), (&next, 1)] {
            let v = view.clone();
            button.connect_clicked(move |_| v.shift(months));
        }
        let v = view.clone();
        nyx_theme::subscribe(&view.root, move || v.area.queue_draw());
        view.update_title();
        view
    }

    /// Calls `f` whenever another month comes into view.
    pub fn connect_month(&self, f: impl Fn() + 'static) {
        *self.on_month.borrow_mut() = Some(Box::new(f));
    }

    /// The days the month's grid shows, the last one excluded.
    pub fn grid(&self) -> (NaiveDate, NaiveDate) {
        let month = self.data.borrow().month;
        let rows = month_weeks(month, prefs::first_weekday());
        let first = rows.first().map(|r| r[0]).unwrap_or(month);
        let last = rows.last().map(|r| r[6]).unwrap_or(month);
        (first, last + chrono::Duration::days(1))
    }

    pub fn set_busy(&self, busy: HashSet<NaiveDate>) {
        self.data.borrow_mut().busy = busy;
        self.area.queue_draw();
    }

    /// Marks the days the main view shows and turns to that month.
    pub fn show(&self, from: NaiveDate, to: NaiveDate, anchor: NaiveDate) {
        let turned = {
            let mut d = self.data.borrow_mut();
            d.range = (from, to);
            let month = anchor.with_day(1).unwrap_or(anchor);
            std::mem::replace(&mut d.month, month) != month
        };
        self.update_title();
        self.area.queue_draw();
        if turned {
            self.month_changed();
        }
    }

    fn shift(&self, months: i32) {
        {
            let mut d = self.data.borrow_mut();
            d.month = if months < 0 {
                d.month.checked_sub_months(Months::new(months.unsigned_abs())).unwrap_or(d.month)
            } else {
                d.month.checked_add_months(Months::new(months as u32)).unwrap_or(d.month)
            };
        }
        self.update_title();
        self.area.queue_draw();
        self.month_changed();
    }

    fn month_changed(&self) {
        if let Some(f) = self.on_month.borrow().as_ref() {
            f();
        }
    }

    fn update_title(&self) {
        self.title.set_text(&self.data.borrow().month.format("%B %Y").to_string());
    }
}

fn draw_mini(area: &gtk::DrawingArea, cr: &gtk::cairo::Context, width: f64, d: &Data) {
    let c = draw::colors();
    let tertiary = c.tertiary;
    let rows = month_weeks(d.month, prefs::first_weekday());
    let cw = width / 7.0;
    let today = Local::now().date_naive();
    if let Some(first) = rows.first() {
        for (i, day) in first.iter().enumerate() {
            draw::set(cr, tertiary);
            let letter: String = day.format("%a").to_string().chars().take(1).collect();
            draw::text(area, cr, &letter, i as f64 * cw, 3.0, cw, Style { scale: 0.77, bold: true, center: true, ..draw::NORMAL });
        }
    }
    let (from, to) = d.range;
    for (r, row) in rows.iter().enumerate() {
        let y = HEAD_H + r as f64 * CELL_H;
        // The shown range as one band per row.
        let inside: Vec<usize> = (0..7).filter(|i| row[*i] >= from && row[*i] < to).collect();
        if let (Some(a), Some(b)) = (inside.first(), inside.last()) {
            draw::set(cr, c.bg.mix(c.accent, 0.16));
            draw::rounded(cr, *a as f64 * cw, y, (b - a + 1) as f64 * cw, CELL_H, 7.0);
            let _ = cr.fill();
        }
        for (i, day) in row.iter().enumerate() {
            let x = i as f64 * cw;
            let inside_month = day.month() == d.month.month();
            if *day == today && inside_month {
                draw::set(cr, c.accent);
                cr.arc(x + cw / 2.0, y + CELL_H / 2.0, 12.0, 0.0, std::f64::consts::TAU);
                let _ = cr.fill();
                draw::set(cr, c.on_accent);
            } else if !inside_month {
                draw::set(cr, tertiary.mix(c.bg, 0.4));
            } else {
                draw::set(cr, c.text);
            }
            let style = Style { scale: 0.92, bold: *day == today, center: true, ..draw::NORMAL };
            draw::text(area, cr, &day.day().to_string(), x, y + 5.0, cw, style);
            if inside_month && *day != today && d.busy.contains(day) {
                draw::set(cr, tertiary);
                cr.arc(x + cw / 2.0, y + CELL_H - 3.5, 1.5, 0.0, std::f64::consts::TAU);
                let _ = cr.fill();
            }
        }
    }
}
