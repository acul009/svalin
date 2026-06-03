use futures::StreamExt;
use tokio::{fs, io::AsyncWriteExt};

#[cfg(target_os = "linux")]
use crate::{
    agent::Agent,
    util::location::{Location, LocationError},
};

pub async fn update_agent(url: &str) -> anyhow::Result<()> {
    let res = reqwest::get(url).await?.error_for_status()?;
    let mut download_stream = res.bytes_stream();
    let temp_path = get_update_temp_path()?.ensure_parent_exists().await?;
    let mut installer = fs::File::create(&temp_path).await?;

    while let Some(chunk) = download_stream.next().await {
        let chunk = chunk?;
        installer.write_all(&chunk).await?;
    }

    installer.flush().await?;
    installer.sync_all().await?;

    let mut command = tokio::process::Command::new(temp_path.as_path().as_os_str());
    command.arg("agent").arg("install");
    match command.status().await?.code() {
        None => anyhow::bail!("unknown error while executing installer"),
        Some(0) => {}
        Some(code) => anyhow::bail!("installer exited with code {}", code),
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn get_update_temp_path() -> Result<Location, LocationError> {
    Ok(Agent::temp_dir()?.push("svalin-update"))
}
