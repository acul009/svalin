use std::env;

use anyhow::{Context, anyhow};

#[cfg(target_os = "linux")]
use crate::agent;
use crate::util::location::Location;
#[cfg(target_os = "linux")]
use tokio::{fs::File, io::AsyncWriteExt};
use tokio::{
    fs::{self},
    process::Command,
};

#[cfg(target_os = "windows")]
pub const WINDOWS_SERVICE_NAME: &str = "svalin-agent";

pub mod update;

#[cfg(windows)]
const EXECUTABLE_NAME: &str = "svalin.exe";
#[cfg(not(windows))]
const EXECUTABLE_NAME: &str = "svalin";

pub async fn install_agent() -> anyhow::Result<()> {
    println!("Starting agent installation");
    let current_location = env::current_exe()?;
    let install_to = get_agent_install_location()
        .await?
        .push(EXECUTABLE_NAME)
        .ensure_parent_exists()
        .await
        .context("failed to create installation directory")?;
    println!(
        "Copying from {} to {}",
        current_location.display(),
        install_to
    );

    tokio::fs::copy(&current_location, &install_to)
        .await
        .context("Failed to copy myself into installation directory")?;
    println!("Copied successfully");

    #[cfg(windows)]
    create_installation_entry(&install_to).await?;
    #[cfg(windows)]
    println!("registered application");

    // This has to be the last thing done, as it might restart the service, which would also abort this installer.
    create_service(&install_to).await?;
    println!("created service");

    println!("Installation complete");
    Ok(())
}

pub async fn cleanup_old_installations() -> anyhow::Result<()> {
    let current_location = std::env::current_exe()?;

    let base = get_base_install_location();
    if !current_location.starts_with(&base) {
        return Ok(());
    }

    for char in 'a'..='z' {
        let path = base.clone().push(char.to_string());
        if fs::try_exists(&path).await? && !current_location.starts_with(&path) {
            fs::remove_dir_all(&path).await?;
        }
    }

    Ok(())
}

pub async fn uninstall_agent() -> anyhow::Result<()> {
    #[cfg(not(windows))]
    {
        if tokio::fs::try_exists(agent::data_dir()?)
            .await
            .unwrap_or(false)
        {
            tokio::fs::remove_dir_all(agent::data_dir()?).await?;
        }
        if tokio::fs::try_exists(get_base_install_location())
            .await
            .unwrap_or(false)
        {
            tokio::fs::remove_dir_all(get_base_install_location()).await?;
        }
    }

    #[cfg(windows)]
    {
        todo!("Defer deletion of data and install dir");
        println!("Defered deletion of agent data until next restart.");
    }

    #[cfg(target_os = "windows")]
    remove_installation_entry().await?;

    remove_service().await?;

    #[cfg(windows)]
    {
        todo!();
        println!(
            "To remove any leftover data, please restart your computer. Windows will then delete the remaining files."
        );
    }

    Ok(())
}

async fn get_agent_install_location() -> anyhow::Result<Location> {
    let base = get_base_install_location();

    for char in 'a'..='z' {
        let path = base.clone().push(char.to_string());
        if !fs::try_exists(&path).await? {
            return Ok(path);
        }
    }

    Err(anyhow::anyhow!("No available installation locations"))
}

#[cfg(target_os = "windows")]
fn get_base_install_location() -> Location {
    Location::new(std::env::var_os("ProgramFiles").unwrap_or_else(|| "C:\\Program Files".into()))
}

#[cfg(target_os = "linux")]
fn get_base_install_location() -> Location {
    Location::new("/opt/svalin")
}

#[cfg(target_os = "windows")]
async fn create_service(executable: &Location) -> anyhow::Result<()> {
    let mut command = Command::new("sc.exe");
    command.arg("query").arg(WINDOWS_SERVICE_NAME);
    let output = command.output().await?;

    // Check if the service already exists
    if output.status.success() {
        let mut command = Command::new("sc.exe");
        command
            .arg("config")
            .arg(WINDOWS_SERVICE_NAME)
            .arg("binPath=")
            .arg(executable.display().to_string());
        let output = command.output().await?;

        // Only restart if already running
        let out_string = String::from_utf8_lossy(&output.stdout);
        if out_string.contains("RUNNING") {
            let mut command = Command::new("powershell.exe");
            command
                .arg("-NoProfile")
                .arg("-NonInteractive")
                .arg("-Command")
                .arg(format!("Restart-Service -Name {}", WINDOWS_SERVICE_NAME));

            match command.status().await?.code() {
                Some(0) => Ok(()),
                Some(code) => Err(anyhow!("Failed to restart service, exit code: {code}")),
                None => Err(anyhow!("Failed to restart service")),
            }
        } else {
            Ok(())
        }
    } else {
        let mut command = Command::new("sc.exe");
        command
            .arg("create")
            .arg(WINDOWS_SERVICE_NAME)
            .arg("binPath=")
            .arg(executable.display().to_string())
            .arg("start=")
            .arg("demand")
            .arg("DisplayName=")
            .arg("Svalin Agent");

        match command.status().await?.code() {
            Some(0) => Ok(()),
            Some(code) => Err(anyhow!("Failed to create service, exit code: {code}")),
            None => Err(anyhow!("Failed to create service")),
        }
    }
}

#[cfg(target_os = "linux")]
async fn create_service(executable: &Location) -> anyhow::Result<()> {
    if !systemd_available().await {
        anyhow::bail!(
            "systemd is not available - automated service install not yet supported for your init system"
        );
    }
    create_systemd_service(executable).await
}

#[cfg(target_os = "windows")]
async fn remove_service() -> anyhow::Result<()> {
    let mut command = Command::new("sc.exe");
    command.arg("delete").arg(WINDOWS_SERVICE_NAME);

    match command.status().await?.code() {
        Some(0) => Ok(()),
        Some(code) => Err(anyhow!("Failed to create service, exit code: {code}")),
        None => Err(anyhow!("Failed to create service")),
    }
}

#[cfg(target_os = "linux")]
async fn remove_service() -> anyhow::Result<()> {
    if !systemd_available().await {
        eprintln!(
            "systemd is not available - automated service removal not yet supported for your init system"
        )
    }
    remove_systemd_service().await?;

    Ok(())
}

#[cfg(target_os = "linux")]
async fn systemd_available() -> bool {
    Command::new("systemctl")
        .arg("--version")
        .status()
        .await
        .is_ok()
}

#[cfg(target_os = "linux")]
const SYSTEMD_SERVICE_NAME: &str = "svalin-agent.service";
#[cfg(target_os = "linux")]
const SYSTEMD_SERVICE_LOCATION: &str = "/etc/systemd/system/svalin-agent.service";
#[cfg(target_os = "linux")]
const SYSTEMD_SERVICE_TEMPLATE: &str = "

[Unit]
Description=Svalin Agent
After=network.target

[Service]
Type=simple
User=root
ExecStart={}
Restart=on-failure

[Install]
WantedBy=multi-user.target
";

#[cfg(target_os = "linux")]
async fn create_systemd_service(executable: &Location) -> anyhow::Result<()> {
    let service_file_contents =
        SYSTEMD_SERVICE_TEMPLATE.replace("{}", &format!("{} agent", executable));
    let mut service_file = File::create(SYSTEMD_SERVICE_LOCATION).await?;
    service_file
        .write_all(service_file_contents.as_bytes())
        .await?;
    service_file.flush().await?;

    Command::new("systemctl")
        .arg("daemon-reload")
        .status()
        .await?;

    let Some(status) = Command::new("systemctl")
        .arg("is-active")
        .arg(SYSTEMD_SERVICE_NAME)
        .status()
        .await?
        .code()
    else {
        anyhow::bail!("failed to get status code");
    };

    match status {
        // Service was already installed and is currently running. Restarting to apply the update
        0 => {
            if Command::new("systemctl")
                .arg("restart")
                .arg(SYSTEMD_SERVICE_NAME)
                .status()
                .await?
                .success()
            {
                Ok(())
            } else {
                Err(anyhow!("failed to restart service"))
            }
        }
        // Service installed, but not running. We'll just keep it this way.
        3 => Ok(()),
        4 => Err(anyhow!("service installation failed.")),
        _ => Err(anyhow!(
            "unknown status code from systemctl status: {}",
            status
        )),
    }
}

#[cfg(target_os = "linux")]
async fn remove_systemd_service() -> anyhow::Result<()> {
    let status = Command::new("systemctl")
        .arg("disable")
        .arg("--now")
        .arg(SYSTEMD_SERVICE_NAME)
        .status()
        .await?;

    match status.code() {
        Some(0) | Some(1) => {
            if tokio::fs::try_exists(SYSTEMD_SERVICE_LOCATION)
                .await
                .unwrap_or(false)
            {
                tokio::fs::remove_file(SYSTEMD_SERVICE_LOCATION).await?;
            }

            Ok(())
        }
        _ => Err(anyhow!("failed to stop service")),
    }
}

const REGISTRY_PATH: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\svalin-agent";

#[cfg(target_os = "windows")]
async fn create_installation_entry(executable: &Location) -> anyhow::Result<()> {
    use std::os::windows::fs::MetadataExt;

    let key = windows_registry::LOCAL_MACHINE.create(REGISTRY_PATH)?;
    key.set_string("DisplayName", "Svalin Agent")?;
    key.set_string("DisplayVersion", crate::commit())?;
    key.set_string(
        "InstallLocation",
        executable
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .display()
            .to_string(),
    )?;

    let metadata = tokio::fs::File::open(executable).await?.metadata().await?;
    key.set_u32("EstimatedSize", (metadata.file_size() / 1024) as u32)?;

    key.set_string(
        "UninstallString",
        format!("{} agent uninstall", executable.display()),
    )?;

    Ok(())
}

#[cfg(target_os = "windows")]
async fn remove_installation_entry() -> anyhow::Result<()> {
    windows_registry::LOCAL_MACHINE.remove_tree(REGISTRY_PATH)?;
    Ok(())
}
