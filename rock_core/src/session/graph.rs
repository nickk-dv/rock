use crate::error::Error;
use crate::errors as err;
use crate::intern::{InternPool, NameID};
use crate::session::{Package, PackageID};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct PackageGraph {
    unique: HashMap<PathBuf, PackageID>,
    packages: HashMap<PackageID, Package>,
}

impl PackageGraph {
    pub(super) fn new(cap: usize) -> PackageGraph {
        PackageGraph { unique: HashMap::with_capacity(cap), packages: HashMap::with_capacity(cap) }
    }

    #[inline]
    pub fn package(&self, package_id: PackageID) -> &Package {
        self.packages.get(&package_id).unwrap()
    }
    #[inline]
    pub fn package_mut(&mut self, package_id: PackageID) -> &mut Package {
        self.packages.get_mut(&package_id).unwrap()
    }
    #[inline]
    pub fn package_count(&self) -> usize {
        self.packages.len()
    }
    #[inline]
    pub fn all_packages(&self) -> &HashMap<PackageID, Package> {
        &self.packages
    }

    #[inline]
    pub fn find_package_dep(&self, package_id: PackageID, dep_name: NameID) -> Option<PackageID> {
        let package = self.package(package_id);
        package.deps.iter().copied().find(|&dep_id| {
            let dep = self.package(dep_id);
            dep.name_id == dep_name
        })
    }

    #[must_use]
    pub(super) fn next_id(&self) -> PackageID {
        PackageID(self.packages.len() as u32)
    }

    #[inline]
    pub(super) fn get_unique(&self, root_dir: &PathBuf) -> Option<PackageID> {
        self.unique.get(root_dir).copied()
    }

    #[must_use]
    pub(super) fn add(&mut self, package: Package, root_dir: &PathBuf) -> PackageID {
        let package_id = PackageID(self.packages.len() as u32);
        assert!(self.packages.insert(package_id, package).is_none());
        assert!(self.unique.insert(root_dir.clone(), package_id).is_none());
        package_id
    }

    pub(super) fn add_dep(
        &mut self,
        from: PackageID,
        to: PackageID,
        intern_name: &InternPool<'_, NameID>,
        manifest_path: &PathBuf,
    ) -> Result<(), Error> {
        let mut path = vec![from];

        if self.find_cycle(from, to, &mut path) {
            let relation = self.cycle_relation_msg(&path, intern_name);
            Err(err::session_pkg_dep_cycle(relation, manifest_path))
        } else {
            let package = self.package_mut(from);
            let existing = package.deps.iter().find(|&&p| p == to);
            assert!(existing.is_none());
            package.deps.push(to);
            Ok(())
        }
    }

    fn find_cycle(&self, target: PackageID, current: PackageID, path: &mut Vec<PackageID>) -> bool {
        path.push(current);
        if target == current {
            return true;
        }
        let data = self.package(current);
        for &dep in &data.deps {
            if self.find_cycle(target, dep, path) {
                return true;
            }
        }
        path.pop();
        false
    }

    fn cycle_relation_msg(
        &self,
        path: &[PackageID],
        intern_name: &InternPool<'_, NameID>,
    ) -> String {
        let mut msg = String::with_capacity(128);
        let relation_count = path.len() - 1;

        for relation_idx in 0..relation_count {
            let from_id = path[relation_idx];
            let to_id = path[relation_idx + 1];
            let from_name = intern_name.get(self.package(from_id).name_id);
            let to_name = intern_name.get(self.package(to_id).name_id);

            if relation_idx == 0 {
                msg.push_str("attempting to add dependency from `");
                msg.push_str(from_name);
                msg.push_str("` to `");
                msg.push_str(to_name);
                msg.push('`');
            } else {
                msg.push_str("which depends on `");
                msg.push_str(to_name);
                msg.push('`');
            }

            if relation_idx + 1 == relation_count {
                msg.push_str(", completing the cycle...");
            } else {
                msg.push_str("...\n");
            }
        }
        msg
    }
}
