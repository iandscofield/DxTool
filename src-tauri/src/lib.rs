//! Desktop app shell. Commands here are thin wrappers around the `dxcore` engine.

use dxcore::model::Inventory;

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
        .invoke_handler(tauri::generate_handler![app_info, get_inventory, relaunch_as_admin])
        .run(tauri::generate_context!())
        .expect("error while running DxTool");
}
