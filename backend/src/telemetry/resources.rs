use std::sync::{Mutex, OnceLock};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

pub fn render_prometheus() -> String {
    static SYSTEM: OnceLock<Mutex<System>> = OnceLock::new();
    let Ok(pid) = sysinfo::get_current_pid() else {
        return String::new();
    };
    let Ok(mut system) = SYSTEM.get_or_init(|| Mutex::new(System::new())).lock() else {
        return String::new();
    };
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing()
            .with_cpu()
            .with_memory()
            .with_disk_usage(),
    );
    let Some(process) = system.process(pid) else {
        return String::new();
    };
    let disk = process.disk_usage();
    format!(
        "# TYPE xfchess_process_resident_memory_bytes gauge\n\
         xfchess_process_resident_memory_bytes {}\n\
         # TYPE xfchess_process_cpu_cores gauge\n\
         xfchess_process_cpu_cores {}\n\
         # TYPE xfchess_process_read_bytes_total counter\n\
         xfchess_process_read_bytes_total {}\n\
         # TYPE xfchess_process_written_bytes_total counter\n\
         xfchess_process_written_bytes_total {}\n",
        process.memory(),
        process.cpu_usage() / 100.0,
        disk.total_read_bytes,
        disk.total_written_bytes
    )
}
