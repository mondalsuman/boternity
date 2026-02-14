//! Permission checking and capability enforcement for skill execution.
//!
//! Before a skill runs, its requested capabilities are validated against the
//! user's permission grants. Any denied capability terminates execution
//! immediately with a [`PermissionError`].

use std::collections::HashSet;

use boternity_types::skill::{Capability, PermissionGrant, SkillManifest};
use chrono::Utc;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Error returned when a skill lacks a required capability.
#[derive(Debug, thiserror::Error)]
pub enum PermissionError {
    /// A specific capability was denied for the skill.
    #[error("capability {capability:?} denied for skill '{skill_name}'")]
    Denied {
        capability: Capability,
        skill_name: String,
    },

    /// The skill has no permission grants at all.
    #[error("no permission grants exist for skill '{skill_name}'")]
    NoGrants { skill_name: String },
}

// ---------------------------------------------------------------------------
// CapabilityEnforcer
// ---------------------------------------------------------------------------

/// Validates whether a skill has permission for specific operations.
///
/// Built from a set of [`PermissionGrant`]s, the enforcer holds only the
/// granted capabilities and provides O(1) lookup for each check.
/// Permission violations terminate execution immediately.
#[derive(Debug)]
pub struct CapabilityEnforcer {
    granted: HashSet<Capability>,
    skill_name: String,
}

impl CapabilityEnforcer {
    /// Create a new enforcer from the given permission grants.
    ///
    /// Only grants with `granted = true` are included in the capability set.
    /// Returns a `PermissionError::NoGrants` if the grants slice is empty.
    pub fn new(skill_name: &str, grants: &[PermissionGrant]) -> Result<Self, PermissionError> {
        if grants.is_empty() {
            return Err(PermissionError::NoGrants {
                skill_name: skill_name.to_string(),
            });
        }

        let granted = grants
            .iter()
            .filter(|g| g.granted)
            .map(|g| g.capability.clone())
            .collect();

        Ok(Self {
            granted,
            skill_name: skill_name.to_string(),
        })
    }

    /// Check whether a single capability is granted.
    ///
    /// Returns `Ok(())` if the capability is in the granted set,
    /// `Err(PermissionError::Denied)` otherwise.
    pub fn check(&self, capability: &Capability) -> Result<(), PermissionError> {
        if self.granted.contains(capability) {
            Ok(())
        } else {
            Err(PermissionError::Denied {
                capability: capability.clone(),
                skill_name: self.skill_name.clone(),
            })
        }
    }

    /// Check that all capabilities in the slice are granted.
    ///
    /// Short-circuits on the first denied capability.
    pub fn check_all(&self, capabilities: &[Capability]) -> Result<(), PermissionError> {
        for cap in capabilities {
            self.check(cap)?;
        }
        Ok(())
    }

    /// Returns a reference to the set of granted capabilities.
    pub fn granted_capabilities(&self) -> &HashSet<Capability> {
        &self.granted
    }

    /// Quick boolean check for a single capability.
    pub fn has_capability(&self, capability: &Capability) -> bool {
        self.granted.contains(capability)
    }
}

// ---------------------------------------------------------------------------
// Permission management functions
// ---------------------------------------------------------------------------

/// Create permission grants from a skill manifest.
///
/// If `approved` is true, all capabilities declared in the manifest are
/// granted. If false, they are recorded but denied (user must approve later).
pub fn create_grants_from_manifest(
    manifest: &SkillManifest,
    approved: bool,
) -> Vec<PermissionGrant> {
    let capabilities = manifest
        .metadata
        .as_ref()
        .and_then(|m| m.capabilities.as_ref())
        .cloned()
        .unwrap_or_default();

    let now = Utc::now();

    capabilities
        .into_iter()
        .map(|capability| PermissionGrant {
            skill_name: manifest.name.clone(),
            capability,
            granted: approved,
            granted_at: now,
        })
        .collect()
}

/// Revoke a specific capability from the grants list.
///
/// Sets `granted = false` for the matching capability. If the capability
/// is not in the list, this is a no-op.
pub fn revoke_capability(grants: &mut Vec<PermissionGrant>, capability: &Capability) {
    for grant in grants.iter_mut() {
        if &grant.capability == capability {
            grant.granted = false;
        }
    }
}

/// Grant a specific capability in the grants list.
///
/// Sets `granted = true` for the matching capability. If the capability
/// is not in the list, a new grant entry is added.
pub fn grant_capability(grants: &mut Vec<PermissionGrant>, capability: &Capability) {
    for grant in grants.iter_mut() {
        if &grant.capability == capability {
            grant.granted = true;
            return;
        }
    }

    // Capability not found -- add a new grant entry.
    // We don't know the skill name from this context, but the grants list
    // should already have entries with the skill name set.
    let skill_name = grants
        .first()
        .map(|g| g.skill_name.clone())
        .unwrap_or_default();

    grants.push(PermissionGrant {
        skill_name,
        capability: capability.clone(),
        granted: true,
        granted_at: Utc::now(),
    });
}

/// Merge child and parent grants, producing a combined grant set.
///
/// Child grants take precedence when both child and parent have a grant for
/// the same capability. Parent-only grants are inherited as-is.
pub fn merge_inherited_grants(
    child: &[PermissionGrant],
    parent: &[PermissionGrant],
) -> Vec<PermissionGrant> {
    let child_caps: HashSet<&Capability> = child.iter().map(|g| &g.capability).collect();

    let mut merged: Vec<PermissionGrant> = child.to_vec();

    for parent_grant in parent {
        if !child_caps.contains(&parent_grant.capability) {
            merged.push(parent_grant.clone());
        }
    }

    merged
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use boternity_types::skill::{Capability, SkillManifest, SkillMetadata};

    fn make_grant(skill: &str, cap: Capability, granted: bool) -> PermissionGrant {
        PermissionGrant {
            skill_name: skill.to_string(),
            capability: cap,
            granted,
            granted_at: Utc::now(),
        }
    }

    fn make_manifest(name: &str, caps: Vec<Capability>) -> SkillManifest {
        SkillManifest {
            name: name.to_string(),
            description: "test skill".to_string(),
            license: None,
            compatibility: None,
            allowed_tools: None,
            metadata: Some(SkillMetadata {
                author: None,
                version: None,
                skill_type: None,
                capabilities: Some(caps),
                dependencies: None,
                conflicts_with: None,
                trust_tier: None,
                parents: None,
                secrets: None,
                categories: None,
            }),
        }
    }

    #[test]
    fn enforcer_allows_granted_capability() {
        let grants = vec![
            make_grant("test-skill", Capability::HttpGet, true),
            make_grant("test-skill", Capability::ReadFile, true),
        ];

        let enforcer = CapabilityEnforcer::new("test-skill", &grants).unwrap();
        assert!(enforcer.check(&Capability::HttpGet).is_ok());
        assert!(enforcer.check(&Capability::ReadFile).is_ok());
        assert!(enforcer.has_capability(&Capability::HttpGet));
    }

    #[test]
    fn enforcer_denies_non_granted_capability() {
        let grants = vec![make_grant("test-skill", Capability::HttpGet, true)];

        let enforcer = CapabilityEnforcer::new("test-skill", &grants).unwrap();
        let err = enforcer.check(&Capability::WriteFile).unwrap_err();

        match err {
            PermissionError::Denied {
                capability,
                skill_name,
            } => {
                assert_eq!(capability, Capability::WriteFile);
                assert_eq!(skill_name, "test-skill");
            }
            _ => panic!("expected Denied error"),
        }
    }

    #[test]
    fn enforcer_denies_explicitly_denied_capability() {
        let grants = vec![
            make_grant("test-skill", Capability::HttpGet, true),
            make_grant("test-skill", Capability::WriteFile, false),
        ];

        let enforcer = CapabilityEnforcer::new("test-skill", &grants).unwrap();
        assert!(enforcer.check(&Capability::HttpGet).is_ok());
        assert!(enforcer.check(&Capability::WriteFile).is_err());
        assert!(!enforcer.has_capability(&Capability::WriteFile));
    }

    #[test]
    fn revoke_then_check_returns_denied() {
        let mut grants = vec![
            make_grant("test-skill", Capability::HttpGet, true),
            make_grant("test-skill", Capability::ReadFile, true),
        ];

        // Revoke HttpGet
        revoke_capability(&mut grants, &Capability::HttpGet);

        let enforcer = CapabilityEnforcer::new("test-skill", &grants).unwrap();
        assert!(enforcer.check(&Capability::HttpGet).is_err());
        assert!(enforcer.check(&Capability::ReadFile).is_ok());
    }

    #[test]
    fn merge_inherited_grants_combines_both_sets() {
        let child = vec![make_grant("child-skill", Capability::HttpGet, true)];

        let parent = vec![
            make_grant("parent-skill", Capability::ReadFile, true),
            make_grant("parent-skill", Capability::HttpGet, false), // overridden by child
        ];

        let merged = merge_inherited_grants(&child, &parent);
        assert_eq!(merged.len(), 2); // HttpGet from child, ReadFile from parent

        // Child's HttpGet grant takes precedence (granted = true)
        let http_get = merged
            .iter()
            .find(|g| g.capability == Capability::HttpGet)
            .unwrap();
        assert!(http_get.granted);

        // Parent's ReadFile inherited
        let read_file = merged
            .iter()
            .find(|g| g.capability == Capability::ReadFile)
            .unwrap();
        assert!(read_file.granted);
    }

    #[test]
    fn empty_grants_returns_no_grants_error() {
        let grants: Vec<PermissionGrant> = vec![];
        let err = CapabilityEnforcer::new("test-skill", &grants).unwrap_err();

        match err {
            PermissionError::NoGrants { skill_name } => {
                assert_eq!(skill_name, "test-skill");
            }
            _ => panic!("expected NoGrants error"),
        }
    }

    #[test]
    fn check_all_validates_every_capability() {
        let grants = vec![
            make_grant("test-skill", Capability::HttpGet, true),
            make_grant("test-skill", Capability::ReadFile, true),
        ];

        let enforcer = CapabilityEnforcer::new("test-skill", &grants).unwrap();

        assert!(enforcer
            .check_all(&[Capability::HttpGet, Capability::ReadFile])
            .is_ok());

        assert!(enforcer
            .check_all(&[Capability::HttpGet, Capability::WriteFile])
            .is_err());
    }

    #[test]
    fn create_grants_from_manifest_approved() {
        let manifest = make_manifest("my-skill", vec![Capability::HttpGet, Capability::ReadFile]);
        let grants = create_grants_from_manifest(&manifest, true);

        assert_eq!(grants.len(), 2);
        assert!(grants.iter().all(|g| g.granted));
        assert!(grants.iter().all(|g| g.skill_name == "my-skill"));
    }

    #[test]
    fn create_grants_from_manifest_denied() {
        let manifest = make_manifest("my-skill", vec![Capability::HttpGet]);
        let grants = create_grants_from_manifest(&manifest, false);

        assert_eq!(grants.len(), 1);
        assert!(!grants[0].granted);
    }

    #[test]
    fn grant_capability_adds_new_entry() {
        let mut grants = vec![make_grant("test-skill", Capability::HttpGet, true)];

        grant_capability(&mut grants, &Capability::WriteFile);

        assert_eq!(grants.len(), 2);
        let new_grant = grants
            .iter()
            .find(|g| g.capability == Capability::WriteFile)
            .unwrap();
        assert!(new_grant.granted);
        assert_eq!(new_grant.skill_name, "test-skill");
    }

    #[test]
    fn grant_capability_enables_existing_entry() {
        let mut grants = vec![make_grant("test-skill", Capability::HttpGet, false)];

        grant_capability(&mut grants, &Capability::HttpGet);

        assert_eq!(grants.len(), 1);
        assert!(grants[0].granted);
    }

    #[test]
    fn granted_capabilities_returns_correct_set() {
        let grants = vec![
            make_grant("test-skill", Capability::HttpGet, true),
            make_grant("test-skill", Capability::ReadFile, false),
            make_grant("test-skill", Capability::WriteFile, true),
        ];

        let enforcer = CapabilityEnforcer::new("test-skill", &grants).unwrap();
        let caps = enforcer.granted_capabilities();

        assert_eq!(caps.len(), 2);
        assert!(caps.contains(&Capability::HttpGet));
        assert!(caps.contains(&Capability::WriteFile));
        assert!(!caps.contains(&Capability::ReadFile));
    }
}
