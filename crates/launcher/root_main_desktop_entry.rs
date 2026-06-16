// PURPOSE: Root desktop entry — Tauri v2 binary bootstrap. Starts the app_lib::run() loop.

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    app_lib::run();
}
