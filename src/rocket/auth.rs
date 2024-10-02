#![allow(unused)]

use rocket::request::{self, FromRequest, Request};

pub struct Authorization<Principle, Permissions> {
    principle: Principle,
    permissions: Permissions,
}

pub trait Principle {
    type Id;
    fn id(&self) -> Self::Id;
}

pub trait Permissions {
    fn has_permission(&self, permission: &str) -> bool;
    fn has_all_permission(&self, permission: &[&str]) -> bool;
}

#[derive(Debug)]
pub struct AuthError;

#[rocket::async_trait]
impl<'r, PrincipleT, PermissionsT> FromRequest<'r> for Authorization<PrincipleT, PermissionsT>
where
    PrincipleT: Principle,
    PermissionsT: Permissions,
{
    type Error = AuthError;

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        todo!()
    }
}
