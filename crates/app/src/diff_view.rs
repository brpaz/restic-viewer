//! The Snapshot comparison page: lists every path that was added, removed or modified
//! between an older and a newer Snapshot of the same Repository.

use gettextrs::{gettext, ngettext};
use gtk4::prelude::*;
use libadwaita as adw;
use restic_client::{
    DiffChange, DiffEntry, RealResticClient, RepositoryConnection, ResticClient, Snapshot,
};

const RESOURCE_PATH: &str = "/dev/brunopaz/ResticViewer/diff_view.ui";

pub fn build(connection: RepositoryConnection, older: &Snapshot, newer: &Snapshot) -> gtk4::Widget {
    let builder = gtk4::Builder::from_resource(RESOURCE_PATH);
    let toolbar_view: adw::ToolbarView = builder.object("toolbar_view").expect("toolbar_view");
    let window_title: adw::WindowTitle = builder.object("window_title").expect("window_title");
    let stack: gtk4::Stack = builder.object("stack").expect("stack");
    let summary_label: gtk4::Label = builder.object("summary_label").expect("summary_label");
    let changes_view: gtk4::ListView = builder.object("changes_view").expect("changes_view");
    let error_status: adw::StatusPage = builder.object("error_status").expect("error_status");

    window_title.set_title(&gettext("Changes"));
    window_title.set_subtitle(&format!("{} → {}", short_date(older), short_date(newer)));
    changes_view.set_factory(Some(&changes_factory()));

    let from_id = older.id.clone();
    let to_id = newer.id.clone();
    glib::spawn_future_local(async move {
        match RealResticClient::system()
            .diff(&connection, &from_id, &to_id)
            .await
        {
            Ok(changes) if changes.is_empty() => stack.set_visible_child_name("identical"),
            Ok(changes) => {
                summary_label.set_label(&summary(&changes));
                let store = gio::ListStore::new::<glib::BoxedAnyObject>();
                let rows: Vec<_> = changes.into_iter().map(glib::BoxedAnyObject::new).collect();
                store.splice(0, 0, &rows);
                changes_view.set_model(Some(&gtk4::NoSelection::new(Some(store))));
                stack.set_visible_child_name("changes");
            }
            Err(err) => {
                error_status.set_description(Some(&err.to_string()));
                stack.set_visible_child_name("error");
            }
        }
    });

    toolbar_view.upcast()
}

/// The same `YYYY-MM-DD HH:MM:SS` form the Snapshot list shows.
pub fn short_date(snapshot: &Snapshot) -> String {
    snapshot
        .time
        .get(..19)
        .unwrap_or(&snapshot.time)
        .replace('T', " ")
}

fn summary(changes: &[DiffEntry]) -> String {
    let count = |kind| changes.iter().filter(|c| c.change == kind).count();
    let (added, removed, modified) = (
        count(DiffChange::Added),
        count(DiffChange::Removed),
        count(DiffChange::Modified),
    );
    format!(
        "{} · {} · {}",
        ngettext("{} added", "{} added", added as u32).replace("{}", &added.to_string()),
        ngettext("{} removed", "{} removed", removed as u32).replace("{}", &removed.to_string()),
        ngettext("{} modified", "{} modified", modified as u32)
            .replace("{}", &modified.to_string()),
    )
}

fn changes_factory() -> gtk4::SignalListItemFactory {
    let factory = gtk4::SignalListItemFactory::new();
    factory.connect_setup(|_, list_item| {
        let list_item = list_item
            .downcast_ref::<gtk4::ListItem>()
            .expect("factory item is a ListItem");
        let row_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        row_box.append(&gtk4::Image::new());
        row_box.append(
            &gtk4::Label::builder()
                .xalign(0.0)
                .hexpand(true)
                .ellipsize(gtk4::pango::EllipsizeMode::Start)
                .build(),
        );
        list_item.set_child(Some(&row_box));
    });
    factory.connect_bind(|_, list_item| {
        let list_item = list_item
            .downcast_ref::<gtk4::ListItem>()
            .expect("factory item is a ListItem");
        let Some(boxed) = list_item
            .item()
            .and_then(|item| item.downcast::<glib::BoxedAnyObject>().ok())
        else {
            return;
        };
        let Some(row_box) = list_item
            .child()
            .and_then(|c| c.downcast::<gtk4::Box>().ok())
        else {
            return;
        };
        let (Some(icon), Some(label)) = (
            row_box
                .first_child()
                .and_then(|c| c.downcast::<gtk4::Image>().ok()),
            row_box
                .last_child()
                .and_then(|c| c.downcast::<gtk4::Label>().ok()),
        ) else {
            return;
        };

        let entry = boxed.borrow::<DiffEntry>();
        let (icon_name, css_class) = match entry.change {
            DiffChange::Added => ("list-add-symbolic", "success"),
            DiffChange::Removed => ("list-remove-symbolic", "error"),
            DiffChange::Modified => ("document-edit-symbolic", "warning"),
        };
        icon.set_icon_name(Some(icon_name));
        for class in ["success", "error", "warning"] {
            icon.remove_css_class(class);
        }
        icon.add_css_class(css_class);
        label.set_label(&entry.path);
    });
    factory
}
