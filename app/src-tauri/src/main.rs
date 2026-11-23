//! The ventris app. Everything is in the library, so that it can be tested without a window.

// Without this, a release build on Windows opens a console window beside the app's own.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    ventris_app::run()
}
