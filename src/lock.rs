use crate::{
    http::retry,
    npm::{fetch_package_manifest, InfoCache},
};
use futures::future::try_join_all;
use log::error;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ssri::IntegrityOpts;
use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    future::Future,
    pin::Pin,
};

#[derive(Deserialize, Serialize)]
pub(crate) struct Lockfile {
    #[serde(rename = "lockfileVersion")]
    lockfile_version: u8,
    #[serde(flatten, skip_serializing_if = "HashMap::is_empty")]
    i_dont_care: HashMap<String, Value>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    dependencies: BTreeMap<String, Dependency>,
}

#[derive(Deserialize, Serialize)]
pub(crate) struct Dependency {
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolved: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    integrity: Option<String>,
    #[serde(skip_serializing_if = "is_empty_dependencies")]
    dependencies: Option<BTreeMap<String, Dependency>>,
    #[serde(flatten, skip_serializing_if = "HashMap::is_empty")]
    i_dont_care: HashMap<String, Value>,
}

fn is_empty_dependencies(dependencies: &Option<BTreeMap<String, Dependency>>) -> bool {
    dependencies
        .as_ref()
        .map(|deps| deps.is_empty())
        .unwrap_or(true)
}

async fn compute_sha1_ssri(url: &str) -> anyhow::Result<String> {
    let file = retry(5, || async { reqwest::get(url).await?.bytes().await }).await?;

    let mut integrity_opts = IntegrityOpts::new().algorithm(ssri::Sha1);
    integrity_opts.input(&file);
    Ok(integrity_opts.result().to_string())
}

pub(crate) async fn update_lock(
    mut lock: Lockfile,
    cache: &InfoCache,
    registry: &str,
) -> anyhow::Result<Lockfile> {
    // currently we only consider version 1
    if lock.lockfile_version != 1 {
        return Ok(lock);
    }

    let futures = lock
        .dependencies
        .into_iter()
        .map(|(package, dependency)| async move {
            let dependency =
                rewrite_dependency(Cow::from(&package), dependency, cache, registry).await?;
            Ok::<_, anyhow::Error>((package, dependency))
        });
    lock.dependencies = try_join_all(futures).await?.into_iter().collect();

    Ok(lock)
}

fn rewrite_dependency<'a>(
    package: Cow<'a, str>,
    mut dependency: Dependency,
    cache: &'a InfoCache,
    registry: &'a str,
) -> Pin<Box<dyn Future<Output = anyhow::Result<Dependency>> + 'a>> {
    Box::pin(async move {
        if is_dep_match_registry(&dependency, registry) {
            return Ok(dependency);
        }

        let manifest = fetch_package_manifest(cache, registry, &package, &dependency.version).await;
        let manifest = if let Ok(manifest) = manifest {
            manifest.ok_or_else(|| {
                anyhow::Error::msg(format!(
                    "{} {} cannot be found.",
                    &package, &dependency.version
                ))
            })?
        } else {
            error!(
                "Failed to fetch information of {} {}, you may need to re-run this program to retry.",
                &package,
                &dependency.version
            );
            return Ok(dependency);
        };

        let resolved = manifest.dist.tarball.clone();
        dependency.resolved = Some(resolved.clone());

        let integrity = if let Some(integrity) = manifest.dist.integrity {
            integrity
        } else {
            compute_sha1_ssri(&resolved).await?
        };
        dependency.integrity = Some(integrity);

        if let Some(dependencies) = dependency.dependencies {
            let futures = dependencies.into_iter().map(|(pkg, dep)| async {
                rewrite_dependency(Cow::from(&pkg), dep, cache, registry)
                    .await
                    .map(|dep| (pkg, dep))
            });
            dependency.dependencies = Some(try_join_all(futures).await?.into_iter().collect());
        }

        Ok(dependency)
    })
}

pub(crate) fn check_lock(lock: &Lockfile, registry: &str) -> Result<(), &'static str> {
    if lock.lockfile_version != 1 {
        return Err("Only version 1 of lockfile is supported.");
    }

    let ok = lock
        .dependencies
        .values()
        .all(|dep| check_dependency(dep, registry));
    if ok {
        Ok(())
    } else {
        Err("This lockfile contains some packages from other registries.")
    }
}

fn check_dependency(dependency: &Dependency, registry: &str) -> bool {
    if !is_dep_match_registry(&dependency, registry) {
        return false;
    }

    dependency
        .dependencies
        .as_ref()
        .map(|dependencies| {
            dependencies
                .values()
                .all(|dep| check_dependency(dep, registry))
        })
        .unwrap_or(true)
}

#[inline]
fn is_dep_match_registry(dependency: &Dependency, registry: &str) -> bool {
    matches!(&dependency.resolved, Some(url) if !url.starts_with(registry))
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Dependency {
        fn with_resolved(resolved: Option<String>) -> Self {
            Self {
                version: "1.0.0".to_string(),
                resolved,
                integrity: None,
                dependencies: None,
                i_dont_care: HashMap::new(),
            }
        }
    }

    #[tokio::test]
    async fn test_compute_sha1_ssri() {
        let url = "https://registry.npmjs.org/preact-10.0.0.tgz";
        assert_eq!(
            "sha1-bm7tJgu8FHlq5QU+Y6gDxOGPfRc=",
            compute_sha1_ssri(url).await.unwrap()
        );
    }

    #[test]
    fn test_is_dep_match_registry() {
        assert!(is_dep_match_registry(&Dependency::with_resolved(None), ""));
        assert!(!is_dep_match_registry(
            &Dependency::with_resolved(Some(
                "https://registry.npm.taobao.org/react-17.0.0.tgz".to_string()
            )),
            "https://registry.npmjs.org"
        ));
        assert!(is_dep_match_registry(
            &Dependency::with_resolved(Some(
                "https://registry.npmjs.org/react-17.0.0.tgz".to_string()
            )),
            "https://registry.npmjs.org"
        ));
    }
}
