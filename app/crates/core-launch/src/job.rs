//! Windows Job Object: the whole engine tree dies with the job (no orphaned engines with live
//! network paths) and an optional per-profile memory cap (file 04 §11.3).
use std::os::windows::io::AsRawHandle;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::JobObjects::*;

pub struct Job(HANDLE);
unsafe impl Send for Job {}

impl Job {
    pub fn new(ram_limit_mb: Option<u32>) -> std::io::Result<Self> {
        // SAFETY: plain Win32 calls with owned handles; the handle is closed in Drop.
        unsafe {
            let h = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if h.is_null() { return Err(std::io::Error::last_os_error()); }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if let Some(mb) = ram_limit_mb {
                info.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_JOB_MEMORY;
                info.JobMemoryLimit = (mb as usize) * 1024 * 1024;
            }
            let ok = SetInformationJobObject(h, JobObjectExtendedLimitInformation, &info as *const _ as *const _, std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32);
            if ok == 0 { let e = std::io::Error::last_os_error(); CloseHandle(h); return Err(e); }
            Ok(Self(h))
        }
    }
    pub fn assign(&self, child: &tokio::process::Child) -> std::io::Result<()> {
        let ph = child.raw_handle().ok_or_else(|| std::io::Error::other("no handle"))? as HANDLE;
        // SAFETY: valid job handle and live child process handle.
        if unsafe { AssignProcessToJobObject(self.0, ph) } == 0 { return Err(std::io::Error::last_os_error()); }
        Ok(())
    }
}

impl Drop for Job { fn drop(&mut self) { unsafe { CloseHandle(self.0); } } }

#[allow(dead_code)]
fn _assert_handle(_: &dyn AsRawHandle) {}
