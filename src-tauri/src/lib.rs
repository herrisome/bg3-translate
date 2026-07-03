//! BG3 MOD 汉化工具 — Tauri 后端入口。

pub mod commands;
pub mod config;
pub mod error;
pub mod formats;
pub mod pak;
pub mod translation;
pub mod types;

use commands::AppState;
use std::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState::default())
        .manage::<Mutex<Option<types::LlmSettings>>>(Mutex::new(None))
        .invoke_handler(tauri::generate_handler![
            commands::open_mod,
            commands::read_file_entries,
            commands::write_file_entries,
            commands::repack_mod,
            commands::translate_entries,
            commands::save_llm_settings,
            commands::load_llm_settings,
        ])
        .run(tauri::generate_context!())
        .expect("运行 Tauri 应用时出错");
}
