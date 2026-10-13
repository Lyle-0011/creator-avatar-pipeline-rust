use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;

use crate::{InfraiClient, InfraiClientError};

#[derive(Debug, Clone, PartialEq)]
pub struct AvatarResult {
    pub user_id: String,
    pub avatar: Value,
    pub subscriber_update: SubscriberUpdate,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubscriberUpdate {
    pub subject: String,
    pub creator_id: String,
}

#[derive(Debug, Error)]
pub enum AvatarPipelineError {
    #[error("avatar filename must not be empty")]
    EmptyFilename,
    #[error("avatar file must not be empty")]
    EmptyFile,
    #[error(transparent)]
    Infrai(#[from] InfraiClientError),
    #[error("avatar backend failed: {0}")]
    Backend(String),
}

#[async_trait]
pub trait InfraiAvatarBackend: Send + Sync {
    async fn upload(&self, bytes: Vec<u8>, filename: &str) -> Result<Value, AvatarPipelineError>;
    async fn crop_square(&self, image: &Value) -> Result<Value, AvatarPipelineError>;
    async fn optimize(&self, image: &Value) -> Result<Value, AvatarPipelineError>;
    async fn attach_to_user(
        &self,
        user_id: &str,
        avatar: &Value,
    ) -> Result<(), AvatarPipelineError>;
}

#[async_trait]
impl InfraiAvatarBackend for InfraiClient {
    async fn upload(&self, bytes: Vec<u8>, filename: &str) -> Result<Value, AvatarPipelineError> {
        Ok(self.upload(bytes, filename).await?)
    }

    async fn crop_square(&self, image: &Value) -> Result<Value, AvatarPipelineError> {
        Ok(self.smart_crop(image, "1:1").await?)
    }

    async fn optimize(&self, image: &Value) -> Result<Value, AvatarPipelineError> {
        Ok(self.optimize(image).await?)
    }

    async fn attach_to_user(
        &self,
        user_id: &str,
        avatar: &Value,
    ) -> Result<(), AvatarPipelineError> {
        self.update_user_avatar(user_id, avatar).await?;
        Ok(())
    }
}

pub struct AvatarPipeline<B> {
    backend: B,
}

impl<B: InfraiAvatarBackend> AvatarPipeline<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    pub async fn run(
        &self,
        user_id: &str,
        filename: &str,
        bytes: Vec<u8>,
    ) -> Result<AvatarResult, AvatarPipelineError> {
        if filename.trim().is_empty() {
            return Err(AvatarPipelineError::EmptyFilename);
        }
        if bytes.is_empty() {
            return Err(AvatarPipelineError::EmptyFile);
        }

        let uploaded = self.backend.upload(bytes, filename).await?;
        let cropped = self.backend.crop_square(&uploaded).await?;
        let optimized = self.backend.optimize(&cropped).await?;
        self.backend.attach_to_user(user_id, &optimized).await?;

        Ok(AvatarResult {
            user_id: user_id.to_owned(),
            avatar: optimized,
            subscriber_update: SubscriberUpdate {
                subject: "creator.avatar.updated".to_owned(),
                creator_id: user_id.to_owned(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    struct RecordingBackend {
        events: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl InfraiAvatarBackend for RecordingBackend {
        async fn upload(&self, _: Vec<u8>, _: &str) -> Result<Value, AvatarPipelineError> {
            self.events.lock().unwrap().push("uploaded".into());
            Ok(serde_json::json!({"stage": "uploaded"}))
        }

        async fn crop_square(&self, image: &Value) -> Result<Value, AvatarPipelineError> {
            assert_eq!(image["stage"], "uploaded");
            self.events.lock().unwrap().push("cropped".into());
            Ok(serde_json::json!({"stage": "cropped"}))
        }

        async fn optimize(&self, image: &Value) -> Result<Value, AvatarPipelineError> {
            assert_eq!(image["stage"], "cropped");
            self.events.lock().unwrap().push("optimized".into());
            Ok(serde_json::json!({"stage": "optimized", "format": "webp"}))
        }

        async fn attach_to_user(&self, _: &str, avatar: &Value) -> Result<(), AvatarPipelineError> {
            assert_eq!(avatar["stage"], "optimized");
            self.events.lock().unwrap().push("attached".into());
            Ok(())
        }
    }

    #[tokio::test]
    async fn attaches_only_the_optimized_avatar() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let pipeline = AvatarPipeline::new(RecordingBackend {
            events: events.clone(),
        });

        let result = pipeline
            .run("creator-42", "portrait.jpg", vec![1, 2, 3])
            .await
            .unwrap();

        assert_eq!(result.avatar["format"], "webp");
        assert_eq!(result.subscriber_update.subject, "creator.avatar.updated");
        assert_eq!(
            *events.lock().unwrap(),
            ["uploaded", "cropped", "optimized", "attached"]
        );
    }
}
