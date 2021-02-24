use crate::WsStream;

use heng_protocol::internal::http::{AcquireTokenOutput, AcquireTokenRequest};
use heng_protocol::signature::calc_signature;

use anyhow::{format_err, Result};
use reqwest::header::HeaderValue;
use tracing::{error, info};

#[tracing::instrument(err)]
pub async fn get_token(remote_domain: &str, access_key: &str, secret_key: &str) -> Result<String> {
    let token_url = format!("http://{}/v1/judgers/token", remote_domain);

    let body = AcquireTokenRequest {
        max_task_count: 8,
        name: None,
        core_count: None,
        software: None,
    };

    let http_client = reqwest::Client::new();
    let mut req = http_client.post(&token_url).json(&body).build()?;

    {
        req.headers_mut()
            .insert("x-heng-accesskey", HeaderValue::from_str(access_key)?);

        let body = req.body().and_then(|b| b.as_bytes()).unwrap_or(&[]);
        let query = req.url().query().unwrap_or("");
        let signature = calc_signature(
            req.method(),
            req.url().path(),
            query,
            req.headers(),
            body,
            secret_key,
        );
        req.headers_mut().insert(
            "x-heng-signature",
            HeaderValue::from_str(&signature).unwrap(),
        );
    };

    let res = http_client.execute(req).await?;

    if res.status().is_success() {
        let output = res.json::<AcquireTokenOutput>().await?;
        Ok(output.token)
    } else {
        let status = res.status();
        let text = res.text().await.unwrap();
        error!(?status, ?text, "failed to acquire token");
        Err(format_err!("failed to acquire token"))
    }
}

#[tracing::instrument(err)]
pub async fn connect_ws(
    remote_domain: &str,
    access_key: &str,
    secret_key: &str,
    token: &str,
) -> Result<WsStream> {
    let mut req = http::Request::new(());

    let uri = format!(
        "ws://{}/v1/judgers/websocket?token={}",
        remote_domain, token
    );
    *req.uri_mut() = uri.parse().unwrap();

    req.headers_mut().insert(
        "x-heng-accesskey",
        HeaderValue::from_str(access_key).unwrap(),
    );
    req.headers_mut()
        .insert("content-length", HeaderValue::from(0));

    let signature = calc_signature(
        req.method(),
        req.uri().path(),
        req.uri().query().unwrap_or(""),
        req.headers(),
        &[],
        secret_key,
    );

    req.headers_mut().insert(
        "x-heng-signature",
        HeaderValue::from_str(&signature).unwrap(),
    );

    info!("connecting to {}", req.uri());
    let (ws_stream, _) = tokio_tungstenite::connect_async(req).await?;
    info!("connected");
    Ok(ws_stream)
}
