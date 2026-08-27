use std::sync::Arc;

use anyhow::{Context as _, Result, anyhow, bail};
use base64::Engine as _;
use cloud_llm_client::{WebSearchResponse, WebSearchResult};
use futures::AsyncReadExt as _;
use gpui::{App, AppContext as _, Task};
use http_client::{HttpClient, HttpClientWithUrl, Method, Url};
use serde::Deserialize;
use settings::{RegisterSetting, Settings, SettingsContent, WebSearchProviderContent};
use web_search::{WebSearchProvider, WebSearchProviderId};

#[derive(Clone, Debug, PartialEq, RegisterSetting)]
pub struct WebSearchSettings {
    pub provider: WebSearchProviderContent,
    pub searxng: SearxngSettings,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct SearxngSettings {
    pub url: String,
    pub auth_username: Option<String>,
    pub auth_password: Option<String>,
}

impl Settings for WebSearchSettings {
    fn from_settings(content: &SettingsContent) -> Self {
        let web_search = content.web_search.clone().unwrap();
        let searxng = web_search.searxng.unwrap_or_default();
        Self {
            provider: web_search.provider.unwrap_or_default(),
            searxng: SearxngSettings {
                url: searxng.url.unwrap_or_default(),
                auth_username: searxng.auth_username.filter(|value| !value.is_empty()),
                auth_password: searxng.auth_password.filter(|value| !value.is_empty()),
            },
        }
    }
}

pub const SEARXNG_WEB_SEARCH_PROVIDER_ID: &str = "searxng";

pub struct SearxngWebSearchProvider {
    http_client: Arc<HttpClientWithUrl>,
    base_url: String,
    auth_username: Option<String>,
    auth_password: Option<String>,
}

impl SearxngWebSearchProvider {
    pub fn new(
        http_client: Arc<HttpClientWithUrl>,
        base_url: String,
        auth_username: Option<String>,
        auth_password: Option<String>,
    ) -> Self {
        Self {
            http_client,
            base_url,
            auth_username,
            auth_password,
        }
    }
}

impl WebSearchProvider for SearxngWebSearchProvider {
    fn id(&self) -> WebSearchProviderId {
        WebSearchProviderId(SEARXNG_WEB_SEARCH_PROVIDER_ID.into())
    }

    fn search(&self, query: String, cx: &mut App) -> Task<Result<WebSearchResponse>> {
        let http_client = self.http_client.clone();
        let base_url = self.base_url.clone();
        let auth_username = self.auth_username.clone();
        let auth_password = self.auth_password.clone();
        cx.background_spawn(async move {
            perform_searxng_search(http_client, base_url, auth_username, auth_password, query).await
        })
    }
}

#[derive(Debug, Deserialize)]
struct SearxngResponse {
    #[serde(default)]
    results: Vec<SearxngResult>,
}

#[derive(Debug, Deserialize)]
struct SearxngResult {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    content: String,
}

async fn perform_searxng_search(
    http_client: Arc<HttpClientWithUrl>,
    base_url: String,
    auth_username: Option<String>,
    auth_password: Option<String>,
    query: String,
) -> Result<WebSearchResponse> {
    let base_url = base_url.trim_end_matches('/');
    let mut url = Url::parse(&format!("{base_url}/search"))
        .with_context(|| format!("invalid SearXNG URL: {base_url}"))?;
    url.query_pairs_mut()
        .append_pair("q", &query)
        .append_pair("format", "json");

    let mut request = http_client::Request::builder()
        .method(Method::GET)
        .uri(url.as_str());

    if let (Some(username), Some(password)) = (&auth_username, &auth_password) {
        let credentials =
            base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
        request = request.header("Authorization", format!("Basic {credentials}"));
    }

    let request = request
        .header("Accept", "application/json")
        .body(http_client::AsyncBody::default())
        .context("failed to build SearXNG request")?;

    let mut response = http_client
        .send(request)
        .await
        .context("failed to contact SearXNG instance")?;

    let status = response.status();
    let mut body = String::new();
    response
        .body_mut()
        .read_to_string(&mut body)
        .await
        .context("failed to read SearXNG response body")?;

    if !status.is_success() {
        let hint = match status.as_u16() {
            401 => " Check that auth_username and auth_password are correct.",
            403 | 404 => {
                " The SearXNG JSON API may be disabled on this instance (enable format=json)."
            }
            _ => "",
        };
        bail!("SearXNG search failed with status {status}.{hint}\nBody: {body}");
    }

    let trimmed = body.trim_start();
    if trimmed.starts_with('<') {
        bail!(
            "SearXNG returned HTML instead of JSON. The JSON API is likely disabled on this instance; enable `format=json` in the SearXNG settings."
        );
    }

    let parsed: SearxngResponse = serde_json::from_str(&body).map_err(|error| {
        anyhow!(
            "failed to parse SearXNG JSON response ({error}). The JSON API may be disabled on this instance (enable format=json).\nBody: {body}"
        )
    })?;

    Ok(WebSearchResponse {
        results: parsed
            .results
            .into_iter()
            .map(|result| WebSearchResult {
                title: result.title,
                url: result.url,
                text: result.content,
            })
            .collect(),
    })
}
