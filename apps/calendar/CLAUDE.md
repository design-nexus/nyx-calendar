# Calendar — notes for working on this app

- GTK4 (`gtk4` 0.11, `v4_18`) + Rust, no libadwaita. The window is `nyx_ui::frame`. The
  header reads left to right: the sidebar button, Today, the arrows and the range, the
  view switch; then Quick add (only while it fits: `fit_quick_add`), Find (a search strip
  over the page) and New event (the editor). The sidebar: the mini month (busy days
  dotted), each account's calendars as a section of tick boxes in its colour (⋯ for the
  account card), this computer (+ for a new local calendar), the imported copy, Sets as
  chips, and the sync state at its foot (click: Refresh). Under 980 px the sidebar is a
  drawer and the week is five days (`five_days`). Settings is `nyx_ui::settings_card`.
  Colors are `@theme_*` tokens; event colors are the calendars' own, from EDS.
- Events, calendars and which calendars are shown belong to Evolution Data Server.
  `worker.rs` owns `nyx_eds::Registry` and the open `Calendar`s on one thread with its
  own GLib main context (EDS signals arrive there) and sends `worker::Update` values to
  `window::apply` on the GTK thread. Never call EDS from the GTK thread.
- Accounts are GOA's: "Add account" and "Sign in" open Settings → Online Accounts
  (`nyx-settings --section online-accounts`). The app stores no credentials.
- `crates/nyx-calendar-core` turns EDS items into local-time `Event`s and lays out a
  week (`WeekLayout`); its tests cover overlaps, midnight and all-day lanes.
- Offline: EDS serves its saved copy; calendars report `online()` and the window shows
  an Offline banner and readout.
- Right-click menus (`menus.rs`, `widgets::popup_menu`): events and empty time come
  through `draw::Hooks::event_menu`/`empty_menu` from each view's hit tests; calendars
  and sets in the sidebar. Turning a calendar off is EDS's `enabled` (every app;
  `Registry::set_enabled`); Settings → Calendars lists them all (`Registry::all_sources`).
  New events use prefs `event_minutes`, `default_alert`, `new_events_calendar`.
- Checks: `cargo test -p nyx-calendar-core -p nyx-calendar` and
  `cargo clippy --workspace --all-targets -- -D warnings`. Layout:
  `CALENDAR_SNAPSHOT=/tmp/cal.png CALENDAR_SNAPSHOT_W=800 nyx-calendar` under scratch
  `XDG_CONFIG_HOME`/`XDG_STATE_HOME` (copy Omarchy's colors.toml in for the real theme).
  It prints the sidebar width and whether the nav is compact. `CALENDAR_OPEN_EVENT=<title>`
  opens that event's popover once the week has loaded (capture it with `grim -T`).
- Shared crates come from nyx-core on GitHub, pinned by `rev` (Cargo.toml and
  Cargo.lock). To build against unpushed nyx-core changes, push them first and bump the
  rev; never commit a `file://` source (CI refuses it, and the one-line install can't fetch it).
- `apps/calendard` (`nyx-calendard`): GTK-free user service. EDS's `EReminderWatcher`
  (through `nyx_eds::Reminders`) → org.freedesktop.Notifications with Open, Snooze
  10 min, Dismiss; quiet while `evolution-alarm-notify` runs. Writes
  `~/.local/share/nyx-calendar/agenda.sqlite` (schema in `agenda.rs`; a contract for
  Glimpse). The installer enables and starts the unit; the app also starts it on
  open. `nyx-calendard --write-agenda` writes once. End to end:
  `tools/test-calendard.sh` (scratch EDS, fake notification server).
- Views: `days.rs` (day and week), `month.rs`, `agenda.rs` (also year-wide search
  results), `minical.rs`; shared drawing in `draw.rs`. Clicks go through `draw::Hooks`
  to `details.rs` (popover), `quick.rs` (natural-language create) and `editor.rs`.
  Changes run through `actions.rs` on the worker (`worker::ask`). Delete keeps an export
  of the series for Undo.
- Planning (`nyx_calendar_core::plan`): working hours (prefs), free time, `suggest`,
  `clashes`, meeting and map links. The editor's Find a time and clash line use it,
  through the worker. Sets live in prefs and apply by EDS visibility. The reminders
  overlay (`worker::reminders_between`) adds `task:<list>` pseudo-events to the views
  only; clicking one opens Reminders. `year.rs` is the year view.
- Control tools (`control.rs`): `calendar.list/read/next/search/free_slots/suggest_times` read;
  `calendar.create/edit/delete` change. Before Assistant uses them, its `nyx-apps`
  ability needs them and its policy must treat the changing ones as edits.
- Moving an event to another calendar (`actions::move_to`; editor's Calendar choice and the
  event menu's Move to): exports the series, `import_all` into the target, removes the
  original (or the copy if that fails), with Undo.
- Time zones (`editor.rs`): the editor's times are wall-clock in its Time zone choice
  (chrono-tz names); an event's own zone is read from its DTSTART TZID (`zone_in_ics`) and
  written through `Draft::zone`. Views show local time.
- Account groups in the sidebar fold (prefs `collapsed_groups`, by heading). A banner shows
  for GOA `ms_graph`/`exchange` accounts when evolution-ews's EDS backends are missing.
- Find a time takes people (its "With" line): `Calendar::free_busy` on the editor's chosen
  calendar asks its server (Exchange/Microsoft 365, CalDAV with scheduling) for their busy
  times, merged into `plan::suggest`. It doesn't invite them; the editor has no attendees yet.
  nyx-eds is pinned to a newer nyx-core commit than nyx-core/nyx-accounts.
