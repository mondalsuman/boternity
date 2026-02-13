# Phase 6: Skill System + WASM Sandbox - Research

**Researched:** 2026-02-13
**Domain:** WASM sandboxing, plugin architecture, skill manifest format, OS-level sandboxing, TUI, dependency resolution
**Confidence:** HIGH (Wasmtime/WIT), HIGH (Agent Skills spec), MEDIUM (OS sandbox), HIGH (TUI)

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

**Primary recommendation:** Use Wasmtime's Component Model with WIT-defined interfaces for sandboxed tool skills, the agentskills.io SKILL.md format as the manifest standard (extending with boternity-specific fields in the `metadata` section), and ratatui for the interactive TUI skill browser.

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
| serde_yml | 0.0.12 | YAML frontmatter parsing for SKILL.md | Parse YAML frontmatter in skill manifests; serde-yaml is deprecated, serde_yml is the maintained fork |
| toml (already in workspace) | 0.8 | TOML parsing for per-bot skill config | Bot-level skill configuration files (which skills are attached, overrides) |
| reqwest (already in workspace) | 0.12 | HTTP client for registry API calls | Fetching skill metadata and packages from skills.sh and custom registries |
| sha2 (already in workspace) | 0.10 | Integrity verification for downloaded skills | Hash verification of skill packages from registries |
| tempfile (already in workspace) | 3.x | Temporary extraction during skill install | Safe temp directories for extracting skill archives before validation |
| dirs (already in workspace) | 6.x | Platform directory resolution | Resolving `~/.boternity/skills/` path cross-platform |
| wit-bindgen | 0.41.x | Guest-side WIT binding generation | Generating Rust bindings for skill authors writing tool-based WASM skills |

### OS-Level Sandboxing (Platform-Specific)

| Component | Platform | Purpose | Notes |
|-----------|----------|---------|-------|
| sandbox-exec + Seatbelt profiles | macOS | OS-level filesystem/network restriction | Apple's sandbox-exec with dynamically generated .sbpl profiles; deprecated but universally used (Chrome, Firefox, Codex all use it) |
| Landlock + seccomp-BPF | Linux | Filesystem + syscall filtering | Landlock for capability-based filesystem access (Linux 5.13+); seccomp-BPF for syscall filtering; no external dependencies |
| libseccomp-rs | Linux | Rust bindings for seccomp | Rust interface to libseccomp library; BPF filter generation |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| wasmtime | wasmer | Wasmer is commercial-focused; wasmtime has better Component Model support; wasmtime is the Bytecode Alliance reference implementation |
| wasmtime | extism | Extism wraps wasmtime but adds overhead and opinionated plugin model; direct wasmtime gives us full control over WIT interfaces and WASI capabilities |
| serde_yml | serde-saphyr | serde-saphyr is newer (Sept 2025) but less mature; serde_yml is a maintained fork of the original serde-yaml |
| petgraph | custom DAG | Dependency resolution has subtle cycle detection and ordering requirements; petgraph is battle-tested |
| ratatui | cursive | Ratatui is more widely adopted, better documented, and already aligned with crossterm (in workspace) |

**Installation (new dependencies only):**
```bash
cargo add wasmtime@40 --features component-model,async,cranelift
cargo add wasmtime-wasi@40
cargo add serde_yml@0.0.12
cargo add semver@1
cargo add petgraph@0.7
cargo add ratatui@0.30 --features crossterm
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
    registry_client.rs  # HTTP client for skills.sh / custom registries
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
use wasmtime_wasi::WasiCtxBuilder;

// Generate Rust bindings from WIT
bindgen!({
    world: "skill-plugin",
    path: "wit/boternity-skill.wit",
    async: true,
});

struct SkillState {
    wasi: wasmtime_wasi::WasiCtx,
    // Per-skill capability grants
    capabilities: HashSet<Capability>,
    // Audit context
    invocation_id: Uuid,
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

    // Add WASI with restricted capabilities
    let wasi = WasiCtxBuilder::new()
        // No filesystem access by default
        // No network by default
        // No env vars
        // Null stdio (capture output)
        .build();

    let mut store = Store::new(engine, SkillState {
        wasi,
        capabilities,
        invocation_id: Uuid::now_v7(),
    });

    // Enable fuel for CPU limiting
    store.set_fuel(1_000_000)?; // 1M fuel units

    // Apply memory limits
    store.limiter(|state| state);

    let plugin = SkillPlugin::instantiate_async(&mut store, &component, &linker).await?;
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
    // Boternity-specific extensions
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
    let manifest: SkillManifest = serde_yml::from_str(yaml)?;
    Ok((manifest, body))
}
```

### Pattern 4: Prompt-Based Skill Integration

**What:** Inject prompt-based skills into the system prompt using XML tags.
**When to use:** When a bot has prompt-based skills attached.

```rust
// Insert between <identity> and <user_context> sections
// Each active prompt skill gets its own tagged section
fn inject_skill_prompts(
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

    // Insert <skills> block after </identity> and before <user_context>
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
        // Fallback: prepend skills block
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

    // Build graph
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

### Pattern 6: OS-Level Sandbox Dispatch

**What:** Platform-specific sandbox wrapper for defense-in-depth.
**When to use:** Wrapping WASM execution in an additional OS-level sandbox.

```rust
#[cfg(target_os = "macos")]
fn apply_os_sandbox(config: &SandboxConfig) -> anyhow::Result<()> {
    // Generate Seatbelt profile (.sbpl)
    // Use sandbox-exec to apply restrictions
    // Key restrictions: read-only filesystem (except allowed paths),
    // no network (unless capability granted), no process spawning
    todo!("Generate .sbpl profile and apply via sandbox-exec")
}

#[cfg(target_os = "linux")]
fn apply_os_sandbox(config: &SandboxConfig) -> anyhow::Result<()> {
    // Apply Landlock for filesystem restrictions
    // Apply seccomp-BPF for syscall filtering
    // Block: connect, bind, listen (unless network capability granted)
    // Block: execve (no subprocess spawning)
    todo!("Apply Landlock + seccomp-BPF filters")
}
```

### Anti-Patterns to Avoid

- **Loading full skill content at startup:** Skills use progressive disclosure -- only name + description loaded initially (~100 tokens). Full body loaded on activation. Do NOT load all skill bodies into memory.
- **Dynamic trait objects for skill types:** Use enums (`SkillType::Prompt` / `SkillType::Tool`) instead of trait objects. There are exactly two skill types, not an open set.
- **Sharing Wasmtime Engine across trust tiers:** Use separate `Engine` configurations for verified vs untrusted tiers. Untrusted needs stricter fuel limits, memory caps, and disabled features.
- **Mutable WASM state between invocations:** Create fresh `Store` per invocation. WASM skills should be stateless between calls. State leaks between invocations are a security risk.
- **Running OS sandbox and WASM in the same process:** The OS sandbox (seccomp/seatbelt) should wrap the entire WASM host process or use a subprocess model. Applying seccomp to the host process would break the host's own I/O.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| WASM runtime | Custom WASM interpreter | Wasmtime 40.x | Formally verified, AOT-compiled, standards-compliant, Bytecode Alliance-backed |
| WASM capability isolation | Custom permission checks at syscall level | WasiCtxBuilder capability model | WASI's capability-based security is exactly this -- preopened_dir, allow_tcp, socket_addr_check |
| Semver parsing/comparison | Regex-based version parsing | `semver` crate | Edge cases in pre-release ordering, build metadata, range matching are enormous |
| Dependency graph resolution | Custom recursive resolution | `petgraph` with `toposort()` | Cycle detection, topological ordering, edge cases in diamond dependencies |
| YAML frontmatter parsing | Custom string splitting | `serde_yml` with frontmatter extraction | YAML has many edge cases (multiline strings, anchors, type coercion) |
| TUI widgets | Custom terminal rendering | `ratatui` widgets (Table, List, Paragraph, Tabs) | Table rendering, scrolling, input handling, responsive layout are all solved |
| Execution time limits | Custom timer threads | Wasmtime fuel consumption + epoch interruption | Fuel is deterministic per-instruction; epoch interruption is lightweight and cannot be bypassed by malicious WASM |
| Memory limits | Custom memory tracking | Wasmtime `ResourceLimiter` trait | Intercepts all memory growth requests; works at the linear memory level |
| SKILL.md format | Custom skill manifest format | agentskills.io specification | Open standard; compatible with Claude Code, Codex, Cursor, Gemini CLI |

**Key insight:** The WASM Component Model + WASI provides a complete capability-based security model out of the box. The host defines exactly which capabilities each guest receives through WasiCtxBuilder and custom host imports. Do not attempt to build a capability system on top of raw WASM -- use WASI's built-in model.

## Common Pitfalls

### Pitfall 1: Mixing WASI Preview 1 and Preview 2 APIs
**What goes wrong:** Wasmtime supports both P1 (POSIX-ish file descriptors) and P2 (Component Model interfaces). Using P1 APIs with Component Model components causes linker failures.
**Why it happens:** Older tutorials and examples use P1. The `wasmtime_wasi::p1` module is for legacy core modules, not components.
**How to avoid:** Always use `wasmtime_wasi::p2` APIs for Component Model components. Use `WasiCtxBuilder::new().build()` (not `.build_p1()`). Link with `wasmtime_wasi::p2::add_to_linker_async()`.
**Warning signs:** Linker errors about missing imports like `wasi_snapshot_preview1`.

### Pitfall 2: Forgetting Fuel Configuration
**What goes wrong:** A malicious or buggy WASM skill enters an infinite loop, consuming host CPU indefinitely.
**Why it happens:** Fuel consumption is disabled by default in Wasmtime. Must be explicitly enabled in `Config` AND set on each `Store`.
**How to avoid:** Enable with `Config::new().consume_fuel(true)`. Set per-store with `store.set_fuel(limit)`. Use different limits per trust tier.
**Warning signs:** Skills that hang without timeout.

### Pitfall 3: OS Sandbox Restricting the Host Process
**What goes wrong:** Applying seccomp or Landlock to the main boternity process blocks legitimate host operations (DB writes, LLM API calls, etc.).
**Why it happens:** OS-level sandboxing affects the entire process, not just WASM execution within it.
**How to avoid:** Run sandboxed skill execution in a separate subprocess. The parent process spawns a child with sandbox restrictions applied before the child executes WASM. Alternatively, use the WASM sandbox as the primary barrier and reserve OS sandbox for the subprocess model.
**Warning signs:** Host process cannot write to SQLite, cannot make HTTP requests after applying sandbox.

### Pitfall 4: Capability Check Bypass Through Host Imports
**What goes wrong:** WASM guest calls a host-provided import (e.g., `http-get`) that the skill doesn't have permission for, and the host executes it.
**Why it happens:** WIT host imports are linked at instantiation time. If the host function doesn't check capabilities, any guest can call it.
**How to avoid:** Every host import function MUST check the skill's capability grants before executing. Use a `CapabilityEnforcer` that wraps all host functions and validates permissions before execution.
**Warning signs:** Skills accessing resources they shouldn't have.

### Pitfall 5: Progressive Disclosure Token Bloat
**What goes wrong:** Loading all skill instructions into the system prompt at once consumes the entire context window.
**Why it happens:** A bot with 10+ skills, each with ~5000 tokens of instructions, adds 50K+ tokens to every prompt.
**How to avoid:** Only inject active/relevant skills. Use the progressive disclosure architecture: Level 1 (metadata ~100 tokens per skill), Level 2 (full instructions only for activated skills), Level 3 (reference files only on demand).
**Warning signs:** Token budget exhausted before user message is processed.

### Pitfall 6: YAML Frontmatter Parsing Edge Cases
**What goes wrong:** YAML type coercion silently converts values. `version: 1.0` becomes float `1.0`, not string `"1.0"`. `name: true` becomes boolean.
**Why it happens:** YAML's implicit typing is aggressive.
**How to avoid:** Always quote version strings in SKILL.md (`version: "1.0"`). Validate manifest fields with explicit type checks after parsing. Use `#[serde(deserialize_with = ...)]` for fields that must be strings.
**Warning signs:** Version comparisons failing because `1.0` parsed as float.

### Pitfall 7: Diamond Dependency Conflicts
**What goes wrong:** Skill A depends on Skill C v1.x, Skill B depends on Skill C v2.x. Installing both A and B creates an unresolvable conflict.
**Why it happens:** Skills pin to semver ranges, and major version bumps are incompatible.
**How to avoid:** Detect conflicts during dependency resolution BEFORE installation. Show clear error: "Skill A requires skill-c ^1.0, but Skill B requires skill-c ^2.0. Cannot install both." Let user resolve manually (per locked decision).
**Warning signs:** Silent version override causing one skill to break.

### Pitfall 8: Wasmtime Component vs Core Module Confusion
**What goes wrong:** Trying to instantiate a core WASM module (`.wasm` compiled with wasm32-wasip1) as a Component, or vice versa.
**Why it happens:** Core modules and Components have different binary formats. The Component Model wraps core modules.
**How to avoid:** Skills must be compiled as Components (using `cargo component build` or `wasm-tools component new`). Validate the binary header on load. Components use the Component Model binary format, not the core WASM format.
**Warning signs:** "not a component" errors during instantiation.

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
| serde-yaml (deprecated) | serde_yml 0.0.12 | 2024 | Original serde-yaml archived; serde_yml is maintained fork |
| Manual seccomp BPF programs | Landlock (Linux 5.13+) | 2021+ | Landlock provides unprivileged filesystem sandboxing without BPF expertise |
| tui-rs (unmaintained) | ratatui 0.30 | 2023 fork | Active community, modular workspace, no_std support |
| WASIp2 HTTP | WASIp3 HTTP (initial) | Wasmtime 40 (2025-12) | Wasmtime 40 adds initial WASIp3 support for wasi:http |

**Deprecated/outdated:**
- `serde-yaml`: Archived. Use `serde_yml` instead.
- `tui-rs`: Unmaintained. Use `ratatui` instead.
- WASI Preview 1 for new projects: Use P2 with Component Model.
- `wasm32-wasi` target: Renamed to `wasm32-wasip1`. New components use `wasm32-wasip2`.

## Open Questions

1. **skills.sh Registry API**
   - What we know: CLI uses `npx skills find/add`, the web directory at skills.sh exists, skills are published as GitHub repos.
   - What's unclear: No documented REST API for programmatic access. The skills.sh registry appears to be GitHub-based (owner/repo format for install), not a traditional package registry with an API.
   - Recommendation: Implement the registry client as a GitHub API wrapper (fetching SKILL.md from repos) rather than assuming a REST API exists. The `skills.sh` CLI uses npm/GitHub internally. For v1, support `git clone`-based installation from GitHub URLs.

2. **OS-Level Sandbox for Defense-in-Depth**
   - What we know: macOS Seatbelt works (used by Chrome, Firefox, Codex); Linux Landlock + seccomp work; both are platform-specific with no unified Rust abstraction.
   - What's unclear: Whether to run WASM execution in a subprocess (applying OS sandbox to subprocess) or apply OS sandbox in-process. The `sandbox-runtime` crate is TypeScript-only (Anthropic's implementation), not a Rust library.
   - Recommendation: Phase 1: rely primarily on WASM sandbox (Wasmtime + WASI capabilities), which provides strong isolation. Phase 2: add OS-level sandbox as subprocess model (spawn child process with seccomp/seatbelt, child runs Wasmtime). This avoids the complexity of cross-platform sandbox implementation blocking the core skill system.

3. **ComposioHQ/awesome-claude-skills Registry Format**
   - What we know: It's a GitHub repo with curated skills following the agentskills.io SKILL.md format. Skills are directories in the repo.
   - What's unclear: Whether to index the repo directly or use a pre-built index.
   - Recommendation: Clone/fetch the repo, parse SKILL.md files from each directory. Simple file-based discovery.

4. **Skill Publishing (`bnity skill publish`)**
   - What we know: User wants this command. skills.sh uses GitHub as the backing store.
   - What's unclear: Where does "publishing" go? skills.sh doesn't seem to have an upload API.
   - Recommendation: For v1, `publish` validates the skill, creates a GitHub repo (or pushes to existing), and optionally submits to skills.sh via their contribution process. Full registry publishing is a stretch goal.

5. **WASM Component Compilation for Skill Authors**
   - What we know: Rust skill authors use `cargo component build` (via cargo-component). Other languages use wit-bindgen + wasm-tools.
   - What's unclear: The exact toolchain setup for skill authors. Whether boternity should provide a `bnity skill build` command that wraps the compilation.
   - Recommendation: Provide a `bnity skill build` command that wraps `cargo component build` with the correct WIT path and target settings. Include a skill template generator (`bnity skill create --type tool`) that scaffolds the Cargo.toml with the right dependencies.

## Sources

### Primary (HIGH confidence)
- [agentskills.io/specification](https://agentskills.io/specification) - Complete SKILL.md format spec: frontmatter fields, directory structure, progressive disclosure, validation
- [Wasmtime Component Model API](https://docs.wasmtime.dev/api/wasmtime/component/index.html) - Component, Linker, bindgen! macro, instantiation workflow
- [Wasmtime Config API](https://docs.wasmtime.dev/api/wasmtime/struct.Config.html) - All configuration options: fuel, epoch interruption, memory limits, feature gates
- [WasiCtxBuilder API](https://docs.wasmtime.dev/api/wasmtime_wasi/struct.WasiCtxBuilder.html) - Complete capability configuration: filesystem, network, env, stdio, clock, random
- [Wasmtime ResourceLimiter](https://docs.rs/wasmtime/latest/wasmtime/trait.ResourceLimiter.html) - Memory and table growth limiting API
- [Wasmtime WASI P2 Plugin Tutorial](https://docs.wasmtime.dev/wasip2-plugins.html) - End-to-end plugin system with WIT, bindgen, Component instantiation
- [bindgen! Macro Reference](https://docs.wasmtime.dev/api/wasmtime/component/macro.bindgen.html) - Full options: world, path, async, ownership, trappable errors, interface remapping

### Secondary (MEDIUM confidence)
- [Building Native Plugin Systems with WASM Components](https://tartanllama.xyz/posts/wasm-plugins/) - Practical WIT + Wasmtime plugin architecture with C/JS/Rust guests
- [Agent Sandbox Deep Dive](https://pierce.dev/notes/a-deep-dive-on-agent-sandboxes) - Defense-in-depth analysis: Landlock, seccomp, Seatbelt patterns from Codex
- [Anthropic sandbox-runtime](https://github.com/anthropic-experimental/sandbox-runtime) - TypeScript reference for OS-level sandboxing approach (not a Rust crate)
- [Wasmtime WASI Status](https://eunomia.dev/blog/2025/02/16/wasi-and-the-webassembly-component-model-current-status/) - WASI P2 maturity analysis

### Tertiary (LOW confidence)
- skills.sh registry protocol: No documented REST API found. CLI-based (npx skills) using GitHub as backend. Registry integration may need reverse-engineering or GitHub API approach.
- serde_yml v0.0.12: docs.rs build failed; library may have stability issues. Validate before committing.

## Metadata

**Confidence breakdown:**
- Standard stack (Wasmtime + WASI): HIGH -- official Bytecode Alliance docs, stable v40 release, well-documented APIs
- Standard stack (agentskills.io spec): HIGH -- official spec fetched directly, clear field definitions
- Architecture patterns (WIT + Component Model): HIGH -- official Wasmtime tutorial + multiple verified sources
- Architecture patterns (OS sandbox): MEDIUM -- patterns verified from Codex/Chrome implementations, but no unified Rust library
- Pitfalls: HIGH -- derived from official docs (fuel disabled by default, P1 vs P2 confusion) and verified community reports
- Registry integration: LOW -- skills.sh has no documented API; ComposioHQ is a GitHub repo, not a registry service

**Research date:** 2026-02-13
**Valid until:** 2026-03-15 (Wasmtime releases monthly; check for v41+ changes)
