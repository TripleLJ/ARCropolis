use std::fmt;

use gh_updater::{ReleaseFinderConfig, ReleaseManager};
use semver::Version;
use zip::ZipArchive;

pub enum VersionDifference {
    ChangeToStable(String),
    ChangeToBeta(String),
    Regular(String),
}

impl fmt::Display for VersionDifference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ChangeToStable(ver) => write!(f, "An uninstalled stable version of ARCropolis ({})", ver),
            Self::ChangeToBeta(ver) => write!(f, "A new beta version of ARCropolis ({})", ver),
            Self::Regular(ver) => write!(f, "A new update for ARCropolis ({})", ver),
        }
    }
}

fn compare_tags(current: &str, target: &str) -> Result<Option<VersionDifference>, semver::Error> {
    let current = Version::parse(current)?;
    let target = Version::parse(target)?;

    if current.pre.is_empty() && !target.pre.is_empty() {
        Ok(Some(VersionDifference::ChangeToBeta(target.to_string())))
    } else if !current.pre.is_empty() && target.pre.is_empty() && current < target {
        Ok(Some(VersionDifference::ChangeToStable(target.to_string())))
    } else if target > current {
        Ok(Some(VersionDifference::Regular(target.to_string())))
    } else {
        Ok(None)
    }
}

pub struct PendingUpdate {
    release: ReleaseManager,
    pub header_text: String,
    pub date: String,
    pub body: String,
}

pub fn find_update(beta_enabled: bool) -> Option<PendingUpdate> {
    let release = ReleaseFinderConfig::new("ARCropolis")
        .with_author("Raytwo")
        .with_repository("ARCropolis")
        .with_prereleases(beta_enabled)
        .find_release();

    let (release, prerelease) = match release {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to check for updates: {:?}", e);
            return None;
        },
    };

    let prerelease_tag = prerelease
        .as_ref()
        .map(|x| Version::parse(x.get_release_tag().trim_start_matches('v')).expect("Failed to parse version strings!"));
    let release_tag = release
        .as_ref()
        .map(|x| Version::parse(x.get_release_tag().trim_start_matches('v')).expect("Failed to parse version strings!"));

    let release = match (prerelease_tag, release_tag) {
        (None, None) => {
            error!("No github releases were found!");
            return None;
        },
        (prerelease_tag, release_tag) => {
            if prerelease_tag > release_tag {
                prerelease.unwrap()
            } else {
                // even if they are equal it won't matter
                release.unwrap()
            }
        },
    };

    let version_difference = match compare_tags(env!("CARGO_PKG_VERSION"), release.get_release_tag().trim_start_matches('v')) {
        Ok(diff) => diff,
        Err(e) => {
            error!("Failed to parse version strings: {:?}", e);
            return None;
        },
    };

    if version_difference.is_none() {
        return None;
    }

    let date = release.data["published_at"]
        .as_str()
        .and_then(|published_at| {
            let mut parts = published_at.split('-');
            let year = parts.next()?;
            let month = parts.next()?;
            let day = parts.next()?.get(..2)?;
            Some(format!("{}/{}/{}", month, day, year))
        })
        .unwrap_or_default();
    let header_text = format!(
        "{} ({})",
        release.get_release_tag().trim_start_matches('v'),
        release.data["name"].as_str().unwrap_or_default()
    );
    let body = release.data["body"].as_str().unwrap_or_default().to_string();

    Some(PendingUpdate {
        release,
        header_text,
        date,
        body,
    })
}

pub fn install(update: PendingUpdate) -> bool {
    let Some(release) = update.release.get_asset_by_name("release.zip") else {
        error!("The release has no release.zip asset, nothing to install");
        return false;
    };

    let mut zip = match ZipArchive::new(std::io::Cursor::new(release)) {
        Ok(zip) => zip,
        Err(e) => {
            error!("Failed to parse zip data: {:?}", e);
            return false;
        },
    };

    if let Err(e) = zip.extract("sd:/") {
        panic!("ARCropolis failed to extract update ZIP. Reason: {:?}", e);
    }

    unsafe { skyline::nn::oe::RequestToRelaunchApplication() }
}
