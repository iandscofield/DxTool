//! Desktop app shell. Commands here are thin wrappers around the `dxcore` engine.

use dxcore::model::{CheckInfo, DriverUpdate, FixEvent, FixInfo, FixRequest, FixRun, Inventory, MachineRecord, ScanEvent, ScanReport};
use serde_json::Value;
use tauri::ipc::Channel;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: &'static str,
    is_admin: bool,
    computer_name: String,
    firmware: &'static str,
    debug_build: bool,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        is_admin: dxcore::platform::is_admin(),
        computer_name: dxcore::platform::computer_name(),
        firmware: dxcore::platform::firmware_type(),
        debug_build: cfg!(debug_assertions),
    }
}

/// Collects the full hardware inventory. Runs on a worker thread so the window stays responsive.
#[tauri::command]
async fn get_inventory() -> Result<Inventory, String> {
    tauri::async_runtime::spawn_blocking(dxcore::inventory::collect)
        .await
        .map_err(|e| e.to_string())
}

/// Every health check, for the Custom scan picker.
#[tauri::command]
fn scan_catalog() -> Vec<CheckInfo> {
    dxcore::health::catalog()
}

/// Runs a Fast, Full or Custom scan, streaming progress to the window.
#[tauri::command]
async fn run_scan(kind: String, checks: Vec<String>, on_event: Channel<ScanEvent>) -> Result<ScanReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let send = |e: ScanEvent| {
            let _ = on_event.send(e);
        };
        let mut report = dxcore::health::run_scan(&kind, &checks, &send);
        // Every scan is kept in the per-machine history.
        report.saved = dxcore::history::save(&report).is_ok();
        report
    })
    .await
    .map_err(|e| e.to_string())
}

/// Id of the PC DxTool is running on (matches the history folder name).
#[tauri::command]
async fn current_machine() -> Result<dxcore::model::MachineInfo, String> {
    tauri::async_runtime::spawn_blocking(dxcore::machine::collect).await.map_err(|e| e.to_string())
}

#[tauri::command]
fn list_machines() -> Vec<MachineRecord> {
    dxcore::history::machines()
}

#[tauri::command]
async fn load_scan(machine_id: String, scan_id: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || dxcore::history::load(&machine_id, &scan_id))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn delete_scan(machine_id: String, scan_id: String) -> Result<(), String> {
    dxcore::history::delete(&machine_id, &scan_id)
}

#[tauri::command]
fn data_dir() -> String {
    dxcore::history::data_dir().to_string_lossy().to_string()
}

#[tauri::command]
fn load_settings() -> Value {
    dxcore::history::load_settings()
}

#[tauri::command]
fn save_settings(settings: Value) -> Result<(), String> {
    dxcore::history::save_settings(&settings)
}

/// Saves a report as HTML or PDF in the data folder and returns where it went.
#[tauri::command]
async fn export_report(machine_id: String, name: String, html: String, format: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || dxcore::report::export(&machine_id, &name, &html, &format))
        .await
        .map_err(|e| e.to_string())?
        .map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    dxcore::platform::open_path(&path)
}

#[tauri::command]
fn show_in_folder(path: String) -> Result<(), String> {
    dxcore::platform::show_in_folder(&path)
}

#[tauri::command]
fn fix_catalog() -> Vec<FixInfo> {
    dxcore::fixes::catalog()
}

/// Runs the chosen fixes (restore point first unless skipped), streaming progress.
#[tauri::command]
async fn run_fixes(requests: Vec<FixRequest>, skip_restore_point: bool, on_event: Channel<FixEvent>) -> Result<FixRun, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let send = |e: FixEvent| {
            let _ = on_event.send(e);
        };
        dxcore::fixes::run_fixes(&requests, skip_restore_point, &send)
    })
    .await
    .map_err(|e| e.to_string())
}

/// Asks Windows Update (online) which driver updates are available.
#[tauri::command]
async fn check_driver_updates(on_line: Channel<String>) -> Result<Vec<DriverUpdate>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        dxcore::fixes::drivers::check(&|l: &str| {
            let _ = on_line.send(l.to_string());
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn list_fix_runs(machine_id: String) -> Vec<FixRun> {
    dxcore::history::fix_runs(&machine_id)
}

#[tauri::command]
fn ntfs_drives() -> Vec<(String, String, String)> {
    dxcore::fixes::ntfs_drives()
}

#[tauri::command]
async fn restore_points() -> Vec<(u64, String, String)> {
    tauri::async_runtime::spawn_blocking(dxcore::fixes::restore_points).await.unwrap_or_default()
}

#[tauri::command]
async fn enable_system_protection() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(dxcore::fixes::enable_system_protection).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn open_system_restore() -> Result<(), String> {
    dxcore::fixes::open_system_restore()
}

#[tauri::command]
fn restart_pc() -> Result<(), String> {
    dxcore::fixes::restart_pc()
}

#[tauri::command]
fn cancel_restart() -> Result<(), String> {
    dxcore::fixes::cancel_restart()
}

/// Restarts DxTool with administrator rights (Windows shows the UAC prompt).
#[tauri::command]
fn relaunch_as_admin(app: tauri::AppHandle) -> Result<(), String> {
    dxcore::platform::relaunch_as_admin()?;
    app.exit(0);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            app_info,
            get_inventory,
            scan_catalog,
            run_scan,
            current_machine,
            list_machines,
            load_scan,
            delete_scan,
            data_dir,
            load_settings,
            save_settings,
            export_report,
            open_path,
            show_in_folder,
            fix_catalog,
            run_fixes,
            check_driver_updates,
            list_fix_runs,
            ntfs_drives,
            restore_points,
            enable_system_protection,
            open_system_restore,
            restart_pc,
            cancel_restart,
            relaunch_as_admin
        ])
        .run(tauri::generate_context!())
        .expect("error while running DxTool");
}
