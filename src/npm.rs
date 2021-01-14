use crate::http::retry;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct PackageInfo {
    versions: HashMap<String, PackageManifest>,
}

impl PackageInfo {
    pub fn get_version_manifest(&self, version: &str) -> Option<&PackageManifest> {
        self.versions.get(version)
    }

    pub fn get_mut_version_manifest(&mut self, version: &str) -> Option<&mut PackageManifest> {
        self.versions.get_mut(version)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct PackageManifest {
    pub dist: Dist,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Dist {
    pub tarball: String,
    pub integrity: Option<String>,
}

pub(crate) async fn fetch_package_info(
    registry: &str,
    package: &str,
) -> reqwest::Result<PackageInfo> {
    retry(5, || async {
        reqwest::get(&format!("{}/{}", registry, package))
            .await?
            .json::<PackageInfo>()
            .await
    })
    .await
}
