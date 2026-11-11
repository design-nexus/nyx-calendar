//! Calendar's tools on the control interface (`nyx_core::control`), for Nyx
//! Assistant: list calendars, read and search events, find free time, and add,
//! change or delete events. Assistant asks before anything that changes a
//! calendar; event text is returned as untrusted content.

use crate::window;
use crate::worker;
use chrono::{DateTime, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime};
use nyx_calendar_core::{Event, Span, whens};
use nyx_core::control::{Reply, text};
use nyx_eds::{Draft, Scope};
use serde_json::{Value, json};

const MAX_EVENTS: usize = 100;

pub fn tools() -> Value {
    let when = json!({"type": "string", "description": "Local time YYYY-MM-DDTHH:MM, a date YYYY-MM-DD (all day), or RFC 3339 with an offset"});
    json!([
        {"name": "calendar.list", "description": "The calendars: name, account, shown or hidden, read-only.", "inputSchema": {"type": "object", "properties": {}}},
        {"name": "calendar.read", "description": "Events between two dates (today unless given), in shown calendars.", "inputSchema": {"type": "object", "properties": {"from": {"type": "string", "description": "YYYY-MM-DD"}, "to": {"type": "string", "description": "YYYY-MM-DD, exclusive"}}}},
        {"name": "calendar.next", "description": "The next events from now (5 unless given), within 30 days.", "inputSchema": {"type": "object", "properties": {"count": {"type": "integer"}}}},
        {"name": "calendar.search", "description": "Events whose title, place or notes contain the words, a year either side of today.", "inputSchema": {"type": "object", "properties": {"query": {"type": "string"}}, "required": ["query"]}},
        {"name": "calendar.free_slots", "description": "Free time on a day between working hours (9 to 17 unless given) at least this long (30 minutes unless given).", "inputSchema": {"type": "object", "properties": {"date": {"type": "string", "description": "YYYY-MM-DD"}, "minutes": {"type": "integer"}, "from_hour": {"type": "integer"}, "to_hour": {"type": "integer"}}, "required": ["date"]}},
        {"name": "calendar.suggest_times", "description": "Times an event this long (minutes) fits in the shown calendars, within the user's working hours, from a date or now. Suggestions only; nothing is booked.", "inputSchema": {"type": "object", "properties": {"minutes": {"type": "integer"}, "from": {"type": "string", "description": "YYYY-MM-DD; today unless given"}, "count": {"type": "integer"}}, "required": ["minutes"]}},
        {"name": "calendar.create", "description": "Add an event. Without an end it lasts an hour (a day for a date).", "inputSchema": {"type": "object", "properties": {"title": {"type": "string"}, "start": when, "end": when, "location": {"type": "string"}, "notes": {"type": "string"}, "calendar": {"type": "string", "description": "Calendar uid from calendar.list; the usual one if left out"}}, "required": ["title", "start"]}},
        {"name": "calendar.edit", "description": "Change an event (by calendar and uid from calendar.read; with rid, only that occurrence).", "inputSchema": {"type": "object", "properties": {"calendar": {"type": "string"}, "uid": {"type": "string"}, "rid": {"type": "string"}, "title": {"type": "string"}, "start": when, "end": when, "location": {"type": "string"}, "notes": {"type": "string"}}, "required": ["calendar", "uid"]}},
        {"name": "calendar.delete", "description": "Delete an event (with rid, only that occurrence).", "inputSchema": {"type": "object", "properties": {"calendar": {"type": "string"}, "uid": {"type": "string"}, "rid": {"type": "string"}}, "required": ["calendar", "uid"]}},
    ])
}

/// API 2: the tools with what they risk and how they're typed, the app, and the
/// `upcoming` topic Glimpse's Calendar panel shows. D-Bus starts Calendar without its
/// window for a call.
pub fn description(app_id: &str) -> Value {
    let x = |risk: &str, extra: Value| {
        let mut v = json!({"risk": risk});
        if let (Some(v), Some(e)) = (v.as_object_mut(), extra.as_object()) {
            v.extend(e.clone());
        }
        v
    };
    let meta = [
        ("calendar.list", x("read_only", json!({"aliases": ["calendars"], "result": "object"}))),
        ("calendar.read", x("read_only", json!({"icon": "x-office-calendar-symbolic", "aliases": [{"word": "today", "arguments": {}}, "day"], "result": "object"}))),
        ("calendar.next", x("read_only", json!({"aliases": ["upcoming", "agenda"], "positional": ["count"], "result": "object"}))),
        ("calendar.search", x("read_only", json!({"aliases": ["find"], "positional": ["query"], "result": "object"}))),
        ("calendar.free_slots", x("read_only", json!({"aliases": ["free"], "positional": ["date"], "result": "object"}))),
        ("calendar.suggest_times", x("read_only", json!({"positional": ["minutes"], "result": "object"}))),
        ("calendar.create", x("writes", json!({"positional": ["title", "start"], "result": "object"}))),
        ("calendar.edit", x("writes", json!({"result": "object"}))),
        ("calendar.delete", x("destructive", json!({"result": "object"}))),
    ];
    let mut list = tools();
    for t in list.as_array_mut().into_iter().flatten() {
        if let Some((_, m)) = meta.iter().find(|(n, _)| t["name"] == *n) {
            t["x-nyx"] = m.clone();
        }
    }
    if let Some(a) = list.as_array_mut() {
        a.push(json!({"name": "calendar.add", "description": "Add an event from words that say when (\"Lunch with Sam tomorrow at 1\"), to the usual calendar.",
            "inputSchema": {"type": "object", "properties": {"text": {"type": "string", "minLength": 1}}, "required": ["text"], "additionalProperties": false},
            "annotations": {"title": "Quick add an event"}, "x-nyx": {"risk": "writes", "positional": ["text"], "aliases": ["new"], "result": "object"}}));
        a.push(json!({"name": "calendar.open", "description": "Open Calendar's window, at a day when given (YYYY-MM-DD).",
            "inputSchema": {"type": "object", "properties": {"date": {"type": "string"}}, "additionalProperties": false},
            "annotations": {"title": "Open Calendar"}, "x-nyx": {"risk": "writes", "confirmation": "never", "positional": ["date"], "result": "none"}}));
    }
    json!({
        "tools": list,
        "nyx": {"api": 2, "app": app_id, "name": "Calendar", "icon": crate::APP_ID, "namespaces": ["calendar"], "aliases": ["cal", "events"],
                "bus_name": app_id, "activation": "dbus", "default": "calendar.search", "open": {"action": "calendar.open"},
                "topics": [{"name": "upcoming", "sensitive": true}],
                "panel": {"id": "calendar", "topics": ["upcoming"], "size": {"min": [420, 320], "preferred": [500, 400]}}}
    })
}

thread_local! {
    static SERVICE: std::cell::RefCell<Option<nyx_core::control::Service>> = const { std::cell::RefCell::new(None) };
    static TIMER: std::cell::RefCell<Option<gtk::glib::SourceId>> = const { std::cell::RefCell::new(None) };
}

/// Register the interface (from `connect_startup`); the `upcoming` topic is read again
/// every minute only while something watches it, and after a change.
pub fn serve(app: &gtk::Application) {
    use gtk::prelude::*;
    let id = app.application_id().map(|s| s.to_string()).unwrap_or_else(|| crate::APP_ID.to_string());
    let window_app = app.clone();
    let Some(service) = nyx_core::control::serve_with(app, description(&id), move |request, answer| {
        let changes = !matches!(request.action.as_str(), "calendar.list" | "calendar.read" | "calendar.next" | "calendar.search" | "calendar.free_slots" | "calendar.suggest_times" | "calendar.open");
        let app = window_app.clone();
        window::when_loaded(move || {
            handle_with(&app, &request.action, request.arguments, Box::new(move |r| {
                answer(r.map_err(|e| nyx_actions::Error::new(nyx_actions::ErrorKind::Failed, e)));
                if changes {
                    publish();
                }
            }));
        });
    }) else {
        return;
    };
    service.on_watch(|_, on| {
        TIMER.with(|t| {
            if let Some(id) = t.borrow_mut().take() {
                id.remove();
            }
            if on {
                publish();
                *t.borrow_mut() = Some(gtk::glib::timeout_add_seconds_local(60, || {
                    publish();
                    gtk::glib::ControlFlow::Continue
                }));
            }
        });
    });
    SERVICE.with(|s| *s.borrow_mut() = Some(service));
}

fn publish() {
    let Some(service) = SERVICE.with(|s| s.borrow().clone()) else { return };
    if !service.watched("upcoming") {
        return;
    }
    window::when_loaded(move || {
        handle("calendar.next", json!({"count": 12}), Box::new(move |r| {
            service.publish("upcoming", r.unwrap_or_else(|e| json!({"error": e})));
        }));
    });
}

fn handle_with(app: &gtk::Application, name: &str, args: Value, reply: Reply) {
    match name {
        "calendar.open" => {
            window::present(app, None);
            if let Some(day) = date_arg(&args, "date") {
                window::go_to(day);
            }
            reply(Ok(Value::Null));
        }
        "calendar.add" => {
            let words = args["text"].as_str().unwrap_or("").trim().to_string();
            let p = nyx_when::parse(&words, Local::now().naive_local());
            let Some(day) = p.date.or_else(|| p.start.map(|_| Local::now().date_naive())) else {
                return reply(Err("say when, such as tomorrow at 1".into()));
            };
            let title = if p.title.trim().is_empty() { words.clone() } else { p.title.trim().to_string() };
            let (start, end) = match p.start {
                Some(t) => {
                    let start = day.and_time(t);
                    let end = p.end.map(|e| day.and_time(e)).or_else(|| p.duration.map(|d| start + d)).unwrap_or(start + Duration::hours(1));
                    (start.format("%Y-%m-%dT%H:%M").to_string(), end.format("%Y-%m-%dT%H:%M").to_string())
                }
                None => (day.to_string(), day.to_string()),
            };
            let said = json!({"title": title, "start": start, "all_day": p.start.is_none()});
            handle("calendar.create", json!({"title": title, "start": start, "end": end}), Box::new(move |r| {
                reply(r.map(|mut v| {
                    if let (Some(v), Some(said)) = (v.as_object_mut(), said.as_object()) {
                        v.extend(said.clone());
                    }
                    v
                }));
            }));
        }
        _ => handle(name, args, reply),
    }
}

pub fn handle(name: &str, args: Value, reply: Reply) {
    match name {
        "calendar.list" => reply(Ok(json!({ "calendars": window::calendars().iter().map(|c| json!({
            "uid": c.uid, "name": c.name, "account": c.group_label(), "shown": c.selected, "read_only": c.read_only, "offline": c.offline,
        })).collect::<Vec<_>>() }))),
        "calendar.read" => {
            let today = Local::now().date_naive();
            let from = date_arg(&args, "from").unwrap_or(today);
            let to = date_arg(&args, "to").filter(|t| *t > from).unwrap_or(from + Duration::days(1));
            let to = to.min(from + Duration::days(92));
            events_reply(from, to, reply, |events| events);
        }
        "calendar.next" => {
            let count = args["count"].as_u64().unwrap_or(5).clamp(1, 50) as usize;
            let now = Local::now().naive_local();
            let today = now.date();
            events_reply(today, today + Duration::days(30), reply, move |events| {
                events
                    .into_iter()
                    .filter(|e| match e.span {
                        Span::Timed { end, .. } => end > now,
                        Span::AllDay { last, .. } => last >= today,
                    })
                    .take(count)
                    .collect()
            });
        }
        "calendar.search" => {
            let query = match text(&args, "query") {
                Ok(q) if !q.trim().is_empty() => q.to_lowercase(),
                _ => return reply(Err("query is required".into())),
            };
            let today = Local::now().date_naive();
            events_reply(today - Duration::days(365), today + Duration::days(365), reply, move |events| {
                events
                    .into_iter()
                    .filter(|e| [&e.title, &e.location, &e.description].iter().any(|t| t.to_lowercase().contains(&query)))
                    .collect()
            });
        }
        "calendar.suggest_times" => {
            let minutes = args["minutes"].as_i64().unwrap_or(30).clamp(5, 12 * 60);
            let count = args["count"].as_u64().unwrap_or(5).clamp(1, 20) as usize;
            let now = Local::now().naive_local();
            let after = date_arg(&args, "from").filter(|d| *d > now.date()).map(|d| d.and_time(NaiveTime::MIN)).unwrap_or(now);
            let hours = crate::prefs::get().working_hours;
            worker::ask(
                move || worker::events_between(after.date(), after.date() + Duration::days(21)),
                move |events| {
                    let busy = nyx_calendar_core::plan::busy(&events, None);
                    let times = nyx_calendar_core::plan::suggest(&busy, after, minutes, &hours, count, 21);
                    reply(Ok(
                        json!({ "minutes": minutes, "times": times.iter().map(|t| t.format("%Y-%m-%dT%H:%M").to_string()).collect::<Vec<_>>() }),
                    ));
                },
            );
        }
        "calendar.free_slots" => {
            let Some(date) = date_arg(&args, "date") else { return reply(Err("date is required (YYYY-MM-DD)".into())) };
            let minutes = args["minutes"].as_i64().unwrap_or(30).clamp(5, 24 * 60);
            let from_hour = args["from_hour"].as_u64().unwrap_or(9).min(23) as u32;
            let to_hour = args["to_hour"].as_u64().unwrap_or(17).clamp(from_hour as u64 + 1, 24) as u32;
            worker::ask(
                move || worker::events_between(date, date + Duration::days(1)),
                move |events| {
                    let slots = free_slots(&events, date, from_hour, to_hour, minutes);
                    reply(Ok(
                        json!({ "date": date.to_string(), "free": slots.iter().map(|(a, b)| json!({"start": a.format("%H:%M").to_string(), "end": b.format("%H:%M").to_string()})).collect::<Vec<_>>() }),
                    ));
                },
            );
        }
        "calendar.create" => {
            let title = match text(&args, "title") {
                Ok(t) if !t.trim().is_empty() => t,
                _ => return reply(Err("title is required".into())),
            };
            let span = match span_args(&args) {
                Ok(Some(span)) => span,
                Ok(None) => return reply(Err("start is required".into())),
                Err(e) => return reply(Err(e)),
            };
            let calendar = args["calendar"].as_str().map(str::to_string).unwrap_or_else(window::default_calendar);
            if !window::calendars().iter().any(|c| c.uid == calendar && !c.read_only) {
                return reply(Err("That calendar can't take new events.".into()));
            }
            let (start, end) = whens(span);
            let draft = Draft {
                summary: Some(title),
                start: Some(Some(start)),
                end: Some(Some(end)),
                location: args["location"].as_str().map(str::to_string),
                description: args["notes"].as_str().map(str::to_string),
                ..Draft::default()
            };
            worker::ask(
                move || worker::with_calendar(&calendar, |cal| cal.create(&draft)).map(|uid| (calendar, uid)),
                move |result| reply(result.map(|(calendar, uid)| json!({"calendar": calendar, "uid": uid}))),
            );
        }
        "calendar.edit" => {
            let (Ok(calendar), Ok(uid)) = (text(&args, "calendar"), text(&args, "uid")) else {
                return reply(Err("calendar and uid are required".into()));
            };
            let rid = args["rid"].as_str().map(str::to_string);
            let mut draft = Draft {
                summary: args["title"].as_str().map(str::to_string),
                location: args["location"].as_str().map(str::to_string),
                description: args["notes"].as_str().map(str::to_string),
                ..Draft::default()
            };
            match span_args(&args) {
                Ok(Some(span)) => {
                    let (start, end) = whens(span);
                    draft.start = Some(Some(start));
                    draft.end = Some(Some(end));
                }
                Ok(None) => {}
                Err(e) => return reply(Err(e)),
            }
            if draft == Draft::default() {
                return reply(Err("Nothing to change.".into()));
            }
            worker::ask(
                move || {
                    let scope = if rid.is_some() { Scope::This } else { Scope::All };
                    worker::with_calendar(&calendar, |cal| cal.modify(&uid, rid.as_deref(), scope, &draft))
                },
                move |result| reply(result.map(|()| json!({"changed": true}))),
            );
        }
        "calendar.delete" => {
            let (Ok(calendar), Ok(uid)) = (text(&args, "calendar"), text(&args, "uid")) else {
                return reply(Err("calendar and uid are required".into()));
            };
            let rid = args["rid"].as_str().map(str::to_string);
            worker::ask(
                move || {
                    let scope = if rid.is_some() { Scope::This } else { Scope::All };
                    worker::with_calendar(&calendar, |cal| cal.remove(&uid, rid.as_deref(), scope))
                },
                move |result| reply(result.map(|()| json!({"deleted": true}))),
            );
        }
        other => reply(Err(format!("Calendar has no tool called {other}."))),
    }
}

fn events_reply(from: NaiveDate, to: NaiveDate, reply: Reply, pick: impl FnOnce(Vec<Event>) -> Vec<Event> + 'static) {
    let names: Vec<(String, String)> = window::calendars().into_iter().map(|c| (c.uid, c.name)).collect();
    worker::ask(
        move || worker::events_between(from, to),
        move |events| {
            let mut picked = pick(events);
            let more = picked.len() > MAX_EVENTS;
            picked.truncate(MAX_EVENTS);
            let list: Vec<Value> = picked.iter().map(|e| event_json(e, &names)).collect();
            reply(Ok(json!({ "events": list, "more": more, "untrusted_content": true })));
        },
    );
}

fn event_json(e: &Event, names: &[(String, String)]) -> Value {
    let (start, end, all_day) = match e.span {
        Span::AllDay { first, last } => (first.to_string(), last.to_string(), true),
        Span::Timed { start, end } => {
            (start.format("%Y-%m-%dT%H:%M").to_string(), end.format("%Y-%m-%dT%H:%M").to_string(), false)
        }
    };
    let notes: String = e.description.chars().take(500).collect();
    json!({
        "calendar": e.calendar,
        "calendar_name": names.iter().find(|(u, _)| *u == e.calendar).map(|(_, n)| n.as_str()).unwrap_or(""),
        "uid": e.uid, "rid": e.rid, "title": e.title, "location": e.location,
        "start": start, "end": end, "all_day": all_day, "repeats": e.repeats, "cancelled": e.cancelled,
        "notes": notes, "url": e.url, "guests": e.attendees.len(),
    })
}

fn date_arg(args: &Value, key: &str) -> Option<NaiveDate> {
    args[key].as_str().and_then(|s| s.get(..10)?.parse().ok())
}

enum Moment {
    Day(NaiveDate),
    At(NaiveDateTime),
}

fn moment(text: &str) -> Option<Moment> {
    let text = text.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(text) {
        return Some(Moment::At(dt.with_timezone(&Local).naive_local()));
    }
    for format in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M", "%Y-%m-%d %H:%M"] {
        if let Ok(at) = NaiveDateTime::parse_from_str(text, format) {
            return Some(Moment::At(at));
        }
    }
    text.parse::<NaiveDate>().ok().map(Moment::Day)
}

/// The event's span from `start` and `end`, or `None` without a start.
fn span_args(args: &Value) -> Result<Option<Span>, String> {
    let Some(start) = args["start"].as_str() else { return Ok(None) };
    let start = moment(start).ok_or_else(|| format!("Couldn't read the start “{start}”."))?;
    let end = match args["end"].as_str() {
        Some(e) => Some(moment(e).ok_or_else(|| format!("Couldn't read the end “{e}”."))?),
        None => None,
    };
    Ok(Some(match (start, end) {
        (Moment::Day(first), Some(Moment::Day(last))) => Span::AllDay { first, last: last.max(first) },
        (Moment::Day(first), _) => Span::AllDay { first, last: first },
        (Moment::At(start), Some(Moment::At(end))) if end > start => Span::Timed { start, end },
        (Moment::At(start), _) => Span::Timed { start, end: start + Duration::hours(1) },
    }))
}

/// Gaps of at least `minutes` between `from_hour` and `to_hour` on `date`.
fn free_slots(events: &[Event], date: NaiveDate, from_hour: u32, to_hour: u32, minutes: i64) -> Vec<(NaiveTime, NaiveTime)> {
    let day_start = date.and_hms_opt(from_hour, 0, 0).unwrap_or_default();
    let day_end = if to_hour >= 24 {
        (date + Duration::days(1)).and_time(NaiveTime::MIN)
    } else {
        date.and_hms_opt(to_hour, 0, 0).unwrap_or_default()
    };
    let busy = nyx_calendar_core::plan::busy(events, None);
    let free = nyx_calendar_core::plan::free_between(&busy, day_start, day_end, minutes);
    // A day that runs to midnight ends at 23:59 here, so the end is still that day.
    let last = NaiveTime::from_hms_opt(23, 59, 0).unwrap_or(NaiveTime::MIN);
    free.into_iter().map(|(a, b)| (a.time(), if b == day_end && to_hour >= 24 { last } else { b.time() })).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_description_is_api_2() {
        let d = nyx_actions::Description::from_value(&description(crate::APP_ID), None).unwrap();
        assert!(d.action("calendar.delete").unwrap().asks());
        assert!(!d.action("calendar.add").unwrap().asks());
        assert_eq!(d.app.panel.as_ref().map(|p| p.id.as_str()), Some("calendar"));
    }

    fn at(h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 22).unwrap().and_hms_opt(h, m, 0).unwrap()
    }

    fn event(start: NaiveDateTime, end: NaiveDateTime) -> Event {
        Event {
            calendar: "c".into(),
            uid: "u".into(),
            rid: None,
            title: "x".into(),
            location: String::new(),
            span: Span::Timed { start, end },
            repeats: false,
            cancelled: false,
            description: String::new(),
            url: None,
            rule: None,
            alarms: Vec::new(),
            organizer: None,
            attendees: Vec::new(),
        }
    }

    #[test]
    fn finds_gaps_in_working_hours() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();
        let events = vec![
            event(at(8, 0), at(9, 30)),
            event(at(11, 0), at(12, 0)),
            event(at(11, 30), at(13, 0)),
            event(at(16, 45), at(18, 0)),
        ];
        let t = |h, m| NaiveTime::from_hms_opt(h, m, 0).unwrap();
        assert_eq!(free_slots(&events, day, 9, 17, 30), vec![(t(9, 30), t(11, 0)), (t(13, 0), t(16, 45))]);
        assert_eq!(free_slots(&events, day, 9, 17, 120), vec![(t(13, 0), t(16, 45))]);
    }

    #[test]
    fn reads_moments() {
        let span = span_args(&json!({"start": "2026-10-22T14:00"})).unwrap().unwrap();
        assert_eq!(span, Span::Timed { start: at(14, 0), end: at(15, 0) });
        let span = span_args(&json!({"start": "2026-10-22", "end": "2026-10-23"})).unwrap().unwrap();
        let d = |day| NaiveDate::from_ymd_opt(2026, 10, day).unwrap();
        assert_eq!(span, Span::AllDay { first: d(22), last: d(23) });
        assert!(span_args(&json!({"start": "soon"})).is_err());
        assert_eq!(span_args(&json!({})).unwrap(), None);
    }
}
