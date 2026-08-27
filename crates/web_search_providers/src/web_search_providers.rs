mod cloud;
mod searxng;

use client::{Client, UserStore};
use gpui::{App, Context, Entity};
use language_model::LanguageModelRegistry;
use settings::{Settings, SettingsStore, WebSearchProviderContent};
use std::sync::Arc;
use web_search::{WebSearchProviderId, WebSearchRegistry};

pub use searxng::WebSearchSettings;

pub fn init(client: Arc<Client>, user_store: Entity<UserStore>, cx: &mut App) {
    let registry = WebSearchRegistry::global(cx);
    registry.update(cx, |registry, cx| {
        register_web_search_providers(registry, client, user_store, cx);
    });
}

fn register_web_search_providers(
    registry: &mut WebSearchRegistry,
    client: Arc<Client>,
    user_store: Entity<UserStore>,
    cx: &mut Context<WebSearchRegistry>,
) {
    apply_web_search_provider_selection(
        registry,
        client.clone(),
        user_store.clone(),
        &LanguageModelRegistry::global(cx),
        cx,
    );

    cx.subscribe(&LanguageModelRegistry::global(cx), {
        let client = client.clone();
        let user_store = user_store.clone();
        move |this, registry, event, cx| {
            if let language_model::Event::DefaultModelChanged = event {
                apply_web_search_provider_selection(
                    this,
                    client.clone(),
                    user_store.clone(),
                    &registry,
                    cx,
                )
            }
        }
    })
    .detach();

    cx.observe_global::<SettingsStore>(move |this, cx| {
        apply_web_search_provider_selection(
            this,
            client.clone(),
            user_store.clone(),
            &LanguageModelRegistry::global(cx),
            cx,
        );
    })
    .detach();
}

fn apply_web_search_provider_selection(
    registry: &mut WebSearchRegistry,
    client: Arc<Client>,
    user_store: Entity<UserStore>,
    language_model_registry: &Entity<LanguageModelRegistry>,
    cx: &mut Context<WebSearchRegistry>,
) {
    let settings = WebSearchSettings::get_global(cx);

    if settings.provider == WebSearchProviderContent::Searxng {
        let url = settings.searxng.url.trim();
        if !url.is_empty() {
            registry.unregister_provider(WebSearchProviderId(
                cloud::ZED_WEB_SEARCH_PROVIDER_ID.into(),
            ));
            registry.set_active_provider(Arc::new(searxng::SearxngWebSearchProvider::new(
                client.http_client(),
                url.to_string(),
                settings.searxng.auth_username.clone(),
                settings.searxng.auth_password.clone(),
            )));
            return;
        }

        log::warn!(
            "web_search.provider is \"searxng\" but web_search.searxng.url is missing or empty; falling back to the Zed Cloud provider"
        );
    }

    registry.unregister_provider(WebSearchProviderId(
        searxng::SEARXNG_WEB_SEARCH_PROVIDER_ID.into(),
    ));
    register_zed_web_search_provider(registry, client, user_store, language_model_registry, cx);
}

fn register_zed_web_search_provider(
    registry: &mut WebSearchRegistry,
    client: Arc<Client>,
    user_store: Entity<UserStore>,
    language_model_registry: &Entity<LanguageModelRegistry>,
    cx: &mut Context<WebSearchRegistry>,
) {
    let using_zed_provider = language_model_registry
        .read(cx)
        .default_model()
        .is_some_and(|default| default.is_provided_by_zed());
    if using_zed_provider {
        registry.register_provider(
            cloud::CloudWebSearchProvider::new(client, user_store, cx),
            cx,
        )
    } else {
        registry.unregister_provider(WebSearchProviderId(
            cloud::ZED_WEB_SEARCH_PROVIDER_ID.into(),
        ));
    }
}
