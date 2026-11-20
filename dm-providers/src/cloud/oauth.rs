use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::{Rng, distr::Alphanumeric};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use url::Url;

const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);

pub struct OAuthEndpoints {
    pub auth_url: &'static str,
    pub scope: &'static str,
}

pub struct LoopbackLogin {
    listener: TcpListener,
    pub redirect_uri: String,
    pub verifier: String,
    state: String,
    pub authorize_url: String,
}

fn random_string(len: usize) -> String {
    rand::rng()
        .sample_iter(Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

impl LoopbackLogin {
    pub async fn start(endpoints: &OAuthEndpoints, client_id: &str) -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .context("opening the local login listener")?;
        let redirect_uri = format!("http://127.0.0.1:{}", listener.local_addr()?.port());
        let verifier = random_string(64);
        let state = random_string(24);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let mut url = Url::parse(endpoints.auth_url)?;
        url.query_pairs_mut()
            .append_pair("client_id", client_id)
            .append_pair("redirect_uri", &redirect_uri)
            .append_pair("response_type", "code")
            .append_pair("scope", endpoints.scope)
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", &state)
            .append_pair("access_type", "offline")
            .append_pair("prompt", "consent");
        Ok(Self {
            listener,
            redirect_uri,
            verifier,
            state,
            authorize_url: url.into(),
        })
    }

    pub async fn wait_for_code(&self) -> Result<String> {
        tokio::time::timeout(LOGIN_TIMEOUT, self.accept_code())
            .await
            .context("login timed out")?
    }

    async fn accept_code(&self) -> Result<String> {
        loop {
            let (mut stream, _) = self.listener.accept().await?;
            let mut buffer = vec![0u8; 8192];
            let read = stream.read(&mut buffer).await?;
            let request = String::from_utf8_lossy(&buffer[..read]);
            let target = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("/");
            let params: HashMap<String, String> = Url::parse("http://localhost")?
                .join(target)?
                .query_pairs()
                .into_owned()
                .collect();

            let outcome = if let Some(error) = params.get("error") {
                Some(Err(anyhow::anyhow!("login was denied: {error}")))
            } else if let Some(code) = params.get("code") {
                if params.get("state") != Some(&self.state) {
                    Some(Err(anyhow::anyhow!("login state mismatch")))
                } else {
                    Some(Ok(code.clone()))
                }
            } else {
                None // e.g. a favicon request, keep waiting
            };
            let body = match &outcome {
                Some(Ok(_)) => "Signed in. You can close this tab and return to Flux.",
                Some(Err(_)) => "Login failed. Return to Flux for details.",
                None => "",
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.shutdown().await;
            if let Some(outcome) = outcome {
                return outcome;
            }
        }
    }
}

#[derive(serde::Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub expires_in: u64,
    pub refresh_token: Option<String>,
}

pub async fn token_request(url: &str, form: &[(&str, &str)]) -> Result<TokenResponse> {
    let response = reqwest::Client::new()
        .post(url)
        .form(form)
        .send()
        .await
        .context("contacting the token endpoint")?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        bail!("token request failed ({status}): {body}");
    }
    response.json().await.context("parsing the token response")
}
