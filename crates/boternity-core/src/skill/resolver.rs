//! Skill dependency resolution with cycle detection.
//!
//! Uses petgraph to build a directed graph of skill dependencies and produce
//! a topological install order. Detects circular dependencies, version
//! conflicts, and `conflicts_with` declarations.

use std::collections::HashMap;

use anyhow::{bail, Context as _};
use petgraph::algo::toposort;
use petgraph::graph::DiGraph;
use semver::VersionReq;

use boternity_types::skill::SkillManifest;

/// Resolve the dependency graph for a skill, returning an ordered install list
/// with dependencies first.
///
/// Builds a directed acyclic graph from skill dependency declarations and
/// performs topological sort. Returns an error if cycles are detected.
pub fn resolve_dependencies(
    skill_name: &str,
    all_skills: &HashMap<String, SkillManifest>,
) -> anyhow::Result<Vec<String>> {
    let mut graph = DiGraph::<String, ()>::new();
    let mut node_indices = HashMap::new();

    // Collect all skills that need to be in the graph by walking dependencies
    // starting from the target skill.
    let mut to_visit = vec![skill_name.to_string()];
    let mut visited = std::collections::HashSet::new();

    while let Some(current) = to_visit.pop() {
        if !visited.insert(current.clone()) {
            continue;
        }

        let idx = *node_indices
            .entry(current.clone())
            .or_insert_with(|| graph.add_node(current.clone()));

        if let Some(manifest) = all_skills.get(&current) {
            if let Some(ref metadata) = manifest.metadata {
                if let Some(ref deps) = metadata.dependencies {
                    for dep in deps {
                        let dep_idx = *node_indices
                            .entry(dep.clone())
                            .or_insert_with(|| graph.add_node(dep.clone()));
                        // Edge from skill -> dependency (skill depends on dep)
                        graph.add_edge(idx, dep_idx, ());
                        to_visit.push(dep.clone());
                    }
                }
            }
        }
    }

    // Topological sort: errors on cycles.
    match toposort(&graph, None) {
        Ok(sorted) => {
            // toposort returns nodes in topological order (dependents before
            // dependencies). We reverse so dependencies come first.
            let result: Vec<String> = sorted
                .into_iter()
                .rev()
                .map(|idx| graph[idx].clone())
                .collect();
            Ok(result)
        }
        Err(cycle) => {
            let cycle_node = &graph[cycle.node_id()];
            bail!(
                "Circular dependency detected involving skill: {}",
                cycle_node
            );
        }
    }
}

/// Check for version conflicts among shared dependencies in the install list.
///
/// If two skills in the install list both depend on the same skill but with
/// incompatible version ranges, returns an error.
pub fn check_version_conflicts(
    install_list: &[String],
    all_skills: &HashMap<String, SkillManifest>,
) -> anyhow::Result<()> {
    // Collect (dep_name, version_req_string, requesting_skill) for all skills
    let mut dep_versions: HashMap<String, Vec<(String, String)>> = HashMap::new();

    for skill_name in install_list {
        if let Some(manifest) = all_skills.get(skill_name) {
            if let Some(ref metadata) = manifest.metadata {
                if let Some(ref deps) = metadata.dependencies {
                    for dep in deps {
                        // Parse "dep_name@version_req" format, or plain dep name
                        let (dep_name, version_str) = if let Some((name, ver)) = dep.split_once('@')
                        {
                            (name.to_string(), ver.to_string())
                        } else {
                            continue; // No version constraint, skip
                        };

                        dep_versions
                            .entry(dep_name)
                            .or_default()
                            .push((skill_name.clone(), version_str));
                    }
                }
            }
        }
    }

    // Check each shared dependency for incompatible version ranges
    for (dep_name, requesters) in &dep_versions {
        if requesters.len() < 2 {
            continue;
        }

        for i in 0..requesters.len() {
            for j in (i + 1)..requesters.len() {
                let (ref skill_a, ref range_a_str) = requesters[i];
                let (ref skill_b, ref range_b_str) = requesters[j];

                let range_a = VersionReq::parse(range_a_str).with_context(|| {
                    format!(
                        "Invalid version requirement '{}' in skill '{}'",
                        range_a_str, skill_a
                    )
                })?;

                let range_b = VersionReq::parse(range_b_str).with_context(|| {
                    format!(
                        "Invalid version requirement '{}' in skill '{}'",
                        range_b_str, skill_b
                    )
                })?;

                // Check if ranges are compatible by testing representative versions
                if !version_ranges_compatible(&range_a, &range_b) {
                    bail!(
                        "Skill '{}' requires {} {}, but Skill '{}' requires {} {}. Cannot install both.",
                        skill_a, dep_name, range_a, skill_b, dep_name, range_b
                    );
                }
            }
        }
    }

    Ok(())
}

/// Check if two version requirement ranges could be satisfied simultaneously.
///
/// Tests representative versions from common major ranges to see if any version
/// satisfies both requirements.
fn version_ranges_compatible(a: &VersionReq, b: &VersionReq) -> bool {
    use semver::Version;

    // Test representative versions across common ranges
    let test_versions = [
        Version::new(0, 1, 0),
        Version::new(1, 0, 0),
        Version::new(1, 5, 0),
        Version::new(1, 99, 0),
        Version::new(2, 0, 0),
        Version::new(2, 5, 0),
        Version::new(2, 99, 0),
        Version::new(3, 0, 0),
        Version::new(4, 0, 0),
        Version::new(5, 0, 0),
    ];

    test_versions
        .iter()
        .any(|v| a.matches(v) && b.matches(v))
}

/// Check that a skill does not conflict with any currently installed skills.
///
/// Checks bidirectionally: both the new skill's `conflicts_with` list and each
/// installed skill's `conflicts_with` list.
pub fn check_conflicts_with(
    skill_name: &str,
    installed_skills: &[String],
    all_skills: &HashMap<String, SkillManifest>,
) -> anyhow::Result<()> {
    // Check if the new skill declares conflicts with any installed skill
    if let Some(manifest) = all_skills.get(skill_name) {
        if let Some(ref metadata) = manifest.metadata {
            if let Some(ref conflicts) = metadata.conflicts_with {
                for installed in installed_skills {
                    if conflicts.contains(installed) {
                        bail!(
                            "Skill '{}' conflicts with installed skill '{}'",
                            skill_name,
                            installed
                        );
                    }
                }
            }
        }
    }

    // Check reverse: if any installed skill declares conflicts with the new skill
    for installed in installed_skills {
        if let Some(manifest) = all_skills.get(installed) {
            if let Some(ref metadata) = manifest.metadata {
                if let Some(ref conflicts) = metadata.conflicts_with {
                    if conflicts.contains(&skill_name.to_string()) {
                        bail!(
                            "Installed skill '{}' conflicts with skill '{}'",
                            installed,
                            skill_name
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use boternity_types::skill::{SkillManifest, SkillMetadata};

    /// Helper to create a minimal SkillManifest with optional dependencies and conflicts.
    fn make_manifest(
        name: &str,
        deps: Option<Vec<&str>>,
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
                capabilities: None,
                dependencies: deps.map(|d| d.into_iter().map(String::from).collect()),
                conflicts_with: conflicts.map(|c| c.into_iter().map(String::from).collect()),
                trust_tier: None,
                parents: None,
                secrets: None,
                categories: None,
            }),
        }
    }

    /// Helper to create a manifest with versioned dependencies (dep@version format).
    fn make_manifest_with_versioned_deps(
        name: &str,
        deps: Vec<&str>,
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
                capabilities: None,
                dependencies: Some(deps.into_iter().map(String::from).collect()),
                conflicts_with: None,
                trust_tier: None,
                parents: None,
                secrets: None,
                categories: None,
            }),
        }
    }

    #[test]
    fn test_simple_chain_a_b_c() {
        let mut skills = HashMap::new();
        skills.insert("a".into(), make_manifest("a", Some(vec!["b"]), None));
        skills.insert("b".into(), make_manifest("b", Some(vec!["c"]), None));
        skills.insert("c".into(), make_manifest("c", None, None));

        let result = resolve_dependencies("a", &skills).unwrap();
        // c must come before b, b before a
        let pos_a = result.iter().position(|s| s == "a").unwrap();
        let pos_b = result.iter().position(|s| s == "b").unwrap();
        let pos_c = result.iter().position(|s| s == "c").unwrap();
        assert!(pos_c < pos_b, "c must be installed before b");
        assert!(pos_b < pos_a, "b must be installed before a");
    }

    #[test]
    fn test_fan_out_a_depends_on_b_and_c() {
        let mut skills = HashMap::new();
        skills.insert("a".into(), make_manifest("a", Some(vec!["b", "c"]), None));
        skills.insert("b".into(), make_manifest("b", None, None));
        skills.insert("c".into(), make_manifest("c", None, None));

        let result = resolve_dependencies("a", &skills).unwrap();
        let pos_a = result.iter().position(|s| s == "a").unwrap();
        let pos_b = result.iter().position(|s| s == "b").unwrap();
        let pos_c = result.iter().position(|s| s == "c").unwrap();
        assert!(pos_b < pos_a, "b must be installed before a");
        assert!(pos_c < pos_a, "c must be installed before a");
    }

    #[test]
    fn test_circular_dependency_detected() {
        let mut skills = HashMap::new();
        skills.insert("a".into(), make_manifest("a", Some(vec!["b"]), None));
        skills.insert("b".into(), make_manifest("b", Some(vec!["a"]), None));

        let err = resolve_dependencies("a", &skills).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("Circular dependency detected"),
            "Expected circular dependency error, got: {}",
            msg
        );
    }

    #[test]
    fn test_diamond_dependency_resolves() {
        // A -> B, A -> C, B -> D, C -> D (diamond on D)
        let mut skills = HashMap::new();
        skills.insert("a".into(), make_manifest("a", Some(vec!["b", "c"]), None));
        skills.insert("b".into(), make_manifest("b", Some(vec!["d"]), None));
        skills.insert("c".into(), make_manifest("c", Some(vec!["d"]), None));
        skills.insert("d".into(), make_manifest("d", None, None));

        let result = resolve_dependencies("a", &skills).unwrap();
        let pos_a = result.iter().position(|s| s == "a").unwrap();
        let pos_b = result.iter().position(|s| s == "b").unwrap();
        let pos_c = result.iter().position(|s| s == "c").unwrap();
        let pos_d = result.iter().position(|s| s == "d").unwrap();
        assert!(pos_d < pos_b, "d must be installed before b");
        assert!(pos_d < pos_c, "d must be installed before c");
        assert!(pos_b < pos_a, "b must be installed before a");
        assert!(pos_c < pos_a, "c must be installed before a");
    }

    #[test]
    fn test_version_conflict_detected() {
        let mut skills = HashMap::new();
        skills.insert(
            "a".into(),
            make_manifest_with_versioned_deps("a", vec!["shared@^1.0"]),
        );
        skills.insert(
            "b".into(),
            make_manifest_with_versioned_deps("b", vec!["shared@^2.0"]),
        );

        let install_list = vec!["a".into(), "b".into()];
        let err = check_version_conflicts(&install_list, &skills).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("Cannot install both"),
            "Expected version conflict error, got: {}",
            msg
        );
    }

    #[test]
    fn test_compatible_versions_pass() {
        let mut skills = HashMap::new();
        skills.insert(
            "a".into(),
            make_manifest_with_versioned_deps("a", vec!["shared@^1.0"]),
        );
        skills.insert(
            "b".into(),
            make_manifest_with_versioned_deps("b", vec!["shared@>=1.2, <2.0"]),
        );

        let install_list = vec!["a".into(), "b".into()];
        assert!(check_version_conflicts(&install_list, &skills).is_ok());
    }

    #[test]
    fn test_conflicts_with_prevents_installation() {
        let mut skills = HashMap::new();
        skills.insert(
            "new_skill".into(),
            make_manifest("new_skill", None, Some(vec!["existing_skill"])),
        );
        skills.insert(
            "existing_skill".into(),
            make_manifest("existing_skill", None, None),
        );

        let installed = vec!["existing_skill".to_string()];
        let err = check_conflicts_with("new_skill", &installed, &skills).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("conflicts with"),
            "Expected conflict error, got: {}",
            msg
        );
    }

    #[test]
    fn test_reverse_conflicts_with_detected() {
        let mut skills = HashMap::new();
        skills.insert("new_skill".into(), make_manifest("new_skill", None, None));
        skills.insert(
            "existing_skill".into(),
            make_manifest("existing_skill", None, Some(vec!["new_skill"])),
        );

        let installed = vec!["existing_skill".to_string()];
        let err = check_conflicts_with("new_skill", &installed, &skills).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("conflicts with"),
            "Expected reverse conflict error, got: {}",
            msg
        );
    }

    #[test]
    fn test_no_conflicts_passes() {
        let mut skills = HashMap::new();
        skills.insert("a".into(), make_manifest("a", None, None));
        skills.insert("b".into(), make_manifest("b", None, None));

        let installed = vec!["b".to_string()];
        assert!(check_conflicts_with("a", &installed, &skills).is_ok());
    }
}
