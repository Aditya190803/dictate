use super::{ApiErrorDetails, NetworkErrorDetails, TranscriptionError, TranscriptionProvider};
use async_trait::async_trait;
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub enum AuthStyle {
    Bearer,
    XApiKey,
}

#[derive(Debug, Clone)]
pub struct OnlineProviderOptions {
    pub provider_name: &'static str,
    pub api_key: String,
    pub timeout_seconds: u64,
    pub max_retries: u32,
    pub model: String,
    pub base_url: String,
    pub auth_style: AuthStyle,
}

pub struct OnlineTranscriptionProvider {
    options: OnlineProviderOptions,
    client: reqwest::Client,
}

impl OnlineTranscriptionProvider {
    pub fn new(options: OnlineProviderOptions) -> Result<Self, TranscriptionError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(options.timeout_seconds))
            .build()
            .map_err(|e| {
                TranscriptionError::NetworkError(NetworkErrorDetails {
                    provider: options.provider_name.to_string(),
                    error_type: "HTTP client error".to_string(),
                    error_message: e.to_string(),
                })
            })?;

        Ok(Self { options, client })
    }

    async fn transcribe_attempt(
        &self,
        audio_data: &[u8],
        language: Option<&str>,
    ) -> Result<String, TranscriptionError> {
        let url = format!(
            "{}/audio/transcriptions",
            self.options.base_url.trim_end_matches('/')
        );

        let audio_part = reqwest::multipart::Part::bytes(audio_data.to_vec())
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| {
                TranscriptionError::NetworkError(NetworkErrorDetails {
                    provider: self.options.provider_name.to_string(),
                    error_type: "HTTP client error".to_string(),
                    error_message: e.to_string(),
                })
            })?;

        let mut form = reqwest::multipart::Form::new()
            .part("file", audio_part)
            .text("model", self.options.model.clone());

        if let Some(lang) = language {
            form = form.text("language", lang.to_string());
        }

        let mut request = self.client.post(&url).multipart(form);
        request = match self.options.auth_style {
            AuthStyle::Bearer => {
                request.header("Authorization", format!("Bearer {}", self.options.api_key))
            }
            AuthStyle::XApiKey => request.header("x-api-key", self.options.api_key.clone()),
        };

        let response = request.send().await.map_err(|e| {
            TranscriptionError::NetworkError(NetworkErrorDetails {
                provider: self.options.provider_name.to_string(),
                error_type: if e.is_timeout() {
                    "Request timeout".to_string()
                } else if e.is_connect() {
                    "Connection failed".to_string()
                } else if e.is_request() {
                    "Request error".to_string()
                } else {
                    "Network error".to_string()
                },
                error_message: e.to_string(),
            })
        })?;

        let status = response.status();
        let response_text = response.text().await.map_err(|e| {
            TranscriptionError::NetworkError(NetworkErrorDetails {
                provider: self.options.provider_name.to_string(),
                error_type: "Response reading error".to_string(),
                error_message: e.to_string(),
            })
        })?;

        if status.is_success() {
            let json: Value = serde_json::from_str(&response_text)
                .map_err(|e| TranscriptionError::JsonError(e.to_string()))?;
            let text = json.get("text").and_then(|t| t.as_str()).ok_or_else(|| {
                TranscriptionError::ApiError(ApiErrorDetails {
                    provider: self.options.provider_name.to_string(),
                    status_code: Some(status.as_u16()),
                    error_code: None,
                    error_message: "No text field in response".to_string(),
                    raw_response: Some(response_text.clone()),
                })
            })?;
            return Ok(text.to_string());
        }

        let (error_code, error_message) = parse_error_body(&response_text);

        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(TranscriptionError::AuthenticationFailed {
                provider: self.options.provider_name.to_string(),
                details: Some(error_message),
            });
        }

        Err(TranscriptionError::ApiError(ApiErrorDetails {
            provider: self.options.provider_name.to_string(),
            status_code: Some(status.as_u16()),
            error_code,
            error_message,
            raw_response: Some(response_text),
        }))
    }
}

#[async_trait]
impl TranscriptionProvider for OnlineTranscriptionProvider {
    async fn transcribe_with_language(
        &self,
        audio_data: Vec<u8>,
        language: Option<String>,
    ) -> Result<String, TranscriptionError> {
        const MAX_FILE_SIZE: usize = 25 * 1024 * 1024;
        if audio_data.len() > MAX_FILE_SIZE {
            return Err(TranscriptionError::FileTooLarge(audio_data.len()));
        }

        let mut retries = 0;
        loop {
            match self
                .transcribe_attempt(&audio_data, language.as_deref())
                .await
            {
                Ok(result) => return Ok(result),
                Err(e) => {
                    retries += 1;
                    if retries > self.options.max_retries {
                        return Err(e);
                    }

                    if matches!(e, TranscriptionError::AuthenticationFailed { .. }) {
                        return Err(e);
                    }

                    let delay = Duration::from_millis(1000 * (1 << (retries - 1)).min(8));
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }
}

fn parse_error_body(response_text: &str) -> (Option<String>, String) {
    if let Ok(json) = serde_json::from_str::<Value>(response_text) {
        let code = json
            .get("error")
            .and_then(|e| e.get("code"))
            .and_then(|c| c.as_str())
            .or_else(|| {
                json.get("error")
                    .and_then(|e| e.get("type"))
                    .and_then(|t| t.as_str())
            })
            .map(std::string::ToString::to_string);

        let message = json
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or(response_text)
            .to_string();

        return (code, message);
    }

    (None, response_text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_options() -> OnlineProviderOptions {
        OnlineProviderOptions {
            provider_name: "Test",
            api_key: "test-key".to_string(),
            timeout_seconds: 30,
            max_retries: 0,
            model: "test-model".to_string(),
            base_url: "https://example.test/v1".to_string(),
            auth_style: AuthStyle::Bearer,
        }
    }

    #[test]
    fn test_provider_creation() {
        let provider = OnlineTranscriptionProvider::new(test_options());
        assert!(provider.is_ok());
    }

    #[tokio::test]
    async fn test_file_size_validation() {
        let provider = OnlineTranscriptionProvider::new(test_options()).unwrap();
        let large_data = vec![0u8; 26 * 1024 * 1024];
        let result = provider.transcribe_with_language(large_data, None).await;
        assert!(matches!(result, Err(TranscriptionError::FileTooLarge(_))));
    }

    #[test]
    fn test_parse_error_body() {
        let (code, message) = parse_error_body(r#"{"error":{"code":"bad","message":"Nope"}}"#);
        assert_eq!(code, Some("bad".to_string()));
        assert_eq!(message, "Nope");

        let (code, message) = parse_error_body("plain error");
        assert_eq!(code, None);
        assert_eq!(message, "plain error");
    }
}
