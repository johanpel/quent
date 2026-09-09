// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Utilities for server implementations

use crate::{analyzer_cache::AnalyzerCache, state::ServiceState, timeline_cache::TimelineCache};
use axum::Router as AxumRouter;
use quent_collector::{CollectorServer, server::CollectorService};
use quent_query_engine_analyzer::ui::UiAnalyzer;

use tonic::transport::{Server as GrpcServer, server::Router};
use tower_http::cors::CorsLayer;
use uuid::Uuid;

pub mod analyzer_cache;
pub mod error;
mod state;
mod timeline_cache;
mod ui;

pub fn initialize_tracing(log_level: &str) {
    use tracing_subscriber::{
        EnvFilter,
        fmt::{self, format::FmtSpan},
        layer::SubscriberExt,
        registry,
        util::SubscriberInitExt,
    };
    registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(format!("{log_level},h2=off,tonic=off"))),
        )
        .with(
            fmt::layer()
                .with_target(true)
                .with_span_events(FmtSpan::CLOSE)
                .with_writer(std::io::stderr),
        )
        .init();
}

/// Builds a collector service with a hard incoming gRPC message-size limit.
///
/// Events in a message larger than `max_decoding_message_size` are dropped.
/// Later batches queued on the same stream may also be dropped because the
/// current client does not retry rejected streams.
pub fn collector_service<C, F>(
    make: F,
    max_decoding_message_size: usize,
) -> Result<Router, Box<dyn std::error::Error>>
where
    C: quent_collector::CollectorSink + Send + Sync + 'static,
    F: Fn(Uuid) -> Result<C, String> + Send + Sync + 'static,
{
    let collector = CollectorService::<C>::new(make);
    Ok(GrpcServer::builder().add_service(
        CollectorServer::new(collector).max_decoding_message_size(max_decoding_message_size),
    ))
}

pub fn analyzer_service_router<A>(
    importer: Box<analyzer_cache::ImporterFn<A>>,
    lister: Box<analyzer_cache::ListerFn>,
    cors: Option<String>,
) -> Result<AxumRouter, Box<dyn std::error::Error>>
where
    A: UiAnalyzer + Send + Sync + 'static,
    <A as UiAnalyzer>::EntityRef: serde::Serialize,
{
    analyzer_service_router_with_routes::<A>(importer, lister, cors, AxumRouter::new())
}

/// Build the analyzer router and merge integration-owned routes before common
/// CORS and embedded-UI fallback layers are installed.
pub fn analyzer_service_router_with_routes<A>(
    importer: Box<analyzer_cache::ImporterFn<A>>,
    lister: Box<analyzer_cache::ListerFn>,
    cors: Option<String>,
    additional_routes: AxumRouter,
) -> Result<AxumRouter, Box<dyn std::error::Error>>
where
    A: UiAnalyzer + Send + Sync + 'static,
    <A as UiAnalyzer>::EntityRef: serde::Serialize,
{
    let state = ServiceState {
        analyzers: AnalyzerCache::<A>::new(importer, lister),
        timelines: TimelineCache::new(),
    };

    let mut http_routes = axum::Router::new()
        .nest("/api/engines", ui::routes(state))
        .merge(additional_routes);

    #[cfg(feature = "swagger")]
    {
        use utoipa::OpenApi;
        use utoipa_swagger_ui::SwaggerUi;
        let api = ui::ApiDoc::openapi();
        http_routes =
            http_routes.merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api));
    }

    if let Some(cors) = cors {
        let cors = CorsLayer::new()
            .allow_origin(cors.parse::<axum::http::HeaderValue>().unwrap())
            .allow_methods([
                axum::http::Method::GET,
                axum::http::Method::POST,
                axum::http::Method::OPTIONS,
            ])
            .allow_headers([axum::http::header::CONTENT_TYPE]);
        http_routes = http_routes.layer(cors);
    }

    #[cfg(feature = "ui")]
    {
        http_routes = http_routes.fallback(axum::routing::get(ui::embedded::serve));
    }

    Ok(http_routes)
}
