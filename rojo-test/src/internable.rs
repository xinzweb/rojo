use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

use rbx_dom_weak::RbxId;
use serde::Serialize;
use tempfile::{tempdir, TempDir};

use librojo::web_interface::{Instance, ReadResponse, ServerInfoResponse, SubscribeResponse};
use rojo_insta_ext::RedactionMap;

use crate::util::{
    copy_recursive, get_rojo_path, get_serve_tests_path, get_working_dir_path, KillOnDrop,
};

pub trait InternAndRedact<T> {
    fn intern_and_redact(&self, redactions: &mut RedactionMap, extra: T) -> serde_yaml::Value;
}

impl<I, T> InternAndRedact<T> for I
where
    I: Serialize + Internable<T>,
{
    fn intern_and_redact(&self, redactions: &mut RedactionMap, extra: T) -> serde_yaml::Value {
        self.intern(redactions, extra);
        redactions.redacted_yaml(self)
    }
}

pub trait Internable<T> {
    fn intern(&self, redactions: &mut RedactionMap, extra: T);
}

impl Internable<RbxId> for ReadResponse<'_> {
    fn intern(&self, redactions: &mut RedactionMap, root_id: RbxId) {
        redactions.intern(root_id);

        let root_instance = self.instances.get(&root_id).unwrap();

        for &child_id in root_instance.children.iter() {
            self.intern(redactions, child_id);
        }
    }
}

impl<'a> Internable<&'a HashMap<RbxId, Instance<'_>>> for Instance<'a> {
    fn intern(
        &self,
        redactions: &mut RedactionMap,
        other_instances: &HashMap<RbxId, Instance<'_>>,
    ) {
        redactions.intern(self.id);

        for child_id in self.children.iter() {
            let child = &other_instances[child_id];
            child.intern(redactions, other_instances);
        }
    }
}

impl Internable<()> for SubscribeResponse<'_> {
    fn intern(&self, redactions: &mut RedactionMap, _extra: ()) {
        for message in &self.messages {
            for update in &message.updated_instances {
                redactions.intern(update.id);
            }

            let mut added_roots = Vec::new();

            for (id, added) in &message.added_instances {
                let parent_id = added.parent.unwrap();
                let parent_redacted = redactions.get_redacted_value(parent_id);

                // Here, we assume that instances are only added to other
                // instances that we've already interned. If that's not true,
                // then we'll have some dangling unredacted IDs.
                if let Some(parent_redacted) = parent_redacted {
                    added_roots.push((id, parent_redacted));
                }
            }

            // Sort the input by the redacted key, which should match the
            // traversal order we need for the tree.
            added_roots.sort_unstable_by(|a, b| a.1.cmp(&b.1));

            for (root_id, _redacted_id) in added_roots {
                message.added_instances[root_id].intern(redactions, &message.added_instances);
            }
        }
    }
}
