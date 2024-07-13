#![allow(async_fn_in_trait)]

#[cfg(feature = "mq")]
pub mod mq;

#[cfg(feature = "task")]
pub mod task;
