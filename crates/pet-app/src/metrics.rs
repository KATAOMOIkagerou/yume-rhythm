use std::mem::size_of;
use windows_sys::Win32::Foundation::FILETIME;
use windows_sys::Win32::System::ProcessStatus::{
    K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessSample {
    /// Private committed bytes (`PrivateUsage` from `PROCESS_MEMORY_COUNTERS_EX`).
    pub private_bytes: u64,
    /// Resident working set bytes (`WorkingSetSize`).
    pub working_set_bytes: u64,
    /// Cumulative kernel + user CPU time in 100ns ticks.
    pub cpu_time_100ns: u64,
    /// Wall timestamp in monotonic milliseconds.
    pub timestamp_ms: f64,
}

impl ProcessSample {
    pub fn capture(timestamp_ms: f64) -> Self {
        let mut private_bytes = 0u64;
        let mut working_set_bytes = 0u64;
        let mut cpu_time_100ns = 0u64;

        unsafe {
            let proc_handle = GetCurrentProcess();
            let mut mem: PROCESS_MEMORY_COUNTERS_EX = std::mem::zeroed();
            mem.cb = size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
            if K32GetProcessMemoryInfo(
                proc_handle,
                (&mut mem as *mut PROCESS_MEMORY_COUNTERS_EX).cast(),
                mem.cb,
            ) != 0
            {
                private_bytes = mem.PrivateUsage as u64;
                working_set_bytes = mem.WorkingSetSize as u64;
            }

            let mut creation = FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            };
            let mut exit = FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            };
            let mut kernel = FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            };
            let mut user = FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            };
            if GetProcessTimes(proc_handle, &mut creation, &mut exit, &mut kernel, &mut user) != 0 {
                cpu_time_100ns = filetime_to_u64(kernel).saturating_add(filetime_to_u64(user));
            }
        }

        Self {
            private_bytes,
            working_set_bytes,
            cpu_time_100ns,
            timestamp_ms,
        }
    }

    pub fn private_mib(&self) -> f64 {
        (self.private_bytes as f64) / (1024.0 * 1024.0)
    }

    pub fn working_set_mib(&self) -> f64 {
        (self.working_set_bytes as f64) / (1024.0 * 1024.0)
    }

    /// Computes normalized whole-system CPU percentage between two samples (0..100%).
    pub fn cpu_percent_since(&self, earlier: &ProcessSample) -> f64 {
        let dt_ms = self.timestamp_ms - earlier.timestamp_ms;
        if dt_ms <= 1.0 {
            return 0.0;
        }
        let dcpu_ms = self.cpu_time_100ns.saturating_sub(earlier.cpu_time_100ns) as f64 / 10_000.0;
        let logical_cpus = std::thread::available_parallelism()
            .map(|n| n.get() as f64)
            .unwrap_or(1.0)
            .max(1.0);
        ((dcpu_ms / (dt_ms * logical_cpus)) * 100.0).clamp(0.0, 100.0)
    }
}

fn filetime_to_u64(ft: FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_sample_captures_nonzero_memory() {
        let s = ProcessSample::capture(1000.0);
        assert!(s.private_bytes > 0, "private memory should be > 0");
        assert!(s.working_set_bytes > 0, "working set should be > 0");
    }
}
