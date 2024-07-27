use derive_entity::DeriveEntity;
use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod derive_entity;

#[proc_macro_derive(Entity, attributes(entity, key, column))]
pub fn derive_sql(input: TokenStream) -> TokenStream {
    let derive_input: DeriveInput = parse_macro_input!(input as DeriveInput);
    let derive_entity = DeriveEntity::try_from(derive_input)
        .expect("failed to understand macro arguments");
    let entity: proc_macro2::TokenStream = derive_entity.try_into()
        .expect("could create token stream");

    entity.into()
}

