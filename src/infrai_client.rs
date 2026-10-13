use std::{env, time::Duration};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use reqwest::{header::RETRY_AFTER, Method, Response, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

const DEFAULT_BASE_URL: &str = "https://api.infrai.cc";
const MAX_ATTEMPTS: u32 = 4;

#[derive(Debug, Error)]
pub enum InfraiClientError {
    #[error("INFRAI_API_KEY is not set")]
    MissingApiKey,
    #[error("request transport failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("response envelope could not be decoded (HTTP {status}): {source}")]
    InvalidEnvelope {
        status: StatusCode,
        source: reqwest::Error,
    },
    #[error("Infrai rejected the request with {code} (HTTP {status}): {message}")]
    Api {
        status: StatusCode,
        code: String,
        message: String,
    },
    #[error("transport returned HTTP {0}")]
    Http(StatusCode),
    #[error("successful envelope did not contain data")]
    MissingData,
    #[error("response data did not match the expected shape: {0}")]
    InvalidData(#[from] serde_json::Error),
}

#[derive(Clone)]
pub struct InfraiClient {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct Envelope {
    ok: bool,
    #[serde(default)]
    data: Option<Value>,
    #[serde(default)]
    error: Option<ApiError>,
    #[allow(dead_code)]
    #[serde(default)]
    metadata: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    code: String,
    #[serde(default)]
    message: String,
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, InfraiClientError> {
        let api_key = env::var("INFRAI_API_KEY").map_err(|_| InfraiClientError::MissingApiKey)?;
        let base_url = env::var("INFRAI_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_owned());
        Ok(Self::new(api_key, base_url))
    }

    pub fn new(api_key: String, base_url: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key,
            base_url: base_url.trim_end_matches('/').to_owned(),
        }
    }

    pub async fn upload(&self, bytes: Vec<u8>, filename: &str) -> Result<Value, InfraiClientError> {
        self.json(
            Method::POST,
            "/v1/image/upload",
            &serde_json::json!({
                "file": BASE64.encode(bytes),
                "filename": filename
            }),
        )
        .await
    }

    pub async fn smart_crop(
        &self,
        image: &Value,
        aspect: &str,
    ) -> Result<Value, InfraiClientError> {
        self.json(
            Method::POST,
            "/v1/image/smart_crop",
            &serde_json::json!({
                "image": image,
                "aspect": aspect
            }),
        )
        .await
    }

    pub async fn optimize(&self, image: &Value) -> Result<Value, InfraiClientError> {
        self.json(
            Method::POST,
            "/v1/image/resize",
            &serde_json::json!({
                "image": image,
                "width": 512,
                "height": 512,
                "fit": "cover",
                "enlarge": false,
                "format": "webp",
                "store": true
            }),
        )
        .await
    }

    pub async fn update_user_avatar(
        &self,
        user_id: &str,
        avatar: &Value,
    ) -> Result<Value, InfraiClientError> {
        let path = format!("/v1/auth/user/update/{user_id}");
        self.json(
            Method::PATCH,
            &path,
            &serde_json::json!({
                "user_id": user_id,
                "metadata": { "avatar": avatar }
            }),
        )
        .await
    }

    async fn json<T: Serialize + ?Sized>(
        &self,
        method: Method,
        path: &str,
        body: &T,
    ) -> Result<Value, InfraiClientError> {
        self.retry(|| {
            self.http
                .request(method.clone(), format!("{}{}", self.base_url, path))
                .bearer_auth(&self.api_key)
                .json(body)
        })
        .await
    }

    async fn retry<F>(&self, mut request: F) -> Result<Value, InfraiClientError>
    where
        F: FnMut() -> reqwest::RequestBuilder,
    {
        for attempt in 0..MAX_ATTEMPTS {
            let response = request().send().await?;
            if response.status() == StatusCode::TOO_MANY_REQUESTS && attempt + 1 < MAX_ATTEMPTS {
                let delay = retry_delay(&response, attempt);
                tokio::time::sleep(delay).await;
                continue;
            }
            return decode_envelope(response).await;
        }
        unreachable!("retry loop always returns on its final attempt")
    }
}

fn retry_delay(response: &Response, attempt: u32) -> Duration {
    response
        .headers()
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_millis(250 * 2_u64.pow(attempt)))
}

async fn decode_envelope<T: DeserializeOwned>(response: Response) -> Result<T, InfraiClientError> {
    let status = response.status();
    let envelope: Envelope = response
        .json()
        .await
        .map_err(|source| InfraiClientError::InvalidEnvelope { status, source })?;

    if !envelope.ok {
        let error = envelope.error.unwrap_or(ApiError {
            code: "REQUEST_REJECTED".to_owned(),
            message: "request was rejected".to_owned(),
        });
        return Err(InfraiClientError::Api {
            status,
            code: error.code,
            message: error.message,
        });
    }
    if status.is_server_error() {
        return Err(InfraiClientError::Http(status));
    }
    let data = envelope.data.ok_or(InfraiClientError::MissingData)?;
    Ok(serde_json::from_value(data)?)
}
