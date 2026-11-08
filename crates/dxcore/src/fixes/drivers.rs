//! Driver updates from Windows Update, matched against the drivers installed now.

use std::collections::HashMap;
use std::time::Duration;

use serde_json::Value;

use super::runner::powershell;
use super::tail;
use crate::model::{DriverUpdate, FixStepResult};
use crate::wmiutil::{ns_query, RowExt};

/// Asks Windows Update (online) which driver updates are available for this PC.
pub fn check(on_line: &dyn Fn(&str)) -> Result<Vec<DriverUpdate>, String> {
    let script = r#"
$ErrorActionPreference = 'Stop'
$s = New-Object -ComObject Microsoft.Update.Session
$s.ClientApplicationID = 'DxTool'
$r = $s.CreateUpdateSearcher().Search("IsInstalled=0 and Type='Driver' and IsHidden=0")
$out = foreach ($u in $r.Updates) {
  [pscustomobject]@{
    updateId = $u.Identity.UpdateID; title = $u.Title; manufacturer = [string]$u.DriverManufacturer
    driverClass = [string]$u.DriverClass; hardwareId = [string]$u.DriverHardwareID
    newDate = $(if ($u.DriverVerDate) { $u.DriverVerDate.ToString('yyyy-MM-dd') } else { '' })
    sizeBytes = [int64]$u.MaxDownloadSize
  }
}
'DXJSON' + (ConvertTo-Json -InputObject @($out) -Compress -Depth 3)
"#;
    on_line("Asking Windows Update for driver updates. This can take a few minutes…");
    let (code, lines) = powershell(script, Duration::from_secs(15 * 60), &|l: &str| {
        if !l.starts_with("DXJSON") {
            on_line(l)
        }
    })?;
    let json = lines.iter().find_map(|l| l.strip_prefix("DXJSON")).ok_or_else(|| {
        let msg = lines.iter().rev().find(|l| !l.trim().is_empty()).cloned().unwrap_or_default();
        format!("Windows Update didn't answer (code {code}). Check the internet connection. {msg}")
    })?;
    let mut updates: Vec<DriverUpdate> = serde_json::from_str(json).map_err(|e| format!("Unexpected answer from Windows Update: {e}"))?;

    // Match each update to the device it's for, to show the installed version.
    let devices = ns_query("ROOT\\CIMV2", "SELECT Name, PNPDeviceID, HardwareID FROM Win32_PnPEntity");
    let drivers: HashMap<String, (String, String)> = ns_query("ROOT\\CIMV2", "SELECT DeviceID, DriverVersion, DriverDate FROM Win32_PnPSignedDriver")
        .iter()
        .filter_map(|d| {
            Some((
                d.s("DeviceID")?.to_uppercase(),
                (
                    d.s("DriverVersion").unwrap_or_default(),
                    d.s("DriverDate").and_then(|x| crate::fmt::wmi_to_naive(&x)).map(|t| t.format("%Y-%m-%d").to_string()).unwrap_or_default(),
                ),
            ))
        })
        .collect();
    for u in &mut updates {
        let hw = u.hardware_id.to_uppercase();
        if let Some(dev) = devices.iter().find(|d| !hw.is_empty() && d.list("HardwareID").iter().any(|h| h.to_uppercase() == hw)) {
            u.device_name = dev.s("Name").unwrap_or_default();
            if let Some((v, d)) = dev.s("PNPDeviceID").and_then(|id| drivers.get(&id.to_uppercase())) {
                u.installed_version = v.clone();
                u.installed_date = d.clone();
            }
        }
        // Titles end in the version: "Intel - Display - 31.0.101.5186" or "... Update (1.0.4.55794)".
        u.new_version = u
            .title
            .split(|c: char| c.is_whitespace() || c == '(' || c == ')')
            .rev()
            .find(|t| t.chars().next().is_some_and(|c| c.is_ascii_digit()) && t.matches('.').count() >= 2 && t.chars().all(|c| c.is_ascii_digit() || c == '.'))
            .unwrap_or("")
            .to_string();
        // For firmware, the "installed driver" is a generic Microsoft one; compare with the real BIOS.
        if u.is_firmware() {
            if let Some(b) = ns_query("ROOT\\CIMV2", "SELECT SMBIOSBIOSVersion, ReleaseDate FROM Win32_BIOS").first() {
                u.installed_version = format!("BIOS {}", b.s("SMBIOSBIOSVersion").unwrap_or_default());
                u.installed_date = b.s("ReleaseDate").and_then(|x| crate::fmt::wmi_to_naive(&x)).map(|t| t.format("%Y-%m-%d").to_string()).unwrap_or_default();
            }
            u.warning = "This is a BIOS/firmware update. Keep the PC plugged in and don't turn it off while it installs; a failed firmware update can stop the PC from starting.".into();
        }
        if !u.new_date.is_empty() && !u.installed_date.is_empty() && u.new_date < u.installed_date {
            u.warning = format!("The offered version ({}) is older than what's installed ({}). Don't install it.", u.new_date, u.installed_date);
        }
        if u.device_name.is_empty() {
            let part = u.title.split(" - ").nth(1).unwrap_or(&u.title).to_string();
            u.device_name = format!("{part} (device not connected right now)");
        }
    }
    on_line(&format!("Found {} driver update(s).", updates.len()));
    Ok(updates)
}

fn is_guid(s: &str) -> bool {
    s.len() == 36 && s.chars().enumerate().all(|(i, c)| if matches!(i, 8 | 13 | 18 | 23) { c == '-' } else { c.is_ascii_hexdigit() })
}

/// Downloads and installs the chosen driver updates (options: { updateIds: [...] }).
pub fn install(o: &Value, emit: &dyn Fn(&str), progress: &dyn Fn(f64), s: &mut FixStepResult) -> Result<(), String> {
    let ids: Vec<String> = o
        .get("updateIds")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str()).filter(|v| is_guid(v)).map(str::to_string).collect())
        .unwrap_or_default();
    if ids.is_empty() {
        return Err("No driver updates were selected. Check for driver updates first and tick the ones to install.".into());
    }
    let list = ids.iter().map(|i| format!("'{i}'")).collect::<Vec<_>>().join(",");
    let script = format!(
        r#"
$ErrorActionPreference = 'Stop'
$ids = @({list})
$s = New-Object -ComObject Microsoft.Update.Session
$s.ClientApplicationID = 'DxTool'
'Finding the selected updates…'
$r = $s.CreateUpdateSearcher().Search("IsInstalled=0 and Type='Driver'")
$col = New-Object -ComObject Microsoft.Update.UpdateColl
foreach ($u in $r.Updates) {{ if ($ids -contains $u.Identity.UpdateID) {{ if (-not $u.EulaAccepted) {{ $u.AcceptEula() }}; [void]$col.Add($u) }} }}
"Found $($col.Count) of $($ids.Count) selected updates."
if ($col.Count -eq 0) {{ 'DXRESULT|none'; exit 3 }}
'Downloading… 10%'
$d = $s.CreateUpdateDownloader(); $d.Updates = $col; $dr = $d.Download()
"Download finished (result $($dr.ResultCode)). 50%"
'Installing…'
$i = $s.CreateUpdateInstaller(); $i.Updates = $col; $ir = $i.Install()
for ($k = 0; $k -lt $col.Count; $k++) {{ $x = $ir.GetUpdateResult($k); "DXITEM|$($x.ResultCode)|$('{{0:X8}}' -f $x.HResult)|$($col.Item($k).Title)" }}
"DXRESULT|$($ir.ResultCode)|$($ir.RebootRequired)"
'Done. 100%'
"#
    );
    let (_, lines) = powershell(&script, Duration::from_secs(60 * 60), &|l: &str| {
        if let Some(p) = l.find('%').and_then(|i| l[..i].rsplit(' ').next()?.parse::<f64>().ok()) {
            progress(p);
        }
        if !l.starts_with("DX") {
            emit(l.trim_end_matches(|c: char| c.is_ascii_digit() || c == '%' || c == ' '));
        }
    })?;
    s.output_tail = tail(&lines);
    let mut ok = 0;
    let mut bad = 0;
    for l in lines.iter().filter_map(|l| l.strip_prefix("DXITEM|")) {
        let parts: Vec<&str> = l.splitn(3, '|').collect();
        let (code, hr, title) = (parts.first().copied().unwrap_or(""), parts.get(1).copied().unwrap_or(""), parts.get(2).copied().unwrap_or(""));
        // OperationResultCode: 2 = succeeded, 3 = succeeded with errors
        if code == "2" || code == "3" {
            ok += 1;
            s.details.push(format!("Installed: {title}"));
        } else {
            bad += 1;
            s.details.push(format!("Failed (0x{hr}): {title}"));
        }
    }
    let summary_line = lines.iter().find_map(|l| l.strip_prefix("DXRESULT|")).unwrap_or("");
    s.needs_restart = summary_line.ends_with("|True");
    if summary_line == "none" {
        s.status = "failed".into();
        s.summary = "The selected updates are no longer offered by Windows Update (they may already be installed). Check again.".into();
    } else if bad == 0 && ok > 0 {
        s.summary = format!("Installed {ok} driver update(s).{}", if s.needs_restart { " Restart the PC to finish." } else { "" });
    } else if ok > 0 {
        s.status = "warning".into();
        s.summary = format!("Installed {ok} driver update(s); {bad} failed.");
    } else {
        s.status = "failed".into();
        s.summary = if lines.iter().any(|l| l.contains("0x80240044") || l.to_lowercase().contains("access is denied")) {
            "Installing drivers needs administrator rights.".into()
        } else {
            "The driver updates could not be installed. See the output for details.".into()
        };
    }
    Ok(())
}
