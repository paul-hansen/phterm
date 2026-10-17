//! Tests for the OSC 9 and OSC 777 notification escapes
use super::*;
use k9::assert_equal as assert_eq;

struct CollectAlerts(Arc<Mutex<Vec<Alert>>>);

impl AlertHandler for CollectAlerts {
    fn alert(&mut self, alert: Alert) {
        self.0.lock().unwrap().push(alert);
    }
}

fn notifications(escapes: &str) -> Vec<(Option<String>, String)> {
    let mut term = TestTerm::new(5, 20, 0);
    let alerts = Arc::new(Mutex::new(vec![]));
    term.set_notification_handler(Box::new(CollectAlerts(Arc::clone(&alerts))));
    term.print(escapes);

    let alerts = alerts.lock().unwrap();
    alerts
        .iter()
        .filter_map(|alert| match alert {
            Alert::ToastNotification { title, body, .. } => Some((title.clone(), body.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn osc9() {
    assert_eq!(
        notifications("\x1b]9;Build done; 3 errors\x1b\\"),
        vec![(None, "Build done; 3 errors".to_string())]
    );
    assert_eq!(notifications("\x1b]9;\x1b\\"), vec![]);
    // ConEmu sub-commands are not notifications
    assert_eq!(notifications("\x1b]9;12\x1b\\\x1b]9;4;1;50\x1b\\"), vec![]);
}

#[test]
fn osc777() {
    assert_eq!(
        notifications("\x1b]777;notify;Claude Code;Waiting; needs input\x1b\\"),
        vec![(
            Some("Claude Code".to_string()),
            "Waiting; needs input".to_string()
        )]
    );
    assert_eq!(
        notifications("\x1b]777;notify;;the tea is ready\x1b\\"),
        vec![(None, "the tea is ready".to_string())]
    );
    assert_eq!(
        notifications("\x1b]777;notify;the tea is ready\x1b\\"),
        vec![(None, "the tea is ready".to_string())]
    );
    assert_eq!(notifications("\x1b]777;notify;;\x1b\\"), vec![]);
}

#[test]
fn conemu_cwd() {
    let mut term = TestTerm::new(5, 20, 0);
    term.print("\x1b]9;9;\"/home/me/my;project\"\x1b\\");
    assert_eq!(
        term.get_current_dir().map(|url| url.path().to_string()),
        Some("/home/me/my;project".to_string())
    );

    // A relative path can't be represented, and doesn't
    // clobber the current directory
    term.print("\x1b]9;9;relative\x1b\\");
    assert_eq!(
        term.get_current_dir().map(|url| url.path().to_string()),
        Some("/home/me/my;project".to_string())
    );
}
