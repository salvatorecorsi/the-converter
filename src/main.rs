#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod app;
mod command;
fn main() {
    command::main();
}
