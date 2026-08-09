use super::{ApiErrorDetails, NetworkErrorDetails, TranscriptionError, TranscriptionProvider};
use async_trait::async_trait;
use serde_json::Value;
use std::time::Duration;

/// Authentication style for API requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthStyle {
    /// `Authorization: Bearer <key>` — Mistral, Groq.
    Bearer,
    /// `Authorization: Token <key>` — Deepgram.
    Token,
}

/// Request/response shape of the upstream API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiDialect {
    /// `POST {base}/audio/transcriptions`, multipart form, `{"text": ...}`.
    OpenAiCompatible,
    /// `POST {base}/v1/listen?model=..`, raw audio body, transcript nested under
    /// `results.channels[0].alternatives[0].transcript`.
    Deepgram,
}

/// Options for configuring an online transcription provider.
#[derive(Debug, Clone)]
pub struct OnlineProviderOptions {
    pub provider_name: &'static str,
    pub api_key: String,
    pub timeout_seconds: u64,
    pub max_retries: u32,
    pub model: String,
    pub base_url: String,
    pub auth_style: AuthStyle,
    pub dialect: ApiDialect,
}

/// Provider that transcribes audio via a REST API (Mistral, Groq, etc.).
pub struct OnlineTranscriptionProvider {
    options: OnlineProviderOptions,
    client: reqwest::Client,
}

impl OnlineTranscriptionProvider {
    /// Create a new provider. Builds the HTTP client with the configured timeout.
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

    /// Perform a single transcription attempt.
    async fn transcribe_attempt(
        &self,
        audio_data: &[u8],
        language: Option<&str>,
    ) -> Result<String, TranscriptionError> {
        let base = self.options.base_url.trim_end_matches('/');

        let mut request = match self.options.dialect {
            ApiDialect::OpenAiCompatible => {
                let url = format!("{}/audio/transcriptions", base);

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

                self.client.post(&url).multipart(form)
            }
            ApiDialect::Deepgram => {
                // Deepgram takes the raw audio as the request body and reads
                // options from the query string. `smart_format` gives punctuation
                // and capitalisation, which the polish step then refines.
                let mut url = format!(
                    "{}/v1/listen?model={}&smart_format=true",
                    base, self.options.model
                );
                // Deepgram has no "auto" sentinel: omitting `language` lets it
                // use the model default, and nova-3 needs `multi` to detect.
                match language {
                    Some(lang) if !lang.eq_ignore_ascii_case("auto") => {
                        url.push_str("&language=");
                        url.push_str(lang);
                    }
                    _ => {}
                }

                self.client
                    .post(&url)
                    .header("Content-Type", "audio/wav")
                    .body(audio_data.to_vec())
            }
        };

        request = match self.options.auth_style {
            AuthStyle::Bearer => {
                request.header("Authorization", format!("Bearer {}", self.options.api_key))
            }
            AuthStyle::Token => {
                request.header("Authorization", format!("Token {}", self.options.api_key))
            }
        };

        let response = request.send().await.map_err(|e| {
            let error_type = if e.is_timeout() {
                "Request timeout"
            } else if e.is_connect() {
                "Connection failed"
            } else if e.is_request() {
                "Request error"
            } else {
                "Network error"
            };
            TranscriptionError::NetworkError(NetworkErrorDetails {
                provider: self.options.provider_name.to_string(),
                error_type: error_type.to_string(),
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
            let transcript = match self.options.dialect {
                ApiDialect::OpenAiCompatible => json.get("text").and_then(|t| t.as_str()),
                ApiDialect::Deepgram => json
                    .get("results")
                    .and_then(|r| r.get("channels"))
                    .and_then(|c| c.get(0))
                    .and_then(|c| c.get("alternatives"))
                    .and_then(|a| a.get(0))
                    .and_then(|a| a.get("transcript"))
                    .and_then(|t| t.as_str()),
            };
            return transcript.map(|t| t.to_string()).ok_or_else(|| {
                TranscriptionError::ApiError(ApiErrorDetails {
                    provider: self.options.provider_name.to_string(),
                    status_code: Some(status.as_u16()),
                    error_code: None,
                    error_message: "No text field in response".to_string(),
                    raw_response: Some(response_text),
                })
            });
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

        let mut last_err = None;
        for attempt in 1..=self.options.max_retries.saturating_add(1) {
            match self
                .transcribe_attempt(&audio_data, language.as_deref())
                .await
            {
                Ok(result) => return Ok(result),
                Err(e) => {
                    // Don't retry auth failures
                    if matches!(e, TranscriptionError::AuthenticationFailed { .. }) {
                        return Err(e);
                    }
                    last_err = Some(e);
                    if attempt <= self.options.max_retries {
                        let delay = Duration::from_millis(1000 * (1 << (attempt - 1)).min(8));
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            TranscriptionError::NetworkError(NetworkErrorDetails {
                provider: self.options.provider_name.to_string(),
                error_type: "Retry exhausted".to_string(),
                error_message: "All transcription attempts failed".to_string(),
            })
        }))
    }
}

/// Parse an API error response body into (error_code, error_message).
fn parse_error_body(response_text: &str) -> (Option<String>, String) {
    if let Ok(json) = serde_json::from_str::<Value>(response_text) {
        let error = json.get("error");
        let code = error
            .and_then(|e| e.get("code").or_else(|| e.get("type")))
            .and_then(|c| c.as_str())
            .map(|s| s.to_string());
        let message = error
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
            dialect: ApiDialect::OpenAiCompatible,
        }
    }

    #[test]
    fn test_provider_creation() {
        assert!(OnlineTranscriptionProvider::new(test_options()).is_ok());
    }

    #[tokio::test]
    async fn deepgram_dialect_parses_nested_transcript() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/listen")
            .match_query(mockito::Matcher::UrlEncoded(
                "model".into(),
                "nova-3".into(),
            ))
            .match_header("authorization", "Token dg-key")
            .with_status(200)
            .with_body(
                r#"{"results":{"channels":[{"alternatives":[{"transcript":"hello from deepgram"}]}]}}"#,
            )
            .create_async()
            .await;

        let provider = OnlineTranscriptionProvider::new(OnlineProviderOptions {
            provider_name: "Deepgram",
            api_key: "dg-key".to_string(),
            timeout_seconds: 30,
            max_retries: 0,
            model: "nova-3".to_string(),
            base_url: server.url(),
            auth_style: AuthStyle::Token,
            dialect: ApiDialect::Deepgram,
        })
        .unwrap();

        let text = provider
            .transcribe_with_language(vec![0u8; 32], Some("auto".to_string()))
            .await
            .unwrap();

        assert_eq!(text, "hello from deepgram");
        mock.assert_async().await;
    }

    /// `auto` is dictate's sentinel, not a Deepgram language code — sending it
    /// verbatim makes Deepgram reject the request.
    #[tokio::test]
    async fn deepgram_dialect_omits_auto_language() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/listen")
            .match_query(mockito::Matcher::Exact(
                "model=nova-3&smart_format=true".into(),
            ))
            .with_status(200)
            .with_body(r#"{"results":{"channels":[{"alternatives":[{"transcript":"ok"}]}]}}"#)
            .create_async()
            .await;

        let provider = OnlineTranscriptionProvider::new(OnlineProviderOptions {
            provider_name: "Deepgram",
            api_key: "dg-key".to_string(),
            timeout_seconds: 30,
            max_retries: 0,
            model: "nova-3".to_string(),
            base_url: server.url(),
            auth_style: AuthStyle::Token,
            dialect: ApiDialect::Deepgram,
        })
        .unwrap();

        provider
            .transcribe_with_language(vec![0u8; 32], Some("auto".to_string()))
            .await
            .unwrap();
        mock.assert_async().await;
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
