// Recursion limit bump is to support Ritz, a JSX-like proc macro used for
// Rojo's web UI currently.
#![recursion_limit = "1024"]

#[macro_use]
mod impl_from;

pub mod cli;
pub mod commands;
pub mod project;

#[cfg(test)]
mod tree_view;

mod auth_cookie;
mod change_processor;
mod common_setup;
mod message_queue;
mod multimap;
mod path_map;
mod path_serializer;
mod serve_session;
mod session_id;
mod snapshot;
mod snapshot_middleware;
mod vfs;
mod web;

pub use crate::session_id::SessionId;
pub use crate::web::interface as web_interface;

use std::error::Error;

use rbx_dom_weak::{RbxInstanceProperties, RbxTree};

use crate::{
    project::Project,
    snapshot::{
        apply_patch_set, compute_patch_set, InstanceContext, InstancePropertiesWithMeta, RojoTree,
    },
    snapshot_middleware::snapshot_project_node,
    vfs::{RealFetcher, Vfs, WatchMode},
};

pub fn build_project(project: &Project) -> Result<RbxTree, Box<dyn Error>> {
    let vfs = Vfs::new(RealFetcher::new(WatchMode::Disabled));

    let mut tree = RojoTree::new(InstancePropertiesWithMeta {
        properties: RbxInstanceProperties {
            name: "ROOT".to_owned(),
            class_name: "Folder".to_owned(),
            properties: Default::default(),
        },
        metadata: Default::default(),
    });

    let snapshot = snapshot_project_node(
        &InstanceContext::default(),
        project.folder_location(),
        &project.name,
        &project.tree,
        &vfs,
    )
    .expect("snapshot failed")
    .expect("snapshot did not return an instance");

    let root_id = tree.get_root_id();
    let patch_set = compute_patch_set(&snapshot, &tree, root_id);

    apply_patch_set(&mut tree, patch_set);

    Ok(tree.into_inner())
}
