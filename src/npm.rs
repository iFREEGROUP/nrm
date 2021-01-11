use reqwest::Error;
use serde::Deserialize;
use std::collections::HashMap;
use tokio::sync::RwLock;

pub(crate) type InfoCache = RwLock<HashMap<String, PackageInfo>>;

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct PackageInfo {
    versions: HashMap<String, PackageManifest>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct PackageManifest {
    pub dist: Dist,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Dist {
    pub shasum: String,
    pub tarball: String,
    pub integrity: Option<String>,
}

pub(crate) async fn fetch_package_manifest(
    cache: &InfoCache,
    registry: &str,
    package: &str,
    version: &str,
) -> Result<Option<PackageManifest>, Error> {
    let info = { cache.read().await.get(package).cloned() };
    let info = if let Some(info) = info {
        info
    } else {
        let resp = reqwest::get(&format!("{}/{}", registry, package))
            .await?
            .json::<PackageInfo>()
            .await?;
        cache
            .write()
            .await
            .insert(package.to_string(), resp.clone());
        resp
    };

    Ok(info.versions.get(version).cloned())
}
