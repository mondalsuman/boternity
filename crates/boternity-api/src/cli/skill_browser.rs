//! Interactive TUI skill browser.
//!
//! Provides a ratatui-based 3-pane browser for discovering and installing
//! skills from configured registries.
//!
//! Layout:
//! - Left (20%): Category list
//! - Center (50%): Skill list with name, description, tier badge
//! - Right (30%): Detail panel
//!
//! Keybindings: `/` search, Enter install, Esc/q quit, Tab cycle panes,
//! j/k navigate, i details

use std::io;
use std::path::Path;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::prelude::*;
use ratatui::widgets::{
    Block, Borders, List, ListItem, ListState, Paragraph, Wrap,
};

use boternity_core::skill::registry::{DiscoveredSkill, SkillRegistry};
use boternity_infra::skill::registry_client::GitHubRegistryClient;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Which pane currently has focus.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pane {
    Categories,
    Skills,
    Details,
}

/// The TUI browser application state.
struct BrowserState {
    /// All discovered skills loaded from registries.
    all_skills: Vec<DiscoveredSkill>,
    /// Deduplicated category list.
    categories: Vec<String>,
    /// Skills filtered by selected category and search query.
    filtered_skills: Vec<usize>,
    /// Category list selection state.
    category_state: ListState,
    /// Skill list selection state.
    skill_state: ListState,
    /// Active pane.
    active_pane: Pane,
    /// Search query (empty = show all).
    search_query: String,
    /// Whether search input mode is active.
    search_mode: bool,
    /// Whether the user has selected a skill to install.
    selected: Option<DiscoveredSkill>,
    /// Whether the user has quit.
    quit: bool,
}

impl BrowserState {
    fn new(all_skills: Vec<DiscoveredSkill>) -> Self {
        let mut categories: Vec<String> = vec!["All".to_string()];
        let mut cat_set = std::collections::HashSet::new();
        for skill in &all_skills {
            for cat in &skill.categories {
                if cat_set.insert(cat.clone()) {
                    categories.push(cat.clone());
                }
            }
        }

        let filtered: Vec<usize> = (0..all_skills.len()).collect();

        let mut category_state = ListState::default();
        category_state.select(Some(0));
        let mut skill_state = ListState::default();
        if !filtered.is_empty() {
            skill_state.select(Some(0));
        }

        Self {
            all_skills,
            categories,
            filtered_skills: filtered,
            category_state,
            skill_state,
            active_pane: Pane::Skills,
            search_query: String::new(),
            search_mode: false,
            selected: None,
            quit: false,
        }
    }

    /// Recompute filtered skills based on selected category and search query.
    fn refilter(&mut self) {
        let selected_cat = self
            .category_state
            .selected()
            .and_then(|i| self.categories.get(i))
            .cloned()
            .unwrap_or_else(|| "All".to_string());

        let query_lower = self.search_query.to_lowercase();

        self.filtered_skills = self
            .all_skills
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                // Category filter
                let cat_match = selected_cat == "All"
                    || s.categories.iter().any(|c| c == &selected_cat);

                // Search filter
                let search_match = query_lower.is_empty()
                    || s.name.to_lowercase().contains(&query_lower)
                    || s.description.to_lowercase().contains(&query_lower);

                cat_match && search_match
            })
            .map(|(i, _)| i)
            .collect();

        // Reset skill selection
        if self.filtered_skills.is_empty() {
            self.skill_state.select(None);
        } else {
            self.skill_state.select(Some(0));
        }
    }

    /// Get the currently selected skill (if any).
    fn current_skill(&self) -> Option<&DiscoveredSkill> {
        self.skill_state
            .selected()
            .and_then(|i| self.filtered_skills.get(i))
            .and_then(|&idx| self.all_skills.get(idx))
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Launch the interactive TUI skill browser.
///
/// Returns `Some(DiscoveredSkill)` if the user selected a skill, or `None`
/// if they quit without selecting.
pub async fn run_browser(
    registries: &[GitHubRegistryClient],
    _cache_dir: &Path,
) -> Result<Option<DiscoveredSkill>> {
    // Load skills from all registries
    let mut all_skills = Vec::new();
    for registry in registries {
        match registry.list(0, 200).await {
            Ok(skills) => all_skills.extend(skills),
            Err(e) => {
                tracing::warn!(
                    registry = registry.name(),
                    error = %e,
                    "Failed to load skills from registry"
                );
            }
        }
    }

    if all_skills.is_empty() {
        anyhow::bail!(
            "No skills found in configured registries. Check your internet connection."
        );
    }

    // Enter TUI mode
    let mut state = BrowserState::new(all_skills);

    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let result = run_tui_loop(&mut terminal, &mut state);

    // Restore terminal
    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;

    result?;

    Ok(state.selected)
}

// ---------------------------------------------------------------------------
// TUI loop
// ---------------------------------------------------------------------------

fn run_tui_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut BrowserState,
) -> Result<()> {
    loop {
        terminal.draw(|frame| draw(frame, state))?;

        if state.quit || state.selected.is_some() {
            break;
        }

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                handle_key(key.code, state);
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Key handling
// ---------------------------------------------------------------------------

fn handle_key(code: KeyCode, state: &mut BrowserState) {
    if state.search_mode {
        match code {
            KeyCode::Esc => {
                state.search_mode = false;
            }
            KeyCode::Enter => {
                state.search_mode = false;
                state.refilter();
            }
            KeyCode::Backspace => {
                state.search_query.pop();
                state.refilter();
            }
            KeyCode::Char(c) => {
                state.search_query.push(c);
                state.refilter();
            }
            _ => {}
        }
        return;
    }

    match code {
        KeyCode::Char('q') | KeyCode::Esc => {
            state.quit = true;
        }
        KeyCode::Char('/') => {
            state.search_mode = true;
        }
        KeyCode::Tab => {
            state.active_pane = match state.active_pane {
                Pane::Categories => Pane::Skills,
                Pane::Skills => Pane::Details,
                Pane::Details => Pane::Categories,
            };
        }
        KeyCode::BackTab => {
            state.active_pane = match state.active_pane {
                Pane::Categories => Pane::Details,
                Pane::Skills => Pane::Categories,
                Pane::Details => Pane::Skills,
            };
        }
        KeyCode::Char('j') | KeyCode::Down => {
            navigate_down(state);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            navigate_up(state);
        }
        KeyCode::Enter => {
            if let Some(skill) = state.current_skill() {
                state.selected = Some(skill.clone());
            }
        }
        KeyCode::Char('i') => {
            state.active_pane = Pane::Details;
        }
        _ => {}
    }
}

fn navigate_down(state: &mut BrowserState) {
    match state.active_pane {
        Pane::Categories => {
            let len = state.categories.len();
            if len > 0 {
                let i = state
                    .category_state
                    .selected()
                    .map(|i| (i + 1).min(len - 1))
                    .unwrap_or(0);
                state.category_state.select(Some(i));
                state.refilter();
            }
        }
        Pane::Skills => {
            let len = state.filtered_skills.len();
            if len > 0 {
                let i = state
                    .skill_state
                    .selected()
                    .map(|i| (i + 1).min(len - 1))
                    .unwrap_or(0);
                state.skill_state.select(Some(i));
            }
        }
        Pane::Details => {}
    }
}

fn navigate_up(state: &mut BrowserState) {
    match state.active_pane {
        Pane::Categories => {
            let len = state.categories.len();
            if len > 0 {
                let i = state
                    .category_state
                    .selected()
                    .map(|i| i.saturating_sub(1))
                    .unwrap_or(0);
                state.category_state.select(Some(i));
                state.refilter();
            }
        }
        Pane::Skills => {
            let len = state.filtered_skills.len();
            if len > 0 {
                let i = state
                    .skill_state
                    .selected()
                    .map(|i| i.saturating_sub(1))
                    .unwrap_or(0);
                state.skill_state.select(Some(i));
            }
        }
        Pane::Details => {}
    }
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

fn draw(frame: &mut Frame, state: &mut BrowserState) {
    let area = frame.area();

    // Main layout: header + body + footer
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(5),   // body
            Constraint::Length(3), // footer
        ])
        .split(area);

    draw_header(frame, main_layout[0], state);
    draw_body(frame, main_layout[1], state);
    draw_footer(frame, main_layout[2], state);
}

fn draw_header(frame: &mut Frame, area: Rect, state: &BrowserState) {
    let title = if state.search_mode {
        format!(" Skill Browser  |  Search: {}_ ", state.search_query)
    } else if !state.search_query.is_empty() {
        format!(
            " Skill Browser  |  Filter: \"{}\"  ({} skills) ",
            state.search_query,
            state.filtered_skills.len()
        )
    } else {
        format!(
            " Skill Browser  |  {} skills ",
            state.filtered_skills.len()
        )
    };

    let header = Paragraph::new(title)
        .style(Style::default().fg(Color::Cyan).bold())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(header, area);
}

fn draw_body(frame: &mut Frame, area: Rect, state: &mut BrowserState) {
    // 3-pane layout: left (20%) | center (50%) | right (30%)
    let body_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(50),
            Constraint::Percentage(30),
        ])
        .split(area);

    draw_categories(frame, body_layout[0], state);
    draw_skills(frame, body_layout[1], state);
    draw_details(frame, body_layout[2], state);
}

fn draw_categories(frame: &mut Frame, area: Rect, state: &mut BrowserState) {
    let is_active = state.active_pane == Pane::Categories;
    let border_color = if is_active {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let items: Vec<ListItem> = state
        .categories
        .iter()
        .map(|cat| {
            let style = if cat == "All" {
                Style::default().fg(Color::White).bold()
            } else {
                Style::default().fg(Color::Gray)
            };
            ListItem::new(Line::from(Span::styled(cat.as_str(), style)))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Categories ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .bold(),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut state.category_state);
}

fn draw_skills(frame: &mut Frame, area: Rect, state: &mut BrowserState) {
    let is_active = state.active_pane == Pane::Skills;
    let border_color = if is_active {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let items: Vec<ListItem> = state
        .filtered_skills
        .iter()
        .map(|&idx| {
            let skill = &state.all_skills[idx];

            let tier = skill
                .manifest
                .metadata
                .as_ref()
                .and_then(|m| m.trust_tier.as_ref())
                .map(|t| t.to_string())
                .unwrap_or_else(|| "local".to_string());

            let (tier_str, tier_color) = match tier.as_str() {
                "local" => ("[L]", Color::Green),
                "verified" => ("[V]", Color::Yellow),
                _ => ("[U]", Color::Red),
            };

            let count_str = skill
                .install_count
                .map(|c| format!(" ({c})"))
                .unwrap_or_default();

            let desc = if skill.description.len() > 40 {
                format!("{}...", &skill.description[..37])
            } else {
                skill.description.clone()
            };

            let line = Line::from(vec![
                Span::styled(tier_str, Style::default().fg(tier_color)),
                Span::raw(" "),
                Span::styled(&skill.name, Style::default().fg(Color::White).bold()),
                Span::styled(count_str, Style::default().fg(Color::DarkGray)),
                Span::raw("  "),
                Span::styled(desc, Style::default().fg(Color::Gray)),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Skills ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .bold(),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut state.skill_state);
}

fn draw_details(frame: &mut Frame, area: Rect, state: &mut BrowserState) {
    let is_active = state.active_pane == Pane::Details;
    let border_color = if is_active {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let content = if let Some(skill) = state.current_skill() {
        let name = skill.name.clone();
        let description = skill.description.clone();
        let source = skill.source.clone();

        let tier = skill
            .manifest
            .metadata
            .as_ref()
            .and_then(|m| m.trust_tier.as_ref())
            .map(|t| t.to_string())
            .unwrap_or_else(|| "local".to_string());

        let tier_color = match tier.as_str() {
            "local" => Color::Green,
            "verified" => Color::Yellow,
            _ => Color::Red,
        };

        let skill_type = skill
            .manifest
            .metadata
            .as_ref()
            .and_then(|m| m.skill_type.as_ref())
            .map(|t| format!("{:?}", t).to_lowercase())
            .unwrap_or_else(|| "prompt".to_string());

        let version = skill
            .manifest
            .metadata
            .as_ref()
            .and_then(|m| m.version.as_ref())
            .cloned()
            .unwrap_or_else(|| "-".to_string());

        let author = skill
            .manifest
            .metadata
            .as_ref()
            .and_then(|m| m.author.as_ref())
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());

        let capabilities = skill
            .manifest
            .metadata
            .as_ref()
            .and_then(|m| m.capabilities.as_ref())
            .map(|caps| {
                caps.iter()
                    .map(|c| format!("  {:?}", c))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_else(|| "  (none)".to_string());

        let categories = if skill.categories.is_empty() {
            "(none)".to_string()
        } else {
            skill.categories.join(", ")
        };

        let count_str = skill
            .install_count
            .map(|c| format!("{c}"))
            .unwrap_or_else(|| "-".to_string());

        vec![
            Line::from(Span::styled(
                name,
                Style::default().fg(Color::Cyan).bold(),
            )),
            Line::from(""),
            Line::from(Span::styled(
                description,
                Style::default().fg(Color::White),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("Type:     ", Style::default().fg(Color::DarkGray)),
                Span::styled(skill_type, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("Tier:     ", Style::default().fg(Color::DarkGray)),
                Span::styled(tier, Style::default().fg(tier_color)),
            ]),
            Line::from(vec![
                Span::styled("Version:  ", Style::default().fg(Color::DarkGray)),
                Span::styled(version, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("Author:   ", Style::default().fg(Color::DarkGray)),
                Span::styled(author, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("Source:   ", Style::default().fg(Color::DarkGray)),
                Span::styled(source, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("Installs: ", Style::default().fg(Color::DarkGray)),
                Span::styled(count_str, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("Tags:     ", Style::default().fg(Color::DarkGray)),
                Span::styled(categories, Style::default().fg(Color::White)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Capabilities:",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                capabilities,
                Style::default().fg(Color::White),
            )),
        ]
    } else {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                "No skill selected",
                Style::default().fg(Color::DarkGray),
            )),
        ]
    };

    let detail = Paragraph::new(content)
        .block(
            Block::default()
                .title(" Details ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(detail, area);
}

fn draw_footer(frame: &mut Frame, area: Rect, state: &BrowserState) {
    let help = if state.search_mode {
        " Type to search | Enter to apply | Esc to cancel "
    } else {
        " q/Esc quit | / search | Tab panes | j/k nav | Enter install | i details "
    };

    let footer = Paragraph::new(help)
        .style(Style::default().fg(Color::DarkGray))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(footer, area);
}
