//! Host profile (file 01 §10.2). Collected by the app on the user's machine; never sent anywhere.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GpuVendorClass { Intel, Nvidia, Amd, Apple, Qualcomm, Software, Unknown }

impl GpuVendorClass {
    pub fn as_str(self) -> &'static str {
        match self { Self::Intel => "intel", Self::Nvidia => "nvidia", Self::Amd => "amd", Self::Apple => "apple",
                     Self::Qualcomm => "qualcomm", Self::Software => "software", Self::Unknown => "unknown" }
    }
    pub fn from_pci_vendor(id: u32) -> Self {
        match id { 0x8086 => Self::Intel, 0x10DE => Self::Nvidia, 0x1002 | 0x1022 => Self::Amd,
                   0x106B => Self::Apple, 0x5143 | 0x17CB => Self::Qualcomm, 0x1414 => Self::Software, _ => Self::Unknown }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostMonitor { pub width: u32, pub height: u32, pub dpr: f64, pub primary: bool }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostProfile {
    pub os_family: String,          // "windows" | "macos" | "linux"
    pub os_version: String,         // e.g. "10.0.26100"
    pub cpu_arch: String,           // "x86" | "arm"
    pub logical_cores: u32,
    pub physical_mem_gb: f64,
    pub gpu_vendor: GpuVendorClass,
    pub gpu_name: String,           // display only, never put into a set
    pub gpu_render_class: String,   // e.g. "d3d11/intel"
    pub monitors: Vec<HostMonitor>,
    pub is_laptop: bool,
    pub max_touch_points: u32,
}

impl HostProfile {
    pub fn primary_monitor(&self) -> HostMonitor {
        self.monitors.iter().find(|m| m.primary).or(self.monitors.first()).cloned()
            .unwrap_or(HostMonitor { width: 1920, height: 1080, dpr: 1.0, primary: true })
    }
    pub fn host_class(&self) -> String { format!("{}-{}/{}", self.os_family, self.cpu_arch, self.gpu_render_class) }
    pub fn os_label(&self) -> String {
        match self.os_family.as_str() {
            "windows" => {
                let build: u32 = self.os_version.rsplit('.').next().and_then(|b| b.parse().ok()).unwrap_or(0);
                if build >= 22000 { "Windows 11".into() } else { "Windows 10".into() }
            }
            "macos" => "macOS".into(),
            o => o.to_string(),
        }
    }
    pub fn gpu_label(&self) -> String { self.gpu_vendor.as_str().to_uppercase() }

    pub fn example_windows() -> Self {
        Self {
            os_family: "windows".into(), os_version: "10.0.26100".into(), cpu_arch: "x86".into(),
            logical_cores: 12, physical_mem_gb: 16.0, gpu_vendor: GpuVendorClass::Intel,
            gpu_name: "Intel(R) Iris(R) Xe Graphics".into(), gpu_render_class: "d3d11/intel".into(),
            monitors: vec![HostMonitor { width: 1920, height: 1080, dpr: 1.25, primary: true }],
            is_laptop: true, max_touch_points: 0,
        }
    }
}

/// Chrome's reduced User-Agent (frozen OS tokens, minor versions zeroed).
pub fn ua_template(os_family: &str, arch: &str) -> String {
    let os = match (os_family, arch) {
        ("windows", _) => "Windows NT 10.0; Win64; x64",
        ("macos", _) => "Macintosh; Intel Mac OS X 10_15_7",
        ("linux", _) => "X11; Linux x86_64",
        _ => "X11; Linux x86_64",
    };
    format!("Mozilla/5.0 ({os}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{{{{engine.major}}}}.0.0.0 Safari/537.36")
}

pub fn navigator_platform(os_family: &str) -> &'static str {
    match os_family { "windows" => "Win32", "macos" => "MacIntel", _ => "Linux x86_64" }
}

pub fn uach_platform(os_family: &str) -> &'static str {
    match os_family { "windows" => "Windows", "macos" => "macOS", "android" => "Android", _ => "Linux" }
}
