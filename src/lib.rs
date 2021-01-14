mod http;
mod lock;
mod npm;

use std::path::Path;
use tokio::fs;

pub async fn update_lockfile<P: AsRef<Path>>(path: P, registry: &str) -> anyhow::Result<()> {
    let file = fs::read(&path).await?;

    let lockfile = serde_json::from_slice(&file)?;
    let lockfile = lock::update_lock(lockfile, registry).await?;

    fs::write(&path, &serde_json::to_vec_pretty(&lockfile)?).await?;

    Ok(())
}

pub async fn check_lockfile<P: AsRef<Path>>(path: P, registry: &str) -> anyhow::Result<()> {
    let file = fs::read(&path).await?;

    let lockfile = serde_json::from_slice(&file)?;
    lock::check_lock(&lockfile, registry).map_err(anyhow::Error::msg)
}
