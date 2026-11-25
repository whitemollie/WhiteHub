//! Native Windows toast notifications for device events, filtered by the user's rules.
use crate::{battery::manager::Event, settings::Notifications};
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

/// Title and body for an event, or `None` if the rules suppress it.
pub fn compose(rules: &Notifications, e: &Event) -> Option<(String, String)> {
    if rules.do_not_disturb {
        return None;
    }
    match e {
        Event::Connected { name, .. } if rules.on_connect => Some((format!("{name} connected"), "Battery monitoring resumed.".into())),
        Event::Disconnected { name, .. } if rules.on_disconnect => {
            Some((format!("{name} disconnected"), "Its last battery level stays in the WhiteHub window.".into()))
        }
        Event::Low { name, level, .. } if rules.on_low => Some((format!("{name}: battery low"), format!("Battery level: {level}. Time to charge."))),
        _ => None,
    }
}

pub fn send(app: &AppHandle, rules: &Notifications, events: &[Event]) {
    for e in events {
        match compose(rules, e) {
            Some((title, body)) => {
                log::info!("[notify] {title}");
                if let Err(err) = app.notification().builder().title(&title).body(&body).show() {
                    log::warn!("[notify] failed: {err}");
                }
            }
            None => log::debug!("[notify] suppressed {e:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_and_dnd() {
        let mut r = Notifications::default();
        let low = Event::Low { id: "hyperx".into(), name: "Headset".into(), level: "12%".into() };
        assert!(compose(&r, &low).unwrap().1.contains("12%"));
        r.on_low = false;
        assert!(compose(&r, &low).is_none());
        r = Notifications::default();
        r.do_not_disturb = true;
        assert!(compose(&r, &Event::Connected { id: "x".into(), name: "M".into() }).is_none());
    }
}
