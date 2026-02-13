# Phase 6: Skill System + WASM Sandbox - Research

**Researched:** 2026-02-13 (updated with deep dive)
**Domain:** WASM sandboxing, plugin architecture, skill manifest format, OS-level sandboxing, TUI, dependency resolution, registry integration
**Confidence:** HIGH (Wasmtime/WIT), HIGH (Agent Skills spec), HIGH (Registry mechanism), MEDIUM (OS sandbox), HIGH (TUI)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Skill definition & format:**
- Follow skills.sh / Claude skills conventions for format, parameters, and progressive capability discovery
- Two skill types supported: prompt-based (system prompt injection) and tool-based (callable functions with structured I/O)
- Unified format for all skills (local and registry) -- same manifest structure, publish-ready by default
- Global skill library (`~/.boternity/skills/`) with per-bot configuration referencing attached skills and overrides
- Per-bot skill config supports overrides and enable/disable toggles per skill
- Skills declare dependencies on other skills; installing one auto-resolves the dependency tree
- Semantic versioning for skills; bots pin to specific versions
- Tool-based skills return structured JSON; prompt-based skills produce natural language in conversation
- Skills can declare `conflicts_with` to prevent incompatible skills on the same bot

**Permission & trust model:**
- Three trust tiers: local (full trust, shell access), verified registry (relaxed sandbox), untrusted registry (strict WASM sandbox)
- Fine-grained capabilities: specific operations (read-file, write-file, http-get, http-post, exec-command, read-env, etc.)
- Install-time approval: user reviews and approves all required capabilities during installation, no runtime prompts
- Granular revocation: individual capabilities can be revoked, skill degrades gracefully
- Permission violations terminate skill execution immediately (strict enforcement)
- Full audit logging: every skill invocation logged with capabilities used, inputs, outputs, duration
- Users can escalate trust tier (e.g., treat registry skill as local) with clear warnings
- Defense-in-depth: WASM sandbox inside OS-level sandbox (seccomp/seatbelt) -- double barrier
- Configurable resource limits (CPU, memory, I/O) per trust tier, overridable per-skill
- Skills get read-only access to bot memory (no writes)
- Skills can access declared secrets (listed in manifest, approved at install, injected at runtime)
- Enable/disable toggle per-bot -- skills stay installed but can be deactivated

**Registry & discovery UX:**
- Pluggable registry system: skills.sh and ComposioHQ as defaults, users can add custom registry endpoints
- Interactive TUI browser for CLI skill discovery (categories, search, previews)
- Full detail skill preview: name, description, author, version, trust tier, capabilities, dependencies, install count, ratings
- Semver-based update policy: patches auto-update, minor/major require manual approval
- `bnity skill publish` command for publishing local skills with validation, signing, and upload
- Both CLI and web UI for skill management
- Clear source badges showing registry source and trust tier prominently
- Local cache for offline use -- installed skills work offline, registry only needed for discovery/updates
- Dependency conflicts fail with clear explanation -- user resolves manually
- Predefined categories for structured browsing across CLI TUI and web UI

**Skill inheritance & composition:**
- Mixin/composition model -- child composes parent's capabilities additively, no overriding
- Max 3 levels of inheritance depth (skill, parent, grandparent)
- Multiple parent composition -- a skill can mix in capabilities from several parents
- Last-wins ordering for capability conflicts when composing multiple parents
- Child inherits parent's capability permissions -- combined set approved at install
- Automatic skill chaining at runtime -- agent can invoke multiple skills in sequence, output feeding next input
- `bnity skill inspect <name>` shows resolved capability set after all inheritance (also in web UI)
- Circular inheritance detected and prevented at install time with clear cycle error
- Parent update triggers re-validation of dependent children with compatibility warnings
- Explicit `conflicts_with` declarations enforced across inheritance chains

### Claude's Discretion
- Prompt-based skill integration point in system prompt (how/where skills inject into the prompt structure)
- Exact manifest format details (YAML vs TOML, field naming)
- Internal architecture for WASM runtime (Wasmtime configuration specifics)
- OS-level sandbox implementation details (seccomp vs seatbelt per platform)
- Skill chaining orchestration internals
- TUI browser library choice and interaction design

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope.
</user_constraints>

## Summary

This phase introduces a modular skill system for Boternity agents, covering the full lifecycle: defining skills in a standard manifest format (following the agentskills.io specification), executing prompt-based and tool-based skills with appropriate trust levels, sandboxing untrusted registry skills in a Wasmtime WASM runtime with WASI capability-based security, layering OS-level sandboxing on top for defense-in-depth, and providing discovery/installation UX through both CLI TUI and web UI.

The standard approach uses Wasmtime (v40.x) with the WASM Component Model and WIT interface definitions for the tool-based skill sandbox. The agentskills.io specification defines the manifest format (SKILL.md with YAML frontmatter). Ratatui provides the TUI framework for the CLI skill browser. OS-level sandboxing uses platform-specific primitives: macOS Seatbelt (sandbox-exec) and Linux Landlock + seccomp-BPF.

Deep dive research resolved all five open questions from the initial pass: skills.sh is a GitHub-first ecosystem with a web leaderboard API (not a package registry), YAML parsing should use `serde_yaml_ng` (the most practical maintained fork), the `cargo-component` toolchain is well-documented for skill authoring, OS-level sandboxing is practical using the `landlock` crate on Linux and `std::process::Command` wrapping `sandbox-exec` on macOS, and ComposioHQ has 944 skills in a flat directory structure with standard SKILL.md files.

**Primary recommendation:** Use Wasmtime's Component Model with WIT-defined interfaces for sandboxed tool skills, the agentskills.io SKILL.md format as the manifest standard (extending with boternity-specific fields in the `metadata` section), and ratatui for the interactive TUI skill browser. For YAML, use `serde_yaml_ng`. For registry integration, use the GitHub API (clone + scan) for both skills.sh sources and ComposioHQ, with the skills.sh leaderboard for popularity data.

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| wasmtime | 40.x | WASM runtime for sandboxed skill execution | Bytecode Alliance reference runtime; best Rust support; Component Model + WASI P2; fuel/epoch-based execution limits; `ResourceLimiter` for memory control |
| wasmtime-wasi | 40.x | WASI capability-based host functions | Provides WasiCtxBuilder with fine-grained filesystem, network, env, and stdio capability control; capability-based security model maps directly to skill permissions |
| ratatui | 0.30.x | TUI framework for interactive skill browser | De facto Rust TUI library; widgets for tables, lists, search; crossterm backend already in workspace |
| semver | 1.x | Semantic version parsing and comparison | 488M+ downloads; Cargo's own semver library; parsing, comparison, range matching |
| petgraph | 0.7.x | Dependency graph resolution and cycle detection | Standard Rust graph library; `toposort()` for dependency ordering; cycle detection built-in; O(V+E) |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| serde_yaml_ng | 0.10.x | YAML frontmatter parsing for SKILL.md | Parse YAML frontmatter in skill manifests; maintained community fork of dtolnay's serde_yaml with identical API surface |
| toml (already in workspace) | 0.8 | TOML parsing for per-bot skill config | Bot-level skill configuration files (which skills are attached, overrides) |
| reqwest (already in workspace) | 0.12 | HTTP client for registry API calls | Fetching skill metadata and packages from skills.sh and custom registries |
| sha2 (already in workspace) | 0.10 | Integrity verification for downloaded skills | Hash verification of skill packages from registries |
| tempfile (already in workspace) | 3.x | Temporary extraction during skill install | Safe temp directories for extracting skill archives before validation |
| dirs (already in workspace) | 6.x | Platform directory resolution | Resolving `~/.boternity/skills/` path cross-platform |
| landlock | 0.4.x | Linux filesystem sandboxing | Capability-based filesystem restriction for Linux defense-in-depth; official Landlock LSM Rust bindings |
| wit-bindgen | 0.41.x | Guest-side WIT binding generation | Generating Rust bindings for skill authors writing tool-based WASM skills (build-time only) |

### OS-Level Sandboxing (Platform-Specific)

| Component | Platform | Purpose | Notes |
|-----------|----------|---------|-------|
| sandbox-exec + Seatbelt profiles | macOS | OS-level filesystem/network restriction | Apple's sandbox-exec with dynamically generated .sbpl profiles; deprecated but universally used (Chrome, Firefox, Codex all use it); invoked via `std::process::Command` |
| landlock crate (0.4.x) | Linux | Capability-based filesystem restriction | Official Rust bindings; unprivileged; Linux 5.13+; `Ruleset` -> `PathBeneath` rules -> `restrict_self()` |
| libseccomp-rs | Linux | Syscall filtering (network blocking) | Rust interface to libseccomp; BPF filter generation for blocking connect/bind/listen syscalls |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| wasmtime | wasmer | Wasmer is commercial-focused; wasmtime has better Component Model support; wasmtime is the Bytecode Alliance reference implementation |
| wasmtime | extism | Extism wraps wasmtime but adds overhead and opinionated plugin model; direct wasmtime gives us full control over WIT interfaces and WASI capabilities |
| serde_yaml_ng | serde_yml | serde_yml has docs.rs build failures, 0.0.x version numbering, single maintainer; serde_yaml_ng is a more conservative maintained fork with identical API |
| serde_yaml_ng | serde-saphyr | serde-saphyr is newer (v0.0.10), pure Rust, faster -- but deserialization-focused initially, less ecosystem adoption; good future option |
| serde_yaml_ng | serde_yaml2 (yaml-rust2 based) | serde_yaml2 is pure Rust via yaml-rust2, but only v0.1.3 with 4 releases total; less mature |
| petgraph | custom DAG | Dependency resolution has subtle cycle detection and ordering requirements; petgraph is battle-tested |
| ratatui | cursive | Ratatui is more widely adopted, better documented, and already aligned with crossterm (in workspace) |

**Installation (new dependencies only):**
```bash
cargo add wasmtime@40 --features component-model,async,cranelift
cargo add wasmtime-wasi@40
cargo add serde_yaml_ng@0.10
cargo add semver@1
cargo add petgraph@0.7
cargo add ratatui@0.30 --features crossterm
cargo add landlock@0.4  # Linux only, behind cfg(target_os = "linux")
```

Note: `wit-bindgen` is a build-time tool for skill authors, not a runtime dependency of the host.

## Architecture Patterns

### Recommended Project Structure

```
crates/boternity-types/src/
  skill.rs              # Skill domain types: SkillManifest, SkillType, TrustTier, Capability,
                        #   SkillVersion, SkillDependency, SkillConflict, PermissionGrant

crates/boternity-core/src/
  skill/
    mod.rs              # Module re-exports
    registry.rs         # SkillRegistry trait (discover, fetch, publish)
    resolver.rs         # Dependency resolver (petgraph-based DAG, topological sort, cycle detection)
    executor.rs         # SkillExecutor trait (execute prompt-based or tool-based skills)
    permission.rs       # PermissionChecker, CapabilityEnforcer, AuditLogger
    manifest.rs         # Manifest parsing and validation (SKILL.md frontmatter + body)
    inheritance.rs      # Mixin composition resolver (max 3 levels, multi-parent, conflicts_with)
    prompt_injector.rs  # Integrates prompt-based skills into system prompt

crates/boternity-infra/src/
  skill/
    mod.rs              # Module re-exports
    wasm_runtime.rs     # Wasmtime Engine, Component, Linker, Store management
    wasm_executor.rs    # WasmSkillExecutor: loads .wasm component, calls exports, enforces limits
    local_executor.rs   # LocalSkillExecutor: runs local skills with process spawning
    sandbox.rs          # OS-level sandbox orchestration (dispatch to platform-specific impl)
    sandbox_macos.rs    # Seatbelt profile generation and sandbox-exec invocation
    sandbox_linux.rs    # Landlock + seccomp-BPF setup
    registry_client.rs  # HTTP client for GitHub-based skill repositories
    skill_store.rs      # Filesystem-based skill storage (~/.boternity/skills/)
    audit.rs            # SQLite-backed audit log for skill invocations

crates/boternity-api/src/
  cli/
    skill.rs            # bnity skill {create,install,remove,list,inspect,publish,update} commands
    skill_browser.rs    # Interactive TUI browser (ratatui-based)
  http/
    skill.rs            # REST API handlers for web UI skill management

wit/
  boternity-skill.wit   # WIT interface definition for tool-based sandboxed skills

templates/
  skill-template/       # Template for `bnity skill create --type tool`
    wit/world.wit
    src/lib.rs
    Cargo.toml
```

### Pattern 1: WIT-Defined Skill Interface

**What:** Define the contract between host (boternity) and guest (skill WASM component) using WIT.
**When to use:** All tool-based skills that run as WASM components.

```wit
// wit/boternity-skill.wit
package boternity:skill;

interface host {
    // Capabilities the host provides to the skill
    record skill-context {
        bot-name: string,
        bot-slug: string,
        skill-name: string,
        invocation-id: string,
    }

    // Read-only bot memory access
    recall-memory: func(query: string, limit: u32) -> list<string>;

    // HTTP access (gated by capability grant)
    http-get: func(url: string) -> result<string, string>;
    http-post: func(url: string, body: string) -> result<string, string>;

    // File access (gated by capability grant)
    read-file: func(path: string) -> result<string, string>;
    write-file: func(path: string, content: string) -> result<_, string>;

    // Secret access (only declared + approved secrets)
    get-secret: func(name: string) -> result<string, string>;
}

world skill-plugin {
    import host;

    // Every skill must export these
    export get-name: func() -> string;
    export get-description: func() -> string;

    // Tool-based skill execution
    export execute: func(input: string) -> result<string, string>;
}
```

### Pattern 2: Wasmtime Component Instantiation with Resource Limits

**What:** Load and execute WASM skill components with strict resource limits.
**When to use:** Every sandboxed skill execution.

```rust
// Source: Wasmtime official docs + API reference
use wasmtime::component::{Component, Linker, bindgen};
use wasmtime::{Config, Engine, Store, ResourceLimiter};
use wasmtime_wasi::{WasiCtxBuilder, WasiCtx, WasiView, ResourceTable};

// Generate Rust bindings from WIT
bindgen!({
    world: "skill-plugin",
    path: "wit/boternity-skill.wit",
    async: true,
});

struct SkillState {
    ctx: WasiCtx,
    table: ResourceTable,
    // Per-skill capability grants
    capabilities: HashSet<Capability>,
    // Audit context
    invocation_id: Uuid,
}

// Required for wasmtime-wasi integration
impl WasiView for SkillState {
    fn table(&mut self) -> &mut ResourceTable { &mut self.table }
    fn ctx(&mut self) -> &mut WasiCtx { &mut self.ctx }
}

impl ResourceLimiter for SkillState {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> anyhow::Result<bool> {
        // Enforce per-skill memory limit (e.g., 64MB for untrusted)
        let limit = 64 * 1024 * 1024;
        Ok(desired <= limit)
    }

    fn table_growing(
        &mut self,
        current: u64,
        desired: u64,
        maximum: Option<u64>,
    ) -> anyhow::Result<bool> {
        Ok(desired <= 1000)
    }
}

async fn execute_wasm_skill(
    engine: &Engine,
    component_bytes: &[u8],
    input: &str,
    capabilities: HashSet<Capability>,
) -> anyhow::Result<String> {
    let component = Component::from_binary(engine, component_bytes)?;
    let mut linker = Linker::<SkillState>::new(engine);

    // Add WASI P2 host functions
    wasmtime_wasi::add_to_linker_async(&mut linker)?;

    // Add custom host imports (boternity:skill/host)
    // Each host function checks capabilities before executing
    SkillPlugin::add_to_linker(&mut linker, |state| state)?;

    // Build WASI with restricted capabilities
    let wasi = WasiCtxBuilder::new()
        // No filesystem access by default
        // No network by default
        // No env vars
        // Capture stdout/stderr
        .build();

    let mut store = Store::new(engine, SkillState {
        ctx: wasi,
        table: ResourceTable::new(),
        capabilities,
        invocation_id: Uuid::now_v7(),
    });

    // Enable fuel for CPU limiting
    store.set_fuel(1_000_000)?; // 1M fuel units

    // Apply memory limits
    store.limiter(|state| state);

    let (plugin, _instance) = SkillPlugin::instantiate_async(
        &mut store, &component, &linker
    ).await?;
    let result = plugin.call_execute(&mut store, input).await?;
    result.map_err(|e| anyhow::anyhow!("Skill error: {e}"))
}
```

### Pattern 3: SKILL.md Manifest Parsing

**What:** Parse the agentskills.io-compatible SKILL.md with boternity extensions.
**When to use:** Loading any skill from disk or registry.

```rust
use serde::{Deserialize, Serialize};

/// Parsed SKILL.md frontmatter (agentskills.io compatible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub compatibility: Option<String>,
    #[serde(default)]
    pub metadata: Option<SkillMetadata>,
    #[serde(default, rename = "allowed-tools")]
    pub allowed_tools: Option<String>,
}

/// Extended metadata for boternity skills
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub author: Option<String>,
    pub version: Option<String>,
    // Boternity-specific extensions (in metadata to stay agentskills.io compatible)
    #[serde(default, rename = "skill-type")]
    pub skill_type: Option<SkillType>, // "prompt" or "tool"
    #[serde(default)]
    pub capabilities: Option<Vec<String>>,
    #[serde(default)]
    pub dependencies: Option<Vec<String>>,
    #[serde(default, rename = "conflicts-with")]
    pub conflicts_with: Option<Vec<String>>,
    #[serde(default, rename = "trust-tier")]
    pub trust_tier: Option<TrustTier>,
    #[serde(default)]
    pub parents: Option<Vec<String>>,
    #[serde(default)]
    pub secrets: Option<Vec<String>>,
}

/// Parse SKILL.md: extract YAML frontmatter + markdown body
fn parse_skill_md(content: &str) -> anyhow::Result<(SkillManifest, String)> {
    let content = content.trim();
    if !content.starts_with("---") {
        anyhow::bail!("SKILL.md must start with YAML frontmatter (---)");
    }
    let end = content[3..]
        .find("---")
        .ok_or_else(|| anyhow::anyhow!("Missing closing --- for frontmatter"))?;
    let yaml = &content[3..end + 3];
    let body = content[end + 6..].trim().to_string();
    let manifest: SkillManifest = serde_yaml_ng::from_str(yaml)?;
    Ok((manifest, body))
}
```

### Pattern 4: Prompt-Based Skill Integration

**What:** Inject prompt-based skills into the system prompt using XML tags.
**When to use:** When a bot has prompt-based skills attached.

The agentskills.io integration guide recommends XML-tagged `<available_skills>` for metadata injection:

```rust
/// Generate the <available_skills> metadata block for the system prompt.
/// This is Level 1 (progressive disclosure): only name + description + path.
/// ~50-100 tokens per skill.
fn generate_skill_metadata_xml(
    skills: &[(SkillManifest, PathBuf)],
) -> String {
    let entries: Vec<String> = skills
        .iter()
        .map(|(manifest, path)| {
            format!(
                "  <skill>\n    <name>{}</name>\n    <description>{}</description>\n    <location>{}</location>\n  </skill>",
                manifest.name,
                manifest.description,
                path.display()
            )
        })
        .collect();

    format!("<available_skills>\n{}\n</available_skills>", entries.join("\n"))
}

/// Inject full skill instructions (Level 2) for activated skills only.
/// Insert between </identity> and <user_context> in the system prompt.
fn inject_active_skill_prompts(
    base_prompt: &str,
    active_skills: &[(SkillManifest, String)], // (manifest, body)
) -> String {
    if active_skills.is_empty() {
        return base_prompt.to_string();
    }

    let skills_section: String = active_skills
        .iter()
        .map(|(manifest, body)| {
            format!(
                "<skill name=\"{}\">\n{}\n</skill>",
                manifest.name,
                body.trim()
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let insertion = format!("\n\n<skills>\n{}\n</skills>", skills_section);

    if let Some(pos) = base_prompt.find("</identity>") {
        let insert_at = pos + "</identity>".len();
        format!(
            "{}{}{}",
            &base_prompt[..insert_at],
            insertion,
            &base_prompt[insert_at..]
        )
    } else {
        format!("{}\n\n{}", insertion, base_prompt)
    }
}
```

### Pattern 5: Dependency Resolution with Cycle Detection

**What:** Resolve skill dependencies using a DAG with topological sort.
**When to use:** During skill installation to determine install order and detect cycles.

```rust
use petgraph::graph::DiGraph;
use petgraph::algo::toposort;
use petgraph::graph::NodeIndex;
use std::collections::HashMap;

fn resolve_dependencies(
    skill_name: &str,
    all_skills: &HashMap<String, SkillManifest>,
) -> anyhow::Result<Vec<String>> {
    let mut graph = DiGraph::<String, ()>::new();
    let mut indices: HashMap<String, NodeIndex> = HashMap::new();

    // Build graph recursively
    fn add_skill(
        name: &str,
        all: &HashMap<String, SkillManifest>,
        graph: &mut DiGraph<String, ()>,
        indices: &mut HashMap<String, NodeIndex>,
    ) -> anyhow::Result<()> {
        if indices.contains_key(name) {
            return Ok(());
        }
        let idx = graph.add_node(name.to_string());
        indices.insert(name.to_string(), idx);

        if let Some(manifest) = all.get(name) {
            if let Some(ref deps) = manifest.metadata.as_ref().and_then(|m| m.dependencies.as_ref()) {
                for dep in deps {
                    add_skill(dep, all, graph, indices)?;
                    let dep_idx = indices[dep];
                    graph.add_edge(idx, dep_idx, ());
                }
            }
        }
        Ok(())
    }

    add_skill(skill_name, all_skills, &mut graph, &mut indices)?;

    // Topological sort (errors on cycles)
    let sorted = toposort(&graph, None)
        .map_err(|cycle| {
            let node = &graph[cycle.node_id()];
            anyhow::anyhow!("Circular dependency detected involving skill: {node}")
        })?;

    // Reverse: dependencies first, then dependents
    Ok(sorted.into_iter().rev().map(|idx| graph[idx].clone()).collect())
}
```

### Pattern 6: OS-Level Sandbox -- Subprocess Model

**What:** Platform-specific sandbox wrapper for defense-in-depth using subprocess isolation.
**When to use:** Wrapping WASM execution in an additional OS-level sandbox.

```rust
use std::process::Command;

/// macOS: Generate Seatbelt profile and run WASM executor in sandboxed subprocess.
/// Source: Codex/Chrome/Firefox pattern (pierce.dev deep dive)
#[cfg(target_os = "macos")]
fn run_sandboxed_wasm(
    wasm_path: &Path,
    input: &str,
    sandbox_config: &SandboxConfig,
) -> anyhow::Result<String> {
    // Generate .sbpl profile dynamically
    let profile = generate_seatbelt_profile(sandbox_config);

    // Run the WASM executor binary as a sandboxed subprocess
    let output = Command::new("/usr/bin/sandbox-exec")
        .arg("-p")
        .arg(&profile)
        .arg("boternity-wasm-executor") // Separate binary that loads + runs WASM
        .arg("--wasm")
        .arg(wasm_path)
        .arg("--input")
        .arg(input)
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?)
    } else {
        Err(anyhow::anyhow!(
            "Sandboxed WASM execution failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn generate_seatbelt_profile(config: &SandboxConfig) -> String {
    let mut profile = String::from("(version 1)\n(deny default)\n");
    profile.push_str("(allow process-exec)\n");
    profile.push_str("(allow file-read*)\n"); // read-only by default

    // Allow writes only to specific paths
    for path in &config.writable_paths {
        profile.push_str(&format!(
            "(allow file-write* (subpath \"{}\"))\n",
            path.display()
        ));
    }

    // Network: deny by default unless capability granted
    if config.allow_network {
        profile.push_str("(allow network*)\n");
    }

    profile
}

/// Linux: Apply Landlock filesystem restrictions to subprocess.
/// Source: landlock crate official docs (https://landlock.io/rust-landlock/landlock/)
#[cfg(target_os = "linux")]
fn apply_landlock_sandbox(config: &SandboxConfig) -> anyhow::Result<()> {
    use landlock::{
        Access, AccessFs, PathBeneath, PathFd, Ruleset, RulesetAttr,
        RulesetCreatedAttr, ABI,
    };

    let abi = ABI::V3; // Linux 5.19+

    let mut ruleset = Ruleset::default()
        .handle_access(AccessFs::from_all(abi))?
        .create()?;

    // Allow read-only access to common paths
    let read_only = AccessFs::ReadFile | AccessFs::ReadDir | AccessFs::Execute;
    for path in &config.readable_paths {
        ruleset.add_rule(PathBeneath::new(
            PathFd::new(path)?,
            read_only,
        ))?;
    }

    // Allow read-write to specific paths only
    let read_write = read_only | AccessFs::WriteFile | AccessFs::MakeDir;
    for path in &config.writable_paths {
        ruleset.add_rule(PathBeneath::new(
            PathFd::new(path)?,
            read_write,
        ))?;
    }

    // Apply sandbox -- restricts the CURRENT process
    let status = ruleset.restrict_self()?;
    // Check: status.ruleset == RulesetStatus::FullyEnforced
    Ok(())
}
```

### Anti-Patterns to Avoid

- **Loading full skill content at startup:** Skills use progressive disclosure -- only name + description loaded initially (~50-100 tokens per skill). Full body loaded on activation. Do NOT load all skill bodies into memory.
- **Dynamic trait objects for skill types:** Use enums (`SkillType::Prompt` / `SkillType::Tool`) instead of trait objects. There are exactly two skill types, not an open set.
- **Sharing Wasmtime Engine across trust tiers:** Use separate `Engine` configurations for verified vs untrusted tiers. Untrusted needs stricter fuel limits, memory caps, and disabled features.
- **Mutable WASM state between invocations:** Create fresh `Store` per invocation. WASM skills should be stateless between calls. State leaks between invocations are a security risk.
- **Running OS sandbox on the host process directly:** The OS sandbox (seccomp/seatbelt) should wrap a SEPARATE subprocess, not the main boternity process. On Linux, Landlock's `restrict_self()` restricts the calling process permanently -- use it in the child process only.
- **Building a custom registry protocol:** Skills.sh is GitHub-first. Don't design a custom REST API for skill publishing. Use GitHub as the backing store and the GitHub API for discovery.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| WASM runtime | Custom WASM interpreter | Wasmtime 40.x | Formally verified, AOT-compiled, standards-compliant, Bytecode Alliance-backed |
| WASM capability isolation | Custom permission checks at syscall level | WasiCtxBuilder capability model | WASI's capability-based security is exactly this -- preopened_dir, allow_tcp, socket_addr_check |
| Semver parsing/comparison | Regex-based version parsing | `semver` crate | Edge cases in pre-release ordering, build metadata, range matching are enormous |
| Dependency graph resolution | Custom recursive resolution | `petgraph` with `toposort()` | Cycle detection, topological ordering, edge cases in diamond dependencies |
| YAML frontmatter parsing | Custom string splitting | `serde_yaml_ng` with frontmatter extraction | YAML has many edge cases (multiline strings, anchors, type coercion) |
| TUI widgets | Custom terminal rendering | `ratatui` widgets (Table, List, Paragraph, Tabs) | Table rendering, scrolling, input handling, responsive layout are all solved |
| Execution time limits | Custom timer threads | Wasmtime fuel consumption + epoch interruption | Fuel is deterministic per-instruction; epoch interruption is lightweight and cannot be bypassed by malicious WASM |
| Memory limits | Custom memory tracking | Wasmtime `ResourceLimiter` trait | Intercepts all memory growth requests; works at the linear memory level |
| SKILL.md format | Custom skill manifest format | agentskills.io specification | Open standard; compatible with Claude Code, Codex, Cursor, Gemini CLI |
| Linux filesystem sandbox | Custom syscall interception | `landlock` crate (0.4.x) | Official Landlock LSM Rust bindings; unprivileged; best-effort mode handles kernel version differences |
| Skill registry protocol | Custom REST API | GitHub API + git clone | Skills.sh ecosystem is GitHub-native; `owner/repo` format is the standard addressing scheme |

**Key insight:** The WASM Component Model + WASI provides a complete capability-based security model out of the box. The host defines exactly which capabilities each guest receives through WasiCtxBuilder and custom host imports. Do not attempt to build a capability system on top of raw WASM -- use WASI's built-in model.

## Common Pitfalls

### Pitfall 1: Mixing WASI Preview 1 and Preview 2 APIs
**What goes wrong:** Wasmtime supports both P1 (POSIX-ish file descriptors) and P2 (Component Model interfaces). Using P1 APIs with Component Model components causes linker failures.
**Why it happens:** Older tutorials and examples use P1. The `wasmtime_wasi::p1` module is for legacy core modules, not components.
**How to avoid:** Always use `wasmtime_wasi` top-level APIs (which are P2) for Component Model components. Use `WasiCtxBuilder::new().build()` (not `.build_p1()`). Link with `wasmtime_wasi::add_to_linker_async()`.
**Warning signs:** Linker errors about missing imports like `wasi_snapshot_preview1`.

### Pitfall 2: Forgetting Fuel Configuration
**What goes wrong:** A malicious or buggy WASM skill enters an infinite loop, consuming host CPU indefinitely.
**Why it happens:** Fuel consumption is disabled by default in Wasmtime. Must be explicitly enabled in `Config` AND set on each `Store`.
**How to avoid:** Enable with `Config::new().consume_fuel(true)`. Set per-store with `store.set_fuel(limit)`. Use different limits per trust tier.
**Warning signs:** Skills that hang without timeout.

### Pitfall 3: OS Sandbox Restricting the Host Process
**What goes wrong:** Applying seccomp or Landlock to the main boternity process blocks legitimate host operations (DB writes, LLM API calls, etc.).
**Why it happens:** OS-level sandboxing affects the entire process, not just WASM execution within it. Landlock's `restrict_self()` permanently restricts the calling process.
**How to avoid:** Use a subprocess model: parent spawns a child process, child applies OS sandbox to itself, then child executes WASM. Parent stays unrestricted. Communication via stdin/stdout JSON or Unix domain socket.
**Warning signs:** Host process cannot write to SQLite, cannot make HTTP requests after applying sandbox.

### Pitfall 4: Capability Check Bypass Through Host Imports
**What goes wrong:** WASM guest calls a host-provided import (e.g., `http-get`) that the skill doesn't have permission for, and the host executes it.
**Why it happens:** WIT host imports are linked at instantiation time. If the host function doesn't check capabilities, any guest can call it.
**How to avoid:** Every host import function MUST check the skill's capability grants before executing. Use a `CapabilityEnforcer` that wraps all host functions and validates permissions before execution.
**Warning signs:** Skills accessing resources they shouldn't have.

### Pitfall 5: Progressive Disclosure Token Bloat
**What goes wrong:** Loading all skill instructions into the system prompt at once consumes the entire context window.
**Why it happens:** A bot with 10+ skills, each with ~5000 tokens of instructions, adds 50K+ tokens to every prompt.
**How to avoid:** Only inject active/relevant skills. Use the progressive disclosure architecture: Level 1 (metadata ~50-100 tokens per skill in `<available_skills>` XML), Level 2 (full instructions only for activated skills in `<skills>` block), Level 3 (reference files only on demand via filesystem read).
**Warning signs:** Token budget exhausted before user message is processed.

### Pitfall 6: YAML Frontmatter Parsing Edge Cases
**What goes wrong:** YAML type coercion silently converts values. `version: 1.0` becomes float `1.0`, not string `"1.0"`. `name: true` becomes boolean.
**Why it happens:** YAML's implicit typing is aggressive.
**How to avoid:** Always quote version strings in SKILL.md (`version: "1.0"`). Validate manifest fields with explicit type checks after parsing. Use `#[serde(deserialize_with = ...)]` for fields that must be strings. Document this requirement in skill creation templates.
**Warning signs:** Version comparisons failing because `1.0` parsed as float.

### Pitfall 7: Diamond Dependency Conflicts
**What goes wrong:** Skill A depends on Skill C v1.x, Skill B depends on Skill C v2.x. Installing both A and B creates an unresolvable conflict.
**Why it happens:** Skills pin to semver ranges, and major version bumps are incompatible.
**How to avoid:** Detect conflicts during dependency resolution BEFORE installation. Show clear error: "Skill A requires skill-c ^1.0, but Skill B requires skill-c ^2.0. Cannot install both." Let user resolve manually (per locked decision).
**Warning signs:** Silent version override causing one skill to break.

### Pitfall 8: Wasmtime Component vs Core Module Confusion
**What goes wrong:** Trying to instantiate a core WASM module (`.wasm` compiled with wasm32-wasip1) as a Component, or vice versa.
**Why it happens:** Core modules and Components have different binary formats. The Component Model wraps core modules.
**How to avoid:** Skills must be compiled as Components (using `cargo component build` or `wasm-tools component new`). Validate the binary header on load -- check for the Component Model magic bytes. Components use the Component Model binary format, not the core WASM format.
**Warning signs:** "not a component" errors during instantiation.

### Pitfall 9: cargo-component vs Plain Cargo Target Confusion
**What goes wrong:** Skill author uses `cargo build --target wasm32-wasip2` instead of `cargo component build`, or vice versa, and gets unexpected results.
**Why it happens:** As of Rust 1.82, `wasm32-wasip2` is an upstream target that produces components via plain cargo. But `cargo component` is still needed if you use non-WASI WIT interfaces (which boternity skills do -- they import `boternity:skill/host`).
**How to avoid:** Boternity tool-based skills MUST use `cargo component build` because they define custom WIT imports beyond WASI. Document this clearly. The `bnity skill create --type tool` template should set up the project for cargo-component.
**Warning signs:** Missing custom imports, "import not found" errors for boternity:skill/host.

## Deep Dive Findings

### 1. skills.sh Registry Integration (RESOLVED)

**Confidence: HIGH**

skills.sh is NOT a traditional package registry with a REST API. It is a **GitHub-first ecosystem** with a web leaderboard/directory.

**How the ecosystem actually works:**

1. **Skills live in GitHub repos.** The canonical source format is `owner/repo` (e.g., `vercel-labs/agent-skills`, `anthropics/skills`, `ComposioHQ/awesome-claude-skills`). Each repo contains one or more skill directories with SKILL.md files.

2. **The Vercel Skills CLI (`npx skills`)** is the package manager. It implements a three-stage source resolution:
   - **Stage 1: Parse source** -- `parseSource()` accepts `owner/repo`, full GitHub URLs, GitLab URLs, local paths, and direct HTTP URLs.
   - **Stage 2: Discover skills** -- `discoverSkills()` scans the resolved source using a three-tier search: (a) check exact path for SKILL.md, (b) scan priority directories (`skills/`, `skills/.curated/`, `.agents/skills/`, `.claude/skills/`), (c) fallback recursive scan for all SKILL.md files.
   - **Stage 3: Install** -- Creates canonical copy in `.agents/skills/skill-name/`, then symlinks or copies to each agent's skill directory.

3. **skills.sh web directory** is a Vercel-hosted leaderboard that tracks install counts. It has:
   - Sorting: "All Time", "Trending" (24h), "Hot"
   - 200+ skills listed with source repo, skill ID, name, and install count
   - Search interface
   - Top skills: "find-skills" (212K installs), "vercel-react-best-practices" (129.5K)

4. **Update tracking** uses a lock file at `~/.agents/.skill-lock.json` with GitHub tree hashes. The `check` command queries `add-skill.vercel.sh/check-updates` with skill hashes to detect changes.

5. **skills.sh internal API endpoints** (discovered from CLI source):
   - `skills.sh/api/skills` -- Full skill registry (used by `npx skills find` with no query)
   - `skills.sh/api/skills/search` -- POST endpoint for filtered search

**Practical approach for Boternity:**

```
Registry client implementation:

1. GitHub-based installation (primary):
   - Accept `owner/repo` format (e.g., `bnity skill install anthropics/skills --skill pdf`)
   - Use GitHub API to list contents, fetch SKILL.md files
   - Clone repo to temp dir, scan for SKILL.md, present choices
   - Track installed source in .boternity-meta.toml

2. skills.sh discovery (supplementary):
   - Query skills.sh/api/skills for browsable catalog
   - Query skills.sh/api/skills/search for keyword search
   - Each entry includes source repo URL -- resolve to GitHub for installation
   - Use for TUI browser popularity sorting and discovery

3. Well-known endpoint support (extensible):
   - Support /.well-known/skills.json on custom registry domains
   - Future-proofing for enterprise registries

4. Local path support:
   - Accept `./path/to/skill` for local development
   - Scan for SKILL.md in standard locations
```

**Key repos that serve as default registries:**

| Repository | Content | Skill Count | Notes |
|------------|---------|-------------|-------|
| `anthropics/skills` | Official Anthropic skills (pdf, docx, xlsx, pptx) | ~20 | 69.4K stars; some source-available (not open source) |
| `vercel-labs/agent-skills` | Vercel's official collection | ~30 | Frontend, design, testing skills |
| `ComposioHQ/awesome-claude-skills` | Community curated mega-list | ~944 | Flat directory structure, 5 categories |

### 2. YAML Manifest Parsing (RESOLVED)

**Confidence: HIGH**

**Decision: Use `serde_yaml_ng`.**

The YAML parsing ecosystem in Rust has fragmented after dtolnay archived `serde-yaml`. Here is the complete landscape:

| Crate | Version | Status | Backing Parser | Notes |
|-------|---------|--------|----------------|-------|
| `serde_yaml` (dtolnay) | 0.9.x | **Archived/Deprecated** | unsafe-libyaml (C-to-Rust auto-translate) | No longer maintained; do not use for new projects |
| `serde_yaml_ng` | 0.10.x | **Actively maintained** | unsafe-libyaml (same as original) | Most conservative fork; identical API to serde_yaml; drop-in replacement; published on crates.io |
| `serde_yml` | 0.0.12 | Maintained but unstable | yaml-rust2 (pure Rust) | docs.rs build failures; 0.0.x version numbering signals instability; single maintainer |
| `serde_yaml2` | 0.1.3 | Early stage | yaml-rust2 (pure Rust) | Only 4 releases; minimal adoption |
| `serde-saphyr` | 0.0.10 | Active development | saphyr-parser (pure Rust) | Fast, no intermediate tree; configurable budgets for DoS protection; newer |
| `serde_yaml_bw` | ? | Fork | unsafe-libyaml | Less known |

**Why `serde_yaml_ng`:**
1. It is a drop-in replacement for `serde_yaml` -- same API, same types, same behavior.
2. It has the most ecosystem adoption of the maintained forks.
3. Since agentskills.io SKILL.md uses simple YAML frontmatter (flat key-value, no anchors or advanced features), we need reliability over cutting-edge features.
4. The `serde_yaml` -> `serde_yaml_ng` migration is a search-and-replace in imports.

**Important: YAML is mandatory, not optional.** The agentskills.io specification requires YAML frontmatter. We cannot use TOML for the SKILL.md format because that would break compatibility with the 1000+ skills already published in the ecosystem. However, per-bot configuration files (skills.toml) should remain TOML since that's an internal format.

**Updated Standard Stack entry:**
```bash
cargo add serde_yaml_ng@0.10
```

**Frontmatter parsing is simple enough to hand-roll the extraction, then delegate YAML parsing to serde_yaml_ng:**

```rust
fn extract_frontmatter(content: &str) -> anyhow::Result<(&str, &str)> {
    let content = content.trim();
    let Some(rest) = content.strip_prefix("---") else {
        anyhow::bail!("SKILL.md must start with YAML frontmatter (---)");
    };
    // Find the closing ---
    let end = rest.find("\n---")
        .ok_or_else(|| anyhow::anyhow!("Missing closing --- for frontmatter"))?;
    let yaml = &rest[..end];
    let body = &rest[end + 4..]; // skip \n---
    Ok((yaml.trim(), body.trim()))
}
```

### 3. WASM Skill Build Toolchain (RESOLVED)

**Confidence: HIGH**

**The complete workflow for building a tool-based WASM skill:**

**Step 1: Install toolchain**
```bash
# Install cargo-component (the Bytecode Alliance tool)
cargo install cargo-component

# Ensure Rust has the WASM target
rustup target add wasm32-wasip1
```

**Step 2: Create skill project (what `bnity skill create --type tool` should scaffold)**
```bash
cargo component new --lib my-skill
```

This generates:
```
my-skill/
  Cargo.toml
  src/lib.rs
  wit/world.wit
  .vscode/settings.json  # rust-analyzer config
```

**Step 3: Define the WIT interface**

The skill author replaces `wit/world.wit` with the boternity skill interface. Boternity provides this via the template:

```wit
// wit/world.wit -- provided by boternity template
package boternity:skill;

interface host {
    recall-memory: func(query: string, limit: u32) -> list<string>;
    http-get: func(url: string) -> result<string, string>;
    http-post: func(url: string, body: string) -> result<string, string>;
    read-file: func(path: string) -> result<string, string>;
    write-file: func(path: string, content: string) -> result<_, string>;
    get-secret: func(name: string) -> result<string, string>;
}

world skill-plugin {
    import host;
    export get-name: func() -> string;
    export get-description: func() -> string;
    export execute: func(input: string) -> result<string, string>;
}
```

**Step 4: Implement the skill**

```rust
// src/lib.rs -- the skill author writes this
#[allow(warnings)]
mod bindings;

use bindings::Guest;
use bindings::boternity::skill::host;

struct MySkill;

impl Guest for MySkill {
    fn get_name() -> String {
        "my-skill".to_string()
    }

    fn get_description() -> String {
        "Does something useful".to_string()
    }

    fn execute(input: String) -> Result<String, String> {
        // Use host capabilities
        let data = host::http_get("https://api.example.com/data")
            .map_err(|e| format!("HTTP error: {e}"))?;

        Ok(format!("Processed: {data}"))
    }
}

bindings::export!(MySkill with_types_in bindings);
```

**Step 5: Configure Cargo.toml**

```toml
[package]
name = "my-skill"
version = "0.1.0"
edition = "2024"

[package.metadata.component]
package = "boternity:skill"

[dependencies]
wit-bindgen = "0.41"
```

**Step 6: Build**
```bash
cargo component build --release
# Output: target/wasm32-wasip1/release/my_skill.wasm
```

**Why cargo-component, not plain cargo:**
- Boternity skills define custom WIT imports (`boternity:skill/host`) beyond standard WASI.
- Plain `cargo build --target wasm32-wasip2` only works with WASI-standard interfaces.
- `cargo component` handles WIT binding generation, component wrapping, and adapter injection.

**Non-Rust skill authors:**

| Language | Tool | Command | Notes |
|----------|------|---------|-------|
| JavaScript | ComponentizeJS | `componentize-js my-skill.js --wit ./wit --world-name skill-plugin -o my-skill.wasm` | Embeds SpiderMonkey runtime; larger binary (~4MB+) |
| Python | componentize-py | `componentize-py -d wit -w skill-plugin app -o my-skill.wasm` | Embeds CPython runtime; supports native extensions via shared-everything linking |
| C/C++ | wasi-sdk + wasm-tools | `clang ... -o module.wasm && wasm-tools component new module.wasm -o skill.wasm` | Two-step: compile to core module, then wrap as component |
| Go | wit-bindgen-go (experimental) | Build + adapt | Less mature; experimental status |

**What Boternity should provide:**
1. `bnity skill create --type tool [name]` -- scaffolds complete project from template
2. `bnity skill create --type prompt [name]` -- scaffolds SKILL.md + directories
3. `bnity skill build` -- wraps `cargo component build --release` with correct paths
4. `bnity skill validate` -- validates SKILL.md frontmatter + checks WASM binary format
5. Template repo with WIT files, Cargo.toml, example lib.rs, and README

### 4. OS-Level Sandbox Practical Implementation (RESOLVED)

**Confidence: MEDIUM (macOS), HIGH (Linux)**

**Architecture decision: Subprocess model.**

The OS-level sandbox runs in a separate subprocess. The main boternity process spawns a child that:
1. Applies OS-level restrictions to itself
2. Loads and executes the WASM component
3. Returns results via stdout JSON

This avoids the critical pitfall of restricting the host process.

**macOS: Seatbelt via sandbox-exec**

macOS uses the Seatbelt framework through `/usr/bin/sandbox-exec`. Despite being marked "deprecated" in man pages, it remains the standard approach used by Chrome, Firefox, Codex, and many macOS system services.

```
# Minimal Seatbelt profile for WASM skill execution
(version 1)
(deny default)

# Allow basic process operations
(allow process-exec)
(allow process-fork)
(allow sysctl-read)
(allow mach-lookup)

# Allow reading system libraries (needed for dynamic linking)
(allow file-read* (subpath "/usr/lib"))
(allow file-read* (subpath "/System/Library"))
(allow file-read* (subpath "/Library/Frameworks"))

# Allow reading the WASM binary
(allow file-read* (literal "/path/to/skill.wasm"))

# Allow reading skill-specific directories (if capability granted)
; (allow file-read* (subpath "/path/to/allowed/dir"))

# Allow writing to temp dir only
(allow file-write* (subpath "/tmp/boternity-sandbox-XXXX"))

# Network: deny by default
; (allow network*) ; only if network capability granted
```

Implementation via `std::process::Command`:
```rust
let output = Command::new("/usr/bin/sandbox-exec")
    .arg("-p")
    .arg(&profile_string)
    .arg(&executor_binary_path)
    .arg("--wasm").arg(&wasm_path)
    .arg("--input").arg(&input_json)
    .env_clear()  // Start with clean environment
    .env("HOME", temp_dir)
    .output()?;
```

**Linux: Landlock crate (HIGH confidence)**

The `landlock` crate (v0.4.x) provides official Rust bindings for the Landlock LSM. It's maintained by the Landlock project itself.

Key API pattern:
```rust
use landlock::{
    Access, AccessFs, PathBeneath, PathFd,
    Ruleset, RulesetAttr, RulesetCreatedAttr, ABI,
};

// In the CHILD process (not the host!)
fn sandbox_child() -> anyhow::Result<()> {
    let abi = ABI::V3; // Linux 5.19+

    let status = Ruleset::default()
        // Declare what we want to restrict
        .handle_access(AccessFs::from_all(abi))?
        .create()?
        // Allow read to /usr, /lib, /etc (system libs)
        .add_rules(landlock::path_beneath_rules(
            &["/usr", "/lib", "/lib64", "/etc"],
            AccessFs::ReadFile | AccessFs::ReadDir | AccessFs::Execute,
        ))?
        // Allow read to the WASM binary
        .add_rule(PathBeneath::new(
            PathFd::new(&wasm_path)?,
            AccessFs::ReadFile,
        ))?
        // Allow read+write to temp sandbox dir
        .add_rules(landlock::path_beneath_rules(
            &[&sandbox_temp_dir],
            AccessFs::from_all(abi),
        ))?
        .restrict_self()?;

    // Now restricted. Execute WASM here.
    Ok(())
}
```

**Important Landlock details:**
- `restrict_self()` is **permanent** -- no undo. This is why it MUST be in the child process.
- `ABI::V3` requires Linux 5.19+; the crate supports best-effort mode that degrades gracefully on older kernels.
- Landlock handles filesystem only. For network blocking, add seccomp-BPF (block `connect`, `bind`, `listen` syscalls).
- The crate is mature (v0.4.4, MIT/Apache-2.0, maintained by Landlock LSM project).

**Practical phasing:**
- Phase 6a: Rely on WASM sandbox (Wasmtime + WASI). This is strong isolation on its own.
- Phase 6b: Add OS-level sandbox as subprocess model. macOS: sandbox-exec. Linux: landlock + optional seccomp.
- The subprocess model adds ~10-50ms overhead per invocation but provides true defense-in-depth.

### 5. ComposioHQ/awesome-claude-skills Format (RESOLVED)

**Confidence: HIGH**

**Repository structure:**

The repo contains **944 skill directories** in a flat structure at the repo root:

```
ComposioHQ/awesome-claude-skills/
  README.md                    # Categorized index with links
  CONTRIBUTING.md              # Submission guidelines
  airtable-automation/
    SKILL.md                   # Standard agentskills.io format
  slack-automation/
    SKILL.md
  github-automation/
    SKILL.md
  content-research-writer/
    SKILL.md
  skill-creator/
    SKILL.md
  ... (944 directories total)
```

**Skill format:**
Each skill follows the standard agentskills.io SKILL.md format with YAML frontmatter (name + description) and markdown body. They are pure prompt-based skills (no tool-based/WASM skills).

**Categories (from README.md):**
1. Business & Marketing
2. Communication & Writing
3. Creative & Media
4. Development
5. Productivity & Organization

**Indexing approach for Boternity:**

The README.md serves as a human-readable index, but for programmatic discovery:

```
Approach: GitHub API-based indexing

1. Fetch repo tree via GitHub API:
   GET /repos/ComposioHQ/awesome-claude-skills/git/trees/master?recursive=1

2. Filter for SKILL.md files:
   entries.filter(|e| e.path.ends_with("/SKILL.md"))

3. Fetch and parse each SKILL.md frontmatter:
   Only name + description needed for discovery index

4. Cache locally:
   ~/.boternity/cache/composiohq-skills-index.json
   Refresh: daily or on-demand

5. Present in TUI browser:
   Categories from the README can be extracted or hard-coded
   Search by name/description
```

**Key difference from anthropics/skills:**
- ComposioHQ skills are community-contributed, varying quality
- All are prompt-based (system prompt injection), not tool-based
- Naming convention: `service-name` or `task-name` (e.g., `slack-automation`, `content-research-writer`)
- No version metadata beyond git history
- No dependencies between skills

**For Boternity's trust model:**
All ComposioHQ skills are prompt-based and should be treated as **untrusted registry / prompt tier** by default -- they inject text into the system prompt but don't execute code. The WASM sandbox is not needed for these; the risk is prompt injection, which is mitigated by the XML-tagged skill boundaries.

## Code Examples

### Wasmtime Engine Configuration for Skill Execution

```rust
// Source: Wasmtime Config API docs (https://docs.wasmtime.dev/api/wasmtime/struct.Config.html)
use wasmtime::Config;

fn create_engine_config(trust_tier: TrustTier) -> Config {
    let mut config = Config::new();
    config.async_support(true); // Required for tokio integration
    config.wasm_component_model(true); // Enable Component Model
    config.consume_fuel(true); // Enable fuel-based CPU limits
    config.epoch_interruption(true); // Enable epoch-based interruption

    match trust_tier {
        TrustTier::Untrusted => {
            // Strict: disable optional WASM features
            config.wasm_threads(false);
            config.wasm_simd(false);
        }
        TrustTier::Verified => {
            // Relaxed: allow SIMD for performance
            config.wasm_simd(true);
            config.wasm_threads(false);
        }
        TrustTier::Local => {
            // N/A: local skills don't use WASM
            unreachable!("Local skills run natively, not in WASM");
        }
    }

    config
}
```

### WasiCtxBuilder Capability Mapping

```rust
// Source: wasmtime-wasi WasiCtxBuilder docs
// (https://docs.wasmtime.dev/api/wasmtime_wasi/struct.WasiCtxBuilder.html)
use wasmtime_wasi::{WasiCtxBuilder, DirPerms, FilePerms};

fn build_wasi_ctx(
    capabilities: &HashSet<Capability>,
    allowed_dirs: &[(PathBuf, &str)], // (host_path, guest_path)
) -> wasmtime_wasi::WasiCtx {
    let mut builder = WasiCtxBuilder::new();

    // Filesystem: only if capability granted
    if capabilities.contains(&Capability::ReadFile) {
        for (host_path, guest_path) in allowed_dirs {
            builder.preopened_dir(
                host_path,
                guest_path,
                DirPerms::READ,
                FilePerms::READ,
            ).expect("preopened_dir");
        }
    }
    if capabilities.contains(&Capability::WriteFile) {
        for (host_path, guest_path) in allowed_dirs {
            builder.preopened_dir(
                host_path,
                guest_path,
                DirPerms::READ | DirPerms::MUTATE,
                FilePerms::READ | FilePerms::WRITE,
            ).expect("preopened_dir");
        }
    }

    // Network: only if capability granted
    if !capabilities.contains(&Capability::HttpGet)
        && !capabilities.contains(&Capability::HttpPost)
    {
        builder.allow_tcp(false);
        builder.allow_udp(false);
    } else {
        builder.inherit_network();
        // Optionally restrict to specific domains via socket_addr_check
    }

    // No env vars by default (inject specific approved ones)
    // No args by default
    // Capture stdout/stderr for audit logging

    builder.build()
}
```

### Registry Client -- GitHub-Based Skill Discovery

```rust
/// Fetch skills from a GitHub repository (owner/repo format).
/// Mirrors the skills.sh CLI three-tier discovery approach.
async fn discover_skills_from_github(
    client: &reqwest::Client,
    owner: &str,
    repo: &str,
    github_token: Option<&str>,
) -> anyhow::Result<Vec<DiscoveredSkill>> {
    // Fetch repo tree recursively
    let url = format!(
        "https://api.github.com/repos/{owner}/{repo}/git/trees/HEAD?recursive=1"
    );
    let mut req = client.get(&url)
        .header("User-Agent", "boternity");
    if let Some(token) = github_token {
        req = req.header("Authorization", format!("token {token}"));
    }
    let tree: GitHubTree = req.send().await?.json().await?;

    // Find all SKILL.md files
    let skill_paths: Vec<&str> = tree.tree.iter()
        .filter(|e| e.path.ends_with("/SKILL.md") || e.path == "SKILL.md")
        .map(|e| e.path.as_str())
        .collect();

    // Fetch and parse each SKILL.md (frontmatter only for metadata)
    let mut skills = Vec::new();
    for path in skill_paths {
        let raw_url = format!(
            "https://raw.githubusercontent.com/{owner}/{repo}/HEAD/{path}"
        );
        if let Ok(content) = client.get(&raw_url).send().await?.text().await {
            if let Ok((manifest, _body)) = parse_skill_md(&content) {
                let dir_name = path.rsplit('/').nth(1).unwrap_or(path);
                skills.push(DiscoveredSkill {
                    name: manifest.name,
                    description: manifest.description,
                    source: format!("{owner}/{repo}"),
                    path: path.to_string(),
                    manifest,
                });
            }
        }
    }
    Ok(skills)
}

#[derive(Deserialize)]
struct GitHubTree {
    tree: Vec<GitHubTreeEntry>,
}

#[derive(Deserialize)]
struct GitHubTreeEntry {
    path: String,
    #[serde(rename = "type")]
    entry_type: String,
}
```

### Skill Audit Logging

```rust
/// Audit log entry for every skill invocation
#[derive(Debug, Serialize, Deserialize)]
pub struct SkillAuditEntry {
    pub invocation_id: Uuid,
    pub skill_name: String,
    pub skill_version: String,
    pub trust_tier: TrustTier,
    pub capabilities_used: Vec<Capability>,
    pub input_hash: String, // SHA-256 of input (not raw input for privacy)
    pub output_hash: String,
    pub fuel_consumed: Option<u64>,
    pub memory_peak_bytes: Option<usize>,
    pub duration_ms: u64,
    pub success: bool,
    pub error: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub bot_id: Uuid,
}
```

### Skill Storage Layout

```
~/.boternity/
  skills/                          # Global skill library
    my-local-skill/
      SKILL.md                     # Manifest + instructions
      scripts/
      references/
      assets/
    fetched-registry-skill/
      SKILL.md
      skill.wasm                   # Compiled WASM component (tool-based)
      scripts/
      .boternity-meta.toml         # Registry source, version, trust tier, checksums
  cache/
    skills-sh-index.json           # Cached skills.sh leaderboard data
    composiohq-index.json          # Cached ComposioHQ skill index
    github-trees/                  # Cached repo tree hashes for update checks
  bots/
    my-bot/
      skills.toml                  # Per-bot skill config
      # Example skills.toml:
      # [skills.my-local-skill]
      # enabled = true
      # trust_tier = "local"
      # overrides = { temperature = "0.3" }
      #
      # [skills.fetched-registry-skill]
      # enabled = true
      # trust_tier = "untrusted"
      # version = "1.2.3"
      # capabilities = ["http-get", "read-file"]
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| WASI Preview 1 (P1) | WASI Preview 2 (P2) with Component Model | Wasmtime v25+ (2024) | P2 uses WIT interfaces instead of POSIX fd-based API; capability-based by design |
| Core WASM modules | WASM Components | Wasmtime v25+ (2024) | Components have typed imports/exports via WIT; enable true plugin composition |
| serde-yaml (deprecated) | serde_yaml_ng 0.10.x | 2024-2025 | Original serde-yaml archived; serde_yaml_ng is the conservative maintained fork |
| Manual seccomp BPF programs | Landlock (Linux 5.13+) | 2021+ | Landlock provides unprivileged filesystem sandboxing without BPF expertise |
| tui-rs (unmaintained) | ratatui 0.30 | 2023 fork | Active community, modular workspace, no_std support |
| WASIp2 HTTP | WASIp3 HTTP (initial) | Wasmtime 40 (2025-12) | Wasmtime 40 adds initial WASIp3 support for wasi:http |
| cargo build --target wasm32-wasip2 | cargo component build (for custom WIT) | 2024+ | Upstream Rust target works for pure WASI; cargo-component needed for custom interfaces |

**Deprecated/outdated:**
- `serde-yaml`: Archived. Use `serde_yaml_ng` instead.
- `serde_yml`: Unstable (0.0.x, docs.rs failures). Use `serde_yaml_ng` instead.
- `tui-rs`: Unmaintained. Use `ratatui` instead.
- WASI Preview 1 for new projects: Use P2 with Component Model.
- `wasm32-wasi` target: Renamed to `wasm32-wasip1`. New components use `wasm32-wasip2`.

## Open Questions (Updated)

All five original open questions have been resolved. Remaining minor items:

1. **skills.sh API stability**
   - What we know: `skills.sh/api/skills` and `skills.sh/api/skills/search` endpoints exist (discovered from CLI source code analysis).
   - What's unclear: These are undocumented internal APIs. They may change without notice.
   - Recommendation: Use them for discovery/popularity data but don't depend on them for core functionality. The GitHub-based installation path is the stable mechanism.
   - Confidence: MEDIUM

2. **WASM skill binary size budget**
   - What we know: Rust WASM components are relatively small (~1-5MB). JavaScript skills embed SpiderMonkey (~4MB+). Python skills embed CPython (larger).
   - What's unclear: Should Boternity enforce a maximum WASM binary size? What's reasonable?
   - Recommendation: Set a configurable limit (default 50MB) to prevent abuse. Most skills will be 1-10MB.
   - Confidence: MEDIUM

3. **Skill publishing target**
   - What we know: The skills.sh ecosystem is GitHub-native. There is no upload API.
   - What's unclear: How does a skill get listed on skills.sh?
   - Recommendation: `bnity skill publish` should: (a) validate the skill, (b) create/push to a GitHub repo, (c) document how to submit to the skills.sh directory (manual process via Vercel). Publishing to GitHub IS publishing to the ecosystem -- the skills CLI can install from any GitHub repo.
   - Confidence: HIGH

## Sources

### Primary (HIGH confidence)
- [agentskills.io/specification](https://agentskills.io/specification) - Complete SKILL.md format spec: frontmatter fields, directory structure, progressive disclosure, validation
- [agentskills.io/integrate-skills](https://agentskills.io/integrate-skills) - Integration guide: discovery, metadata loading, prompt injection with XML tags, activation logic
- [Vercel Skills CLI README](https://github.com/vercel-labs/skills) - Complete CLI architecture: source parsing, three-tier discovery, installation, lock files, 38+ agent support
- [DeepWiki: vercel-labs/skills](https://deepwiki.com/vercel-labs/skills) - CLI source architecture: parseSource(), discoverSkills(), skill-lock.json, GitHub Trees API for updates, skills.sh API endpoints
- [Wasmtime Component Model API](https://docs.wasmtime.dev/api/wasmtime/component/index.html) - Component, Linker, bindgen! macro, instantiation workflow
- [Wasmtime Config API](https://docs.wasmtime.dev/api/wasmtime/struct.Config.html) - All configuration options: fuel, epoch interruption, memory limits, feature gates
- [WasiCtxBuilder API](https://docs.wasmtime.dev/api/wasmtime_wasi/struct.WasiCtxBuilder.html) - Complete capability configuration: filesystem, network, env, stdio, clock, random
- [Wasmtime ResourceLimiter](https://docs.rs/wasmtime/latest/wasmtime/trait.ResourceLimiter.html) - Memory and table growth limiting API
- [bindgen! Macro Reference](https://docs.wasmtime.dev/api/wasmtime/component/macro.bindgen.html) - Full options: world, path, async, ownership, trappable errors, interface remapping
- [cargo-component README](https://github.com/bytecodealliance/cargo-component) - Project creation, WIT setup, building, Cargo.toml metadata, all commands
- [Landlock Rust crate docs](https://landlock.io/rust-landlock/landlock/) - Complete API: Ruleset, PathBeneath, PathFd, AccessFs, restrict_self()
- [Anthropic skills repo](https://github.com/anthropics/skills) - Official skill examples, plugin marketplace integration
- [ComposioHQ/awesome-claude-skills](https://github.com/ComposioHQ/awesome-claude-skills) - 944 community skills, contribution guide, 5 categories

### Secondary (MEDIUM confidence)
- [Plugins with Rust and WASI Preview 2](https://benw.is/posts/plugins-with-rust-and-wasi) - End-to-end plugin tutorial: WIT, cargo-component, WasiView trait, host setup
- [Building Native Plugin Systems with WASM Components](https://tartanllama.xyz/posts/wasm-plugins/) - Practical WIT + Wasmtime plugin architecture with C/JS/Rust guests
- [Agent Sandbox Deep Dive](https://pierce.dev/notes/a-deep-dive-on-agent-sandboxes) - Defense-in-depth: Landlock, seccomp, Seatbelt patterns from Codex
- [serde-yaml deprecation discussion](https://users.rust-lang.org/t/serde-yaml-deprecation-alternatives/108868) - Community consensus on alternatives; yaml-rust2 and serde-saphyr comparisons
- [Component Model language support](https://component-model.bytecodealliance.org/language-support.html) - ComponentizeJS, componentize-py, wit-bindgen for non-Rust guests
- [skills.sh directory](https://skills.sh) - Web leaderboard with 200+ skills, install counts, search

### Tertiary (LOW confidence)
- skills.sh internal API (`/api/skills`, `/api/skills/search`) -- Undocumented; discovered from CLI source code analysis. May change without notice.
- serde_yaml_ng build stability -- Conservative fork should be stable, but verify docs.rs build before depending.

## Metadata

**Confidence breakdown:**
- Standard stack (Wasmtime + WASI): HIGH -- official Bytecode Alliance docs, stable v40 release, well-documented APIs
- Standard stack (agentskills.io spec): HIGH -- official spec + integration guide fetched directly
- Standard stack (YAML parsing): HIGH -- community landscape fully mapped; serde_yaml_ng is the safe choice
- Architecture patterns (WIT + Component Model): HIGH -- official Wasmtime tutorial + cargo-component docs + multiple verified sources
- Architecture patterns (OS sandbox): MEDIUM (macOS) / HIGH (Linux) -- macOS Seatbelt is underdocumented but proven; Landlock crate is officially documented
- Registry integration: HIGH -- Full CLI source architecture traced via DeepWiki; GitHub-first model confirmed
- Pitfalls: HIGH -- derived from official docs and verified community reports
- WASM skill build toolchain: HIGH -- cargo-component workflow fully documented with code examples

**Research date:** 2026-02-13
**Valid until:** 2026-03-15 (Wasmtime releases monthly; check for v41+ changes)
