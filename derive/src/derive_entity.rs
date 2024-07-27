use std::{cell::LazyCell, clone, collections::HashMap, hash::Hash, ops::Index};

use convert_case::{Case, Casing};
use darling::{FromAttributes, FromDeriveInput, FromField};
use derive_more::Constructor;
use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Ident, Type};
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
    pub columns: Vec<FieldColumn>,
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
        let args = args::DeriveInputArgs::from_derive_input(&derive_input)?;
        let fields = if let Data::Struct(struct_data) = &derive_input.data {
            Ok(&struct_data.fields)
        } else {
            Err(DeriveEntityError::StructRequired)
        }?;

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
                let key = args::Key::from_attributes(&f.attrs).map_err(DeriveEntityError::from)?;
                let field_column = field_columns[&f_ident].clone();
                let key_name = key.name.unwrap_or_else(|| f_ident.to_string());
                Ok(Some((key_name, field_column)))
            })
            .collect::<Result<Vec<Option<(String, FieldColumn)>>, DeriveEntityError>>()?
            .iter()
            .flatten()
            .cloned()
            .collect_vec();

        let keys = index(pks)
            .into_iter()
            .map(|(k, v)| Key::new(k, v))
            .collect_vec();

        let columns = field_columns.into_iter().map(|(k, v)| v).collect_vec();

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
                .ok_or_else(|| DeriveEntityError::FieldRequired)?;

            let field_type = field.ty.clone();

            let column = args::Column::from_field(&field).ok();

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

fn index<K: Hash + Eq + Clone, V>(items: Vec<(K, V)>) -> HashMap<K, Vec<V>> {
    use itertools::Itertools;

    items
        .into_iter()
        .into_group_map_by(|(k, _)| k.clone())
        .into_iter()
        .map(|(k, v)| (k, v.into_iter().map(|(_, vv)| vv).collect_vec()))
        .collect()
}

impl TryFrom<DeriveEntity> for TokenStream {
    type Error = DeriveEntityError;

    fn try_from(entity: DeriveEntity) -> Result<Self, Self::Error> {
        let repo_trait = repo_trait(&entity)?;
        let pgpool_impl = pgpool_impl(&entity)?;
        let tx_impl = tx_impl(&entity)?;

        Ok(quote! {
            #repo_trait

            #pgpool_impl

            #tx_impl
        })
    }
}

fn repo_trait(entity: &DeriveEntity) -> Result<TokenStream, DeriveEntityError> {
    let trait_name = format_ident!("{}Repo", entity.entity);
    let key_fns = entity
        .keys
        .iter()
        .map(|key| {
            let ent = &entity.entity;
            let snake_entity = entity.entity_snake_name();
            let fn_name = format_ident!("find_{}_by_{}", snake_entity, key.name);

            let fn_args = key
                .components
                .iter()
                .map(|c| {
                    let name = &c.field_name;
                    let ty = &c.field_type;
                    quote! {
                        #name: &#ty
                    }
                })
                .collect_vec();
            quote! {
                async fn #fn_name(&self, #(#fn_args), *) -> Result<Option<#ent>, sqlx::Error>;

            }
        })
        .collect_vec();

    println!(
        "{:?}",
        key_fns.iter().map(TokenStream::to_string).collect_vec()
    );
    Ok(quote! {
        pub trait #trait_name {
            #(
                #key_fns
            )*
        }
    })
}

fn pgpool_impl(entity: &DeriveEntity) -> Result<TokenStream, DeriveEntityError> {
    Ok(quote! {})
}

fn tx_impl(entity: &DeriveEntity) -> Result<TokenStream, DeriveEntityError> {
    Ok(quote! {})
}

pub(super) mod args {
    use darling::{FromAttributes, FromDeriveInput, FromField};
    use syn::Ident;

    #[derive(Debug, FromDeriveInput)]
    #[darling(attributes(entity), supports(struct_named))]
    pub(crate) struct DeriveInputArgs {
        #[darling(default)]
        pub name: Option<Ident>,

        #[darling(default)]
        pub table_name: Option<String>,
    }

    #[derive(Debug, FromAttributes)]
    #[darling(attributes(key))]
    pub(crate) struct Key {
        // #[darling(default)]
        pub name: Option<String>,
    }

    #[derive(Debug, FromField)]
    #[darling(attributes(column))]
    pub(crate) struct Column {
        pub name: String,
    }
}

#[cfg(test)]
mod tests {
    use super::index;

    #[test]
    fn test_index() {
        let items = vec![("a", 1), ("b", 2), ("a", 3)];
        let ix = index(items);
        println!("{ix:?}");
    }
}
