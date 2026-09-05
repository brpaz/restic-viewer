//! Resolves the version/tag/commit/build-date shown on the About page.
//!
//! CI writes `build-info.env` (repo root, KEY=VALUE lines) before invoking
//! flatpak-builder — the Flatpak module's `dir` source excludes `.git`, so `git`
//! itself isn't available inside the sandboxed build, and CI has already computed
//! these from the real checkout (with full history/tags) beforehand. The build date
//! is the *commit's* date (`git log -1 --format=%ct`, the SOURCE_DATE_EPOCH
//! convention), not the wall-clock build time, so the same commit always produces the
//! same embedded value regardless of when or where it's built.
//!
//! Locally (no `build-info.env`, e.g. this repo isn't `git init`-ed yet, or a plain
//! `cargo build` outside CI), this falls back to live `git`/`date` commands, or
//! "unknown"/today's date if those aren't available either.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("set by cargo");
    compile_blueprints(&manifest_dir);

    let build_info_path = Path::new(&manifest_dir).join("../../build-info.env");

    let from_file = std::fs::read_to_string(&build_info_path)
        .ok()
        .map(|contents| parse_env_file(&contents));

    let version = from_file
        .as_ref()
        .and_then(|m| m.get("VERSION").cloned())
        .unwrap_or_else(git_describe);
    let commit = from_file
        .as_ref()
        .and_then(|m| m.get("COMMIT").cloned())
        .unwrap_or_else(git_short_commit);
    let build_date = from_file
        .as_ref()
        .and_then(|m| m.get("BUILD_DATE").cloned())
        .unwrap_or_else(commit_date_or_today);

    println!("cargo:rustc-env=RESTIC_VIEWER_TAG={version}");
    println!("cargo:rustc-env=RESTIC_VIEWER_GIT_COMMIT={commit}");
    println!("cargo:rustc-env=RESTIC_VIEWER_BUILD_DATE={build_date}");

    println!("cargo:rerun-if-changed={}", build_info_path.display());
    println!("cargo:rerun-if-changed=../../.git/HEAD");
}

/// Compiles `resources/*.blp` (Blueprint) to `.ui` (GtkBuilder XML) with
/// `blueprint-compiler`, then bundles those into a GResource embedded in the binary
/// via `gio::resources_register_include!("compiled.gresource")` at runtime (see
/// `main.rs`). Blueprint files are the widget layout; Rust code loads them with
/// `gtk4::Builder` and wires signals/dynamic behavior — see ADR-0005.
fn compile_blueprints(manifest_dir: &str) {
    let resources_dir = Path::new(manifest_dir).join("resources");
    let out_dir = std::env::var("OUT_DIR").expect("set by cargo");
    let compiled_ui_dir = Path::new(&out_dir).join("blueprint");
    std::fs::create_dir_all(&compiled_ui_dir).expect("create blueprint output dir");

    let blueprint_files: Vec<std::path::PathBuf> = std::fs::read_dir(&resources_dir)
        .expect("read resources dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "blp"))
        .collect();

    let status = Command::new("blueprint-compiler")
        .arg("batch-compile")
        .arg(&compiled_ui_dir)
        .arg(&resources_dir)
        .args(&blueprint_files)
        .status()
        .expect("run blueprint-compiler (is it installed? `dnf install blueprint-compiler` / bundled in the GNOME SDK)");
    assert!(status.success(), "blueprint-compiler failed");

    glib_build_tools::compile_resources(
        &[&compiled_ui_dir],
        "resources/resources.gresource.xml",
        "compiled.gresource",
    );

    println!("cargo:rerun-if-changed=resources");
}

fn parse_env_file(contents: &str) -> HashMap<String, String> {
    contents
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .collect()
}

fn git_describe() -> String {
    run_git(&["describe", "--tags", "--always", "--dirty"]).unwrap_or_else(|| "unknown".to_string())
}

fn git_short_commit() -> String {
    run_git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".to_string())
}

fn commit_date_or_today() -> String {
    if let Some(epoch) = run_git(&["log", "-1", "--format=%ct"])
        && let Some(date) = format_epoch_as_date(&epoch)
    {
        return date;
    }
    Command::new("date")
        .args(["-u", "+%Y-%m-%d"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn format_epoch_as_date(epoch_seconds: &str) -> Option<String> {
    Command::new("date")
        .args(["-u", "-d", &format!("@{epoch_seconds}"), "+%Y-%m-%d"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn run_git(args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
