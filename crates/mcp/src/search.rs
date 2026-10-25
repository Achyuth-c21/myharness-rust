//! MCP tool search — roadmap Phase 5 (`docs/12` §5.1; `docs/features/
//! mcp-skills-tools.md` §5).
//!
//! A server can expose hundreds of tools; the app must not inject all of them
//! into every request. Search returns **metadata and schema only**. Selecting
//! a result is not an approval and does not grant execution; the session's
//! frozen [`ToolSnapshot`](crate::ToolSnapshot) and the tool policy still
//! decide whether a call can run.
//!
//! The shipped ranking/scoring was not observed (`docs/14` §9). This is a
//! rebuild-defined, deterministic score: exact name, name prefix, name
//! substring, then title/description match, ties by namespaced name.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::tool::ToolDescriptor;

/// Default result count for one search.
pub const DEFAULT_SEARCH_LIMIT: usize = 20;

/// Hard result bound; a caller cannot ask for an unbounded catalog.
pub const MAX_SEARCH_LIMIT: usize = 100;

/// One search hit. `cached` means the schema came from the on-disk cache and
/// has no live session behind it yet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSearchHit {
    /// Namespaced descriptor.
    pub tool: ToolDescriptor,
    /// Rebuild-defined relevance score (higher is better).
    pub score: u32,
    /// True when only the schema cache supplied this entry.
    pub cached: bool,
}

/// Search a catalog. `disabled` is checked first and always wins: a disabled
/// tool remains unavailable even when the query matches it exactly.
pub fn search_tools(
    tools: &[ToolDescriptor],
    query: &str,
    limit: usize,
    disabled: &BTreeSet<String>,
    cached: bool,
) -> Vec<ToolSearchHit> {
    search_catalogs(tools, &[], query, limit, disabled)
        .into_iter()
        .map(|mut hit| {
            hit.cached = cached;
            hit
        })
        .collect()
}

/// Search live and cached catalogs together. Live entries win on a duplicate
/// name (the caller should already have removed them); the merged ranking is
/// stable and bounded by [`MAX_SEARCH_LIMIT`].
pub fn search_catalogs(
    live: &[ToolDescriptor],
    cached: &[ToolDescriptor],
    query: &str,
    limit: usize,
    disabled: &BTreeSet<String>,
) -> Vec<ToolSearchHit> {
    let limit = limit.clamp(1, MAX_SEARCH_LIMIT);
    let query = query.trim().to_lowercase();
    let tokens: Vec<&str> = query.split_whitespace().collect();
    let mut hits: Vec<ToolSearchHit> = Vec::new();
    for (tools, is_cached) in [(live, false), (cached, true)] {
        for tool in tools {
            if is_disabled(&tool.name, disabled) {
                continue;
            }
            if let Some(score) = score(tool, &query, &tokens) {
                hits.push(ToolSearchHit {
                    tool: tool.clone(),
                    score,
                    cached: is_cached,
                });
            }
        }
    }
    hits.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.cached.cmp(&right.cached))
            .then_with(|| left.tool.name.cmp(&right.tool.name))
    });
    hits.truncate(limit);
    hits
}

fn is_disabled(name: &str, disabled: &BTreeSet<String>) -> bool {
    if disabled.contains(name) {
        return true;
    }
    let server = name.split_once(':').map(|(server, _)| server);
    server.is_some_and(|server| disabled.contains(&format!("{server}:*")))
}

fn score(tool: &ToolDescriptor, query: &str, tokens: &[&str]) -> Option<u32> {
    if query.is_empty() {
        return Some(0);
    }
    let name = tool.name.to_lowercase();
    let short_name = tool.tool.to_lowercase();
    let title = tool.title.as_deref().unwrap_or("").to_lowercase();
    let description = tool.description.as_deref().unwrap_or("").to_lowercase();

    let mut score = 0;
    if name == query || short_name == query {
        score += 100;
    } else if name.starts_with(query) || short_name.starts_with(query) {
        score += 80;
    } else if name.contains(query) || short_name.contains(query) {
        score += 60;
    }
    if title.contains(query) {
        score += 30;
    }
    if description.contains(query) {
        score += 20;
    }
    // Every token must match at least one searchable field; this prevents a
    // broad query from returning an unrelated catalog.
    if !tokens
        .iter()
        .all(|token| name.contains(token) || title.contains(token) || description.contains(token))
    {
        return None;
    }
    if score == 0 {
        // Token-only matches are still useful, but rank below a full query.
        score = 10;
    }
    Some(score)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tool(name: &str, description: &str) -> ToolDescriptor {
        let (server, short) = name.split_once(':').unwrap();
        ToolDescriptor {
            name: name.into(),
            server: server.into(),
            tool: short.into(),
            title: None,
            description: Some(description.into()),
            input_schema: json!({"type": "object"}),
            output_schema: None,
        }
    }

    #[test]
    fn an_exact_name_ranks_first() {
        let tools = vec![
            tool("github:issues", "list issues"),
            tool("github:issues_comments", "comments"),
        ];
        let hits = search_tools(&tools, "github:issues", 20, &BTreeSet::new(), false);
        assert_eq!(hits[0].tool.name, "github:issues");
        assert!(hits[0].score > hits[1].score);
    }

    #[test]
    fn disabled_tools_never_appear_even_on_an_exact_match() {
        let tools = vec![tool("github:issues", "list issues")];
        let mut disabled = BTreeSet::new();
        disabled.insert("github:issues".to_string());
        assert!(search_tools(&tools, "github:issues", 20, &disabled, false).is_empty());
    }

    #[test]
    fn a_server_wildcard_disables_the_whole_server() {
        let tools = vec![tool("github:issues", "list issues")];
        let mut disabled = BTreeSet::new();
        disabled.insert("github:*".to_string());
        assert!(search_tools(&tools, "issues", 20, &disabled, false).is_empty());
    }

    #[test]
    fn the_limit_is_bounded() {
        let tools: Vec<_> = (0..150)
            .map(|index| tool(&format!("s:t{index}"), "fixture"))
            .collect();
        let hits = search_tools(&tools, "", 1_000, &BTreeSet::new(), false);
        assert_eq!(hits.len(), MAX_SEARCH_LIMIT);
    }

    #[test]
    fn a_multi_token_query_requires_every_token() {
        let tools = vec![tool("github:issues", "list repository issues")];
        assert_eq!(
            search_tools(&tools, "github issues", 20, &BTreeSet::new(), false).len(),
            1
        );
        assert!(search_tools(&tools, "github pull", 20, &BTreeSet::new(), false).is_empty());
    }

    #[test]
    fn cache_hits_are_labelled() {
        let tools = vec![tool("github:issues", "list issues")];
        let hits = search_tools(&tools, "issues", 20, &BTreeSet::new(), true);
        assert!(hits[0].cached);
    }
}
