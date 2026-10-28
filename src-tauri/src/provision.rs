//! Automatic provisioning of the virtual capture device used for system-wide
//! routing. Installers are downloaded from the official vendors, signature
//! checked and then launched elevated; nothing is redistributed with the app.

use crate::audio::devices;
use crate::models::AppView;
use crate::state::{emit_view, AppState};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tauri::{AppHandle, State};

const VB_CABLE_URL: &str =
    "https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack45.zip";
const DEVICE_TIMEOUT: Duration = Duration::from_secs(45);
const ERROR_GRACE: Duration = Duration::from_secs(3);

struct Provisioned {
    input_device: Option<String>,
}

#[tauri::command]
pub async fn install_virtual_cable(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppView, String> {
    let outcome = tauri::async_runtime::spawn_blocking(provision)
        .await
        .map_err(|err| format!("Falha ao preparar o cabo virtual: {err}"))?;

    state.refresh_devices();
    if let Ok(installed) = &outcome {
        select_devices(&state, installed.input_device.as_deref());
        state.ensure_default_preset();
        if !state.engine.info().running {
            if let Err(err) = state.start_engine("route", None) {
                state.push_warning(format!(
                    "Cabo virtual pronto, mas a rota não iniciou: {err}"
                ));
            }
        }
    }
    emit_view(&app, &state);
    outcome.map(|_| state.view())
}

fn select_devices(state: &AppState, installed: Option<&str>) {
    let devices = state.devices.read();
    let mut settings = state.store.snapshot().settings;
    let previous = settings.clone();
    let mut changed = false;

    if let Some(name) = installed {
        let current_valid = settings
            .input_device
            .as_deref()
            .and_then(|current| devices.inputs.iter().find(|device| device.name == current))
            .map(|device| device.is_virtual)
            .unwrap_or(false);
        if !current_valid {
            settings.input_device = Some(name.to_string());
            changed = true;
        }
    }

    // Pin the physical output now: once the system default becomes the cable,
    // "system default" would route the app's own output back into the capture.
    let output_valid = settings
        .output_device
        .as_deref()
        .and_then(|current| devices.outputs.iter().find(|device| device.name == current))
        .map(|device| !device.is_virtual)
        .unwrap_or(false);
    if !output_valid {
        if let Some(default) = devices
            .outputs
            .iter()
            .find(|device| device.is_default && !device.is_virtual)
        {
            settings.output_device = Some(default.name.clone());
            changed = true;
        }
    }
    drop(devices);

    if !changed {
        return;
    }
    state.store.mutate(|config| config.settings = settings.clone());
    state.restart_if_needed(&previous, &settings);
}

fn provision() -> Result<Provisioned, String> {
    if let Some(name) = virtual_input() {
        return Ok(Provisioned {
            input_device: Some(name),
        });
    }

    let result = platform::install();
    let detected = match &result {
        Ok(()) => wait_for_virtual_input(DEVICE_TIMEOUT),
        // The installer may report a non-zero code after a successful driver
        // registration, so give the endpoint a moment before failing.
        Err(_) => wait_for_virtual_input(ERROR_GRACE),
    };

    match (result, detected) {
        (_, Some(name)) => Ok(Provisioned {
            input_device: Some(name),
        }),
        (Err(err), None) => Err(err),
        (Ok(()), None) => Err(
            "O cabo virtual foi instalado, mas nenhum dispositivo de captura apareceu. \
             Reinicie o computador e clique em Atualizar."
                .into(),
        ),
    }
}

fn virtual_input() -> Option<String> {
    devices::enumerate()
        .inputs
        .into_iter()
        .find(|device| device.is_virtual)
        .map(|device| device.name)
}

fn wait_for_virtual_input(timeout: Duration) -> Option<String> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(name) = virtual_input() {
            return Some(name);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(750));
    }
}

fn work_dir() -> Result<PathBuf, String> {
    let dir = std::env::temp_dir()
        .join("AudioPulse")
        .join("virtual-cable");
    std::fs::create_dir_all(&dir)
        .map_err(|err| format!("Não foi possível preparar a pasta temporária: {err}"))?;
    Ok(dir)
}

fn download(url: &str, dest: &Path) -> Result<(), String> {
    let _ = std::fs::remove_file(dest);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(180))
        .user_agent(concat!("AudioPulse/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|err| format!("Não foi possível iniciar o download: {err}"))?;
    let mut response = client
        .get(url)
        .send()
        .map_err(|err| format!("Não foi possível baixar o cabo virtual: {err}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "O download do cabo virtual falhou (HTTP {}).",
            response.status()
        ));
    }
    let mut file = std::fs::File::create(dest)
        .map_err(|err| format!("Não foi possível criar o arquivo temporário: {err}"))?;
    std::io::copy(&mut response, &mut file)
        .map_err(|err| format!("Não foi possível salvar o download: {err}"))?;
    Ok(())
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    pub fn install() -> Result<(), String> {
        log::info!("provisioning VB-Cable");
        let dir = work_dir()?;
        let zip = dir.join("VBCABLE_Driver_Pack45.zip");
        download(VB_CABLE_URL, &zip)?;

        let extract = dir.join("extract");
        let _ = std::fs::remove_dir_all(&extract);
        let script = format!(
            "Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force",
            quote(&zip),
            quote(&extract)
        );
        let (code, stdout) = powershell(&script)?;
        if code != 0 {
            return Err(format!(
                "Não foi possível extrair o pacote do VB-Cable: {}",
                stdout.trim()
            ));
        }

        let setup = ["VBCABLE_Setup_x64.exe", "VBCABLE_Setup.exe"]
            .iter()
            .map(|name| extract.join(name))
            .find(|path| path.is_file())
            .ok_or_else(|| "O pacote baixado não contém o instalador do VB-Cable.".to_string())?;
        verify_signature(&setup)?;

        let script = format!(
            "try {{ $p = Start-Process -FilePath '{}' -WorkingDirectory '{}' \
             -ArgumentList '-i','-h' -Verb RunAs -Wait -PassThru; exit $p.ExitCode }} \
             catch {{ exit 1223 }}",
            quote(&setup),
            quote(&extract)
        );
        let (code, stdout) = powershell(&script)?;
        match code {
            0 => {
                log::info!("VB-Cable installer finished");
                Ok(())
            }
            1223 => Err(
                "A instalação do cabo virtual foi cancelada. Ele é necessário para \
                 processar o áudio de outros aplicativos."
                    .into(),
            ),
            other => Err(format!(
                "O instalador do VB-Cable terminou com o código {other}. {}",
                stdout.trim()
            )),
        }
    }

    fn verify_signature(path: &Path) -> Result<(), String> {
        let script = format!(
            "$s = Get-AuthenticodeSignature -LiteralPath '{}'; \
             if ($s.Status -ne 'Valid') {{ Write-Output ('status=' + $s.Status); exit 4 }}; \
             if ($null -eq $s.SignerCertificate) {{ Write-Output 'certificado ausente'; exit 5 }}; \
             if ($s.SignerCertificate.Subject -notmatch 'BUREL|VB-Audio') \
             {{ Write-Output $s.SignerCertificate.Subject; exit 5 }}; exit 0",
            quote(path)
        );
        let (code, stdout) = powershell(&script)?;
        if code == 0 {
            log::info!("VB-Cable installer signature verified");
            return Ok(());
        }
        Err(format!(
            "A assinatura digital do instalador do VB-Cable não pôde ser verificada ({}). \
             Instalação abortada.",
            stdout.trim()
        ))
    }

    fn quote(path: &Path) -> String {
        path.to_string_lossy().replace('\'', "''")
    }

    fn powershell(script: &str) -> Result<(i32, String), String> {
        let output = Command::new("powershell")
            .creation_flags(CREATE_NO_WINDOW)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                script,
            ])
            .output()
            .map_err(|err| format!("Não foi possível executar o PowerShell: {err}"))?;
        Ok((
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).to_string(),
        ))
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use std::process::Command;

    const BLACKHOLE_FALLBACK_URL: &str =
        "https://existential.audio/downloads/BlackHole2ch-0.7.1.pkg";
    const BLACKHOLE_FALLBACK_SHA256: &str =
        "57b540f27a3e29c37e310e01bee0fdfab76733087e47f997ef9dccf851400dcf";
    const CASK_API: &str = "https://formulae.brew.sh/api/cask/blackhole-2ch.json";

    pub fn install() -> Result<(), String> {
        log::info!("provisioning BlackHole");
        let dir = work_dir()?;
        let pkg = dir.join("BlackHole2ch.pkg");
        let (url, expected) = package_source();
        download(&url, &pkg)?;
        verify_digest(&pkg, expected.as_deref())?;
        verify_signature(&pkg)?;
        run_installer(&pkg)?;
        Ok(())
    }

    /// The vendor no longer exposes a stable unversioned URL, so read the
    /// current signed package location from the Homebrew cask metadata and
    /// fall back to a pinned version if that endpoint is unavailable.
    fn package_source() -> (String, Option<String>) {
        let fetched = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(concat!("AudioPulse/", env!("CARGO_PKG_VERSION")))
            .build()
            .ok()
            .and_then(|client| client.get(CASK_API).send().ok())
            .and_then(|response| response.json::<serde_json::Value>().ok())
            .and_then(|value| {
                let url = value.get("url")?.as_str()?.to_string();
                let sha = value.get("sha256")?.as_str()?.to_string();
                Some((url, Some(sha)))
            });
        fetched.unwrap_or_else(|| {
            (
                BLACKHOLE_FALLBACK_URL.to_string(),
                Some(BLACKHOLE_FALLBACK_SHA256.to_string()),
            )
        })
    }

    fn verify_digest(path: &Path, expected: Option<&str>) -> Result<(), String> {
        let Some(expected) = expected else {
            return Ok(());
        };
        use sha2::{Digest, Sha256};
        let bytes = std::fs::read(path)
            .map_err(|err| format!("Não foi possível ler o instalador baixado: {err}"))?;
        let digest = Sha256::digest(&bytes);
        let actual: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        if actual.eq_ignore_ascii_case(expected.trim()) {
            Ok(())
        } else {
            Err("O download do BlackHole não corresponde ao checksum publicado. \
                 Instalação abortada."
                .into())
        }
    }

    fn verify_signature(path: &Path) -> Result<(), String> {
        let pkg = path.to_string_lossy().to_string();
        let output = Command::new("pkgutil")
            .args(["--check-signature", pkg.as_str()])
            .output()
            .map_err(|err| format!("Não foi possível verificar o instalador: {err}"))?;
        let text = String::from_utf8_lossy(&output.stdout);
        if output.status.success()
            && text.contains("Developer ID Installer")
            && text.contains("Existential Audio")
        {
            log::info!("BlackHole installer signature verified");
            Ok(())
        } else {
            Err("A assinatura digital do instalador do BlackHole não pôde ser verificada. \
                 Instalação abortada."
                .into())
        }
    }

    fn run_installer(path: &Path) -> Result<(), String> {
        let shell_path = path.to_string_lossy().replace('\'', "'\\''");
        let script = format!(
            "do shell script \"installer -pkg '{}' -target /; killall coreaudiod\" \
             with administrator privileges",
            shell_path.replace('\\', "\\\\").replace('"', "\\\"")
        );
        let output = Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map_err(|err| format!("Não foi possível executar o instalador: {err}"))?;
        if output.status.success() {
            log::info!("BlackHole installer finished");
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("-128") || stderr.to_lowercase().contains("cancel") {
            return Err(
                "A instalação do cabo virtual foi cancelada. Ele é necessário para \
                 processar o áudio de outros aplicativos."
                    .into(),
            );
        }
        Err(format!(
            "Não foi possível instalar o BlackHole: {}",
            stderr.trim()
        ))
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod platform {
    pub fn install() -> Result<(), String> {
        Err(
            "A instalação automática do cabo virtual está disponível no Windows e no macOS. \
             No Linux, crie um sink de monitor com pactl e selecione-o como captura."
                .into(),
        )
    }
}
