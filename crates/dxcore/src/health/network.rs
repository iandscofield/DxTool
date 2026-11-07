//! Network check. Uses only what Windows already knows; DxTool sends nothing online.

use std::time::Duration;

use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

use super::{field, item, num_field, run_hidden, Ctx};
use crate::fmt;
use crate::model::{CheckResult, Finding, Status};
use crate::wmiutil::{ns_query, RowExt};

fn connectivity(v: u64) -> &'static str {
    match v {
        0 => "Disconnected",
        1 => "No traffic",
        2 => "Local subnet only",
        3 => "Local network only",
        4 => "Internet",
        _ => "Unknown",
    }
}

pub fn network(_ctx: &Ctx, r: &mut CheckResult) -> Result<(), String> {
    let profiles = ns_query("ROOT\\StandardCimv2", "SELECT Name, InterfaceAlias, NetworkCategory, IPv4Connectivity, IPv6Connectivity FROM MSFT_NetConnectionProfile");
    let configs = ns_query("ROOT\\CIMV2", "SELECT Description, InterfaceIndex, IPAddress, DefaultIPGateway, DNSServerSearchOrder, DHCPEnabled, MACAddress FROM Win32_NetworkAdapterConfiguration WHERE IPEnabled = TRUE");
    let adapters = ns_query("ROOT\\StandardCimv2", "SELECT Name, InterfaceDescription, InterfaceIndex, Status, LinkSpeed, NdisPhysicalMedium, HardwareInterface, Virtual FROM MSFT_NetAdapter");

    let internet = profiles.iter().any(|p| p.u("IPv4Connectivity") == Some(4) || p.u("IPv6Connectivity") == Some(4));
    r.facts.push(field("Internet (as Windows sees it)", if internet { "Connected" } else { "Not connected" }));

    for (i, p) in profiles.iter().enumerate() {
        let alias = p.s("InterfaceAlias").unwrap_or_default();
        let ad = adapters.iter().find(|a| a.s("Name").as_deref() == Some(alias.as_str()));
        let idx = ad.and_then(|a| a.u("InterfaceIndex"));
        let cfg = configs.iter().find(|c| c.u("InterfaceIndex") == idx && idx.is_some());
        let v4 = p.u("IPv4Connectivity").unwrap_or(0);
        let v6 = p.u("IPv6Connectivity").unwrap_or(0);
        let best = v4.max(v6);
        let ips = cfg.map(|c| c.list("IPAddress")).unwrap_or_default();
        let gw = cfg.map(|c| c.list("DefaultIPGateway")).unwrap_or_default();
        let dns = cfg.map(|c| c.list("DNSServerSearchOrder")).unwrap_or_default();
        let speed = ad.and_then(|a| a.f("LinkSpeed"));
        let mut it = item(
            format!("net{i}"),
            p.s("Name").unwrap_or_else(|| alias.clone()),
            alias.clone(),
            vec![
                field("Adapter", alias.clone()),
                field("Connectivity", connectivity(best)),
                field("Network type", match p.u("NetworkCategory").unwrap_or(0) { 1 => "Private", 2 => "Domain", _ => "Public" }),
                field("IP addresses", ips.join(", ")),
                field("Gateway", gw.join(", ")),
                field("DNS servers", dns.join(", ")),
                num_field("Link speed", speed.map(fmt::bits_per_sec).unwrap_or_default(), speed.unwrap_or(0.0)),
                field("DHCP", cfg.and_then(|c| c.b("DHCPEnabled")).map(fmt::yes_no).unwrap_or("")),
            ],
        );
        it.status = if best == 4 { Status::Ok } else { Status::Info };
        if ips.iter().any(|ip| ip.starts_with("169.254.")) {
            it.status = Status::Warn;
            r.findings.push(
                Finding::new(format!("apipa_{i}"), Status::Warn, format!("{alias} could not get an address from the router"), "The adapter gave itself a temporary 169.254.x.x address, which means it did not hear back from the router's DHCP server. It cannot reach the internet like this.")
                    .advice("Restart the router and the PC, check the cable or Wi-Fi connection, or use the network reset in Fixes.")
                    .fix("network_reset"),
            );
        }
        // Wired link running slower than it should
        let wired = ad.and_then(|a| a.u("NdisPhysicalMedium")) == Some(14);
        if let Some(s) = speed {
            if wired && s > 0.0 && s <= 100_000_000.0 {
                r.findings.push(
                    Finding::new(format!("slow_link_{i}"), Status::Info, format!("{alias} is connected at only {}", fmt::bits_per_sec(s)), "Most wired connections today run at 1 Gbps or more. 100 Mbps or less usually means a damaged or old cable, or a slow port on the switch or router.")
                        .advice("Try a different network cable (Cat5e or better) or a different port."),
                );
            }
        }
        r.items.push(it);
    }
    if profiles.is_empty() {
        r.findings.push(Finding::new("no_network", Status::Warn, "Not connected to any network", "No network connection is active.").advice("Connect a network cable or join a Wi-Fi network."));
    } else if !internet {
        r.findings.push(
            Finding::new("no_internet", Status::Warn, "Windows reports no internet access", "The PC is connected to a network but Windows cannot reach the internet through it.")
                .advice("Restart the router and PC. If other devices work, try the network reset in Fixes.")
                .fix("network_reset"),
        );
    }

    // Wi-Fi signal
    if let Ok((_, out)) = run_hidden("netsh.exe", &["wlan", "show", "interfaces"], Duration::from_secs(10)) {
        let get = |key: &str| out.lines().find_map(|l| {
            let (k, v) = l.split_once(':')?;
            (k.trim().eq_ignore_ascii_case(key)).then(|| v.trim().to_string())
        });
        if let (Some(ssid), Some(sig)) = (get("SSID"), get("Signal")) {
            let pct: f64 = sig.trim_end_matches('%').trim().parse().unwrap_or(0.0);
            r.facts.push(num_field("Wi-Fi", format!("{ssid} · signal {sig} · {}", get("Radio type").unwrap_or_default()), pct));
            if pct > 0.0 && pct < 40.0 {
                r.findings.push(
                    Finding::new("wifi_weak", if pct < 25.0 { Status::Warn } else { Status::Info }, format!("Weak Wi-Fi signal ({sig})"), "A weak signal makes the connection slow and unreliable.")
                        .advice("Move closer to the router, remove obstacles, or add a mesh point or Wi-Fi extender."),
                );
            }
        }
    }

    // Proxy
    if let Ok(k) = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings") {
        if k.get_value::<u32, _>("ProxyEnable").unwrap_or(0) == 1 {
            let server: String = k.get_value("ProxyServer").unwrap_or_default();
            r.facts.push(field("Proxy", server.clone()));
            r.findings.push(
                Finding::new("proxy_set", Status::Info, "A proxy server is set", format!("Web traffic is sent through {server}. Workplaces use proxies on purpose, but adware sometimes sets one to spy on browsing."))
                    .advice("If you didn't set this up, turn it off in Settings → Network & internet → Proxy."),
            );
        }
    }

    // Hosts file entries
    let hosts = format!("{}\\System32\\drivers\\etc\\hosts", std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into()));
    if let Ok(text) = std::fs::read_to_string(&hosts) {
        let entries: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
        r.facts.push(num_field("Hosts file entries", entries.len().to_string(), entries.len() as f64));
        if !entries.is_empty() {
            r.findings.push(
                Finding::new("hosts_entries", Status::Info, format!("The hosts file has {} custom entries", entries.len()), "The hosts file overrides where website names point. Ad blockers and developers use it; malware can use it to redirect or block sites.")
                    .advice("If you don't recognise these entries, have a technician review them.")
                    .evidence(vec![field("Entries", entries.iter().take(20).cloned().collect::<Vec<_>>().join(" | "))]),
            );
        }
    }

    r.items_label = Some("Connections".into());
    r.summary = if internet {
        let first = r.items.iter().find(|i| i.status == Status::Ok).map(|i| i.subtitle.clone()).unwrap_or_default();
        format!("Connected to the internet via {first}")
    } else if profiles.is_empty() {
        "Not connected".into()
    } else {
        "Connected to a network without internet".into()
    };
    Ok(())
}
