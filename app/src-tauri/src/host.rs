//! Host probe (file 01 §10.2). Stays on this machine; used only to keep profiles host-native.
use core_fp::{GpuVendorClass, HostMonitor, HostProfile};

pub fn probe(monitors: Vec<HostMonitor>) -> HostProfile {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    let cores = std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(4);
    let mem_gb = sys.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0);
    let (os_family, os_version) = os();
    let (vendor, name) = gpu();
    let render_class = format!("{}/{}", if os_family == "windows" { "d3d11" } else { "gl" }, vendor.as_str());
    HostProfile {
        os_family, os_version,
        cpu_arch: if cfg!(target_arch = "aarch64") { "arm".into() } else { "x86".into() },
        logical_cores: cores, physical_mem_gb: (mem_gb * 10.0).round() / 10.0,
        gpu_vendor: vendor, gpu_name: name, gpu_render_class: render_class,
        monitors: if monitors.is_empty() { vec![HostMonitor { width: 1920, height: 1080, dpr: 1.0, primary: true }] } else { monitors },
        is_laptop: has_battery(), max_touch_points: 0,
    }
}

fn os() -> (String, String) {
    let fam = if cfg!(windows) { "windows" } else if cfg!(target_os = "macos") { "macos" } else { "linux" };
    let ver = if cfg!(windows) {
        // sysinfo kernel_version on Windows is the build number, e.g. "26100".
        format!("10.0.{}", sysinfo::System::kernel_version().unwrap_or_default())
    } else {
        sysinfo::System::os_version().unwrap_or_default()
    };
    (fam.into(), ver)
}

#[cfg(windows)]
fn gpu() -> (GpuVendorClass, String) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command",
               "Get-CimInstance Win32_VideoController | Where-Object { $_.PNPDeviceID -like 'PCI*' } | Select-Object -First 1 Name,PNPDeviceID | ConvertTo-Json -Compress"])
        .creation_flags(CREATE_NO_WINDOW).output();
    let Ok(out) = out else { return (GpuVendorClass::Unknown, String::new()) };
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
    let name = v["Name"].as_str().unwrap_or("").to_string();
    let pnp = v["PNPDeviceID"].as_str().unwrap_or("").to_ascii_uppercase();
    let vendor = pnp.split("VEN_").nth(1).and_then(|s| u32::from_str_radix(&s[..4.min(s.len())], 16).ok()).map(GpuVendorClass::from_pci_vendor).unwrap_or(GpuVendorClass::Unknown);
    (vendor, name)
}

#[cfg(not(windows))]
fn gpu() -> (GpuVendorClass, String) { (GpuVendorClass::Unknown, String::new()) }

#[cfg(windows)]
fn has_battery() -> bool {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", "(Get-CimInstance Win32_Battery | Measure-Object).Count"])
        .creation_flags(0x0800_0000).output()
        .ok().and_then(|o| String::from_utf8(o.stdout).ok()).and_then(|s| s.trim().parse::<u32>().ok()).map(|n| n > 0).unwrap_or(false)
}
#[cfg(not(windows))]
fn has_battery() -> bool { std::path::Path::new("/sys/class/power_supply/BAT0").exists() }
