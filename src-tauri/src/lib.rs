//! Desktop app shell. Commands here are thin wrappers around the `dxcore` engine.

use dxcore::model::{CheckInfo, Inventory, MachineRecord, ScanEvent, ScanReport};
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
            relaunch_as_admin
        ])
        .run(tauri::generate_context!())
        .expect("error while running DxTool");
}
