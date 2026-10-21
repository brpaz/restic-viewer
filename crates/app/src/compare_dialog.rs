//! Picks the Snapshot to compare another one with. The Snapshot just before the one being
//! viewed is listed first, since "what changed since last time?" is the usual question.

use std::rc::Rc;

use gettextrs::{gettext, ngettext};
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use restic_client::Snapshot;

use crate::diff_view::short_date;
use crate::repository_detail::subtitle;

const RESOURCE_PATH: &str = "/dev/brunopaz/ResticViewer/compare_dialog.ui";

/// Calls `on_pick` with the chosen Snapshot and closes the dialog.
pub fn present(
    parent: &adw::ApplicationWindow,
    current: &Snapshot,
    others: Vec<Snapshot>,
    on_pick: impl Fn(Snapshot) + 'static,
) {
    let builder = gtk4::Builder::from_resource(RESOURCE_PATH);
    let dialog: adw::Dialog = builder.object("dialog").expect("dialog");
    let filter_entry: gtk4::SearchEntry = builder.object("filter_entry").expect("filter_entry");
    let list: gtk4::ListBox = builder.object("list").expect("list");
    let empty_status: adw::StatusPage = builder.object("empty_status").expect("empty_status");
    list.set_placeholder(Some(&empty_status));

    let current = Rc::new(current.clone());
    let candidates = Rc::new(ordered_candidates(&current, others));
    let on_pick = Rc::new(on_pick);

    let render = {
        let list = list.clone();
        let dialog = dialog.clone();
        let current = current.clone();
        let candidates = candidates.clone();
        move |query: &str| {
            while let Some(row) = list.row_at_index(0) {
                list.remove(&row);
            }
            for candidate in candidates
                .iter()
                .filter(|c| query.is_empty() || matches_query(&c.snapshot, query))
            {
                list.append(&build_row(&dialog, &current, candidate, &on_pick));
            }
        }
    };
    render("");
    filter_entry.connect_search_changed(move |entry| render(&entry.text().trim().to_lowercase()));

    dialog.present(Some(parent));
}

fn build_row(
    dialog: &adw::Dialog,
    current: &Snapshot,
    candidate: &Candidate,
    on_pick: &Rc<impl Fn(Snapshot) + 'static>,
) -> adw::ActionRow {
    let snapshot = &candidate.snapshot;
    let mut details = subtitle(snapshot);
    if let Some(offset) = offset_between(current, snapshot) {
        details = format!("{} · {details}", offset_label(offset));
    }
    let row = adw::ActionRow::builder()
        .title(short_date(snapshot))
        .subtitle(details)
        .activatable(true)
        .build();

    if let Some(warning) = mismatch_warning(current, snapshot) {
        let icon = gtk4::Image::from_icon_name("dialog-warning-symbolic");
        icon.set_tooltip_text(Some(&warning));
        row.add_prefix(&icon);
    }
    if candidate.is_previous {
        let badge = gtk4::Label::new(Some(&gettext("Previous")));
        badge.add_css_class("accent");
        badge.add_css_class("caption-heading");
        row.add_suffix(&badge);
    }

    row.connect_activated({
        let dialog = dialog.clone();
        let snapshot = snapshot.clone();
        let on_pick = on_pick.clone();
        move |_| {
            dialog.close();
            on_pick(snapshot.clone());
        }
    });
    row
}

struct Candidate {
    snapshot: Snapshot,
    /// The newest Snapshot older than the current one.
    is_previous: bool,
}

/// Newest first, except that the Snapshot immediately before `current` leads the list.
fn ordered_candidates(current: &Snapshot, mut others: Vec<Snapshot>) -> Vec<Candidate> {
    others.sort_by(|a, b| b.time.cmp(&a.time));
    let previous = others.iter().position(|s| s.time < current.time);
    let mut candidates: Vec<Candidate> = others
        .into_iter()
        .map(|snapshot| Candidate {
            snapshot,
            is_previous: false,
        })
        .collect();
    if let Some(index) = previous {
        let mut previous = candidates.remove(index);
        previous.is_previous = true;
        candidates.insert(0, previous);
    }
    candidates
}

fn matches_query(snapshot: &Snapshot, query: &str) -> bool {
    snapshot.hostname.to_lowercase().contains(query)
        || snapshot.id.starts_with(query)
        || short_date(snapshot).contains(query)
        || snapshot
            .tags
            .iter()
            .chain(&snapshot.paths)
            .any(|text| text.to_lowercase().contains(query))
}

/// Comparing Snapshots of different machines or backed-up paths is rarely intended, so
/// those rows carry a warning.
fn mismatch_warning(current: &Snapshot, other: &Snapshot) -> Option<String> {
    if current.hostname != other.hostname {
        Some(gettext("From a different host"))
    } else if current.paths != other.paths {
        Some(gettext("Backed up different paths"))
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    LessThanAnHour,
    Hours,
    Days,
    Months,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Offset {
    amount: u64,
    unit: Unit,
    later: bool,
}

/// How far `other` is from `current` in time, from the difference in seconds
/// (`other` minus `current`).
fn describe_offset(seconds: i64) -> Offset {
    const HOUR: u64 = 3600;
    const DAY: u64 = 24 * HOUR;
    let later = seconds > 0;
    let magnitude = seconds.unsigned_abs();
    let (amount, unit) = if magnitude < HOUR {
        (0, Unit::LessThanAnHour)
    } else if magnitude < 2 * DAY {
        (magnitude / HOUR, Unit::Hours)
    } else if magnitude < 60 * DAY {
        (magnitude / DAY, Unit::Days)
    } else {
        (magnitude / (30 * DAY), Unit::Months)
    };
    Offset {
        amount,
        unit,
        later,
    }
}

fn offset_between(current: &Snapshot, other: &Snapshot) -> Option<Offset> {
    let unix = |snapshot: &Snapshot| {
        glib::DateTime::from_iso8601(&snapshot.time, None)
            .ok()
            .map(|time| time.to_unix())
    };
    Some(describe_offset(unix(other)? - unix(current)?))
}

fn offset_label(offset: Offset) -> String {
    let count = offset.amount.min(u64::from(u32::MAX)) as u32;
    let span = match offset.unit {
        Unit::LessThanAnHour => {
            return if offset.later {
                gettext("less than an hour later")
            } else {
                gettext("less than an hour earlier")
            };
        }
        Unit::Hours => ngettext("{} hour", "{} hours", count),
        Unit::Days => ngettext("{} day", "{} days", count),
        Unit::Months => ngettext("{} month", "{} months", count),
    }
    .replace("{}", &offset.amount.to_string());
    if offset.later {
        gettext("{} later").replace("{}", &span)
    } else {
        gettext("{} earlier").replace("{}", &span)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(id: &str, time: &str) -> Snapshot {
        Snapshot {
            id: id.to_string(),
            time: time.to_string(),
            hostname: "laptop".to_string(),
            tags: vec![],
            paths: vec!["/home".to_string()],
            summary: None,
        }
    }

    #[test]
    fn the_previous_snapshot_leads_and_the_rest_are_newest_first() {
        let current = snapshot("c", "2026-10-10T00:00:00Z");
        let others = vec![
            snapshot("old", "2026-10-01T00:00:00Z"),
            snapshot("new", "2026-10-15T00:00:00Z"),
            snapshot("prev", "2026-10-08T00:00:00Z"),
            snapshot("newest", "2026-10-20T00:00:00Z"),
        ];

        let order: Vec<(String, bool)> = ordered_candidates(&current, others)
            .into_iter()
            .map(|c| (c.snapshot.id, c.is_previous))
            .collect();

        let expected: Vec<(String, bool)> = [
            ("prev", true),
            ("newest", false),
            ("new", false),
            ("old", false),
        ]
        .into_iter()
        .map(|(id, previous)| (id.to_string(), previous))
        .collect();
        assert_eq!(order, expected);
    }

    #[test]
    fn without_an_older_snapshot_nothing_is_marked_previous() {
        let current = snapshot("c", "2026-10-01T00:00:00Z");
        let others = vec![snapshot("a", "2026-10-05T00:00:00Z")];

        assert!(
            ordered_candidates(&current, others)
                .iter()
                .all(|c| !c.is_previous)
        );
    }

    #[test]
    fn offsets_pick_a_readable_unit() {
        let minutes = 60;
        let hour = 3600;
        let day = 86_400;
        assert_eq!(describe_offset(30 * minutes - 1).unit, Unit::LessThanAnHour);
        assert_eq!(
            describe_offset(-5 * hour),
            Offset {
                amount: 5,
                unit: Unit::Hours,
                later: false
            }
        );
        assert_eq!(
            describe_offset(3 * day),
            Offset {
                amount: 3,
                unit: Unit::Days,
                later: true
            }
        );
        assert_eq!(
            describe_offset(-90 * day),
            Offset {
                amount: 3,
                unit: Unit::Months,
                later: false
            }
        );
    }

    #[test]
    fn different_hosts_and_paths_are_flagged() {
        let current = snapshot("c", "2026-10-10T00:00:00Z");
        let mut other_host = snapshot("h", "2026-10-09T00:00:00Z");
        other_host.hostname = "server".to_string();
        let mut other_paths = snapshot("p", "2026-10-09T00:00:00Z");
        other_paths.paths = vec!["/etc".to_string()];

        assert!(mismatch_warning(&current, &snapshot("s", "2026-10-09T00:00:00Z")).is_none());
        assert!(mismatch_warning(&current, &other_host).is_some());
        assert!(mismatch_warning(&current, &other_paths).is_some());
    }
}
