// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Select the generated wrapper contract for a pinned Quent revision.

use std::path::Path;

use proc_macro2::TokenStream;
use quote::quote;

use crate::error::Result;
use crate::revision;
use crate::spec::ViewerSpec;

/// Cargo package of Quent's current I/O crate.
pub(crate) const IO_PACKAGE: &str = "quent-io";
/// Cargo package used before [`IO_PACKAGE`] was introduced.
pub(crate) const LEGACY_IO_PACKAGE: &str = "quent-exporter";
/// Cargo package that provides the optional NVTX HTTP routes.
pub(crate) const NVTX_SERVER_PACKAGE: &str = "nvtx-server";

/// Boundary whose descendants provide the `quent-io` package.
const IO_PACKAGE_BOUNDARY: &str = "aa1e9b1b394f5f978215b69cd5c526291c4b4723";
/// Boundary whose descendants provide NVTX routes and the extensible analyzer router.
const NVTX_ROUTES_BOUNDARY: &str = "f40e69c2d4405c765c6270221e2a58e58ef704a6";
/// Last `upstream/main` revision before context-inventory indexing was introduced.
///
/// PR #699 is required to be the next change merged after this revision. Only strict
/// descendants use context-inventory indexing; this revision, its ancestors, and
/// revisions on older branches use query-engine indexing.
const CONTEXT_INVENTORY_PREDECESSOR: &str = "5e6818e89ccde0a41dc08a12819938a093969665";
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NvtxRoutes {
    Enabled,
    Disabled,
}

impl NvtxRoutes {
    pub(crate) fn server_package(self) -> Option<&'static str> {
        (self == Self::Enabled).then_some(NVTX_SERVER_PACKAGE)
    }

    pub(crate) fn code(self) -> NvtxCode {
        match self {
            Self::Enabled => NvtxCode {
                imports: quote! {
                    use quent_query_engine_server::analyzer_service_router_with_routes;
                    use nvtx_server::{import_context_events, routes as nvtx_routes};
                },
                setup: quote! {
                    let nvtx_root = root.clone();
                    let nvtx_importer = move |id: uuid::Uuid| {
                        import_context_events(&nvtx_root, id)
                    };
                },
                router: quote! {
                    analyzer_service_router_with_routes::<Analyzer>(
                        Box::new(importer),
                        Box::new(lister),
                        None,
                        nvtx_routes(Box::new(nvtx_importer)),
                    )
                },
            },
            Self::Disabled => NvtxCode {
                imports: quote! {
                    use quent_query_engine_server::analyzer_service_router;
                },
                setup: quote! {},
                router: quote! {
                    analyzer_service_router::<Analyzer>(Box::new(importer), Box::new(lister), None)
                },
            },
        }
    }
}

pub(crate) struct NvtxCode {
    pub(crate) imports: TokenStream,
    pub(crate) setup: TokenStream,
    pub(crate) router: TokenStream,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ContextIndexing {
    QueryEngines,
    ContextInventory,
}

impl ContextIndexing {
    pub(crate) fn code(self) -> ContextIndexingCode {
        match self {
            Self::QueryEngines => ContextIndexingCode {
                import: quote! {
                    use quent_query_engine_server::analyzer_cache::index_query_engines;
                },
                lister: quote! {
                    let lister_root = root.clone();
                    let lister = move || index_query_engines(&lister_root);
                },
            },
            Self::ContextInventory => ContextIndexingCode {
                import: quote! {
                    use quent_query_engine_server::analyzer_cache::index_contexts;
                },
                lister: quote! {
                    let lister_root = root.clone();
                    let lister = move || {
                        index_contexts(&lister_root, |id| {
                            Ok(<Viewer as QuentViewer>::context_inventory(
                                &lister_root.join(id.to_string()),
                            )?)
                        })
                    };
                },
            },
        }
    }
}

pub(crate) struct ContextIndexingCode {
    pub(crate) import: TokenStream,
    pub(crate) lister: TokenStream,
}

pub(crate) struct WrapperCompatibility {
    pub(crate) nvtx_routes: NvtxRoutes,
    pub(crate) io_package: &'static str,
    pub(crate) context_indexing: ContextIndexing,
}

impl WrapperCompatibility {
    pub(crate) async fn resolve(repository: &Path, spec: &ViewerSpec) -> Result<Self> {
        let revision = revision::PinnedRevision::fetch(repository, &spec.quent).await?;
        let nvtx_routes = if revision.contains(NVTX_ROUTES_BOUNDARY).await? {
            NvtxRoutes::Enabled
        } else {
            NvtxRoutes::Disabled
        };
        let io_package = if revision.contains(IO_PACKAGE_BOUNDARY).await? {
            IO_PACKAGE
        } else {
            LEGACY_IO_PACKAGE
        };
        let context_indexing = if revision
            .is_strict_descendant_of(CONTEXT_INVENTORY_PREDECESSOR)
            .await?
        {
            ContextIndexing::ContextInventory
        } else {
            ContextIndexing::QueryEngines
        };
        Ok(Self {
            nvtx_routes,
            io_package,
            context_indexing,
        })
    }
}
