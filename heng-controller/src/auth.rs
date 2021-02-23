use crate::redis::RedisModule;
use crate::Config;

use std::sync::Arc;

use anyhow::Result;

pub struct AuthModule {
    redis_module: Arc<RedisModule>,
    root_access_key: Box<str>,
    root_secret_key: Box<str>,
}

pub enum ClientKind {
    Root,
    External,
    Internal,
}

pub struct Client {
    pub kind: ClientKind,
    pub access_key: Box<str>,
}

impl AuthModule {
    pub fn new(config: &Config, redis_module: Arc<RedisModule>) -> Self {
        Self {
            redis_module,
            root_access_key: config.auth.root_access_key.as_str().into(),
            root_secret_key: config.auth.root_secret_key.as_str().into(),
        }
    }

    pub fn lookup(&self, access_key: &str) -> Result<Option<(ClientKind, Box<str>)>> {
        if access_key == &*self.root_access_key {
            Ok(Some((ClientKind::Root, self.root_secret_key.clone())))
        } else {
            todo!()
        }
    }
}
