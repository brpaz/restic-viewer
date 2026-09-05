mod add_repository;
mod config;
mod entry_tree;
mod keyring;
mod repository_detail;
mod restore_dialog;

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use config::{RepositoryEntry, RepositoryStore};

const APP_ID: &str = "dev.brunopaz.ResticViewer";
const RESOURCE_PREFIX: &str = "/dev/brunopaz/ResticViewer";

type ReloadSidebarCell = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

fn main() -> glib::ExitCode {
    gio::resources_register_include!("compiled.gresource").expect("register app resources");

    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &adw::Application) {
    let store = Rc::new(RepositoryStore::new());
    let entries: Rc<RefCell<Vec<RepositoryEntry>>> = Rc::new(RefCell::new(Vec::new()));

    let builder = gtk4::Builder::from_resource(&format!("{RESOURCE_PREFIX}/window.ui"));
    let window: adw::ApplicationWindow = builder
        .object("window")
        .expect("window.ui defines `window`");
    let sidebar_page: adw::NavigationPage = builder
        .object("sidebar_page")
        .expect("window.ui defines `sidebar_page`");
    let sidebar_stack: gtk4::Stack = builder
        .object("sidebar_stack")
        .expect("window.ui defines `sidebar_stack`");
    let repo_list: gtk4::ListBox = builder
        .object("repo_list")
        .expect("window.ui defines `repo_list`");
    let add_button: gtk4::Button = builder
        .object("add_button")
        .expect("window.ui defines `add_button`");

    window.set_application(Some(app));

    let detail_view = repository_detail::build(window.clone());
    let detail_page = adw::NavigationPage::builder()
        .title("Repository")
        .child(&detail_view.widget)
        .build();

    let split_view = adw::NavigationSplitView::builder()
        .sidebar(&sidebar_page)
        .content(&detail_page)
        .build();
    window.set_content(Some(&split_view));

    let reload_sidebar_cell: ReloadSidebarCell = Rc::new(RefCell::new(None));

    let refresh_sidebar = {
        let sidebar_stack = sidebar_stack.clone();
        let repo_list = repo_list.clone();
        let entries = entries.clone();
        let store = store.clone();
        let window = window.clone();
        let reload_sidebar_cell = reload_sidebar_cell.clone();
        move |loaded: Vec<RepositoryEntry>| {
            while let Some(row) = repo_list.row_at_index(0) {
                repo_list.remove(&row);
            }
            let reload = reload_sidebar_cell
                .borrow()
                .clone()
                .expect("reload handle set before first refresh");
            for entry in &loaded {
                repo_list.append(&repository_row(
                    entry,
                    store.clone(),
                    window.clone(),
                    reload.clone(),
                ));
            }
            sidebar_stack.set_visible_child_name(if loaded.is_empty() { "empty" } else { "list" });
            *entries.borrow_mut() = loaded;
        }
    };

    let reload_sidebar: Rc<dyn Fn()> = {
        let store = store.clone();
        let refresh_sidebar = refresh_sidebar.clone();
        Rc::new(move || refresh_sidebar(store.load()))
    };
    *reload_sidebar_cell.borrow_mut() = Some(reload_sidebar.clone());

    detail_view.show_placeholder();

    repo_list.connect_row_selected({
        let entries = entries.clone();
        move |_, row| match row {
            Some(row) => {
                if let Some(entry) = entries.borrow().get(row.index() as usize) {
                    detail_view.load(entry.clone());
                }
            }
            None => detail_view.show_placeholder(),
        }
    });

    reload_sidebar();

    add_button.connect_clicked({
        let window = window.clone();
        let store = store.clone();
        let reload_sidebar = reload_sidebar.clone();
        move |_| {
            let reload_sidebar = reload_sidebar.clone();
            add_repository::present(&window, store.clone(), move |_entry| {
                reload_sidebar();
            });
        }
    });

    let about_action = gio::SimpleAction::new("about", None);
    about_action.connect_activate({
        let window = window.clone();
        move |_, _| present_about_dialog(&window)
    });
    app.add_action(&about_action);

    window.present();
}

fn present_about_dialog(parent: &adw::ApplicationWindow) {
    let debug_info = format!(
        "Version: {}\nTag: {}\nGit commit: {}\nBuild date: {}",
        env!("CARGO_PKG_VERSION"),
        env!("RESTIC_VIEWER_TAG"),
        env!("RESTIC_VIEWER_GIT_COMMIT"),
        env!("RESTIC_VIEWER_BUILD_DATE"),
    );

    let about = adw::AboutDialog::builder()
        .application_name("Restic Viewer")
        .application_icon("drive-harddisk-symbolic")
        .developer_name("Bruno Paz")
        .version(env!("CARGO_PKG_VERSION"))
        .comments("Browse and restore restic Repositories.")
        .debug_info(debug_info)
        .build();

    about.present(Some(parent));
}

fn repository_row(
    entry: &RepositoryEntry,
    store: Rc<RepositoryStore>,
    window: adw::ApplicationWindow,
    reload_sidebar: Rc<dyn Fn()>,
) -> adw::ActionRow {
    let subtitle = match &entry.backend {
        restic_client::Backend::Local { path } => path.display().to_string(),
        restic_client::Backend::Sftp { host, path, .. } => format!("sftp:{host}:{path}"),
        restic_client::Backend::S3 {
            endpoint, bucket, ..
        } => format!("s3:{endpoint}/{bucket}"),
    };
    let row = adw::ActionRow::builder()
        .title(entry.name.as_str())
        .subtitle(subtitle)
        .build();

    let rename_button = gtk4::Button::builder()
        .icon_name("document-edit-symbolic")
        .tooltip_text("Rename")
        .valign(gtk4::Align::Center)
        .css_classes(["flat"])
        .build();
    rename_button.connect_clicked({
        let window = window.clone();
        let store = store.clone();
        let reload_sidebar = reload_sidebar.clone();
        let id = entry.id.clone();
        let current_name = entry.name.clone();
        move |_| {
            present_rename_dialog(
                &window,
                store.clone(),
                id.clone(),
                current_name.clone(),
                reload_sidebar.clone(),
            )
        }
    });

    let remove_button = gtk4::Button::builder()
        .icon_name("user-trash-symbolic")
        .tooltip_text("Remove")
        .valign(gtk4::Align::Center)
        .css_classes(["flat"])
        .build();
    remove_button.connect_clicked({
        let window = window.clone();
        let store = store.clone();
        let reload_sidebar = reload_sidebar.clone();
        let id = entry.id.clone();
        let name = entry.name.clone();
        move |_| {
            present_remove_confirmation(
                &window,
                store.clone(),
                id.clone(),
                name.clone(),
                reload_sidebar.clone(),
            )
        }
    });

    row.add_suffix(&rename_button);
    row.add_suffix(&remove_button);
    row
}

fn present_rename_dialog(
    parent: &adw::ApplicationWindow,
    store: Rc<RepositoryStore>,
    id: String,
    current_name: String,
    reload_sidebar: Rc<dyn Fn()>,
) {
    let builder = gtk4::Builder::from_resource(&format!("{RESOURCE_PREFIX}/rename_dialog.ui"));
    let dialog: adw::Dialog = builder
        .object("dialog")
        .expect("rename_dialog.ui defines `dialog`");
    let name_row: adw::EntryRow = builder
        .object("name_row")
        .expect("rename_dialog.ui defines `name_row`");
    let cancel_button: gtk4::Button = builder
        .object("cancel_button")
        .expect("rename_dialog.ui defines `cancel_button`");
    let save_button: gtk4::Button = builder
        .object("save_button")
        .expect("rename_dialog.ui defines `save_button`");

    name_row.set_text(&current_name);

    cancel_button.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            dialog.close();
        }
    });

    save_button.connect_clicked({
        let dialog = dialog.clone();
        let name_row = name_row.clone();
        move |_| {
            let new_name = name_row.text().to_string();
            if new_name.is_empty() {
                return;
            }
            let _ = store.rename(&id, &new_name);
            reload_sidebar();
            dialog.close();
        }
    });

    dialog.present(Some(parent));
}

fn present_remove_confirmation(
    parent: &adw::ApplicationWindow,
    store: Rc<RepositoryStore>,
    id: String,
    name: String,
    reload_sidebar: Rc<dyn Fn()>,
) {
    let dialog = adw::AlertDialog::new(
        Some("Remove Repository?"),
        Some(&format!(
            "\"{name}\" will be removed from Restic Viewer. The Repository itself and its data are not affected."
        )),
    );
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("remove", "Remove");
    dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
    dialog.set_close_response("cancel");
    dialog.set_default_response(Some("cancel"));

    dialog.connect_response(None, move |_, response| {
        if response != "remove" {
            return;
        }
        let _ = store.remove(&id);
        glib::spawn_future_local({
            let id = id.clone();
            async move {
                let _ = crate::keyring::delete(&id, crate::keyring::CredentialKind::Password).await;
                let _ = crate::keyring::delete(&id, crate::keyring::CredentialKind::BackendSecret)
                    .await;
            }
        });
        reload_sidebar();
    });

    dialog.present(Some(parent));
}
