mod http;
mod lock;
mod npm;

use std::{collections::HashMap, path::Path};
use tokio::{fs, sync::RwLock};

pub async fn update_lockfile<P: AsRef<Path>>(path: P, registry: &str) -> anyhow::Result<()> {
    let cache = RwLock::new(HashMap::with_capacity(100));

    let file = fs::read(&path).await?;

    let lockfile = serde_json::from_slice(&file)?;
    let lockfile = lock::update_lock(lockfile, &cache, registry).await?;

    fs::write(&path, &serde_json::to_vec_pretty(&lockfile)?).await?;

    Ok(())
}

pub async fn check_lockfile<P: AsRef<Path>>(path: P, registry: &str) -> anyhow::Result<()> {
    let file = fs::read(&path).await?;

    let lockfile = serde_json::from_slice(&file)?;
    lock::check_lock(&lockfile, registry).map_err(anyhow::Error::msg)
}
