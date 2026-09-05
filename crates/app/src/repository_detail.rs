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

use crate::config::RepositoryEntry;
use crate::entry_tree;
use crate::keyring::{self, CredentialKind};
use crate::restore_dialog;

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
    let nav_view: adw::NavigationView = builder.object("nav_view").expect("nav_view");
    let error_status: adw::StatusPage = builder.object("error_status").expect("error_status");
    let unlock_password_row: adw::PasswordEntryRow = builder
        .object("unlock_password_row")
        .expect("unlock_password_row");
    let unlock_button: gtk4::Button = builder.object("unlock_button").expect("unlock_button");

    nav_view.push(&snapshot_list_page);

    let generation = Rc::new(Cell::new(0u64));
    let current_entry: Rc<RefCell<Option<RepositoryEntry>>> = Rc::new(RefCell::new(None));
    let current_connection: Rc<RefCell<Option<RepositoryConnection>>> = Rc::new(RefCell::new(None));
    let current_snapshots: Rc<RefCell<Vec<Snapshot>>> = Rc::new(RefCell::new(Vec::new()));

    let start_load: Rc<dyn Fn(RepositoryEntry, Option<String>)> = {
        let stack = stack.clone();
        let snapshot_list = snapshot_list.clone();
        let error_status = error_status.clone();
        let generation = generation.clone();
        let current_connection = current_connection.clone();
        let current_snapshots = current_snapshots.clone();
        let nav_view = nav_view.clone();
        let snapshot_list_page = snapshot_list_page.clone();
        let parent = parent.clone();
        Rc::new(
            move |entry: RepositoryEntry, override_password: Option<String>| {
                let my_generation = generation.get() + 1;
                generation.set(my_generation);
                stack.set_visible_child_name("loading");
                while nav_view.pop() {}

                let stack = stack.clone();
                let snapshot_list = snapshot_list.clone();
                let error_status = error_status.clone();
                let generation = generation.clone();
                let current_connection = current_connection.clone();
                let current_snapshots = current_snapshots.clone();
                let nav_view = nav_view.clone();
                let snapshot_list_page = snapshot_list_page.clone();
                let parent = parent.clone();

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
                            populate_snapshot_list(
                                &snapshot_list,
                                &snapshots,
                                &nav_view,
                                &snapshot_list_page,
                                &connection,
                                &parent,
                            );
                            *current_connection.borrow_mut() = Some(connection);
                            *current_snapshots.borrow_mut() = snapshots;
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

fn populate_snapshot_list(
    list: &gtk4::ListBox,
    snapshots: &[Snapshot],
    nav_view: &adw::NavigationView,
    list_page: &adw::NavigationPage,
    connection: &RepositoryConnection,
    parent: &adw::ApplicationWindow,
) {
    while let Some(row) = list.row_at_index(0) {
        list.remove(&row);
    }
    for snapshot in snapshots {
        let date = snapshot.time.get(..19).unwrap_or(&snapshot.time);
        let tags = if snapshot.tags.is_empty() {
            String::new()
        } else {
            format!(" · {}", snapshot.tags.join(", "))
        };
        let subtitle = format!("{}{tags}", snapshot.hostname);
        let row = adw::ActionRow::builder()
            .title(date)
            .subtitle(subtitle)
            .activatable(true)
            .build();

        let restore_button = gtk4::Button::builder()
            .icon_name("edit-undo-symbolic")
            .tooltip_text("Restore this Snapshot")
            .valign(gtk4::Align::Center)
            .build();
        restore_button.add_css_class("flat");
        restore_button.connect_clicked({
            let parent = parent.clone();
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
            let nav_view = nav_view.clone();
            let connection = connection.clone();
            let snapshot = snapshot.clone();
            let parent = parent.clone();
            move |_| {
                let tree_widget = entry_tree::build(
                    parent.clone(),
                    connection.clone(),
                    snapshot.id.clone(),
                    snapshot.paths.clone(),
                );
                let title = snapshot.time.get(..19).unwrap_or(&snapshot.time);
                let page = adw::NavigationPage::builder()
                    .title(title)
                    .child(&wrap_in_toolbar(tree_widget))
                    .build();
                nav_view.push(&page);
            }
        });

        list.append(&row);
    }
    let _ = list_page; // kept for symmetry / future title updates
}

fn wrap_in_toolbar(content: gtk4::Widget) -> gtk4::Widget {
    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&adw::HeaderBar::new());
    toolbar_view.set_content(Some(&content));
    toolbar_view.upcast()
}
