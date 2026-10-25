//! Project folders, instructions, and memory bindings (roadmap Phase 6;
//! `docs/features/projects.md`, `REQ-PROJ-01..06`).
//!
//! The service is a thin, validating façade over [`myharness_store`]: the
//! shipped project behavior was never observed beyond the create form's
//! validation surface, so everything here is rebuild-defined and recorded in
//! `docs/14` §9 —
//!
//! - **Identity** is the project id; duplicate names are allowed (strong
//!   inference recorded in the feature doc).
//! - **Bounds** are resource limits, not observed caps: a name is at most 200
//!   characters, instructions at most 16 KiB. Both are refused, not truncated.
//! - **The folder is canonicalized by the bridge** against the effective
//!   policy before this layer sees it (one path validator, one truth —
//!   `crates/bridge/src/policy.rs`). This layer only refuses an empty cwd.
//! - **The stored cwd is never rewritten** by edits: past sessions ran in the
//!   recorded folder, and a silently moved folder would lie about where the
//!   work happened (`REQ-PROJ-06`). A different folder is a different project.
//! - **Deletion removes only the project row.** Sessions keep their own cwd
//!   and their `project_id` resolves to "project missing" afterwards; files on
//!   disk are never touched (`REQ-PROJ-06`, `TEST-SESSION-005`).
//!
//! There is no project-level audit chain: `audit_events` is session-scoped in
//! the rebuild model and no project audit was captured; the omission is
//! recorded in `docs/14` §9, not hidden.

use myharness_store::{NewProject, ProjectRow, Store, StoreError};

/// Resource bound: longest project name this rebuild accepts.
pub const MAX_NAME_CHARS: usize = 200;
/// Resource bound: largest instructions payload this rebuild accepts.
pub const MAX_INSTRUCTION_BYTES: usize = 16 * 1024;

/// Project-layer failures. `NotFound` and validation shapes mirror the store's
/// error posture; the bridge maps these onto its typed wire errors.
#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("the requested project does not exist")]
    NotFound,
    #[error("{0}")]
    Validation(&'static str),
    #[error("the underlying store failed: {0}")]
    Store(#[from] StoreError),
}

/// The project service. Borrows the store — the chat service owns it and
/// lends it out per call.
pub struct ProjectService<'a> {
    store: &'a Store,
}

impl<'a> ProjectService<'a> {
    pub fn new(store: &'a Store) -> ProjectService<'a> {
        ProjectService { store }
    }

    /// Create a project. The name is trimmed and must be non-empty; the
    /// instructions are trimmed and empty becomes `None`; the cwd must be
    /// non-empty (the bridge owns canonicalization and policy).
    pub fn create(
        &self,
        name: &str,
        instructions: Option<&str>,
        cwd: &str,
        memory_enabled: bool,
    ) -> Result<ProjectRow, ProjectError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(ProjectError::Validation("the project name is required"));
        }
        if name.chars().count() > MAX_NAME_CHARS {
            return Err(ProjectError::Validation("the project name is too long"));
        }
        let instructions = instructions.map(str::trim).filter(|s| !s.is_empty());
        if let Some(text) = instructions {
            if text.len() > MAX_INSTRUCTION_BYTES {
                return Err(ProjectError::Validation(
                    "the project instructions are too long",
                ));
            }
        }
        if cwd.trim().is_empty() {
            return Err(ProjectError::Validation("the project folder is required"));
        }
        Ok(self.store.create_project(NewProject {
            name,
            instructions,
            cwd,
            memory_enabled,
            extra: serde_json::json!({}),
        })?)
    }

    /// One project by id.
    pub fn read(&self, id: &str) -> Result<ProjectRow, ProjectError> {
        match self.store.read_project(id) {
            Ok(row) => Ok(row),
            Err(StoreError::NotFound) => Err(ProjectError::NotFound),
            Err(err) => Err(err.into()),
        }
    }

    /// All projects, newest activity first.
    pub fn list(&self) -> Result<Vec<ProjectRow>, ProjectError> {
        Ok(self.store.list_projects()?)
    }

    /// Update name/instructions/memory. The folder is intentionally not
    /// editable (module docs).
    pub fn update(
        &self,
        id: &str,
        name: &str,
        instructions: Option<&str>,
        memory_enabled: bool,
    ) -> Result<(), ProjectError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(ProjectError::Validation("the project name is required"));
        }
        if name.chars().count() > MAX_NAME_CHARS {
            return Err(ProjectError::Validation("the project name is too long"));
        }
        let instructions = instructions.map(str::trim).filter(|s| !s.is_empty());
        if let Some(text) = instructions {
            if text.len() > MAX_INSTRUCTION_BYTES {
                return Err(ProjectError::Validation(
                    "the project instructions are too long",
                ));
            }
        }
        self.store
            .update_project(id, name, instructions, memory_enabled)?;
        Ok(())
    }

    /// Delete the project. Returns the number of sessions that still
    /// reference the deleted id, so the caller can say exactly what the
    /// deletion left behind (`REQ-PROJ-06`).
    pub fn delete(&self, id: &str) -> Result<usize, ProjectError> {
        Ok(self.store.delete_project(id)?)
    }

    /// The sessions associated with one project.
    pub fn sessions(
        &self,
        project_id: &str,
        archived: Option<bool>,
    ) -> Result<Vec<myharness_store::SessionRow>, ProjectError> {
        Ok(self.store.list_sessions_for_project(project_id, archived)?)
    }

    /// Bump the project's activity stamp (a session in it turned).
    pub fn touch(&self, id: &str) -> Result<(), ProjectError> {
        self.store.touch_project(id)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ProjectError, ProjectService, MAX_INSTRUCTION_BYTES, MAX_NAME_CHARS};

    fn store(dir: &std::path::Path) -> myharness_store::Store {
        myharness_store::Store::open(&dir.join("state.db"), &dir.join("files")).expect("store")
    }

    #[test]
    fn create_trims_and_normalizes() {
        let dir = tempfile::tempdir().expect("temp");
        let store = store(dir.path());
        let service = ProjectService::new(&store);
        let project = service
            .create("  Paper reviews  ", Some("  be concise  "), "/tmp/papers", true)
            .expect("create");
        assert_eq!(project.name, "Paper reviews");
        assert_eq!(project.instructions.as_deref(), Some("be concise"));
        assert!(project.memory_enabled);
    }

    #[test]
    fn empty_instructions_store_as_none() {
        let dir = tempfile::tempdir().expect("temp");
        let store = store(dir.path());
        let service = ProjectService::new(&store);
        let project = service
            .create("p", Some("   "), "/tmp/p", false)
            .expect("create");
        assert_eq!(project.instructions, None);
        assert!(!project.memory_enabled);
    }

    #[test]
    fn bounds_are_refused_not_truncated() {
        let dir = tempfile::tempdir().expect("temp");
        let store = store(dir.path());
        let service = ProjectService::new(&store);
        let long_name = "x".repeat(MAX_NAME_CHARS + 1);
        assert!(matches!(
            service.create(&long_name, None, "/tmp/p", true),
            Err(ProjectError::Validation("the project name is too long"))
        ));
        let long_instructions = "y".repeat(MAX_INSTRUCTION_BYTES + 1);
        assert!(matches!(
            service.create("p", Some(&long_instructions), "/tmp/p", true),
            Err(ProjectError::Validation(
                "the project instructions are too long"
            ))
        ));
        assert!(matches!(
            service.create("   ", None, "/tmp/p", true),
            Err(ProjectError::Validation("the project name is required"))
        ));
        assert!(matches!(
            service.create("p", None, "   ", true),
            Err(ProjectError::Validation("the project folder is required"))
        ));
    }

    #[test]
    fn update_never_changes_the_folder_and_delete_reports_orphans() {
        let dir = tempfile::tempdir().expect("temp");
        let store = store(dir.path());
        let service = ProjectService::new(&store);
        let project = service.create("p", None, "/tmp/kept", true).expect("create");
        service
            .update(&project.id, "renamed", Some("rules"), false)
            .expect("update");
        let updated = service.read(&project.id).expect("read");
        assert_eq!(updated.name, "renamed");
        assert_eq!(updated.cwd, "/tmp/kept");
        assert!(!updated.memory_enabled);

        // A session associated first, so deletion can report it.
        store
            .create_session(myharness_store::NewSession {
                surface: "chat",
                title: "s",
                cwd: Some("/tmp/kept"),
                project_id: Some(&project.id),
                ..Default::default()
            })
            .expect("session");
        let orphaned = service.delete(&project.id).expect("delete");
        assert_eq!(orphaned, 1);
        assert!(matches!(service.read(&project.id), Err(ProjectError::NotFound)));
        // The session survives with its own cwd and its stale association. The
        // store keeps the stale id queryable; "project missing" is resolved at
        // the bridge when the project row itself is read — never a silent
        // retarget (`REQ-PROJ-06`).
        let all = store.list_sessions(None).expect("all");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].cwd.as_deref(), Some("/tmp/kept"));
        assert_eq!(all[0].project_id.as_deref(), Some(project.id.as_str()));
        let stale = service.sessions(&project.id, None).expect("sessions");
        assert_eq!(stale.len(), 1);
    }

    #[test]
    fn list_orders_by_activity() {
        let dir = tempfile::tempdir().expect("temp");
        let store = store(dir.path());
        let service = ProjectService::new(&store);
        let first = service.create("first", None, "/tmp/a", true).expect("c");
        let _second = service.create("second", None, "/tmp/b", true).expect("c");
        // Touch lands in a later millisecond than the creations, so the order
        // assertion is deterministic, not an id tie-break coin flip.
        std::thread::sleep(std::time::Duration::from_millis(3));
        service.touch(&first.id).expect("touch");
        let listed = service.list().expect("list");
        assert_eq!(listed[0].name, "first");
        assert_eq!(listed[1].name, "second");
    }
}
