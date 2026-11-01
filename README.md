> Development branch: Online Accounts now uses the isolated native preview.
> Runtime EDS/EWS dependencies have been replaced by `nyx-pim` and libical.
> Existing EDS data is retained and can be explicitly exported/imported with
> `nyx-pim-migrate`. Provider and migration limitations are tracked in
> the sibling `nyx-accounts/docs/IMPLEMENTATION.md`; this branch is not merge-ready.

# Calendar

A calendar for Omarchy: the calendars from your Google, personal Microsoft,
Nextcloud and other online accounts, and calendars kept on this computer, in one week view. Part of the
[Nyx](https://github.com/design-nexus/nyx) apps.

- Day, week, month and agenda views, with a month in the sidebar
- Type “Lunch with Sam tomorrow at 1” to add an event; a full editor for repeats, alerts, places and notes
- Repeating events, with changes to one, the rest, or all of them
- Answer invitations; see the organizer and who's coming
- Reminders as desktop notifications, with Snooze
- Import and export .ics files; search a year of events
- Calendars grouped by account, with their colors; click one to show or hide it
- Works offline from the saved copy and catches up when you're back online
- Follows your Omarchy theme; works from the keyboard and tiled at half-screen

Accounts are added in **Settings → Online Accounts**. This feature branch uses
native account authentication and an isolated SQLite calendar store with libical.
Personal Microsoft accounts use Graph; EWS and enterprise accounts are unsupported.
Existing EDS stores remain untouched and can be copied through the explicit migration tool.

## Install

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
