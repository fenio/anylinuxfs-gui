mod cli;
mod commands;

use std::sync::{Arc, Mutex};
use commands::{
    list_disks, mount_disk, unmount_disk, eject_disk, force_cleanup,
    get_mount_status, check_cli,
    get_log_content, start_log_stream, start_disk_watcher, stop_watchers,
    get_config, update_config,
    start_shell, write_shell, resize_shell, stop_shell,
    list_images, install_image, uninstall_image,
    list_packages, add_packages, remove_packages,
    WatcherState, PtyState,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(Arc::new(WatcherState::default()))
        .manage(Arc::new(Mutex::new(PtyState::default())))
        .invoke_handler(tauri::generate_handler![
            list_disks,
            mount_disk,
            unmount_disk,
            eject_disk,
            force_cleanup,
            get_mount_status,
            check_cli,
            get_log_content,
            start_log_stream,
            start_disk_watcher,
            stop_watchers,
            get_config,
            update_config,
            start_shell,
            write_shell,
            resize_shell,
            stop_shell,
            list_images,
            install_image,
            uninstall_image,
            list_packages,
            add_packages,
            remove_packages,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
