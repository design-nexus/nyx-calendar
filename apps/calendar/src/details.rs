//! The popover for one event: when, your answer to an invitation, the call, where,
//! who, and what can be done with it.

use crate::actions;
use crate::draw;
use crate::editor;
use crate::widgets::{self, MenuEntry, item, separator, sub};
use crate::window;
use chrono::Datelike;
use gtk::prelude::*;
use nyx_calendar_core::{Attendee, Event, Repeat, Response, Span};
use nyx_theme::Rgb;
use std::rc::Rc;

const FACES_SHOWN: usize = 5;

/// "Fri 13 Nov · 1:30–3:00 PM", "Fri 13 Nov", "Fri 13 – Sat 14 Nov".
fn when_line(span: Span) -> String {
    let day = |d: chrono::NaiveDate| d.format("%a %-d %b").to_string();
    match span {
        Span::AllDay { first, last } if first == last => day(first),
        Span::AllDay { first, last } if first.month() == last.month() => {
            format!("{} – {}", first.format("%a %-d"), day(last))
        }
        Span::AllDay { first, last } => format!("{} – {}", day(first), day(last)),
        Span::Timed { start, end }
            if start.date() == end.date() || (end.time() == chrono::NaiveTime::MIN && end - start <= chrono::Duration::days(1)) =>
        {
            format!("{} · {}", day(start.date()), draw::short_range(start, end))
        }
        Span::Timed { .. } => draw::when_text(span),
    }
}

/// "6 people · 4 going, 1 maybe".
fn people_line(people: &[&Attendee]) -> String {
    let count = |r: Response| people.iter().filter(|a| a.response == r).count();
    let mut parts = Vec::new();
    for (n, what) in [(count(Response::Accepted), "going"), (count(Response::Tentative), "maybe"), (count(Response::Declined), "not going")] {
        if n > 0 {
            parts.push(format!("{n} {what}"));
        }
    }
    let total = if people.len() == 1 { "1 person".to_string() } else { format!("{} people", people.len()) };
    if parts.is_empty() { total } else { format!("{total} · {}", parts.join(", ")) }
}

fn initials(name: &str) -> String {
    let words: Vec<&str> = name.split(|c: char| c.is_whitespace() || c == '.' || c == '@').filter(|w| !w.is_empty()).collect();
    let letters: String = match words.as_slice() {
        [] => String::new(),
        [one] => one.chars().take(1).collect(),
        [first, .., last] if !name.contains('@') => [first.chars().next(), last.chars().next()].into_iter().flatten().collect(),
        [first, second, ..] => [first.chars().next(), second.chars().next()].into_iter().flatten().collect(),
    };
    letters.to_uppercase()
}

/// A round face with someone's initials on a swatch picked by their name, ringed in
/// the popover's colour so faces can overlap.
fn face(person: &Attendee, size: i32) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.add_css_class("face");
    area.set_content_width(size);
    area.set_content_height(size);
    area.set_valign(gtk::Align::Center);
    let label = person.label().to_string();
    let answer = match person.response {
        Response::Accepted => " · going",
        Response::Tentative => " · maybe",
        Response::Declined => " · not going",
        _ => "",
    };
    area.set_tooltip_text(Some(&format!("{label}{answer}")));
    let text = initials(&label);
    let seed = label.chars().map(|c| c as usize).sum::<usize>();
    area.set_draw_func(move |area, cr, w, h| {
        let swatches = nyx_theme::swatches();
        let color = swatches.get(seed % swatches.len().max(1)).map(|(_, c)| Rgb::hex(c)).unwrap_or_else(|| Rgb::hex(&nyx_theme::palette().accent));
        let ring = Rgb::hex(&nyx_theme::look().popover);
        let (cx, cy, r) = (w as f64 / 2.0, h as f64 / 2.0, w.min(h) as f64 / 2.0);
        draw::set(cr, ring);
        cr.arc(cx, cy, r, 0.0, std::f64::consts::TAU);
        let _ = cr.fill();
        draw::set(cr, color);
        cr.arc(cx, cy, r - 2.0, 0.0, std::f64::consts::TAU);
        let _ = cr.fill();
        cr.set_source_rgb(1.0, 1.0, 1.0);
        let style = draw::Style { scale: 0.72, bold: true, center: true, ..draw::NORMAL };
        let (_, th) = draw::text_size(area, &text, style);
        draw::text(area, cr, &text, 0.0, cy - th / 2.0, w as f64, style);
    });
    area
}

/// A line of the popover: a quiet icon and its text.
fn line(text: &str, icon: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("detail-row");
    let image = gtk::Image::from_icon_name(icon);
    image.add_css_class("detail-icon");
    image.set_valign(gtk::Align::Start);
    row.append(&image);
    let label = widgets::label(text, "");
    label.set_wrap(true);
    label.set_xalign(0.0);
    label.set_hexpand(true);
    label.set_max_width_chars(34);
    label.set_selectable(true);
    row.append(&label);
    row
}

/// What ⋯ offers: the rest of the event's right-click menu.
fn more_entries(event: &Event, read_only: bool, close: Rc<dyn Fn()>) -> Vec<MenuEntry> {
    let calendars = window::calendars();
    let mut out = Vec::new();
    if !read_only {
        let (e, c) = (event.clone(), close.clone());
        out.push(item("Duplicate…", move || {
            c();
            let start = editor::Start { title: e.title.clone(), span: Some(e.span), calendar: Some(e.calendar.clone()), repeat: None };
            editor::open(None, Some(start));
        }));
        let targets: Vec<MenuEntry> = calendars
            .iter()
            .filter(|c| !c.read_only && c.uid != event.calendar)
            .map(|cal| {
                let (e, uid, c) = (event.clone(), cal.uid.clone(), close.clone());
                item(&format!("{} · {}", cal.name, cal.group_label()), move || {
                    c();
                    actions::move_to(&e, uid.clone());
                })
            })
            .collect();
        if !targets.is_empty() {
            out.push(sub("Move to", targets));
        }
        out.push(separator());
    }
    let (e, c) = (event.clone(), close.clone());
    out.push(item("Export as .ics…", move || {
        c();
        actions::export(&e);
    }));
    if let Some(cal) = calendars.iter().find(|c| c.uid == event.calendar) {
        let (uid, c) = (cal.uid.clone(), close.clone());
        out.push(item(&format!("Hide {}", cal.name), move || {
            c();
            crate::worker::set_selected(uid.clone(), false);
        }));
    }
    out
}

pub fn show(parent: &gtk::Widget, rect: gtk::gdk::Rectangle, event: Event) {
    // A reminder in the overlay belongs to Reminders.
    if let Some(list) = event.calendar.strip_prefix("task:") {
        let program = nyx_accounts::find_bin("nyx-reminders").unwrap_or_else(|| "nyx-reminders".into());
        if std::process::Command::new(program).args(["--section", &format!("list:{list}")]).spawn().is_err() {
            window::toast("Install Nyx Reminders to open reminders.");
        }
        let _ = (parent, rect);
        return;
    }
    let calendars = window::calendars();
    let calendar = calendars.iter().find(|c| c.uid == event.calendar);
    let read_only = calendar.is_none_or(|c| c.read_only);
    let color = calendar.and_then(|c| c.color.as_deref()).map(Rgb::hex).unwrap_or_else(|| Rgb::hex(&nyx_theme::palette().accent));

    let popover = gtk::Popover::new();
    popover.add_css_class("event-popover");
    // Layout checks open it without a click, and the compositor grants no grab without one.
    if std::env::var_os("CALENDAR_OPEN_EVENT").is_some() {
        popover.set_autohide(false);
    }
    let card = widgets::vbox(12);
    card.add_css_class("event-details");
    let close: Rc<dyn Fn()> = {
        let p = popover.clone();
        Rc::new(move || p.popdown())
    };

    // The head: the calendar's colour, the title and when, then Edit, Delete and ⋯.
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let bar = gtk::DrawingArea::new();
    bar.set_content_width(4);
    bar.set_draw_func(move |_, cr, w, h| {
        draw::set(cr, color);
        draw::rounded(cr, 0.0, 0.0, w as f64, h as f64, 2.0);
        let _ = cr.fill();
    });
    head.append(&bar);
    let titles = widgets::vbox(2);
    titles.set_hexpand(true);
    let title = widgets::label(&event.title, "event-title");
    if event.cancelled {
        title.add_css_class("cancelled");
    }
    title.set_wrap(true);
    title.set_xalign(0.0);
    title.set_max_width_chars(26);
    title.set_selectable(true);
    titles.append(&title);
    let when = widgets::label(&when_line(event.span), "event-when");
    when.set_wrap(true);
    when.set_xalign(0.0);
    titles.append(&when);
    head.append(&titles);
    let tools = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    tools.set_valign(gtk::Align::Start);
    if !read_only {
        let edit = widgets::icon_button("document-edit-symbolic", "Edit (E)");
        let (e, c) = (event.clone(), close.clone());
        edit.connect_clicked(move |_| {
            c();
            editor::open(Some(&e), None);
        });
        tools.append(&edit);
        let delete = widgets::icon_button("user-trash-symbolic", "Delete (Delete)");
        let (e, c) = (event.clone(), close.clone());
        delete.connect_clicked(move |_| {
            c();
            actions::delete(&e);
        });
        tools.append(&delete);
    }
    let more = widgets::icon_button("view-more-horizontal-symbolic", "More");
    {
        let (e, c) = (event.clone(), close.clone());
        more.connect_clicked(move |b| {
            widgets::popup_menu(b, b.width() as f64 / 2.0, b.height() as f64, more_entries(&e, read_only, c.clone()));
        });
    }
    tools.append(&more);
    head.append(&tools);
    card.append(&head);

    // Your answer to an invitation.
    let me = window::me();
    if let Some(mine) = event.invitation(&me)
        && !read_only
    {
        let invite = widgets::vbox(6);
        invite.add_css_class("invite-box");
        let who = match &event.organizer {
            Some(o) if !o.label().is_empty() => format!("{} invited you", o.label()),
            _ => "You're invited".into(),
        };
        invite.append(&widgets::label(&who, "invite-who"));
        let current = match mine.response {
            Response::Accepted => "yes",
            Response::Tentative => "maybe",
            Response::Declined => "no",
            _ => "",
        };
        let (e, c) = (event.clone(), close.clone());
        let answers = widgets::segmented(
            &widgets::opts(&[("yes", "Going"), ("maybe", "Maybe"), ("no", "Not going")]),
            current,
            move |answer| {
                let response = match answer.as_str() {
                    "yes" => Response::Accepted,
                    "maybe" => Response::Tentative,
                    _ => Response::Declined,
                };
                actions::respond(&e, response);
                c();
            },
        );
        answers.set_homogeneous(true);
        answers.set_hexpand(true);
        invite.append(&answers);
        card.append(&invite);
    }

    if let Some(link) = nyx_calendar_core::plan::meeting_link(&event) {
        let join = widgets::labeled_button("camera-web-symbolic", "Join the video call");
        join.add_css_class("suggested-action");
        join.add_css_class("join-call");
        if let Some(content) = join.child() {
            content.set_halign(gtk::Align::Center);
        }
        join.set_tooltip_text(Some(&format!("{link} (J)")));
        let c = close.clone();
        join.connect_clicked(move |_| {
            c();
            window::open_link(&link);
        });
        card.append(&join);
    }

    let rows = widgets::vbox(8);
    if !event.location.is_empty() {
        let row = line(&event.location, "mark-location-symbolic");
        if let Some(link) = nyx_calendar_core::plan::map_link(&event.location) {
            let map = gtk::Button::with_label("Show on a map");
            map.add_css_class("flat");
            map.add_css_class("link-button");
            map.set_valign(gtk::Align::Start);
            map.connect_clicked(move |_| window::open_link(&link));
            row.append(&map);
        }
        rows.append(&row);
    }
    let mut people: Vec<&Attendee> = Vec::new();
    if let Some(organizer) = &event.organizer
        && !event.attendees.is_empty()
        && !event.attendees.iter().any(|a| a.email.eq_ignore_ascii_case(&organizer.email))
    {
        people.push(organizer);
    }
    people.extend(event.attendees.iter());
    if !people.is_empty() {
        let row = line(&people_line(&people), "system-users-symbolic");
        if let Some(organizer) = &event.organizer
            && event.invitation(&me).is_none()
        {
            row.set_tooltip_text(Some(&format!("Organized by {}", organizer.label())));
        }
        rows.append(&row);
        let faces = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        faces.add_css_class("faces");
        for person in people.iter().take(FACES_SHOWN) {
            faces.append(&face(person, 24));
        }
        if people.len() > FACES_SHOWN {
            faces.append(&widgets::label(&format!("+{}", people.len() - FACES_SHOWN), "faces-more"));
        }
        rows.append(&faces);
    }
    if let Some(rule) = &event.rule {
        let (first, _) = event.days();
        let text = Repeat::parse(rule).map(|r| r.describe(first)).unwrap_or_else(|| "Repeats".into());
        rows.append(&line(&text, "media-playlist-repeat-symbolic"));
    } else if event.repeats {
        rows.append(&line("Changed occurrence of a repeating event", "media-playlist-repeat-symbolic"));
    }
    if event.cancelled {
        rows.append(&line("Cancelled", "dialog-warning-symbolic"));
    }
    if !event.alarms.is_empty() {
        let text = event.alarms.iter().map(|m| editor::alert_label(*m)).collect::<Vec<_>>().join(", ");
        rows.append(&line(&text, "alarm-symbolic"));
    }
    if let Some(calendar) = calendar {
        let whose = if window::is_imported(calendar) {
            "Imported".to_string()
        } else if calendar.account.is_some() || !matches!(calendar.provider.as_str(), "" | "native" | "local") {
            crate::settings_page::provider_name(&calendar.provider).to_string()
        } else {
            "This computer".to_string()
        };
        rows.append(&line(&format!("{} · {whose}", window::display_name(calendar)), "x-office-calendar-symbolic"));
    }
    if !event.description.trim().is_empty() {
        let notes = widgets::label(event.description.trim(), "event-notes");
        notes.set_wrap(true);
        notes.set_xalign(0.0);
        notes.set_max_width_chars(40);
        notes.set_lines(8);
        notes.set_ellipsize(gtk::pango::EllipsizeMode::End);
        notes.set_selectable(true);
        rows.append(&notes);
    }
    let call = nyx_calendar_core::plan::meeting_link(&event);
    if let Some(url) = event
        .url
        .clone()
        .filter(|u| (u.starts_with("https://") || u.starts_with("http://")) && call.as_deref() != Some(u.as_str()))
    {
        let link = gtk::LinkButton::with_label(&url, &url);
        link.set_halign(gtk::Align::Start);
        if let Some(label) = link.child().and_downcast::<gtk::Label>() {
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            label.set_max_width_chars(36);
        }
        rows.append(&link);
    }
    if rows.first_child().is_some() {
        card.append(&rows);
    }

    // E, Delete and J act on the event while the popover is open.
    let keys = gtk::EventControllerKey::new();
    let (e, c) = (event.clone(), close.clone());
    keys.connect_key_pressed(move |_, key, _, _| {
        match key {
            gtk::gdk::Key::j => {
                if let Some(link) = nyx_calendar_core::plan::meeting_link(&e) {
                    c();
                    window::open_link(&link);
                }
            }
            gtk::gdk::Key::e if !read_only => {
                c();
                editor::open(Some(&e), None);
            }
            gtk::gdk::Key::Delete if !read_only => {
                c();
                actions::delete(&e);
            }
            _ => return glib::Propagation::Proceed,
        }
        glib::Propagation::Stop
    });
    popover.add_controller(keys);

    popover.set_child(Some(&card));
    popover.set_parent(parent);
    popover.set_pointing_to(Some(&rect));
    let shown = event.clone();
    popover.connect_closed(move |p| {
        if draw::is_selected(&shown) {
            draw::select(None);
            window::repaint_days();
        }
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    draw::select(Some(&event));
    window::repaint_days();
    popover.popup();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initials_from_names_and_addresses() {
        assert_eq!(initials("Ada Okafor"), "AO");
        assert_eq!(initials("Mary Jane Watson"), "MW");
        assert_eq!(initials("sam"), "S");
        assert_eq!(initials("jordan.lee@example.com"), "JL");
        assert_eq!(initials(""), "");
    }
}
