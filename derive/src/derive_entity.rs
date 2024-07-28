use std::collections::HashMap;

use convert_case::{Case, Casing};
use darling::{FromDeriveInput, FromField};
use derive_more::Constructor;
use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_quote, Data, DeriveInput, Fields, Ident, Type};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum DeriveEntityError {
    #[error("DarlingError: {0}")]
    DarlingError(#[from] darling::Error),

    #[error("attribute must be applied to a struct")]
    StructRequired,

    #[error("attribute must be applied to a field")]
    FieldRequired,

    #[error("a key must be named, either explicitly or on a named field")]
    MissingKeyName,
}

#[derive(Debug, Constructor)]
pub(crate) struct DeriveEntity {
    pub entity: Ident,
    pub snake_name: Option<Ident>,
    pub table_name: String,
    pub _columns: Vec<FieldColumn>,
    pub keys: Vec<Key>,
}

impl DeriveEntity {
    pub fn entity_snake_name(&self) -> Ident {
        let name = self
            .snake_name
            .as_ref()
            .unwrap_or(&self.entity)
            .to_string()
            .to_case(Case::Snake);
        format_ident!("{}", name)
    }
}

#[derive(Debug, Default, Constructor)]
pub(crate) struct Key {
    pub name: String,
    pub unique: bool,
    pub components: Vec<FieldColumn>,
}

#[derive(Debug, Constructor, Clone)]
pub(crate) struct FieldColumn {
    pub field_name: Ident,
    pub field_type: Type,
    pub column_name: String,
}

impl TryFrom<DeriveInput> for DeriveEntity {
    type Error = DeriveEntityError;

    fn try_from(derive_input: DeriveInput) -> Result<Self, Self::Error> {
        let ent_struct = if let Data::Struct(ent_struct) = &derive_input.data {
            Ok(ent_struct)
        } else {
            Err(DeriveEntityError::StructRequired)
        }?;

        let args = args::DeriveInputArgs::from_derive_input(&derive_input)?;

        let fields = &ent_struct.fields;
        let field_columns = field_columns(fields)?;

        let pks = fields
            .iter()
            .map(|f| {
                let is_key = f.attrs.iter().any(|a| {
                    // if the attrs don't have a 'key' ident, stop
                    let path = a.path();
                    path.get_ident()
                        .map(|i| &i.to_string() == "key")
                        .unwrap_or(false)
                });
                if !is_key {
                    return Ok(None);
                }

                let f_ident = f.ident.clone().ok_or(DeriveEntityError::MissingKeyName)?;
                let key = args::Key::from_field(f).map_err(DeriveEntityError::from)?;
                let field_column = field_columns[&f_ident].clone();
                let key_name = key.name.unwrap_or_else(|| f_ident.to_string());
                let key_unique = key.unique.unwrap_or(false);
                Ok(Some((key_name, (field_column, key_unique))))
            })
            .collect::<Result<Vec<Option<(String, (FieldColumn, bool))>>, DeriveEntityError>>()?
            .iter()
            .flatten()
            .cloned()
            .collect_vec();

        let keys = utilities::iterable::index(pks)
            .into_iter()
            .map(|(k, v)| {
                let components = v.iter().map(|(fc, _)| fc.clone()).collect_vec();
                // assumption: only one key in the named key needs to be marked unique
                let unique = v.iter().any(|(_, u)| *u);
                Key::new(k, unique, components)
            })
            .collect_vec();

        let columns = field_columns.into_values().collect_vec();

        let table_name = args
            .table_name
            .unwrap_or_else(|| derive_input.ident.to_string());

        Ok(DeriveEntity::new(
            derive_input.ident,
            args.name,
            table_name,
            columns,
            keys,
        ))
    }
}

fn field_columns(fields: &Fields) -> Result<HashMap<Ident, FieldColumn>, DeriveEntityError> {
    fields
        .iter()
        .try_fold(HashMap::default(), |mut mappings, field| {
            let field_name = field
                .ident
                .as_ref()
                .ok_or(DeriveEntityError::FieldRequired)?;

            let field_type = field.ty.clone();

            let column = args::Column::from_field(field).ok();

            let field_column = FieldColumn::new(
                field_name.clone(),
                field_type,
                column
                    .map(|c| c.name)
                    .unwrap_or_else(|| field_name.to_string()),
            );

            mappings.insert(field_name.clone(), field_column);
            let result: Result<HashMap<Ident, FieldColumn>, DeriveEntityError> = Ok(mappings);

            result
        })
}

impl TryFrom<DeriveEntity> for TokenStream {
    type Error = DeriveEntityError;

    fn try_from(entity: DeriveEntity) -> Result<Self, Self::Error> {
        let repo_trait = repo_trait(&entity)?;

        let pgpool_ty: Type = parse_quote!(sqlx::Pool<sqlx::Postgres>);
        let pgpool_impl = pg_impl(&entity, pgpool_ty)?;

        let tx_impl = tx_impl(&entity)?;

        Ok(quote! {
            #repo_trait

            #pgpool_impl

            #tx_impl
        })
    }
}

struct KeyFn {
    _ent: Ident,
    fn_name: Ident,
    fn_rtn: proc_macro2::TokenStream,
    fn_args: Vec<proc_macro2::TokenStream>,
}

impl KeyFn {
    fn new(entity: &DeriveEntity, key: &Key) -> Self {
        let ent = entity.entity.clone();
        let snake_ent = entity.entity_snake_name();

        let fn_name = if key.unique {
            format_ident!("find_{}_by_{}", snake_ent, key.name)
        } else {
            format_ident!("list_{}_by_{}", snake_ent, key.name)
        };

        let fn_rtn = if key.unique {
            quote! {
                Result<Option<#ent>, sqlx::Error>
            }
        } else {
            quote! {
                Result<Vec<#ent>, sqlx::Error>
            }
        };

        fn map_type(ty: &Type) -> Type {
            match ty {
                Type::Path(p)
                    if p.path
                        .get_ident()
                        .map(Ident::to_string)
                        .is_some_and(|s| s.ends_with("String")) =>
                {
                    let str_ty: Type = parse_quote!(str);
                    str_ty
                }
                _ => ty.clone(),
            }
        }

        let fn_args = key
            .components
            .iter()
            .map(|c| {
                let name = &c.field_name;
                let ty = map_type(&c.field_type);
                quote! {
                    #name: &#ty
                }
            })
            .collect_vec();

        Self {
            _ent: ent,
            fn_name,
            fn_rtn,
            fn_args,
        }
    }
}

struct EntityImpl {
    trait_name: Ident,
}

impl EntityImpl {
    fn new(entity: &DeriveEntity) -> Self {
        let trait_name = format_ident!("{}Repo", entity.entity);
        Self { trait_name }
    }
}

fn repo_trait(entity: &DeriveEntity) -> Result<TokenStream, DeriveEntityError> {
    let EntityImpl { trait_name } = EntityImpl::new(entity);
    let key_fns = entity
        .keys
        .iter()
        .map(|key| {
            let KeyFn {
                fn_name,
                fn_rtn,
                fn_args,
                ..
            } = KeyFn::new(entity, key);

            quote! {
                async fn #fn_name(&self, #(#fn_args), *) -> #fn_rtn;
            }
        })
        .collect_vec();

    Ok(quote! {
        pub trait #trait_name {
            #(
                #key_fns
            )*
        }
    })
}

fn pg_impl(entity: &DeriveEntity, impl_ty: Type) -> Result<TokenStream, DeriveEntityError> {
    let EntityImpl { trait_name } = EntityImpl::new(entity);

    let key_fns = entity
        .keys
        .iter()
        .map(|key| {
            let KeyFn {
                fn_name,
                fn_rtn,
                fn_args,
                ..
            } = KeyFn::new(entity, key);

            let where_clause = key
                .components
                .iter()
                .enumerate()
                .map(|(i, c)| format!("{} = ${}", c.column_name, i + 1))
                .join(" and ");

            let query = format!("select * from {} where {}", entity.table_name, where_clause);

            let binds = key
                .components
                .iter()
                .map(|c| {
                    let f = &c.field_name;
                    quote! {
                        .bind(&#f)
                    }
                })
                .collect_vec();

            let body = if key.unique {
                quote! {
                    sqlx::query_as(#query)
                    #(
                        #binds
                    )*
                    .fetch_optional(self)
                    .await
                }
            } else {
                quote! {
                    sqlx::query_as(#query)
                    #(
                        #binds
                    )*
                    .fetch_all(self)
                    .await
                }
            };

            quote! {
                async fn #fn_name(&self, #(#fn_args), *) -> #fn_rtn {
                    #body
                }
            }
        })
        .collect_vec();

    Ok(quote! {
        impl #trait_name for #impl_ty {
            #(
                #key_fns
            )*
        }
    })
}

fn tx_impl(entity: &DeriveEntity) -> Result<TokenStream, DeriveEntityError> {
    let EntityImpl { trait_name } = EntityImpl::new(entity);

    let key_fns = entity
        .keys
        .iter()
        .map(|key| {
            let KeyFn {
                fn_name,
                fn_rtn,
                fn_args,
                ..
            } = KeyFn::new(entity, key);

            let where_clause = key
                .components
                .iter()
                .enumerate()
                .map(|(i, c)| format!("{} = ${}", c.column_name, i + 1))
                .join(" and ");

            let query = format!("select * from {} where {}", entity.table_name, where_clause);

            let binds = key
                .components
                .iter()
                .map(|c| {
                    let f = &c.field_name;
                    quote! {
                        .bind(&#f)
                    }
                })
                .collect_vec();

            let body = if key.unique {
                quote! {
                    let mut tx = self.lock().await;
                    sqlx::query_as(#query)
                    #(
                        #binds
                    )*
                    .fetch_optional(&mut **tx)
                    .await
                }
            } else {
                quote! {
                    let mut tx = self.lock().await;
                    sqlx::query_as(#query)
                    #(
                        #binds
                    )*
                    .fetch_all(&mut **tx)
                    .await
                }
            };

            quote! {
                async fn #fn_name(&self, #(#fn_args), *) -> #fn_rtn {
                    #body
                }
            }
        })
        .collect_vec();

    Ok(quote! {
        impl #trait_name for tokio::sync::Mutex<sqlx::Transaction<'_, sqlx::Postgres>> {
            #(
                #key_fns
            )*
        }
    })}

pub(super) mod args {
    use darling::{FromDeriveInput, FromField};
    use syn::Ident;

    #[derive(Debug, FromDeriveInput)]
    #[darling(attributes(entity), supports(struct_named))]
    pub(crate) struct DeriveInputArgs {
        #[darling(default)]
        pub name: Option<Ident>,

        #[darling(default)]
        pub table_name: Option<String>,
    }

    #[derive(Debug, FromField)]
    #[darling(attributes(key))]
    pub(crate) struct Key {
        pub name: Option<String>,

        #[darling(default)]
        pub unique: Option<bool>,
    }

    #[derive(Debug, FromField)]
    #[darling(attributes(column))]
    pub(crate) struct Column {
        pub name: String,
    }
}

#[cfg(test)]
mod tests {}
