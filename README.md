# Calendar

A calendar for NyxOS and Omarchy: the calendars from your iCloud, Fastmail,
Nextcloud and other CalDAV accounts, and calendars kept on this computer, in one week view. Part of the
[Nyx](https://github.com/design-nexus/nyx) apps.

- Day, week, month and agenda views, with a month in the sidebar
- Type “Lunch with Sam tomorrow at 1” to add an event; a full editor for repeats, alerts, places and notes
- Repeating events, with changes to one, the rest, or all of them
- Answer invitations; see the organizer and who's coming
- Reminders as desktop notifications, with Snooze
- Import and export .ics files; search a year of events
- Calendars grouped by account, with their colors; click one to show or hide it
- Works offline from the saved copy and catches up when you're back online
- Follows the desktop theme; works from the keyboard and tiled at half-screen

Accounts are added in **Settings → Online Accounts**, with an app password for CalDAV
servers. Google and personal Microsoft calendars need an OAuth client you register with
them yourself; work and school (EWS) accounts aren't supported. Events are kept on this
computer in a SQLite store. On Omarchy, calendars from the older Evolution store are
left as they were and can be copied over with `nyx-pim-migrate`.

## Install

On NyxOS, choose Calendar in Welcome when you set up, or install it later from the Store or with `nyx install nyx-calendar`. On Omarchy:

```sh
curl -fsSL https://raw.githubusercontent.com/design-nexus/nyx-calendar/main/install.sh | bash
```

## Keys

| Key | Action |
| --- | --- |
| N | New event |
| T | Today |
| D / W / M / A | Day, week, month, agenda |
| J / K | Next / previous week |
| R | Refresh |
| Ctrl+F | Find events this week |
| F1 | All shortcuts |
