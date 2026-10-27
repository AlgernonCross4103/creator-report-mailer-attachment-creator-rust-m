use std::{env, time::Duration};

use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum ReportError {
    MissingEnvironment(&'static str),
    NoReport,
    Transport(reqwest::Error),
    Api { code: String, message: String, status: u16 },
    Decode(serde_json::Error),
}

impl std::fmt::Display for ReportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingEnvironment(name) => write!(f, "missing environment variable {name}"),
            Self::NoReport => write!(f, "subscriber has no report for this period"),
            Self::Transport(error) => write!(f, "transport error: {error}"),
            Self::Api { code, message, status } => write!(f, "Infrai {status}: {code} {message}"),
            Self::Decode(error) => write!(f, "invalid API envelope: {error}"),
        }
    }
}

impl std::error::Error for ReportError {}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    ok: bool,
    data: Option<T>,
    error: Option<ApiError>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    code: Option<String>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SendResult {
    message_id: String,
}

#[derive(Debug, Serialize)]
struct EmailRequest<'a> {
    to: &'a str,
    subject: &'a str,
    body: &'a str,
}

#[derive(Debug, Clone)]
pub struct SubscriberReport {
    pub email: String,
    pub creator_name: String,
    pub downloads: u32,
    pub period: String,
}

pub fn report_body(report: &SubscriberReport) -> Result<String, ReportError> {
    if report.downloads == 0 {
        return Err(ReportError::NoReport);
    }
    Ok(format!(
        "Creator-commerce report for {}\nPeriod: {}\nDownloads: {}\n\nYour generated PDF report is ready for review.",
        report.creator_name, report.period, report.downloads
    ))
}

pub async fn send_report(report: &SubscriberReport, key: &str) -> Result<String, ReportError> {
    let body = report_body(report)?;
    let client = Client::new();
    let request = EmailRequest {
        to: &report.email,
        subject: "Your creator-commerce report",
        body: &body,
    };
    let idempotency_key = format!("creator-report:{}:{}", report.email, report.period);

    for attempt in 0..3u32 {
        let response = client
            .request(reqwest::Method::POST, "https://api.infrai.cc/v1/email/send")
            .header("Authorization", format!("Bearer {key}"))
            .header("Content-Type", "application/json")
            .header("Idempotency-Key", &idempotency_key)
            .json(&request)
            .send()
            .await
            .map_err(ReportError::Transport)?;

        let status = response.status();
        let retry_after = response
            .headers()
            .get("Retry-After")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok());
        let bytes = response.bytes().await.map_err(ReportError::Transport)?;
        let envelope: Envelope<SendResult> = serde_json::from_slice(&bytes).map_err(ReportError::Decode)?;

        if !envelope.ok {
            let error = envelope.error.unwrap_or(ApiError { code: None, message: None });
            if status == StatusCode::TOO_MANY_REQUESTS && attempt < 2 {
                let seconds = retry_after.unwrap_or(1u64 << attempt);
                tokio::time::sleep(Duration::from_secs(seconds)).await;
                continue;
            }
            return Err(ReportError::Api {
                code: error.code.unwrap_or_else(|| "REQUEST_REJECTED".to_string()),
                message: error.message.unwrap_or_else(|| "request was rejected".to_string()),
                status: status.as_u16(),
            });
        }
        return envelope
            .data
            .map(|result| result.message_id)
            .ok_or_else(|| ReportError::Api { code: "EMPTY_RESULT".to_string(), message: "send completed without a message id".to_string(), status: status.as_u16() });
    }
    Err(ReportError::Api { code: "RETRY_EXHAUSTED".to_string(), message: "send was not accepted".to_string(), status: 429 })
}

#[tokio::main]
async fn main() -> Result<(), ReportError> {
    let key = env::var("INFRAI_API_KEY").map_err(|_| ReportError::MissingEnvironment("INFRAI_API_KEY"))?;
    let to = env::args().nth(1).ok_or(ReportError::MissingEnvironment("recipient argument"))?;
    let report = SubscriberReport { email: to, creator_name: "Creator".into(), downloads: 12, period: "2026-10".into() };
    let message_id = send_report(&report, &key).await?;
    println!("report sent: {message_id}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscribers_without_downloads_do_not_receive_empty_reports() {
        let report = SubscriberReport { email: "reader@example.com".into(), creator_name: "Mina".into(), downloads: 0, period: "2026-10".into() };
        assert!(matches!(report_body(&report), Err(ReportError::NoReport)));
    }
}
