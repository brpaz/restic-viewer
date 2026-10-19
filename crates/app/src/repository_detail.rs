//! The detail pane: shows a Repository's Snapshot list once selected in the sidebar,
//! handling the async load, an unreachable-Repository error, and a stale/missing
//! Repository Password by re-prompting inline. Selecting a Snapshot pushes a second
//! page (via `AdwNavigationView`) browsing that Snapshot's Entry tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use restic_client::{RealResticClient, RepositoryConnection, ResticClient, ResticError, Snapshot};

use gettextrs::{gettext, ngettext};

use crate::config::RepositoryEntry;
use crate::keyring::{self, CredentialKind};
use crate::restore_dialog;
use crate::{diff_view, entry_tree, health_dialog};

const RESTIC_EXIT_WRONG_PASSWORD: i32 = 12;
const RESOURCE_PATH: &str = "/dev/brunopaz/ResticViewer/repository_detail.ui";

/// Returned handle: embed `widget` in the detail pane, call `show_placeholder()` when
/// nothing is selected, and `load(entry)` when a Repository is selected.
pub struct DetailView {
    pub widget: gtk4::Widget,
    load: Rc<dyn Fn(RepositoryEntry)>,
    show_placeholder: Rc<dyn Fn()>,
}

impl DetailView {
    pub fn load(&self, entry: RepositoryEntry) {
        (self.load)(entry);
    }

    pub fn show_placeholder(&self) {
        (self.show_placeholder)();
    }
}

pub fn build(parent: adw::ApplicationWindow) -> DetailView {
    let builder = gtk4::Builder::from_resource(RESOURCE_PATH);
    let stack: gtk4::Stack = builder.object("stack").expect("stack");
    let snapshot_list_page: adw::NavigationPage = builder
        .object("snapshot_list_page")
        .expect("snapshot_list_page");
    let snapshot_list: gtk4::ListBox = builder.object("snapshot_list").expect("snapshot_list");
    let snapshot_list_empty_status: adw::StatusPage = builder
        .object("snapshot_list_empty_status")
        .expect("snapshot_list_empty_status");
    snapshot_list.set_placeholder(Some(&snapshot_list_empty_status));
    let nav_view: adw::NavigationView = builder.object("nav_view").expect("nav_view");
    let error_status: adw::StatusPage = builder.object("error_status").expect("error_status");
    let unlock_password_row: adw::PasswordEntryRow = builder
        .object("unlock_password_row")
        .expect("unlock_password_row");
    let unlock_button: gtk4::Button = builder.object("unlock_button").expect("unlock_button");

    let compare_button: gtk4::ToggleButton =
        builder.object("compare_button").expect("compare_button");
    let snapshot_search: gtk4::SearchEntry =
        builder.object("snapshot_search").expect("snapshot_search");
    let snapshot_sort_dropdown: gtk4::DropDown = builder
        .object("snapshot_sort_dropdown")
        .expect("snapshot_sort_dropdown");
    let health_button: gtk4::Button = builder.object("health_button").expect("health_button");
    let compare_hint: gtk4::Revealer = builder.object("compare_hint").expect("compare_hint");

    nav_view.push(&snapshot_list_page);

    let view = Rc::new(SnapshotView {
        list: snapshot_list.clone(),
        nav_view: nav_view.clone(),
        parent: parent.clone(),
        compare_hint,
        connection: RefCell::new(None),
        snapshots: RefCell::new(Vec::new()),
        query: RefCell::new(String::new()),
        sort: Cell::new(SnapshotSort::Newest),
        comparing: Cell::new(false),
        picked: RefCell::new(Vec::new()),
    });
    snapshot_search.connect_search_changed({
        let view = view.clone();
        move |entry| {
            *view.query.borrow_mut() = entry.text().trim().to_lowercase();
            view.render();
        }
    });
    snapshot_sort_dropdown.connect_selected_notify({
        let view = view.clone();
        move |dropdown| {
            view.sort.set(SnapshotSort::from_index(dropdown.selected()));
            view.render();
        }
    });
    health_button.connect_clicked({
        let view = view.clone();
        move |_| {
            if let Some(connection) = view.connection.borrow().clone() {
                health_dialog::present(&view.parent, connection);
            }
        }
    });
    compare_button.connect_toggled({
        let view = view.clone();
        move |button| {
            view.comparing.set(button.is_active());
            view.picked.borrow_mut().clear();
            view.render();
        }
    });

    let generation = Rc::new(Cell::new(0u64));
    let current_entry: Rc<RefCell<Option<RepositoryEntry>>> = Rc::new(RefCell::new(None));

    let start_load: Rc<dyn Fn(RepositoryEntry, Option<String>)> = {
        let stack = stack.clone();
        let error_status = error_status.clone();
        let generation = generation.clone();
        let nav_view = nav_view.clone();
        let view = view.clone();
        let compare_button = compare_button.clone();
        let snapshot_search = snapshot_search.clone();
        Rc::new(
            move |entry: RepositoryEntry, override_password: Option<String>| {
                let my_generation = generation.get() + 1;
                generation.set(my_generation);
                stack.set_visible_child_name("loading");
                while nav_view.pop() {}

                let stack = stack.clone();
                let error_status = error_status.clone();
                let generation = generation.clone();
                let view = view.clone();
                let compare_button = compare_button.clone();
                let snapshot_search = snapshot_search.clone();

                glib::spawn_future_local(async move {
                    let password = match override_password {
                        Some(password) => Some(password),
                        None => keyring::lookup(&entry.id, CredentialKind::Password)
                            .await
                            .ok()
                            .flatten(),
                    };

                    let Some(password) = password else {
                        if generation.get() == my_generation {
                            stack.set_visible_child_name("unlock");
                        }
                        return;
                    };

                    let backend_secret =
                        if matches!(entry.backend, restic_client::Backend::S3 { .. }) {
                            keyring::lookup(&entry.id, CredentialKind::BackendSecret)
                                .await
                                .ok()
                                .flatten()
                        } else {
                            None
                        };

                    let connection = RepositoryConnection {
                        backend: entry.backend.clone(),
                        password,
                        backend_secret,
                    };

                    let result = RealResticClient::system().list_snapshots(&connection).await;
                    if generation.get() != my_generation {
                        return; // a different Repository was selected while this was loading
                    }

                    match result {
                        Ok(snapshots) => {
                            compare_button.set_active(false);
                            snapshot_search.set_text("");
                            view.set_snapshots(connection, snapshots);
                            stack.set_visible_child_name("snapshots");
                        }
                        Err(ResticError::NonZeroExit {
                            code: Some(RESTIC_EXIT_WRONG_PASSWORD),
                            ..
                        }) => {
                            stack.set_visible_child_name("unlock");
                        }
                        Err(err) => {
                            error_status.set_description(Some(&err.to_string()));
                            stack.set_visible_child_name("error");
                        }
                    }
                });
            },
        )
    };

    unlock_button.connect_clicked({
        let current_entry = current_entry.clone();
        let unlock_password_row = unlock_password_row.clone();
        let start_load = start_load.clone();
        move |_| {
            let Some(entry) = current_entry.borrow().clone() else {
                return;
            };
            let password = unlock_password_row.text().to_string();
            if password.is_empty() {
                return;
            }
            start_load(entry, Some(password));
        }
    });

    let load: Rc<dyn Fn(RepositoryEntry)> = {
        let current_entry = current_entry.clone();
        let start_load = start_load.clone();
        Rc::new(move |entry: RepositoryEntry| {
            *current_entry.borrow_mut() = Some(entry.clone());
            start_load(entry, None);
        })
    };

    let show_placeholder: Rc<dyn Fn()> = {
        let stack = stack.clone();
        let generation = generation.clone();
        Rc::new(move || {
            generation.set(generation.get() + 1);
            *current_entry.borrow_mut() = None;
            stack.set_visible_child_name("placeholder");
        })
    };

    DetailView {
        widget: stack.upcast(),
        load,
        show_placeholder,
    }
}

#[derive(Clone, Copy)]
enum SnapshotSort {
    Newest,
    Oldest,
    Largest,
}

impl SnapshotSort {
    /// Matches the order of the strings in the sort dropdown (see repository_detail.blp).
    fn from_index(index: u32) -> Self {
        match index {
            1 => Self::Oldest,
            2 => Self::Largest,
            _ => Self::Newest,
        }
    }
}

/// The Snapshot list of the selected Repository and the state that decides which rows it
/// shows: the filter text, the sort order, and the pair picked in compare mode.
struct SnapshotView {
    list: gtk4::ListBox,
    nav_view: adw::NavigationView,
    parent: adw::ApplicationWindow,
    compare_hint: gtk4::Revealer,
    connection: RefCell<Option<RepositoryConnection>>,
    snapshots: RefCell<Vec<Snapshot>>,
    /// Lowercased filter text.
    query: RefCell<String>,
    sort: Cell<SnapshotSort>,
    comparing: Cell<bool>,
    picked: RefCell<Vec<String>>,
}

impl SnapshotView {
    fn set_snapshots(self: &Rc<Self>, connection: RepositoryConnection, snapshots: Vec<Snapshot>) {
        *self.connection.borrow_mut() = Some(connection);
        *self.snapshots.borrow_mut() = snapshots;
        self.render();
    }

    fn visible_snapshots(&self) -> Vec<Snapshot> {
        let query = self.query.borrow();
        let mut snapshots: Vec<Snapshot> = self
            .snapshots
            .borrow()
            .iter()
            .filter(|snapshot| query.is_empty() || matches_query(snapshot, &query))
            .cloned()
            .collect();
        match self.sort.get() {
            SnapshotSort::Newest => snapshots.sort_by(|a, b| b.time.cmp(&a.time)),
            SnapshotSort::Oldest => snapshots.sort_by(|a, b| a.time.cmp(&b.time)),
            SnapshotSort::Largest => snapshots.sort_by(|a, b| {
                let size = |s: &Snapshot| s.summary.map(|summary| summary.total_bytes_processed);
                size(b).cmp(&size(a)).then_with(|| b.time.cmp(&a.time))
            }),
        }
        snapshots
    }

    fn render(self: &Rc<Self>) {
        self.compare_hint.set_reveal_child(self.comparing.get());
        while let Some(row) = self.list.row_at_index(0) {
            self.list.remove(&row);
        }
        let Some(connection) = self.connection.borrow().clone() else {
            return;
        };
        for snapshot in self.visible_snapshots() {
            self.list.append(&self.build_row(&connection, snapshot));
        }
    }

    fn build_row(
        self: &Rc<Self>,
        connection: &RepositoryConnection,
        snapshot: Snapshot,
    ) -> adw::ActionRow {
        let row = adw::ActionRow::builder()
            .title(diff_view::short_date(&snapshot))
            .subtitle(subtitle(&snapshot))
            .activatable(true)
            .build();

        if self.comparing.get() {
            let check = gtk4::CheckButton::new();
            check.set_active(self.picked.borrow().contains(&snapshot.id));
            check.connect_toggled({
                let view = self.clone();
                let connection = connection.clone();
                let snapshot_id = snapshot.id.clone();
                move |check| view.toggle_picked(&connection, &snapshot_id, check.is_active())
            });
            row.add_prefix(&check);
            row.connect_activated(move |_| check.set_active(!check.is_active()));
            return row;
        }

        let restore_button = gtk4::Button::builder()
            .icon_name("edit-undo-symbolic")
            .tooltip_text("Restore this Snapshot")
            .valign(gtk4::Align::Center)
            .build();
        restore_button.add_css_class("flat");
        restore_button.connect_clicked({
            let parent = self.parent.clone();
            let connection = connection.clone();
            let snapshot_id = snapshot.id.clone();
            move |_| {
                restore_dialog::present(
                    &parent,
                    connection.clone(),
                    snapshot_id.clone(),
                    vec![],
                    "Restore Snapshot",
                );
            }
        });
        row.add_suffix(&restore_button);
        row.add_suffix(&gtk4::Image::from_icon_name("go-next-symbolic"));

        row.connect_activated({
            let nav_view = self.nav_view.clone();
            let connection = connection.clone();
            let parent = self.parent.clone();
            move |_| {
                let tree_widget = entry_tree::build(
                    parent.clone(),
                    connection.clone(),
                    snapshot.id.clone(),
                    snapshot.paths.clone(),
                );
                let page = adw::NavigationPage::builder()
                    .title(diff_view::short_date(&snapshot))
                    .child(&tree_widget)
                    .build();
                nav_view.push(&page);
            }
        });
        row
    }

    /// Tracks the Snapshots ticked in compare mode and opens the comparison as soon as
    /// there are two of them, older one first.
    fn toggle_picked(self: &Rc<Self>, connection: &RepositoryConnection, id: &str, picked: bool) {
        {
            let mut ids = self.picked.borrow_mut();
            ids.retain(|existing| existing != id);
            if picked {
                ids.push(id.to_string());
            }
        }
        let ids = self.picked.borrow().clone();
        let [first, second] = ids.as_slice() else {
            return;
        };
        let snapshots = self.snapshots.borrow();
        let (Some(first), Some(second)) = (
            snapshots.iter().find(|s| &s.id == first),
            snapshots.iter().find(|s| &s.id == second),
        ) else {
            return;
        };
        let (older, newer) = if first.time <= second.time {
            (first, second)
        } else {
            (second, first)
        };
        let page = adw::NavigationPage::builder()
            .title(gettext("Changes"))
            .child(&diff_view::build(connection.clone(), older, newer))
            .build();
        self.nav_view.push(&page);
    }
}

fn matches_query(snapshot: &Snapshot, query: &str) -> bool {
    snapshot.hostname.to_lowercase().contains(query)
        || snapshot.id.starts_with(query)
        || snapshot
            .tags
            .iter()
            .chain(&snapshot.paths)
            .any(|text| text.to_lowercase().contains(query))
}

fn subtitle(snapshot: &Snapshot) -> String {
    let mut parts = vec![snapshot.hostname.clone()];
    if !snapshot.tags.is_empty() {
        parts.push(snapshot.tags.join(", "));
    }
    if let Some(summary) = snapshot.summary {
        parts.push(format!(
            "{} · {}",
            ngettext(
                "{} file",
                "{} files",
                summary.total_files_processed.min(u64::from(u32::MAX)) as u32
            )
            .replace("{}", &summary.total_files_processed.to_string()),
            glib::format_size(summary.total_bytes_processed),
        ));
    }
    parts.join(" · ")
}
