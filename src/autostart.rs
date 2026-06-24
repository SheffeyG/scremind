use std::io;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
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

pub fn toggle() -> io::Result<bool> {
    let enabled = is_enabled();
    let new_enabled = if enabled {
        disable_autostart()?
    } else {
        enable_autostart()?
    };

    AUTOSTART_ENABLED.store(new_enabled, Ordering::SeqCst);
    Ok(new_enabled)
}

fn enable_autostart() -> io::Result<bool> {
    let autostart_path = get_autostart_path();
    let exe_path = std::env::current_exe()?;
    create_shortcut(&autostart_path, &exe_path)?;

    let enabled = check_autostart_file();
    if enabled {
        log::info!("Autostart enabled");
        Ok(true)
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "startup shortcut was created but verification failed",
        ))
    }
}

fn disable_autostart() -> io::Result<bool> {
    let autostart_path = get_autostart_path();
    if autostart_path.exists() {
        std::fs::remove_file(&autostart_path)?;
    }

    let enabled = check_autostart_file();
    if !enabled {
        log::info!("Autostart disabled");
        Ok(false)
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "startup shortcut still points to the current executable",
        ))
    }
}

fn check_autostart_file() -> bool {
    let autostart_path = get_autostart_path();
    if !autostart_path.exists() {
        return false;
    }

    let exe_path = std::env::current_exe().unwrap_or_default();
    match get_shortcut_target(&autostart_path) {
        Ok(shortcut_target) => shortcut_target == exe_path,
        Err(e) => {
            log::warn!(
                "Failed to inspect autostart shortcut {}: {}",
                autostart_path.display(),
                e
            );
            false
        }
    }
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

fn create_shortcut(shortcut_path: &Path, target: &Path) -> io::Result<()> {
    let working_dir = target.parent().unwrap_or_else(|| Path::new(""));

    let ps_script = format!(
        r#"$ws = New-Object -ComObject WScript.Shell; $s = $ws.CreateShortcut('{}'); $s.TargetPath = '{}'; $s.WorkingDirectory = '{}'; $s.Save()"#,
        escape_ps_single_quoted(shortcut_path),
        escape_ps_single_quoted(target),
        escape_ps_single_quoted(working_dir)
    );

    run_powershell(&ps_script).map(|_| ())
}

fn get_shortcut_target(shortcut_path: &Path) -> io::Result<PathBuf> {
    let ps_script = format!(
        r#"$ws = New-Object -ComObject WScript.Shell; $s = $ws.CreateShortcut('{}'); $s.TargetPath"#,
        escape_ps_single_quoted(shortcut_path)
    );

    let output = run_powershell(&ps_script)?;
    let target = output.trim();
    if target.is_empty() {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "shortcut target is empty",
        ))
    } else {
        Ok(PathBuf::from(target))
    }
}

fn run_powershell(script: &str) -> io::Result<String> {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("powershell command failed: {}", stderr),
        ))
    }
}

fn escape_ps_single_quoted(path: &Path) -> String {
    path.to_string_lossy().replace('\'', "''")
}
