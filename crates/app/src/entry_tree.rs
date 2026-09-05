//! The Entry tree browser for a single Snapshot: a lazily-loading GTK4 tree
//! (`TreeListModel` + `TreeExpander`) built on top of the restic client seam's
//! `list_tree` calls, with multi-selection feeding a "Restore Selected" action.
//!
//! A directory's children are only fetched the first time its row is expanded —
//! `TreeListModel`'s `create_func` runs for every row as soon as it enters the model
//! (even collapsed ones), so eagerly fetching there would eventually crawl the entire
//! Snapshot tree on load. Loading is instead triggered from `TreeListRow`'s `expanded`
//! notification, guarded by a `loaded` flag on the node so a re-bound row (GTK4 list
//! items are recycled during scrolling) never re-fetches the same directory twice.

use std::cell::{Cell, RefCell};

use gtk4::prelude::*;
use libadwaita as adw;
use restic_client::{Entry, EntryKind, RealResticClient, RepositoryConnection, ResticClient};

use crate::restore_dialog;

struct NodeData {
    entry: Entry,
    connection: RepositoryConnection,
    snapshot_id: String,
    child_store: RefCell<Option<gio::ListStore>>,
    loaded: Cell<bool>,
}

pub fn build(
    parent: adw::ApplicationWindow,
    connection: RepositoryConnection,
    snapshot_id: String,
    root_paths: Vec<String>,
) -> gtk4::Widget {
    let root_store = gio::ListStore::new::<glib::BoxedAnyObject>();
    for path in root_paths {
        let name = path
            .rsplit('/')
            .find(|s| !s.is_empty())
            .unwrap_or(&path)
            .to_string();
        let entry = Entry {
            name,
            path,
            kind: EntryKind::Directory,
            size: None,
            mtime: String::new(),
        };
        root_store.append(&glib::BoxedAnyObject::new(NodeData {
            entry,
            connection: connection.clone(),
            snapshot_id: snapshot_id.clone(),
            child_store: RefCell::new(None),
            loaded: Cell::new(false),
        }));
    }

    let tree_model = gtk4::TreeListModel::new(root_store, false, false, |obj| {
        let boxed = obj.downcast_ref::<glib::BoxedAnyObject>()?;
        let node = boxed.borrow::<NodeData>();
        if node.entry.kind != EntryKind::Directory {
            return None;
        }
        let mut child_store = node.child_store.borrow_mut();
        if child_store.is_none() {
            *child_store = Some(gio::ListStore::new::<glib::BoxedAnyObject>());
        }
        child_store.as_ref().map(|store| store.clone().upcast())
    });

    let selection_model = gtk4::MultiSelection::new(Some(tree_model));

    let factory = gtk4::SignalListItemFactory::new();
    factory.connect_setup(|_, list_item| {
        let list_item = list_item
            .downcast_ref::<gtk4::ListItem>()
            .expect("factory item is a ListItem");
        let name_label = gtk4::Label::builder().xalign(0.0).hexpand(true).build();
        let meta_label = gtk4::Label::builder().css_classes(["dim-label"]).build();
        let row_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        row_box.append(&name_label);
        row_box.append(&meta_label);
        let expander = gtk4::TreeExpander::new();
        expander.set_child(Some(&row_box));
        list_item.set_child(Some(&expander));
    });

    factory.connect_bind(|_, list_item| {
        let list_item = list_item
            .downcast_ref::<gtk4::ListItem>()
            .expect("factory item is a ListItem");
        let Some(row) = list_item
            .item()
            .and_then(|item| item.downcast::<gtk4::TreeListRow>().ok())
        else {
            return;
        };
        let Some(expander) = list_item
            .child()
            .and_then(|child| child.downcast::<gtk4::TreeExpander>().ok())
        else {
            return;
        };
        expander.set_list_row(Some(&row));

        let Some(boxed) = row
            .item()
            .and_then(|item| item.downcast::<glib::BoxedAnyObject>().ok())
        else {
            return;
        };
        {
            let node = boxed.borrow::<NodeData>();
            if let Some(row_box) = expander
                .child()
                .and_then(|c| c.downcast::<gtk4::Box>().ok())
                && let Some(name_label) = row_box
                    .first_child()
                    .and_then(|c| c.downcast::<gtk4::Label>().ok())
                && let Some(meta_label) = name_label
                    .next_sibling()
                    .and_then(|c| c.downcast::<gtk4::Label>().ok())
            {
                name_label.set_label(&node.entry.name);
                meta_label.set_label(&format_meta(&node.entry));
            }
        }

        maybe_load_children(&boxed, &row);
        row.connect_expanded_notify(move |row| maybe_load_children(&boxed, row));
    });

    let list_view = gtk4::ListView::new(Some(selection_model.clone()), Some(factory));
    let scroller: gtk4::Widget = gtk4::ScrolledWindow::builder()
        .child(&list_view)
        .vexpand(true)
        .build()
        .upcast();

    let restore_button = gtk4::Button::builder()
        .label("Restore Selected")
        .sensitive(false)
        .build();
    restore_button.connect_clicked({
        let selection_model = selection_model.clone();
        let connection = connection.clone();
        let snapshot_id = snapshot_id.clone();
        move |_| {
            let paths = selected_entry_paths(&selection_model);
            if paths.is_empty() {
                return;
            }
            restore_dialog::present(
                &parent,
                connection.clone(),
                snapshot_id.clone(),
                paths,
                "Restore Selected",
            );
        }
    });

    selection_model.connect_selection_changed({
        let restore_button = restore_button.clone();
        move |model, _, _| {
            restore_button.set_sensitive(model.selection().size() > 0);
        }
    });

    let header = adw::HeaderBar::new();
    header.pack_end(&restore_button);

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&scroller));
    toolbar_view.upcast()
}

fn selected_entry_paths(model: &gtk4::MultiSelection) -> Vec<String> {
    let mut paths = Vec::new();
    for position in 0..model.n_items() {
        if !model.is_selected(position) {
            continue;
        }
        if let Some(row) = model
            .item(position)
            .and_then(|item| item.downcast::<gtk4::TreeListRow>().ok())
            && let Some(boxed) = row
                .item()
                .and_then(|item| item.downcast::<glib::BoxedAnyObject>().ok())
        {
            paths.push(boxed.borrow::<NodeData>().entry.path.clone());
        }
    }
    paths
}

fn maybe_load_children(boxed: &glib::BoxedAnyObject, row: &gtk4::TreeListRow) {
    if !row.is_expanded() {
        return;
    }
    {
        let node = boxed.borrow::<NodeData>();
        if node.loaded.get() {
            return;
        }
        node.loaded.set(true);
    }

    let boxed = boxed.clone();
    glib::spawn_future_local(async move {
        let (connection, snapshot_id, path, store) = {
            let node = boxed.borrow::<NodeData>();
            (
                node.connection.clone(),
                node.snapshot_id.clone(),
                node.entry.path.clone(),
                node.child_store.borrow().clone(),
            )
        };
        let Some(store) = store else { return };

        if let Ok(children) = RealResticClient::system()
            .list_tree(&connection, &snapshot_id, &path)
            .await
        {
            for child in children {
                store.append(&glib::BoxedAnyObject::new(NodeData {
                    entry: child,
                    connection: connection.clone(),
                    snapshot_id: snapshot_id.clone(),
                    child_store: RefCell::new(None),
                    loaded: Cell::new(false),
                }));
            }
        }
        // A failed fetch just leaves this directory empty rather than surfacing an
        // error inline — acceptable for v1; revisit if this proves confusing.
    });
}

fn format_meta(entry: &Entry) -> String {
    match entry.size {
        Some(size) => format!(
            "{size} B · {}",
            entry.mtime.get(..19).unwrap_or(&entry.mtime)
        ),
        None => entry.mtime.get(..19).unwrap_or(&entry.mtime).to_string(),
    }
}
