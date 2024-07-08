use super::manifest::Repository;
use super::semver::Semver;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Manifest {
    #[serde(rename = "v")]
    pub version: Semver,
    #[serde(rename = "repo")]
    pub repository: Repository,
    #[serde(rename = "deps")]
    pub dependencies: Vec<Dependency>,
}

#[derive(Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    #[serde(rename = "req")]
    pub version_req: Semver,
}
