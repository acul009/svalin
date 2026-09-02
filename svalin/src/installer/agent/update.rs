use std::process::Stdio;

use anyhow::Context;
use tokio::{
    fs,
    io::{AsyncWriteExt, DuplexStream},
};

use crate::{
    agent,
    util::location::{Location, LocationError},
};

pub struct PreparedUpdate(Location);

pub async fn prepare_update_agent(mut source: DuplexStream) -> anyhow::Result<PreparedUpdate> {
    let temp_path = get_update_temp_path()
        .context("error fetching temp path")?
        .ensure_parent_exists()
        .await
        .context("could not create temp dir")?;

    if fs::try_exists(&temp_path).await.unwrap_or(false) {
        let _ = fs::remove_file(&temp_path).await;
    }

    let mut options = fs::File::options();
    options.write(true).create(true);

    #[cfg(target_os = "linux")]
    options.mode(0o755);

    let mut installer = options
        .open(&temp_path)
        .await
        .context("could not create file to download into")?;

    tokio::io::copy(&mut source, &mut installer)
        .await
        .context("error while writing to file")?;

    installer
        .flush()
        .await
        .context("error while flushing file")?;
    installer
        .sync_all()
        .await
        .context("error while syncing file")?;

    Ok(PreparedUpdate(temp_path))
}

pub async fn update_agent(update: &PreparedUpdate) -> anyhow::Result<()> {
    let mut command = tokio::process::Command::new(update.0.as_path().as_os_str());
    let output = command
        .arg("agent")
        .arg("install")
        .stderr(Stdio::piped())
        .spawn()
        .context("could not spawn installer")?
        .wait_with_output()
        .await
        .context("error while waiting for installer")?;

    let err = String::from_utf8_lossy(&output.stderr);

    match output.status.code() {
        None => anyhow::bail!("unknown error while executing installer: {}\n", err),
        Some(0) => {}
        Some(code) => anyhow::bail!("installer exited with code {}\n{}\n", code, err),
    }

    Ok(())
}

fn get_update_temp_path() -> Result<Location, LocationError> {
    #[cfg(target_os = "windows")]
    const EXE_NAME: &str = "svalin-update.exe";
    #[cfg(not(target_os = "windows"))]
    const EXE_NAME: &str = "svalin-update";

    Ok(agent::temp_dir()?.push(EXE_NAME))
}
