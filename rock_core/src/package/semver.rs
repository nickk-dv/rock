use serde::{Deserialize, Serialize};

#[derive(Copy, Clone)]
pub struct Semver {
    major: u32,
    minor: u32,
    patch: u32,
}

impl Semver {
    pub const fn new(major: u32, minor: u32, patch: u32) -> Semver {
        Semver {
            major,
            minor,
            patch,
        }
    }

    //@review patch compatibility rules when on 0.0.P
    pub fn compatible(&self, other: Semver) -> bool {
        if self.major != other.major {
            false
        } else if self.major == 0 {
            self.minor == other.minor
        } else {
            true
        }
    }
}

impl std::fmt::Display for Semver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl Serialize for Semver {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Semver {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse::<Semver>().map_err(serde::de::Error::custom)
    }
}

impl std::str::FromStr for Semver {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err("invalid semver format, expected 3 numbers separated by `.`".into());
        }

        let major = parts[0]
            .parse::<u32>()
            .map_err(|error| format!("failed to parse semver major version\nreason: {error}"))?;
        let minor = parts[1]
            .parse::<u32>()
            .map_err(|error| format!("failed to parse semver minor version\nreason: {error}"))?;
        let patch = parts[2]
            .parse::<u32>()
            .map_err(|error| format!("failed to parse semver patch version\nreason: {error}"))?;

        Ok(Semver {
            major,
            minor,
            patch,
        })
    }
}
