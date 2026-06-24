// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Generation of per-entity observers — the cheap-clone factories for handles.

use convert_case::Case;
use proc_macro2::TokenStream;
use quent_schema::Entity;
use quote::quote;

use super::{event_ident, handle_ident, observer_ident};
use crate::common::to_case;

/// The `{Entity}Observer`: an `Arc`-shared, cloneable factory that mints
/// per-instance handles.
pub(super) fn entity_observer(entity: &Entity) -> TokenStream {
    let entity_pascal = to_case(entity.name(), Case::Pascal);
    let event_ty = event_ident(entity);
    let observer_ty = observer_ident(entity);
    let handle_ty = handle_ident(entity);

    let observer_doc = format!(
        "Observer for `{entity_pascal}` entities. Obtain a per-instance handle \
         with [`Self::handle`]."
    );
    let handle_fn_doc = format!("Create a handle for a fresh `{entity_pascal}` instance.");
    let handle_with_id_doc =
        format!("Create a handle for the `{entity_pascal}` instance identified by `id`.");

    quote! {
        #[doc = #observer_doc]
        #[derive(Clone)]
        pub struct #observer_ty {
            inner: ::std::sync::Arc<::quent_instrumentation_runtime::Observer<#event_ty>>,
        }

        impl #observer_ty {
            #[doc = #handle_fn_doc]
            pub fn handle(&self) -> #handle_ty {
                #handle_ty {
                    inner: ::quent_instrumentation_runtime::Handle::new(
                        ::core::clone::Clone::clone(&self.inner),
                    ),
                }
            }

            #[doc = #handle_with_id_doc]
            pub fn handle_with_id(&self, id: ::uuid::Uuid) -> #handle_ty {
                #handle_ty {
                    inner: ::quent_instrumentation_runtime::Handle::with_id(
                        id,
                        ::core::clone::Clone::clone(&self.inner),
                    ),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::pretty;
    use quent_schema::test_utils::entity;

    #[test]
    fn observer_is_a_cloneable_handle_factory() {
        // The observer is independent of the entity's events.
        let e = entity("Connection", []);
        let expected = quote! {
            #[doc = "Observer for `Connection` entities. Obtain a per-instance handle with [`Self::handle`]."]
            #[derive(Clone)]
            pub struct ConnectionObserver {
                inner: ::std::sync::Arc<::quent_instrumentation_runtime::Observer<ConnectionEvent>>,
            }
            impl ConnectionObserver {
                #[doc = "Create a handle for a fresh `Connection` instance."]
                pub fn handle(&self) -> ConnectionHandle {
                    ConnectionHandle {
                        inner: ::quent_instrumentation_runtime::Handle::new(
                            ::core::clone::Clone::clone(&self.inner),
                        ),
                    }
                }
                #[doc = "Create a handle for the `Connection` instance identified by `id`."]
                pub fn handle_with_id(&self, id: ::uuid::Uuid) -> ConnectionHandle {
                    ConnectionHandle {
                        inner: ::quent_instrumentation_runtime::Handle::with_id(
                            id,
                            ::core::clone::Clone::clone(&self.inner),
                        ),
                    }
                }
            }
        };
        assert_eq!(pretty(entity_observer(&e)), pretty(expected));
    }
}
