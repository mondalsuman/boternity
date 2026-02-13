//! Skill inheritance and mixin composition resolver.
//!
//! Implements the mixin/composition model for skill inheritance:
//! - Child composes parent capabilities additively
//! - Max 3 levels of inheritance depth (skill, parent, grandparent)
//! - Multiple parent composition allowed
//! - Last-wins ordering for capability conflicts across multiple parents
//! - Circular inheritance detected and prevented

use std::collections::{HashMap, HashSet};

use anyhow::bail;

use boternity_types::skill::{Capability, SkillManifest};

/// Maximum allowed inheritance depth (skill -> parent -> grandparent).
const MAX_INHERITANCE_DEPTH: usize = 3;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A skill with all inherited capabilities resolved.
#[derive(Debug, Clone)]
pub struct ResolvedSkill {
    /// The skill's name.
    pub name: String,
    /// Combined capabilities (own + inherited, with last-wins for conflicts).
    pub capabilities: Vec<Capability>,
    /// Resolved parent chain (flattened).
    pub parents: Vec<String>,
    /// Depth of this skill in the inheritance tree.
    pub depth: usize,
}

/// Detailed view of a skill's own and inherited capabilities.
///
/// Powers `bnity skill inspect <name>` for debugging and understanding
/// capability composition.
#[derive(Debug, Clone)]
pub struct InspectedSkill {
    /// The skill's name.
    pub name: String,
    /// Capabilities declared directly on this skill.
    pub own_capabilities: Vec<Capability>,
    /// Capabilities inherited from parent skills.
    pub inherited_capabilities: Vec<Capability>,
    /// Combined capabilities (own + inherited).
    pub combined_capabilities: Vec<Capability>,
    /// The full parent chain (flattened, ordered by resolution).
    pub parent_chain: Vec<String>,
    /// All conflicts_with declarations (own + inherited).
    pub conflicts_with: Vec<String>,
    /// Depth of this skill in the inheritance tree.
    pub depth: usize,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Resolve a skill's inheritance chain, merging capabilities from parents.
///
/// Recursively walks the parent chain up to `MAX_INHERITANCE_DEPTH` levels,
/// composing capabilities additively. When multiple parents declare the same
/// capability, the last parent in the list wins (last-wins ordering).
///
/// The `visited` set is used internally for cycle detection. Callers should
/// pass an empty `HashSet`.
pub fn resolve_inheritance(
    skill_name: &str,
    all_skills: &HashMap<String, SkillManifest>,
    depth: usize,
    visited: &mut HashSet<String>,
) -> anyhow::Result<ResolvedSkill> {
    if depth > MAX_INHERITANCE_DEPTH {
        bail!(
            "Inheritance depth exceeded (max {} levels) for skill: {}",
            MAX_INHERITANCE_DEPTH,
            skill_name
        );
    }

    if !visited.insert(skill_name.to_string()) {
        bail!(
            "Circular inheritance detected: {} appears twice in the chain",
            skill_name
        );
    }

    let manifest = all_skills.get(skill_name).ok_or_else(|| {
        anyhow::anyhow!("Skill '{}' not found in skill registry", skill_name)
    })?;

    // Start with the skill's own capabilities
    let own_capabilities = manifest
        .metadata
        .as_ref()
        .and_then(|m| m.capabilities.as_ref())
        .cloned()
        .unwrap_or_default();

    let parents = manifest
        .metadata
        .as_ref()
        .and_then(|m| m.parents.as_ref())
        .cloned()
        .unwrap_or_default();

    // Resolve each parent recursively, collecting their capabilities
    let mut parent_chain = Vec::new();
    let mut inherited_caps: Vec<Capability> = Vec::new();

    for parent_name in &parents {
        let resolved_parent =
            resolve_inheritance(parent_name, all_skills, depth + 1, visited)?;

        parent_chain.push(parent_name.clone());
        parent_chain.extend(resolved_parent.parents.clone());

        // Add parent capabilities (last-wins: later parents overwrite earlier)
        for cap in &resolved_parent.capabilities {
            // Remove any existing occurrence so the latest parent's version wins
            inherited_caps.retain(|c| c != cap);
            inherited_caps.push(cap.clone());
        }
    }

    // Combine: own capabilities take precedence, then inherited
    let mut combined = inherited_caps;
    for cap in &own_capabilities {
        combined.retain(|c| c != cap);
        combined.push(cap.clone());
    }

    // Remove the current skill from visited so sibling branches can reference it
    // (only circular chains within a single path are errors)
    visited.remove(skill_name);

    Ok(ResolvedSkill {
        name: skill_name.to_string(),
        capabilities: combined,
        parents: parent_chain,
        depth,
    })
}

/// Check for circular inheritance in the parent chain.
///
/// Walks the full parent tree, returning an error with the cycle chain
/// if any circular reference is found.
pub fn check_circular_inheritance(
    skill_name: &str,
    all_skills: &HashMap<String, SkillManifest>,
) -> anyhow::Result<()> {
    let mut visited = Vec::new();
    check_circular_recursive(skill_name, all_skills, &mut visited)
}

fn check_circular_recursive(
    skill_name: &str,
    all_skills: &HashMap<String, SkillManifest>,
    visited: &mut Vec<String>,
) -> anyhow::Result<()> {
    if visited.contains(&skill_name.to_string()) {
        visited.push(skill_name.to_string());
        let cycle_start = visited
            .iter()
            .position(|s| s == skill_name)
            .unwrap();
        let cycle_chain = visited[cycle_start..].join(" -> ");
        bail!("Circular inheritance detected: {}", cycle_chain);
    }

    visited.push(skill_name.to_string());

    if let Some(manifest) = all_skills.get(skill_name) {
        if let Some(ref metadata) = manifest.metadata {
            if let Some(ref parents) = metadata.parents {
                for parent in parents {
                    check_circular_recursive(parent, all_skills, visited)?;
                }
            }
        }
    }

    visited.pop();
    Ok(())
}

/// Collect all `conflicts_with` declarations from a skill and its entire
/// inheritance chain.
///
/// Returns a deduplicated list of skill names that conflict with the given
/// skill (directly or through inherited parent declarations).
pub fn resolve_conflicts_with_across_chain(
    skill_name: &str,
    all_skills: &HashMap<String, SkillManifest>,
) -> anyhow::Result<Vec<String>> {
    let mut conflicts = HashSet::new();
    let mut visited = HashSet::new();
    collect_conflicts_recursive(skill_name, all_skills, &mut conflicts, &mut visited)?;
    Ok(conflicts.into_iter().collect())
}

fn collect_conflicts_recursive(
    skill_name: &str,
    all_skills: &HashMap<String, SkillManifest>,
    conflicts: &mut HashSet<String>,
    visited: &mut HashSet<String>,
) -> anyhow::Result<()> {
    if !visited.insert(skill_name.to_string()) {
        return Ok(()); // Already processed, avoid infinite recursion
    }

    if let Some(manifest) = all_skills.get(skill_name) {
        if let Some(ref metadata) = manifest.metadata {
            // Collect this skill's conflicts_with
            if let Some(ref skill_conflicts) = metadata.conflicts_with {
                for c in skill_conflicts {
                    conflicts.insert(c.clone());
                }
            }

            // Recurse into parents
            if let Some(ref parents) = metadata.parents {
                for parent in parents {
                    collect_conflicts_recursive(parent, all_skills, conflicts, visited)?;
                }
            }
        }
    }

    Ok(())
}

/// Inspect a skill's resolved capabilities, showing own vs inherited.
///
/// Returns a detailed breakdown suitable for `bnity skill inspect <name>`.
pub fn inspect_resolved_capabilities(
    skill_name: &str,
    all_skills: &HashMap<String, SkillManifest>,
) -> anyhow::Result<InspectedSkill> {
    let manifest = all_skills.get(skill_name).ok_or_else(|| {
        anyhow::anyhow!("Skill '{}' not found in skill registry", skill_name)
    })?;

    let own_capabilities = manifest
        .metadata
        .as_ref()
        .and_then(|m| m.capabilities.as_ref())
        .cloned()
        .unwrap_or_default();

    // Resolve full inheritance
    let mut visited = HashSet::new();
    let resolved = resolve_inheritance(skill_name, all_skills, 0, &mut visited)?;

    // Compute inherited = combined - own
    let own_set: HashSet<&Capability> = own_capabilities.iter().collect();
    let inherited_capabilities: Vec<Capability> = resolved
        .capabilities
        .iter()
        .filter(|c| !own_set.contains(c))
        .cloned()
        .collect();

    // Collect conflicts across chain
    let conflicts = resolve_conflicts_with_across_chain(skill_name, all_skills)?;

    Ok(InspectedSkill {
        name: skill_name.to_string(),
        own_capabilities,
        inherited_capabilities,
        combined_capabilities: resolved.capabilities,
        parent_chain: resolved.parents,
        conflicts_with: conflicts,
        depth: resolved.depth,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use boternity_types::skill::{Capability, SkillManifest, SkillMetadata};

    /// Helper to create a SkillManifest with optional capabilities, parents,
    /// and conflicts.
    fn make_manifest(
        name: &str,
        capabilities: Vec<Capability>,
        parents: Option<Vec<&str>>,
        conflicts: Option<Vec<&str>>,
    ) -> SkillManifest {
        SkillManifest {
            name: name.to_string(),
            description: format!("Test skill {}", name),
            license: None,
            compatibility: None,
            allowed_tools: None,
            metadata: Some(SkillMetadata {
                author: None,
                version: None,
                skill_type: None,
                capabilities: Some(capabilities),
                dependencies: None,
                conflicts_with: conflicts.map(|c| c.into_iter().map(String::from).collect()),
                trust_tier: None,
                parents: parents.map(|p| p.into_iter().map(String::from).collect()),
                secrets: None,
                categories: None,
            }),
        }
    }

    #[test]
    fn test_single_parent_inheritance() {
        let mut skills: HashMap<String, SkillManifest> = HashMap::new();
        skills.insert(
            String::from("child"),
            make_manifest("child", vec![Capability::HttpGet], Some(vec!["parent"]), None),
        );
        skills.insert(
            String::from("parent"),
            make_manifest("parent", vec![Capability::ReadFile], None, None),
        );

        let mut visited = HashSet::new();
        let resolved = resolve_inheritance("child", &skills, 0, &mut visited).unwrap();

        assert_eq!(resolved.name, "child");
        assert!(resolved.capabilities.contains(&Capability::HttpGet));
        assert!(resolved.capabilities.contains(&Capability::ReadFile));
        assert_eq!(resolved.parents, vec!["parent"]);
    }

    #[test]
    fn test_multi_parent_last_wins() {
        // Both parents provide HttpGet (same capability), last parent wins.
        // Parent A provides ReadFile, Parent B provides WriteFile.
        let mut skills: HashMap<String, SkillManifest> = HashMap::new();
        skills.insert(
            String::from("child"),
            make_manifest(
                "child",
                vec![],
                Some(vec!["parent_a", "parent_b"]),
                None,
            ),
        );
        skills.insert(
            String::from("parent_a"),
            make_manifest(
                "parent_a",
                vec![Capability::ReadFile, Capability::HttpGet],
                None,
                None,
            ),
        );
        skills.insert(
            String::from("parent_b"),
            make_manifest(
                "parent_b",
                vec![Capability::WriteFile, Capability::HttpGet],
                None,
                None,
            ),
        );

        let mut visited = HashSet::new();
        let resolved = resolve_inheritance("child", &skills, 0, &mut visited).unwrap();

        // All three capabilities should be present
        assert!(resolved.capabilities.contains(&Capability::ReadFile));
        assert!(resolved.capabilities.contains(&Capability::WriteFile));
        assert!(resolved.capabilities.contains(&Capability::HttpGet));

        // HttpGet should appear once (last parent wins deduplication)
        let http_get_count = resolved
            .capabilities
            .iter()
            .filter(|c| **c == Capability::HttpGet)
            .count();
        assert_eq!(http_get_count, 1);
    }

    #[test]
    fn test_three_level_deep_works() {
        let mut skills: HashMap<String, SkillManifest> = HashMap::new();
        skills.insert(
            String::from("child"),
            make_manifest("child", vec![Capability::HttpGet], Some(vec!["parent"]), None),
        );
        skills.insert(
            String::from("parent"),
            make_manifest(
                "parent",
                vec![Capability::ReadFile],
                Some(vec!["grandparent"]),
                None,
            ),
        );
        skills.insert(
            String::from("grandparent"),
            make_manifest("grandparent", vec![Capability::WriteFile], None, None),
        );

        let mut visited = HashSet::new();
        // depth 0 for child, 1 for parent, 2 for grandparent -> all within limit
        let resolved = resolve_inheritance("child", &skills, 0, &mut visited).unwrap();

        assert!(resolved.capabilities.contains(&Capability::HttpGet));
        assert!(resolved.capabilities.contains(&Capability::ReadFile));
        assert!(resolved.capabilities.contains(&Capability::WriteFile));
        assert_eq!(resolved.parents, vec!["parent", "grandparent"]);
    }

    #[test]
    fn test_four_level_deep_fails() {
        // 5 levels: l0 -> l1 -> l2 -> l3 -> l4
        // depth 0 -> 1 -> 2 -> 3 -> 4 exceeds MAX_INHERITANCE_DEPTH (3)
        let mut skills2: HashMap<String, SkillManifest> = HashMap::new();
        skills2.insert(
            String::from("l0"),
            make_manifest("l0", vec![], Some(vec!["l1"]), None),
        );
        skills2.insert(
            String::from("l1"),
            make_manifest("l1", vec![], Some(vec!["l2"]), None),
        );
        skills2.insert(
            String::from("l2"),
            make_manifest("l2", vec![], Some(vec!["l3"]), None),
        );
        skills2.insert(
            String::from("l3"),
            make_manifest("l3", vec![], Some(vec!["l4"]), None),
        );
        skills2.insert(
            String::from("l4"),
            make_manifest("l4", vec![], None, None),
        );

        let mut visited2 = HashSet::new();
        let err = resolve_inheritance("l0", &skills2, 0, &mut visited2).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("Inheritance depth exceeded"),
            "Expected depth exceeded error, got: {}",
            msg
        );
    }

    #[test]
    fn test_circular_inheritance_detected() {
        let mut skills: HashMap<String, SkillManifest> = HashMap::new();
        skills.insert(
            String::from("a"),
            make_manifest("a", vec![], Some(vec!["b"]), None),
        );
        skills.insert(
            String::from("b"),
            make_manifest("b", vec![], Some(vec!["a"]), None),
        );

        let err = check_circular_inheritance("a", &skills).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("Circular inheritance detected"),
            "Expected circular error, got: {}",
            msg
        );
    }

    #[test]
    fn test_conflicts_with_across_chain() {
        let mut skills: HashMap<String, SkillManifest> = HashMap::new();
        skills.insert(
            String::from("child"),
            make_manifest(
                "child",
                vec![],
                Some(vec!["parent"]),
                Some(vec!["enemy_a"]),
            ),
        );
        skills.insert(
            String::from("parent"),
            make_manifest(
                "parent",
                vec![],
                None,
                Some(vec!["enemy_b"]),
            ),
        );

        let conflicts = resolve_conflicts_with_across_chain("child", &skills).unwrap();
        assert!(conflicts.contains(&"enemy_a".to_string()));
        assert!(conflicts.contains(&"enemy_b".to_string()));
        assert_eq!(conflicts.len(), 2);
    }

    #[test]
    fn test_inspect_shows_combined_capabilities() {
        let mut skills: HashMap<String, SkillManifest> = HashMap::new();
        skills.insert(
            String::from("child"),
            make_manifest(
                "child",
                vec![Capability::HttpGet],
                Some(vec!["parent"]),
                Some(vec!["enemy"]),
            ),
        );
        skills.insert(
            String::from("parent"),
            make_manifest(
                "parent",
                vec![Capability::ReadFile, Capability::WriteFile],
                None,
                Some(vec!["other_enemy"]),
            ),
        );

        let inspected = inspect_resolved_capabilities("child", &skills).unwrap();

        assert_eq!(inspected.name, "child");
        assert_eq!(inspected.own_capabilities, vec![Capability::HttpGet]);
        assert!(inspected.inherited_capabilities.contains(&Capability::ReadFile));
        assert!(inspected.inherited_capabilities.contains(&Capability::WriteFile));
        assert_eq!(inspected.combined_capabilities.len(), 3);
        assert_eq!(inspected.parent_chain, vec!["parent"]);
        assert!(inspected.conflicts_with.contains(&"enemy".to_string()));
        assert!(inspected.conflicts_with.contains(&"other_enemy".to_string()));
    }

    #[test]
    fn test_no_parents_returns_own_capabilities() {
        let mut skills: HashMap<String, SkillManifest> = HashMap::new();
        skills.insert(
            String::from("standalone"),
            make_manifest(
                "standalone",
                vec![Capability::HttpGet, Capability::ReadFile],
                None,
                None,
            ),
        );

        let mut visited = HashSet::new();
        let resolved = resolve_inheritance("standalone", &skills, 0, &mut visited).unwrap();

        assert_eq!(resolved.capabilities.len(), 2);
        assert!(resolved.parents.is_empty());
    }
}
