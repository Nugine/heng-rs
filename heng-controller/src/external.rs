use crate::redis::{Connection, RedisModule};

use heng_protocol::external::CreateJudgeRequest;
use mobc_redis::redis;

use std::sync::Arc;

use anyhow::Result;

pub struct ExternalModule {
    redis_module: Arc<RedisModule>,
}

impl ExternalModule {
    pub fn new(redis_module: Arc<RedisModule>) -> Self {
        Self { redis_module }
    }

    async fn get_redis_connection(&self) -> Result<Connection> {
        self.redis_module.get_connection().await
    }

    pub async fn save_judge(&self, task_id: &str, judge: &CreateJudgeRequest) -> Result<()> {
        let content = serde_json::to_string(judge)?;

        redis::pipe()
            .atomic()
            .hset("judge_map", task_id, content)
            .lpush("judge_queue", task_id)
            .query_async(&mut *self.get_redis_connection().await?)
            .await?;

        Ok(())
    }

    pub async fn remove_judge(&self, task_id: &str) -> Result<()> {
        redis::pipe()
            .atomic()
            .lrem("judge_queue", 1, task_id)
            .hdel("judge_map", task_id)
            .query_async(&mut *self.get_redis_connection().await?)
            .await?;

        Ok(())
    }
}
