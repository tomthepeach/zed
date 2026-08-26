use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use settings_macros::{MergeFrom, with_fallible_options};

/// Which backend powers the agent's `search_web` tool.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, MergeFrom,
)]
#[serde(rename_all = "snake_case")]
pub enum WebSearchProviderContent {
    /// Zed Cloud web search (default).
    #[default]
    Zed,
    /// A self-hosted SearXNG instance.
    Searxng,
}

#[with_fallible_options]
#[derive(Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema, MergeFrom, Debug)]
pub struct WebSearchSettingsContent {
    /// Which web search provider to use.
    ///
    /// Default: "zed"
    pub provider: Option<WebSearchProviderContent>,
    /// Settings for the SearXNG provider.
    pub searxng: Option<SearxngWebSearchSettingsContent>,
}

#[with_fallible_options]
#[derive(Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema, MergeFrom, Debug)]
pub struct SearxngWebSearchSettingsContent {
    /// Base URL of the SearXNG instance (e.g. "http://localhost:8080").
    pub url: Option<String>,
    /// Optional HTTP Basic Auth username. Sent only when both username and password are set.
    pub auth_username: Option<String>,
    /// Optional HTTP Basic Auth password. Sent only when both username and password are set.
    pub auth_password: Option<String>,
}
