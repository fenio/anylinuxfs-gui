mod cli;
mod commands;

use std::sync::Arc;
use commands::{
    list_disks, mount_disk, unmount_disk, force_cleanup,
    get_mount_status, check_cli,
    get_log_content, start_log_stream, start_disk_watcher, stop_watchers,
    get_config, update_config,
    WatcherState,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(Arc::new(WatcherState::default()))
        .invoke_handler(tauri::generate_handler![
            list_disks,
            mount_disk,
            unmount_disk,
            force_cleanup,
            get_mount_status,
            check_cli,
            get_log_content,
            start_log_stream,
            start_disk_watcher,
            stop_watchers,
            get_config,
            update_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
