use std::path::Path;

use tokio::io::AsyncWriteExt;

pub async fn override_atomic(file_location: &Path, content: &[u8]) -> anyhow::Result<()> {
    let temp_file = file_location.with_extension("tmp");
    let mut file = tokio::fs::File::options()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&temp_file)
        .await?;

    file.write_all(&content).await?;
    file.flush().await?;
    file.sync_all().await?;

    tokio::fs::rename(&temp_file, &file_location).await?;

    Ok(())
}
