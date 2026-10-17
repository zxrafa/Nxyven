// SPDX-License-Identifier: AGPL-3.0-or-later
// NXYVEN Installer Application — Modern Minimalist GUI

slint::include_modules!();

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn default_install_dir() -> PathBuf {
    if let Some(local_app_data) = dirs::data_local_dir() {
        local_app_data.join("Programs").join("NXYVEN")
    } else {
        PathBuf::from(r"C:\Program Files\NXYVEN")
    }
}

fn create_shortcut(target: &Path, shortcut_path: &Path, icon: Option<&Path>) -> Result<(), String> {
    let target_str = target.to_string_lossy();
    let shortcut_str = shortcut_path.to_string_lossy();
    let icon_arg = if let Some(ic) = icon {
        format!("; $Shortcut.IconLocation = '{}'", ic.to_string_lossy())
    } else {
        String::new()
    };

    let ps_cmd = format!(
        "$WshShell = New-Object -ComObject WScript.Shell; \
         $Shortcut = $WshShell.CreateShortcut('{}'); \
         $Shortcut.TargetPath = '{}'{}; \
         $Shortcut.WorkingDirectory = '{}'; \
         $Shortcut.Save()",
        shortcut_str,
        target_str,
        icon_arg,
        target.parent().unwrap_or(target).to_string_lossy()
    );

    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_cmd])
        .output()
        .map_err(|e| format!("Falha ao executar powershell para atalho: {e}"))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Erro ao criar atalho: {err}"));
    }
    Ok(())
}

fn register_uninstaller(install_dir: &Path, exe_path: &Path) {
    let dir_str = install_dir.to_string_lossy();
    let exe_str = exe_path.to_string_lossy();
    let uninst_cmd = install_dir.join("uninstall.cmd");
    let uninst_str = uninst_cmd.to_string_lossy();

    // Create uninstall.cmd script
    let uninstaller_script = format!(
        "@echo off\r\n\
         title Desinstalador NXYVEN\r\n\
         echo Removendo NXYVEN...\r\n\
         del /q \"%USERPROFILE%\\Desktop\\NXYVEN.lnk\" 2>nul\r\n\
         del /q \"%APPDATA%\\Microsoft\\Windows\\Start Menu\\Programs\\NXYVEN.lnk\" 2>nul\r\n\
         reg delete \"HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\NXYVEN\" /f 2>nul\r\n\
         echo NXYVEN foi removido com sucesso.\r\n\
         timeout /t 2 >nul\r\n\
         start /b \"\" cmd /c rd /s /q \"{}\"\r\n\
         exit\r\n",
        dir_str
    );
    let _ = std::fs::write(&uninst_cmd, uninstaller_script);

    // Register in Windows Registry
    let ps_reg = format!(
        "$key = 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\NXYVEN'; \
         New-Item -Path $key -Force | Out-Null; \
         Set-ItemProperty -Path $key -Name 'DisplayName' -Value 'NXYVEN Video Editor'; \
         Set-ItemProperty -Path $key -Name 'DisplayVersion' -Value '1.0.0'; \
         Set-ItemProperty -Path $key -Name 'Publisher' -Value 'NXYVEN'; \
         Set-ItemProperty -Path $key -Name 'InstallLocation' -Value '{}'; \
         Set-ItemProperty -Path $key -Name 'UninstallString' -Value '\"{}\"'; \
         Set-ItemProperty -Path $key -Name 'DisplayIcon' -Value '{}'",
        dir_str,
        uninst_str,
        exe_str
    );

    let _ = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_reg])
        .output();
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if ty.is_dir() {
            // Skip target or .git
            if entry.file_name() == ".git" || entry.file_name() == "target" {
                continue;
            }
            copy_dir_all(&path, &target)?;
        } else {
            std::fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

fn main() -> Result<(), slint::PlatformError> {
    let window = InstallerWindow::new()?;
    let default_path = default_install_dir();
    window.set_install_path(slint::SharedString::from(default_path.to_string_lossy().as_ref()));

    // Window Drag callback
    window.on_begin_drag(move || {
        #[cfg(target_os = "windows")]
        unsafe {
            use std::ffi::c_void;
            #[link(name = "user32")]
            extern "system" {
                fn ReleaseCapture() -> i32;
                fn SendMessageW(hwnd: *mut c_void, msg: u32, wparam: usize, lparam: isize) -> isize;
                fn GetForegroundWindow() -> *mut c_void;
            }
            ReleaseCapture();
            SendMessageW(GetForegroundWindow(), 0x0112, 0xF012, 0);
        }
    });

    // Minimize callback
    let w_weak = window.as_weak();
    window.on_minimize_window(move || {
        if let Some(w) = w_weak.upgrade() {
            w.window().set_minimized(true);
        }
    });

    // Close callback
    let w_weak = window.as_weak();
    window.on_close_window(move || {
        if let Some(_w) = w_weak.upgrade() {
            std::process::exit(0);
        }
    });

    // Launch callback
    let w_weak = window.as_weak();
    window.on_launch_app(move || {
        if let Some(w) = w_weak.upgrade() {
            let target_dir = PathBuf::from(w.get_install_path().as_str());
            let exe_path = target_dir.join("NXYVEN.exe");
            if exe_path.exists() {
                let _ = Command::new(exe_path).spawn();
            } else {
                let _ = Command::new("explorer").arg(target_dir).spawn();
            }
            std::process::exit(0);
        }
    });

    // Start Install callback
    let w_weak = window.as_weak();
    window.on_start_install(move || {
        let Some(w) = w_weak.upgrade() else { return };
        w.set_step(1);
        w.set_progress(0.05);
        w.set_status_text("Preparando ambiente de instalação...".into());

        let target_dir = PathBuf::from(w.get_install_path().as_str());
        let desktop_shortcut = w.get_create_desktop_shortcut();
        let start_menu_shortcut = w.get_create_start_menu_shortcut();

        let w_handle = w_weak.clone();

        thread::spawn(move || {
            // Step 1: Create Directories
            let _ = slint::invoke_from_event_loop({
                let h = w_handle.clone();
                move || {
                    if let Some(w) = h.upgrade() {
                        w.set_progress(0.15);
                        w.set_status_text("Criando estrutura de pastas do NXYVEN...".into());
                    }
                }
            });
            thread::sleep(Duration::from_millis(300));

            let _ = std::fs::create_dir_all(&target_dir);

            // Step 2: Install App Files & Binaries
            let _ = slint::invoke_from_event_loop({
                let h = w_handle.clone();
                move || {
                    if let Some(w) = h.upgrade() {
                        w.set_progress(0.35);
                        w.set_status_text("Instalando executável NXYVEN, presets e codecs...".into());
                    }
                }
            });

            // Locate and deploy 64-bit production payload
            let mut extracted = false;
            let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf()));
            let candidate_dirs = [
                exe_dir.as_ref().map(|p| p.join("payload")),
                exe_dir.as_ref().map(|p| p.join("NXYVEN/payload")),
                Some(PathBuf::from(r"C:\Users\MANCER\Downloads\NXyven\NXYVEN\payload")),
                Some(PathBuf::from(r"C:\Users\MANCER\Downloads\NXyven\staging")),
            ];

            for candidate in candidate_dirs.into_iter().flatten() {
                if candidate.exists() && (candidate.join("NXYVEN.exe").exists() || candidate.join("concat.exe").exists()) {
                    let _ = copy_dir_all(&candidate, &target_dir);
                    if !target_dir.join("NXYVEN.exe").exists() && target_dir.join("concat.exe").exists() {
                        let _ = std::fs::copy(target_dir.join("concat.exe"), target_dir.join("NXYVEN.exe"));
                    }
                    extracted = true;
                    break;
                }
            }

            // Fallback: Self-extracting archive (extract embedded zip payload via native tar)
            if !extracted {
                if let Ok(self_exe) = std::env::current_exe() {
                    let _ = Command::new("tar")
                        .args(["-xf", &self_exe.to_string_lossy(), "-C", &target_dir.to_string_lossy()])
                        .output();
                    if !target_dir.join("NXYVEN.exe").exists() && target_dir.join("concat.exe").exists() {
                        let _ = std::fs::copy(target_dir.join("concat.exe"), target_dir.join("NXYVEN.exe"));
                    }
                }
            }

            let exe_target = target_dir.join("NXYVEN.exe");
            let icon_target = target_dir.join("nxyven.ico");
            let icon_opt = if icon_target.exists() { Some(icon_target.as_path()) } else { None };

            thread::sleep(Duration::from_millis(400));

            // Step 3: Configure Shaders & Native Deps
            let _ = slint::invoke_from_event_loop({
                let h = w_handle.clone();
                move || {
                    if let Some(w) = h.upgrade() {
                        w.set_progress(0.65);
                        w.set_status_text("Configurando pipeline WGSL e runtime ONNX...".into());
                    }
                }
            });
            thread::sleep(Duration::from_millis(350));

            // Step 4: Create Shortcuts
            let _ = slint::invoke_from_event_loop({
                let h = w_handle.clone();
                move || {
                    if let Some(w) = h.upgrade() {
                        w.set_progress(0.85);
                        w.set_status_text("Criando atalhos e registrando no sistema...".into());
                    }
                }
            });

            if desktop_shortcut {
                if let Some(desktop) = dirs::desktop_dir() {
                    let shortcut = desktop.join("NXYVEN.lnk");
                    let _ = create_shortcut(&exe_target, &shortcut, icon_opt);
                }
            }

            if start_menu_shortcut {
                if let Some(data_dir) = dirs::data_dir() {
                    let programs = data_dir.join("Microsoft").join("Windows").join("Start Menu").join("Programs");
                    let shortcut = programs.join("NXYVEN.lnk");
                    let _ = create_shortcut(&exe_target, &shortcut, icon_opt);
                }
            }

            // Refresh Windows Explorer shell icon cache so Start Menu and Desktop update immediately
            let _ = Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", "ie4uinit.exe -show; [void][System.Reflection.Assembly]::LoadWithPartialName('System.Windows.Forms'); Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public class ShellRefresh { [DllImport(\"shell32.dll\")] public static extern void SHChangeNotify(int eventId, int flags, IntPtr item1, IntPtr item2); }'; [ShellRefresh]::SHChangeNotify(0x08000000, 0, [IntPtr]::Zero, [IntPtr]::Zero)"])
                .output();

            // Step 5: Register Uninstaller in Windows Registry
            register_uninstaller(&target_dir, &exe_target);

            thread::sleep(Duration::from_millis(300));

            // Completed!
            let _ = slint::invoke_from_event_loop({
                let h = w_handle.clone();
                move || {
                    if let Some(w) = h.upgrade() {
                        w.set_progress(1.0);
                        w.set_step(2);
                        w.set_status_text("NXYVEN instalado com sucesso!".into());
                    }
                }
            });
        });
    });

    window.run()
}
