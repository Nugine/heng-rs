use std::future::Future;

use anyhow::Result;
use futures::future::BoxFuture;
use futures::stream::FuturesUnordered;
use futures::StreamExt;

pub struct AutoJoin<'a> {
    set: FuturesUnordered<BoxFuture<'a, Result<()>>>,
}

pub async fn auto_join<'f, 'a: 'f>(
    f: impl FnOnce(&'_ mut AutoJoin<'a>) -> Result<()> + 'f,
) -> Result<()> {
    let mut j = AutoJoin {
        set: FuturesUnordered::new(),
    };
    f(&mut j)?;
    let mut stream = j.set;
    while let Some(result) = stream.next().await {
        result?;
    }
    Ok(())
}

impl<'a> AutoJoin<'a> {
    pub fn spawn(&mut self, f: impl Future<Output = Result<()>> + Send + 'a) {
        self.set.push(Box::pin(f))
    }
}
