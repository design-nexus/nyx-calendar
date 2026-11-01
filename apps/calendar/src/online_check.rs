//! Explicit preview-only disposable recurrence and availability round trip.
use chrono::{TimeZone, Timelike};
use nyx_eds::{Calendar, Draft, Kind, Registry, Scope, When};
pub fn extended() -> Result<(), String> {
    if std::env::var("NYX_ACCOUNTS_PREVIEW").as_deref() != Ok("1") {
        return Err("Preview only".into());
    }
    let account = nyx_accounts::Client::connect()
        .and_then(|c| c.list())
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|a| a.provider == "google" && a.enabled.contains(&nyx_accounts::Service::Calendar))
        .ok_or("Google Calendar is not enabled")?;
    let registry = Registry::connect().map_err(|e| e.to_string())?;
    let refresh = || registry.refresh_accounts(nyx_accounts::Service::Calendar).map_err(|e| e.to_string());
    refresh()?;
    let source = registry
        .sources(Kind::Calendar)
        .into_iter()
        .find(|s| {
            s.backend == "google" && s.account.as_ref() == Some(&account.id) && s.writable && s.uid.ends_with(&account.identity)
        })
        .ok_or("Primary Google calendar unavailable")?;
    let cal = Calendar::open(&registry, &source.uid, Kind::Calendar, None).map_err(|e| e.to_string())?;
    let zone: chrono_tz::Tz = "America/Chicago".parse().unwrap();
    let start = zone.with_ymd_and_hms(2027, 3, 13, 9, 0, 0).single().unwrap().timestamp();
    let uid = cal
        .create(&Draft {
            summary: Some("Nyx disposable DST recurrence test".into()),
            start: Some(Some(When::Time(start))),
            end: Some(Some(When::Time(start + 1800))),
            zone: Some("America/Chicago".into()),
            repeat: Some(Some("FREQ=DAILY;COUNT=3".into())),
            alarms: Some(vec![10]),
            ..Default::default()
        })
        .map_err(|e| e.to_string())?;
    let result = (|| {
        refresh()?;
        refresh()?; // Upload, then independently download provider projection.
        let item = cal.get(&uid, None).map_err(|e| e.to_string())?;
        if item.alarms != vec![10] {
            return Err("Provider reminder did not round trip".into());
        }
        let mut instances: Vec<_> = cal.instances(start - 3600, start + 4 * 86400).into_iter().filter(|i| i.uid == uid).collect();
        instances.sort_by_key(|i| i.start);
        if instances.len() != 3 {
            return Err("Provider recurrence count changed".into());
        }
        let times: Vec<_> = instances
            .iter()
            .map(|i| match i.start {
                Some(When::Time(t)) => Ok(t),
                _ => Err("Missing occurrence time".to_string()),
            })
            .collect::<Result<_, _>>()?;
        if times.iter().any(|t| zone.timestamp_opt(*t, 0).unwrap().hour() != 9) || times[1] - times[0] != 23 * 3600 {
            return Err("Provider recurrence lost its DST wall-clock time".into());
        }
        println!("Google Calendar: recurring event and reminder preserve 09:00 across DST");
        cal.free_busy(start, start + 4 * 86400, &[account.identity.clone()]).map_err(|e| e.to_string())?;
        println!("Google Calendar: provider free/busy request passed");
        Ok(())
    })();
    cal.remove(&uid, None, Scope::All).map_err(|e| format!("Cleanup needed for test {uid}: {e}"))?;
    refresh()?;
    refresh()?;
    println!("Google Calendar: disposable recurring series removed");
    result
}
