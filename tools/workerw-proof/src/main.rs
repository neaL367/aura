#![cfg_attr(target_os = "windows", windows_subsystem = "console")]

#[cfg(target_os = "windows")]
mod win;

#[cfg(target_os = "windows")]
fn main() -> Result<(), windows::core::Error> {
    win::main()
}

#[cfg(not(target_os = "windows"))]
fn main() {
    println!("workerw-proof is only supported on Windows.");
}
