use std::process::Stdio;

use anyhow::Context;
use futures::StreamExt;
use tokio::{fs, io::AsyncWriteExt};

#[cfg(target_os = "linux")]
use crate::{
    agent,
    util::location::{Location, LocationError},
};

pub async fn update_agent(url: &str) -> anyhow::Result<()> {
    let res = reqwest::get(url)
        .await
        .context("could not fetch new file, probably url error")?
        .error_for_status()
        .context("download server returned error")?;
    let mut download_stream = res.bytes_stream();
    let temp_path = get_update_temp_path()
        .context("error fetching temp path")?
        .ensure_parent_exists()
        .await
        .context("could not create temp dir")?;

    if fs::try_exists(&temp_path).await.unwrap_or(false) {
        let _ = fs::remove_file(&temp_path).await;
    }

    let mut installer = fs::File::options()
        .write(true)
        .create(true)
        .mode(0o755)
        .open(&temp_path)
        .await
        .context("could not create file to download into")?;

    while let Some(chunk) = download_stream.next().await {
        let chunk = chunk.context("error while downloading")?;
        installer
            .write_all(&chunk)
            .await
            .context("error while writing to file")?;
    }

    installer
        .flush()
        .await
        .context("error while flushing file")?;
    installer
        .sync_all()
        .await
        .context("error while syncing file")?;
    drop(installer);

    let mut command = tokio::process::Command::new(temp_path.as_path().as_os_str());
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

#[cfg(target_os = "linux")]
fn get_update_temp_path() -> Result<Location, LocationError> {
    Ok(agent::temp_dir()?.push("svalin-update"))
}
