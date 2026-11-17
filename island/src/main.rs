#![cfg_attr(windows, windows_subsystem = "windows")]
fn main() -> std::process::ExitCode {
    daozai::shell::standalone()
}
