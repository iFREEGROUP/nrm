use crate::{
    http::retry,
    npm::{fetch_package_info, PackageInfo},
};
use futures::future::{join_all, try_join_all};
use log::error;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ssri::IntegrityOpts;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Deserialize, Serialize)]
pub(crate) struct Lockfile {
    #[serde(rename = "lockfileVersion")]
    lockfile_version: u8,
    #[serde(flatten, skip_serializing_if = "HashMap::is_empty")]
    i_dont_care: HashMap<String, Value>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    dependencies: BTreeMap<String, Dependency>,
}

#[derive(Clone, Deserialize, Serialize)]
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

pub(crate) async fn update_lock(mut lock: Lockfile, registry: &str) -> anyhow::Result<Lockfile> {
    // currently we only consider version 1
    if lock.lockfile_version != 1 {
        return Ok(lock);
    }

    // collect all packages that the lockfile contains
    let packages = collect_packages(&lock.dependencies, HashMap::new());

    // fetch packages information from registry
    let packages_info = fetch_packages(&packages, registry).await?;

    // write to the lock
    lock.dependencies = lock
        .dependencies
        .iter()
        .map(|(package, dependency)| {
            (
                package.clone(),
                rewrite_dependency(&packages_info, &package, dependency.clone()),
            )
        })
        .collect();

    Ok(lock)
}

fn collect_packages<'a, 'b: 'a>(
    dependencies: &'b BTreeMap<String, Dependency>,
    packages: HashMap<&'a str, HashSet<&'a str>>,
) -> HashMap<&'a str, HashSet<&'a str>> {
    dependencies
        .iter()
        .fold(packages, |mut packages, (package, info)| {
            let set = packages.entry(package).or_default();
            set.insert(&*info.version);
            if let Some(dependencies) = &info.dependencies {
                collect_packages(dependencies, packages)
            } else {
                packages
            }
        })
}

async fn fetch_packages<'a>(
    packages: &'a HashMap<&'a str, HashSet<&'a str>>,
    registry: &'a str,
) -> anyhow::Result<HashMap<&'a str, PackageInfo>> {
    let futures = packages.keys().map(|package| async move {
        fetch_package_info(registry, package)
            .await
            .map(|info| (package, info))
    });
    let mut packages_info = join_all(futures)
        .await
        .into_iter()
        .filter_map(|r| r.ok())
        .map(|(name, info)| (*name, info))
        .collect::<HashMap<_, _>>();

    // registry may not provide integrity, so we need to add them by ourselves
    fetch_missing_ssri(packages, &mut packages_info).await?;

    Ok(packages_info)
}

async fn fetch_missing_ssri(
    packages: &HashMap<&str, HashSet<&str>>,
    packages_info: &mut HashMap<&str, PackageInfo>,
) -> anyhow::Result<()> {
    let mut need_to_compute_ssri = vec![];
    packages.iter().for_each(|(package, versions)| {
        if let Some(info) = packages_info.get(package) {
            for version in versions {
                if let Some(manifest) = info.get_version_manifest(version) {
                    if manifest.dist.integrity.is_none() {
                        need_to_compute_ssri.push((*package, *version, &*manifest.dist.tarball));
                    }
                }
            }
        }
    });

    let futures = need_to_compute_ssri
        .into_iter()
        .map(|(package, version, tarball)| async move {
            compute_sha1_ssri(tarball)
                .await
                .map(|ssri| (package, version, ssri))
        });
    try_join_all(futures)
        .await?
        .into_iter()
        .for_each(|(package, version, ssri)| {
            if let Some(info) = packages_info.get_mut(package) {
                if let Some(mut manifest) = info.get_mut_version_manifest(version) {
                    manifest.dist.integrity = Some(ssri);
                }
            }
        });

    Ok(())
}

fn rewrite_dependency(
    packages_info: &HashMap<&str, PackageInfo>,
    package: &str,
    mut dependency: Dependency,
) -> Dependency {
    let manifest = packages_info
        .get(package)
        .map(|info| info.get_version_manifest(&dependency.version))
        .flatten();
    let manifest = if let Some(manifest) = manifest {
        manifest
    } else {
        error!(
            "Failed to fetch information of {} {}, you may need to re-run this program to retry.",
            &package, &dependency.version
        );
        return dependency;
    };

    if dependency.resolved.is_some() {
        dependency.resolved = Some(manifest.dist.tarball.clone());
        dependency.integrity = manifest.dist.integrity.clone();
    }

    dependency.dependencies = dependency.dependencies.map(|dependencies| {
        dependencies
            .into_iter()
            .map(|(package, dependency)| {
                let dep = rewrite_dependency(packages_info, &package, dependency);
                (package, dep)
            })
            .collect()
    });

    dependency
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
    !matches!(&dependency.resolved, Some(url) if !url.starts_with(registry))
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

        fn with_dependencies(dependencies: Option<BTreeMap<String, Dependency>>) -> Self {
            Self {
                version: "1.0.0".to_string(),
                resolved: None,
                integrity: None,
                dependencies,
                i_dont_care: HashMap::new(),
            }
        }

        fn version(mut self, version: &str) -> Self {
            self.version = version.to_string();
            self
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

    #[test]
    fn test_collect_packages() {
        let mut pkg_a = BTreeMap::new();
        let pkg_b = BTreeMap::new();
        let mut pkg_c = BTreeMap::new();
        pkg_c.insert(
            "d".to_string(),
            Dependency::with_resolved(None).version("4.0.0"),
        );
        pkg_a.insert(
            "b".to_string(),
            Dependency::with_dependencies(Some(pkg_b)).version("2.0.0"),
        );
        pkg_a.insert(
            "c".to_string(),
            Dependency::with_dependencies(Some(pkg_c)).version("3.0.0"),
        );

        let expected = [
            ("b", &["2.0.0"] as &[&str]),
            ("c", &["3.0.0"]),
            ("d", &["4.0.0"]),
        ];
        let mut packages = collect_packages(&pkg_a, HashMap::new())
            .into_iter()
            .map(|(pkg, versions)| (pkg, versions.into_iter().collect::<Vec<_>>()))
            .collect::<Vec<_>>();
        packages.sort();

        packages
            .iter()
            .zip(expected.iter())
            .for_each(|(actual, expected)| {
                assert_eq!(actual.0, expected.0);
                assert_eq!(&actual.1, expected.1);
            })
    }
}
