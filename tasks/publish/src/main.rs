#![deny(clippy::all)]

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use clap::Clap;
use guppy::graph::{PackageGraph, PackageMetadata};
use guppy::MetadataCommand;

use crate::arguments::CargoPublishWorkspace;
use crate::combinators::ConditionallyDo;
use crate::package::PackageWrapper;
use crate::pipeline::commit::create_git;
use crate::pipeline::publish_crate::create_publisher;
use crate::pipeline::update_dependents::create_dependent_updater;
use crate::pipeline::update_manifest::create_manifest_updater;
use crate::topological_workspace::get_topological_workspace;
use std::path::Path;

pub(crate) mod arguments;
pub(crate) mod backup;
pub(crate) mod combinators;
pub(crate) mod package;
pub(crate) mod pipeline;
pub(crate) mod topological_workspace;

// TODO
//  * run post publish crate
//      * set all versions to next -pre (e.g. 0.1.0-pre) at once
//      * commit
fn main() -> anyhow::Result<()> {
    let fake_cargo: CargoPublishWorkspace = CargoPublishWorkspace::parse();
    let args = fake_cargo.get_arguments();

    let mut cmd = MetadataCommand::new();
    cmd.manifest_path(&args.manifest);

    let graph = PackageGraph::from_command(&mut cmd)?;
    let workspace = graph.query_workspace();
    let set = workspace.resolve();

    // topo sorted dependencies
    let components = get_topological_workspace(&set);

    // this is a collection of dependent packages. After updating the key package, its value members
    // should update the key package version field to the new version of the key package
    let dependants_db = create_dependents_db(&components);

    // TODO: create new git branch

    new_publish(&components, &dependants_db, &args.new_version)?;

    Ok(())
}

// A. in topo order do, for each pkg:

// 1. (opt) backup manifest
// 2. set component to new version
// 3. (opt?) have dependents in workspace rely on any version (*), so cargo can "find a valid version"
//      Necessary since the new version is not published yet, but since we have local changes, we can't rely on the crates.io version
//      This would be less of an issue if we developed each crate as a more separate component, and update them often as such; i.e. we wouldn't
//      depend on the unpublished versions while working towards a next release
// 4. publish new version
// 5. do what we would've like to do at 3, set the version of the dependents to the new version of the component (def in 2.)
// 6. (opt) commit changes
// 7. (opt) tag changes

// B. (opt) commit changes
// C. (opt) tag changes
fn new_publish<'g>(
    components: &[PackageMetadata<'g>],
    dependents_db: &'g HashMap<&'g str, HashSet<PackageWrapper<'g>>>,
    new_version: &str,
) -> Result<()> {
    for component in components {
        let path = component.manifest_path();

        let crate_folder = path.parent().with_context(|| {
            format!(
                "Expected parent folder for Cargo manifest at {}",
                path.display()
            )
        })?;

        let kickstart = Ok(());

        let _ = kickstart
            .and_then(|_| set_new_version(new_version, *component)) // set_new_version for component to 'version
            .and_then(|_| set_dependent_version("*", *component, dependents_db)) // set_dependent_version to * locally
            .do_if(|| true, |_| publish(*component)) // publish for component
            .and_then(|_| set_dependent_version(new_version, *component, dependents_db)) // set_dependent_version to 'version
            .do_if(
                || true,
                |_| make_commit(new_version, *component, crate_folder),
            )?; // commit changes

        // give the index time to update
        std::thread::sleep(std::time::Duration::from_secs(5));
    }

    Ok(())
}

// packages required to be reverse topo sorted
fn create_dependents_db<'a>(
    packages: &'a [PackageMetadata<'a>],
) -> HashMap<&'a str, HashSet<PackageWrapper<'a>>> {
    let ws = packages.iter().map(|p| p.name()).collect::<HashSet<_>>();

    packages.iter().fold(HashMap::new(), |mut map, dep| {
        dep.direct_links()
            .filter(|n| ws.contains(n.dep_name()))
            .for_each(|link| {
                let set = map.entry(link.resolved_name()).or_default();

                set.insert(PackageWrapper::new(*dep, link));
            });

        map
    })
}

const DRY_RUN: bool = false;

fn set_new_version(new_version: &str, component: PackageMetadata) -> Result<()> {
    let manifest_updater = create_manifest_updater(DRY_RUN, component);
    manifest_updater.update_dependency_version(new_version)
}

fn set_dependent_version<'a>(
    new_version: &str,
    component: PackageMetadata,
    dependents_db: &HashMap<&'a str, HashSet<PackageWrapper<'a>>>,
) -> Result<()> {
    let dependents_updater = create_dependent_updater(DRY_RUN, dependents_db);
    dependents_updater.update_all(component, new_version)
}

fn publish(component: PackageMetadata) -> Result<()> {
    let mut publisher = create_publisher(DRY_RUN, component)?;
    publisher.publish()
}

fn make_commit(new_version: &str, component: PackageMetadata, crate_folder: &Path) -> Result<()> {
    let mut command = create_git(DRY_RUN, crate_folder);
    command.commit_package(component.name(), new_version);
    command.run()
}