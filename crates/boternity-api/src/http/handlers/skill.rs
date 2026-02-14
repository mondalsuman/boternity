//! Skill management HTTP handlers for the REST API.
//!
//! Provides endpoints for listing, inspecting, attaching, detaching, and
//! configuring skills on bots. Also exposes registry search and skill
//! installation endpoints.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

use boternity_core::skill::inheritance::inspect_resolved_capabilities;
use boternity_types::skill::{BotSkillConfig, Capability, TrustTier};

use crate::http::error::AppError;
use crate::http::extractors::auth::Authenticated;
use crate::http::response::ApiResponse;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / Response DTOs
// ---------------------------------------------------------------------------

/// Response item for listing installed skills.
#[derive(Debug, Serialize)]
pub(crate) struct SkillListItem {
    name: String,
    description: String,
    skill_type: Option<String>,
    trust_tier: Option<TrustTier>,
    version: Option<String>,
    source: serde_json::Value,
    installed: bool,
}

/// Response for skill detail/inspect endpoint.
#[derive(Debug, Serialize)]
pub(crate) struct SkillDetail {
    manifest: serde_json::Value,
    body: String,
    resolved_capabilities: Vec<String>,
    parent_chain: Vec<String>,
    conflicts_with: Vec<String>,
}

/// Response item for a bot's attached skill.
#[derive(Debug, Serialize)]
pub(crate) struct BotSkillItem {
    name: String,
    description: String,
    skill_type: Option<String>,
    trust_tier: Option<TrustTier>,
    enabled: bool,
    overrides: HashMap<String, String>,
}

/// Request body for attaching a skill to a bot.
#[derive(Debug, Deserialize)]
pub struct AttachSkillRequest {
    pub skill_name: String,
    #[serde(default)]
    pub capabilities: Option<Vec<Capability>>,
    #[serde(default)]
    pub overrides: Option<HashMap<String, String>>,
}

/// Request body for updating a bot's skill config.
#[derive(Debug, Deserialize)]
pub struct UpdateSkillConfigRequest {
    pub enabled: Option<bool>,
    #[serde(default)]
    pub overrides: Option<HashMap<String, String>>,
}

/// Query parameters for registry search.
#[derive(Debug, Deserialize)]
pub struct RegistrySearchQuery {
    pub q: String,
}

/// Request body for installing a skill from a registry.
#[derive(Debug, Deserialize)]
pub struct InstallSkillRequest {
    pub source: String,
    pub skill_name: Option<String>,
    #[serde(default)]
    pub capabilities_approved: Vec<Capability>,
}

/// Response item for registry search results.
#[derive(Debug, Serialize)]
pub(crate) struct RegistrySearchItem {
    name: String,
    description: String,
    source: String,
    categories: Vec<String>,
    install_count: Option<u64>,
    trust_tier: Option<TrustTier>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/v1/skills - List all installed skills (global library).
pub async fn list_skills(
    State(state): State<AppState>,
    _auth: Authenticated,
) -> Result<Json<ApiResponse<Vec<SkillListItem>>>, AppError> {
    let start = std::time::Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    let skills = state
        .skill_store
        .list_skills()
        .map_err(|e| AppError::Internal(format!("Failed to list skills: {e}")))?;

    let items: Vec<SkillListItem> = skills
        .iter()
        .map(|s| {
            let meta = s.manifest.metadata.as_ref();
            SkillListItem {
                name: s.manifest.name.clone(),
                description: s.manifest.description.clone(),
                skill_type: meta
                    .and_then(|m| m.skill_type.as_ref())
                    .map(|t| serde_json::to_value(t).ok())
                    .flatten()
                    .and_then(|v| v.as_str().map(String::from)),
                trust_tier: meta.and_then(|m| m.trust_tier.clone()),
                version: meta.and_then(|m| m.version.clone()),
                source: serde_json::to_value(&s.source).unwrap_or_default(),
                installed: true,
            }
        })
        .collect();

    let elapsed = start.elapsed().as_millis() as u64;
    let resp =
        ApiResponse::success(items, request_id, elapsed).with_link("self", "/api/v1/skills");

    Ok(Json(resp))
}

/// GET /api/v1/skills/:name - Get skill details with resolved capabilities.
pub async fn get_skill(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<SkillDetail>>, AppError> {
    let start = std::time::Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    let skill = state
        .skill_store
        .get_skill(&name)
        .map_err(|e| AppError::Internal(format!("Skill not found: {e}")))?;

    // Build a manifest map for inheritance resolution
    let all_skills = state
        .skill_store
        .list_skills()
        .map_err(|e| AppError::Internal(format!("Failed to list skills: {e}")))?;

    let manifest_map: HashMap<String, _> = all_skills
        .into_iter()
        .map(|s| (s.manifest.name.clone(), s.manifest))
        .collect();

    // Try to resolve capabilities (graceful fallback if inheritance fails)
    let (resolved_caps, parent_chain, conflicts) =
        match inspect_resolved_capabilities(&name, &manifest_map) {
            Ok(inspected) => (
                inspected
                    .combined_capabilities
                    .iter()
                    .map(|c| format!("{c:?}"))
                    .collect::<Vec<_>>(),
                inspected.parent_chain,
                inspected.conflicts_with,
            ),
            Err(_) => {
                // Fallback: just use own capabilities
                let own = skill
                    .manifest
                    .metadata
                    .as_ref()
                    .and_then(|m| m.capabilities.as_ref())
                    .map(|caps| caps.iter().map(|c| format!("{c:?}")).collect())
                    .unwrap_or_default();
                (own, Vec::new(), Vec::new())
            }
        };

    let manifest_json = serde_json::to_value(&skill.manifest).unwrap_or_default();

    let detail = SkillDetail {
        manifest: manifest_json,
        body: skill.body,
        resolved_capabilities: resolved_caps,
        parent_chain,
        conflicts_with: conflicts,
    };

    let elapsed = start.elapsed().as_millis() as u64;
    let resp = ApiResponse::success(detail, request_id, elapsed)
        .with_link("self", &format!("/api/v1/skills/{name}"));

    Ok(Json(resp))
}

/// GET /api/v1/bots/:bot_id/skills - List skills attached to a bot.
pub async fn list_bot_skills(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(bot_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<BotSkillItem>>>, AppError> {
    let start = std::time::Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    // Resolve the bot to get its data directory
    let bot = resolve_bot(&state, &bot_id).await?;
    let bot_dir = state.data_dir.join("bots").join(bot.id.0.simple().to_string());

    let config = state
        .skill_store
        .get_bot_skills_config(&bot_dir)
        .map_err(|e| AppError::Internal(format!("Failed to read bot skills config: {e}")))?;

    let mut items = Vec::new();
    for (_key, skill_config) in &config.skills {
        // Try to get global skill info for description
        let (description, skill_type) = match state.skill_store.get_skill(&skill_config.skill_name)
        {
            Ok(skill) => {
                let st = skill
                    .manifest
                    .metadata
                    .as_ref()
                    .and_then(|m| m.skill_type.as_ref())
                    .map(|t| serde_json::to_value(t).ok())
                    .flatten()
                    .and_then(|v| v.as_str().map(String::from));
                (skill.manifest.description.clone(), st)
            }
            Err(_) => (String::new(), None),
        };

        items.push(BotSkillItem {
            name: skill_config.skill_name.clone(),
            description,
            skill_type,
            trust_tier: skill_config.trust_tier.clone(),
            enabled: skill_config.enabled,
            overrides: skill_config.overrides.clone(),
        });
    }

    let elapsed = start.elapsed().as_millis() as u64;
    let resp = ApiResponse::success(items, request_id, elapsed)
        .with_link("self", &format!("/api/v1/bots/{bot_id}/skills"));

    Ok(Json(resp))
}

/// POST /api/v1/bots/:bot_id/skills - Attach a skill to a bot.
pub async fn attach_skill(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(bot_id): Path<String>,
    Json(body): Json<AttachSkillRequest>,
) -> Result<impl IntoResponse, AppError> {
    let start = std::time::Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    // Validate skill exists in global store
    if !state.skill_store.skill_exists(&body.skill_name) {
        return Err(AppError::Validation(format!(
            "Skill '{}' not found in global store",
            body.skill_name
        )));
    }

    let bot = resolve_bot(&state, &bot_id).await?;
    let bot_dir = state.data_dir.join("bots").join(bot.id.0.simple().to_string());

    let mut config = state
        .skill_store
        .get_bot_skills_config(&bot_dir)
        .map_err(|e| AppError::Internal(format!("Failed to read bot skills config: {e}")))?;

    // Add skill to config
    let skill_config = BotSkillConfig {
        skill_name: body.skill_name.clone(),
        enabled: true,
        trust_tier: None,
        version: None,
        overrides: body.overrides.unwrap_or_default(),
        capabilities: body.capabilities,
    };

    config
        .skills
        .insert(body.skill_name.clone(), skill_config.clone());

    state
        .skill_store
        .save_bot_skills_config(&bot_dir, &config)
        .map_err(|e| AppError::Internal(format!("Failed to save bot skills config: {e}")))?;

    let elapsed = start.elapsed().as_millis() as u64;
    let resp = ApiResponse::success(
        serde_json::to_value(&skill_config).unwrap_or_default(),
        request_id,
        elapsed,
    );

    Ok((StatusCode::CREATED, Json(resp)))
}

/// DELETE /api/v1/bots/:bot_id/skills/:name - Detach a skill from a bot.
pub async fn detach_skill(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path((bot_id, skill_name)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let start = std::time::Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    let bot = resolve_bot(&state, &bot_id).await?;
    let bot_dir = state.data_dir.join("bots").join(bot.id.0.simple().to_string());

    let mut config = state
        .skill_store
        .get_bot_skills_config(&bot_dir)
        .map_err(|e| AppError::Internal(format!("Failed to read bot skills config: {e}")))?;

    if config.skills.remove(&skill_name).is_none() {
        return Err(AppError::Validation(format!(
            "Skill '{skill_name}' is not attached to this bot"
        )));
    }

    state
        .skill_store
        .save_bot_skills_config(&bot_dir, &config)
        .map_err(|e| AppError::Internal(format!("Failed to save bot skills config: {e}")))?;

    let elapsed = start.elapsed().as_millis() as u64;
    let resp = ApiResponse::success(
        serde_json::json!({"detached": true, "skill_name": skill_name}),
        request_id,
        elapsed,
    );

    Ok((StatusCode::OK, Json(resp)))
}

/// PATCH /api/v1/bots/:bot_id/skills/:name - Update skill config (enable/disable, overrides).
pub async fn update_skill_config(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path((bot_id, skill_name)): Path<(String, String)>,
    Json(body): Json<UpdateSkillConfigRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = std::time::Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    let bot = resolve_bot(&state, &bot_id).await?;
    let bot_dir = state.data_dir.join("bots").join(bot.id.0.simple().to_string());

    let mut config = state
        .skill_store
        .get_bot_skills_config(&bot_dir)
        .map_err(|e| AppError::Internal(format!("Failed to read bot skills config: {e}")))?;

    let skill_config = config.skills.get_mut(&skill_name).ok_or_else(|| {
        AppError::Validation(format!(
            "Skill '{skill_name}' is not attached to this bot"
        ))
    })?;

    if let Some(enabled) = body.enabled {
        skill_config.enabled = enabled;
    }
    if let Some(overrides) = body.overrides {
        skill_config.overrides = overrides;
    }

    let updated = serde_json::to_value(&*skill_config).unwrap_or_default();

    state
        .skill_store
        .save_bot_skills_config(&bot_dir, &config)
        .map_err(|e| AppError::Internal(format!("Failed to save bot skills config: {e}")))?;

    let elapsed = start.elapsed().as_millis() as u64;
    let resp = ApiResponse::success(updated, request_id, elapsed);

    Ok(Json(resp))
}

/// GET /api/v1/registry/search?q=query - Search registries for skills.
pub async fn search_registry(
    State(state): State<AppState>,
    _auth: Authenticated,
    Query(query): Query<RegistrySearchQuery>,
) -> Result<Json<ApiResponse<Vec<RegistrySearchItem>>>, AppError> {
    use boternity_core::skill::registry::{RegistryType, SkillRegistry};
    use boternity_infra::skill::registry_client::{
        default_registry_configs, GitHubRegistryClient, SkillsShClient,
    };

    let start = std::time::Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    let cache_dir = state.data_dir.join("cache").join("registries");
    let configs = default_registry_configs();

    let mut results = Vec::new();

    for config in &configs {
        if !config.enabled {
            continue;
        }

        let search_result = match &config.registry_type {
            RegistryType::GitHub { owner, repo } => {
                let client = GitHubRegistryClient::new(
                    owner.clone(),
                    repo.clone(),
                    config.name.clone(),
                    cache_dir.clone(),
                );
                client.search(&query.q, 20).await
            }
            RegistryType::SkillsSh => {
                let client = SkillsShClient::new(cache_dir.clone());
                // Convert SkillsShEntry to DiscoveredSkill format
                match client.search_api(&query.q).await {
                    Ok(entries) => Ok(entries
                        .into_iter()
                        .map(|e| {
                            boternity_core::skill::registry::DiscoveredSkill {
                                name: e.name,
                                description: e.description,
                                source: "skills-sh".to_string(),
                                path: e.path.unwrap_or_default(),
                                manifest: boternity_types::skill::SkillManifest {
                                    name: String::new(),
                                    description: String::new(),
                                    license: None,
                                    compatibility: None,
                                    metadata: None,
                                    allowed_tools: None,
                                },
                                install_count: e.install_count,
                                categories: e.categories,
                            }
                        })
                        .collect()),
                    Err(e) => Err(e),
                }
            }
            RegistryType::Custom { .. } => continue,
        };

        match search_result {
            Ok(skills) => {
                for skill in skills {
                    let trust_tier = skill
                        .manifest
                        .metadata
                        .as_ref()
                        .and_then(|m| m.trust_tier.clone());
                    results.push(RegistrySearchItem {
                        name: skill.name,
                        description: skill.description,
                        source: skill.source,
                        categories: skill.categories,
                        install_count: skill.install_count,
                        trust_tier,
                    });
                }
            }
            Err(e) => {
                tracing::warn!(
                    registry = %config.name,
                    error = %e,
                    "Registry search failed, skipping"
                );
            }
        }
    }

    let elapsed = start.elapsed().as_millis() as u64;
    let resp = ApiResponse::success(results, request_id, elapsed)
        .with_link("self", "/api/v1/registry/search");

    Ok(Json(resp))
}

/// POST /api/v1/skills/install - Install a skill from a registry.
pub async fn install_skill(
    State(state): State<AppState>,
    _auth: Authenticated,
    Json(body): Json<InstallSkillRequest>,
) -> Result<impl IntoResponse, AppError> {
    use boternity_core::skill::registry::{RegistryType, SkillRegistry};
    use boternity_infra::skill::registry_client::{
        default_registry_configs, GitHubRegistryClient,
    };

    let start = std::time::Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    let cache_dir = state.data_dir.join("cache").join("registries");
    let configs = default_registry_configs();

    // Find the registry matching the source
    let registry_config = configs
        .iter()
        .find(|c| c.name == body.source)
        .ok_or_else(|| {
            AppError::Validation(format!("Registry '{}' not found", body.source))
        })?;

    // Search for the skill
    let skill_name = body
        .skill_name
        .as_deref()
        .unwrap_or(&body.source);

    let (content, wasm_bytes) = match &registry_config.registry_type {
        RegistryType::GitHub { owner, repo } => {
            let client = GitHubRegistryClient::new(
                owner.clone(),
                repo.clone(),
                registry_config.name.clone(),
                cache_dir,
            );
            let skills = client
                .search(skill_name, 1)
                .await
                .map_err(|e| AppError::Internal(format!("Registry search failed: {e}")))?;

            let found = skills.first().ok_or_else(|| {
                AppError::Validation(format!("Skill '{skill_name}' not found in registry"))
            })?;

            client
                .fetch_skill(found)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to fetch skill: {e}")))?
        }
        _ => {
            return Err(AppError::Validation(
                "Install is only supported from GitHub registries".to_string(),
            ));
        }
    };

    // Install to global store
    let meta = boternity_types::skill::SkillMeta {
        source: boternity_types::skill::SkillSource::Registry {
            registry_name: registry_config.name.clone(),
            repo: body.source.clone(),
            path: skill_name.to_string(),
        },
        installed_at: chrono::Utc::now(),
        version: "0.1.0".parse().unwrap(),
        checksum: String::new(),
        trust_tier: TrustTier::Untrusted,
    };

    let install_path = state
        .skill_store
        .install_skill(skill_name, &content, Some(meta), wasm_bytes.as_deref())
        .map_err(|e| AppError::Internal(format!("Failed to install skill: {e}")))?;

    // Ensure Tool-type skills have a WASM binary (pre-compiled or stub)
    if let Ok((manifest, body)) = boternity_core::skill::manifest::parse_skill_md(&content) {
        let is_tool = manifest
            .metadata
            .as_ref()
            .and_then(|m| m.skill_type.as_ref())
            .map(|t| matches!(t, boternity_types::skill::SkillType::Tool))
            .unwrap_or(false);

        if is_tool {
            boternity_infra::skill::wasm_compiler::ensure_wasm_binary(
                &install_path,
                &body,
                wasm_bytes.as_deref(),
            )
            .map_err(|e| AppError::Internal(format!("Failed to generate WASM: {e}")))?;
        }
    }

    let elapsed = start.elapsed().as_millis() as u64;
    let resp = ApiResponse::success(
        serde_json::json!({
            "installed": true,
            "skill_name": skill_name,
            "path": install_path.display().to_string(),
        }),
        request_id,
        elapsed,
    );

    Ok((StatusCode::CREATED, Json(resp)))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve a bot by ID or slug, returning the full bot domain object.
async fn resolve_bot(
    state: &AppState,
    id_or_slug: &str,
) -> Result<boternity_types::bot::Bot, AppError> {
    match state.bot_service.get_bot_by_slug(id_or_slug).await {
        Ok(bot) => Ok(bot),
        Err(_) => {
            let id = id_or_slug
                .parse()
                .map_err(|_| AppError::Bot(boternity_types::error::BotError::NotFound))?;
            state
                .bot_service
                .get_bot(&id)
                .await
                .map_err(AppError::from)
        }
    }
}
