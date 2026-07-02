#![windows_subsystem = "windows"]

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    scremind::app::run()
}
