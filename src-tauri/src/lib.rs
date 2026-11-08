//! Desktop app shell. Commands here are thin wrappers around the `dxcore` engine.

use dxcore::model::{AvEvent, AvStatus, QuarantineItem, VirusScan, CheckInfo, DriverUpdate, FixEvent, FixInfo, FixRequest, FixRun, Inventory, MachineRecord, ScanEvent, ScanReport};
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

// ───────────────────────── Virus scanning ─────────────────────────

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn av_status() -> Result<AvStatus, String> {
    blocking(dxcore::av::status).await
}

#[tauri::command]
async fn virus_scan(engine: String, kind: String, paths: Vec<String>, on_event: Channel<AvEvent>) -> Result<VirusScan, String> {
    blocking(move || dxcore::av::scan(&engine, &kind, &paths, &|e| {
        let _ = on_event.send(e);
    }))
    .await?
}

#[tauri::command]
fn av_cancel() {
    dxcore::av::cancel();
}

#[tauri::command]
async fn av_update(engine: String, on_event: Channel<AvEvent>) -> Result<String, String> {
    blocking(move || {
        let send = |e: AvEvent| {
            let _ = on_event.send(e);
        };
        if engine == "defender" { dxcore::av::defender::update(&send) } else { dxcore::av::clamav::update(&send) }
    })
    .await?
}

#[tauri::command]
async fn clamav_install(on_event: Channel<AvEvent>) -> Result<String, String> {
    blocking(move || dxcore::av::clamav::install(&|e| {
        let _ = on_event.send(e);
    }))
    .await?
}

#[tauri::command]
async fn clamav_remove() -> Result<(), String> {
    blocking(dxcore::av::clamav::remove).await?
}

#[tauri::command]
fn virus_scans(machine_id: String) -> Vec<VirusScan> {
    dxcore::av::scans(&machine_id)
}

#[tauri::command]
fn delete_virus_scan(machine_id: String, scan_id: String) -> Result<(), String> {
    dxcore::av::delete_scan(&machine_id, &scan_id)
}

#[tauri::command]
async fn defender_remove(machine_id: String, scan_id: String) -> Result<VirusScan, String> {
    blocking(move || dxcore::av::defender::remove_active(&machine_id, &scan_id)).await?
}

#[tauri::command]
fn threat_quarantine(machine_id: String, scan_id: String, path: String) -> Result<QuarantineItem, String> {
    dxcore::av::quarantine(&machine_id, &scan_id, &path)
}

#[tauri::command]
fn threat_delete(machine_id: String, scan_id: String, path: String) -> Result<(), String> {
    dxcore::av::delete_threat(&machine_id, &scan_id, &path)
}

#[tauri::command]
fn quarantine_list() -> Vec<QuarantineItem> {
    dxcore::av::quarantine_list()
}

#[tauri::command]
fn quarantine_restore(id: String) -> Result<String, String> {
    dxcore::av::restore(&id)
}

#[tauri::command]
fn quarantine_delete(id: String) -> Result<(), String> {
    dxcore::av::delete_quarantined(&id)
}

#[tauri::command]
fn open_windows_security() -> Result<(), String> {
    dxcore::av::open_windows_security()
}

// ───────────────────────── Claude ─────────────────────────

#[tauri::command]
fn claude_status() -> Value {
    dxcore::mcp::claude_status()
}

#[tauri::command]
fn claude_setup(remove: bool) -> Result<String, String> {
    dxcore::mcp::claude_setup(remove)
}

/// Runs one MCP tool so the Settings page can show what Claude will see.
#[tauri::command]
async fn mcp_try_tool(name: String) -> Result<String, String> {
    blocking(move || dxcore::mcp::call_tool(&name, &serde_json::json!({}))).await?
}

// ───────────────────────── Updates ─────────────────────────

#[tauri::command]
fn install_kind() -> &'static str {
    dxcore::update::install_kind()
}

/// Looks up the newest DxTool release on GitHub (only when asked).
#[tauri::command]
async fn update_check() -> Result<dxcore::update::UpdateInfo, String> {
    blocking(dxcore::update::check).await?
}

#[derive(Clone, serde::Serialize)]
struct Progress {
    done: u64,
    total: u64,
}

/// Downloads and installs the newest version, then closes this window so the new one can take over.
#[tauri::command]
async fn update_install(app: tauri::AppHandle, on_progress: Channel<Progress>) -> Result<String, String> {
    let msg = blocking(move || {
        dxcore::update::install(&|done, total| {
            let _ = on_progress.send(Progress { done, total });
        })
    })
    .await??;
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(1500));
        app.exit(0);
    });
    Ok(msg)
}

/// `DxTool.exe --mcp`: serve Claude over stdin/stdout instead of opening a window.
pub fn run_mcp() {
    dxcore::mcp::serve(env!("CARGO_PKG_VERSION"));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
            relaunch_as_admin,
            av_status,
            virus_scan,
            av_cancel,
            av_update,
            clamav_install,
            clamav_remove,
            virus_scans,
            delete_virus_scan,
            defender_remove,
            threat_quarantine,
            threat_delete,
            quarantine_list,
            quarantine_restore,
            quarantine_delete,
            open_windows_security,
            claude_status,
            claude_setup,
            mcp_try_tool,
            install_kind,
            update_check,
            update_install
        ])
        .setup(|_| {
            dxcore::update::cleanup();
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running DxTool");
}
