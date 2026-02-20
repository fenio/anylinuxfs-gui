mod cache;
mod cli;
mod commands;
mod error;
mod paths;

pub use error::{AppError, AppResult};
pub use paths::{get_socket_path, get_log_path, COMMAND_TIMEOUT_SECS, MOUNT_TIMEOUT_SECS};

use std::sync::{Arc, Mutex};
use tauri::Manager;
use tauri::tray::TrayIconBuilder;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri_plugin_log::{Target, TargetKind};

#[cfg(target_os = "macos")]
fn set_dock_visible(visible: bool) {
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
    use objc2_foundation::MainThreadMarker;
    // Safe: tray/window event handlers always run on the main thread in Tauri
    let mtm = unsafe { MainThreadMarker::new_unchecked() };
    let app = NSApplication::sharedApplication(mtm);
    let policy = if visible {
        NSApplicationActivationPolicy::Regular
    } else {
        NSApplicationActivationPolicy::Accessory
    };
    app.setActivationPolicy(policy);
}

use commands::{
    list_disks, mount_disk, unmount_disk, eject_disk, force_cleanup,
    get_mount_status, check_cli,
    get_log_content, start_log_stream, start_disk_watcher, stop_watchers,
    get_config, update_config,
    start_shell, write_shell, resize_shell, stop_shell,
    list_images, install_image, uninstall_image,
    list_packages, add_packages, remove_packages,
    list_custom_actions, create_custom_action, update_custom_action, delete_custom_action,
    WatcherState, PtyState,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::LogDir { file_name: None }),
            Target::new(TargetKind::Webview),
        ]).build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(Arc::new(WatcherState::default()))
        .manage(Arc::new(Mutex::new(PtyState::default())))
        .setup(|app| {
            let show_item = MenuItemBuilder::with_id("show", "Show").build(app)?;
            let unmount_item = MenuItemBuilder::with_id("unmount", "Unmount").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
            let menu = MenuBuilder::new(app)
                .item(&show_item)
                .item(&unmount_item)
                .separator()
                .item(&quit_item)
                .build()?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .icon_as_template(true)
                .menu(&menu)
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click { button: tauri::tray::MouseButton::Left, .. } = event {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                                #[cfg(target_os = "macos")]
                                set_dock_visible(false);
                            } else {
                                #[cfg(target_os = "macos")]
                                set_dock_visible(true);
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .on_menu_event(|app, event| {
                    match event.id().as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                #[cfg(target_os = "macos")]
                                set_dock_visible(true);
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "unmount" => {
                            let app = app.clone();
                            tauri::async_runtime::spawn(async move {
                                let _ = unmount_disk(app).await;
                            });
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = window.hide();
                    #[cfg(target_os = "macos")]
                    set_dock_visible(false);
                }
                tauri::WindowEvent::Destroyed => {
                    let app = window.app_handle();
                    // Stop watchers
                    let watcher_state = app.state::<Arc<WatcherState>>();
                    watcher_state.shutdown();
                    // Stop PTY
                    let pty_arc = app.state::<Arc<Mutex<PtyState>>>().inner().clone();
                    let lock_result = pty_arc.lock();
                    if let Ok(mut state) = lock_result {
                        state.shutdown();
                    }
                }
                _ => {}
            }
        })
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
            list_custom_actions,
            create_custom_action,
            update_custom_action,
            delete_custom_action,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
