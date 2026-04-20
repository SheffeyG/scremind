use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

pub static AUTOSTART_ENABLED: AtomicBool = AtomicBool::new(false);

const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn init() {
    let enabled = check_autostart_file();
    AUTOSTART_ENABLED.store(enabled, Ordering::SeqCst);
    log::info!("Autostart initialized: enabled={}", enabled);
}

pub fn is_enabled() -> bool {
    AUTOSTART_ENABLED.load(Ordering::SeqCst)
}

pub fn toggle() {
    let autostart_path = get_autostart_path();
    let enabled = AUTOSTART_ENABLED.load(Ordering::SeqCst);

    if enabled {
        let _ = std::fs::remove_file(&autostart_path);
        AUTOSTART_ENABLED.store(false, Ordering::SeqCst);
        log::info!("Autostart disabled");
    } else {
        let exe_path = std::env::current_exe().unwrap_or_default();
        create_shortcut(&autostart_path, &exe_path.to_string_lossy());
        AUTOSTART_ENABLED.store(true, Ordering::SeqCst);
        log::info!("Autostart enabled");
    }
}

fn check_autostart_file() -> bool {
    let autostart_path = get_autostart_path();
    if !autostart_path.exists() {
        return false;
    }

    let exe_path = std::env::current_exe().unwrap_or_default();
    let shortcut_target = get_shortcut_target(&autostart_path);
    shortcut_target == exe_path.to_string_lossy().to_string()
}

fn get_autostart_path() -> PathBuf {
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    PathBuf::from(appdata)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join("Startup")
        .join("scremind.lnk")
}

fn create_shortcut(shortcut_path: &PathBuf, target: &str) {
    let working_dir = std::path::Path::new(target)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let ps_script = format!(
        r#"$ws = New-Object -ComObject WScript.Shell; $s = $ws.CreateShortcut('{}'); $s.TargetPath = '{}'; $s.WorkingDirectory = '{}'; $s.Save()"#,
        shortcut_path.to_string_lossy().replace('\'', "''"),
        target.replace('\'', "''"),
        working_dir.replace('\'', "''")
    );

    let _ = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}

fn get_shortcut_target(shortcut_path: &PathBuf) -> String {
    let ps_script = format!(
        r#"$ws = New-Object -ComObject WScript.Shell; $s = $ws.CreateShortcut('{}'); $s.TargetPath"#,
        shortcut_path.to_string_lossy().replace('\'', "''")
    );

    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => String::new(),
    }
}
