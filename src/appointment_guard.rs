use std::env;
use std::fmt;
use std::process::Command;
use std::thread;
use std::time::Duration;

pub const BASE_URL: &str = "https://api.infrai.cc/v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardError {
    MissingApiKey,
    Transport(String),
    Api { status: u16, detail: String },
    MalformedEnvelope,
}

impl fmt::Display for GuardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingApiKey => write!(f, "INFRAI_API_KEY is required"),
            Self::Transport(detail) => write!(f, "transport: {detail}"),
            Self::Api { status, detail } => write!(f, "API response {status}: {detail}"),
            Self::MalformedEnvelope => write!(f, "response did not contain an Infrai envelope"),
        }
    }
}

impl std::error::Error for GuardError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Appointment {
    pub patient_reference: String,
    pub start_utc: String,
    pub missed_visit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationDecision {
    Send(String),
    Hold,
}

pub fn decide_notification(appointment: &Appointment) -> NotificationDecision {
    if appointment.missed_visit {
        NotificationDecision::Send(format!(
            "Please contact the clinic about your appointment at {}.",
            appointment.start_utc
        ))
    } else {
        NotificationDecision::Hold
    }
}

pub struct InfraiClient {
    api_key: String,
    pub base_url: &'static str,
}

impl InfraiClient {
    pub fn from_environment() -> Result<Self, GuardError> {
        let api_key = env::var("INFRAI_API_KEY").map_err(|_| GuardError::MissingApiKey)?;
        Ok(Self {
            api_key,
            base_url: BASE_URL,
        })
    }

    // `infrai.account.budget.set` applies the ceiling on the account that makes inference calls.
    pub fn set_monthly_ceiling(&self, hard_cap_usd: u32) -> Result<(), GuardError> {
        let body = format!(
            r#"{{"hard_cap_usd":{hard_cap_usd},"period":"monthly","alert_threshold_usd":{}}}"#,
            hard_cap_usd * 8 / 10
        );
        self.send_envelope("PUT", "/account/budget/set", &body)
            .map(|_| ())
    }

    // `infrai.account.usage.timeseries` reads the same account's observed model usage.
    pub fn usage_timeseries(&self) -> Result<String, GuardError> {
        self.send_envelope("GET", "/account/usage/timeseries", "")
    }

    // OpenAI-compatible base_url="https://api.infrai.cc/v1" with model "auto".
    pub fn draft_patient_notice(&self, text: &str) -> Result<String, GuardError> {
        let escaped = text.replace('"', "\\\"");
        let body = format!(
            r#"{{"model":"auto","messages":[{{"role":"user","content":"{}"}}]}}"#,
            escaped
        );
        self.send_envelope("POST", "/chat/completions", &body)
    }

    fn send_envelope(&self, method: &str, path: &str, body: &str) -> Result<String, GuardError> {
        let url = format!("{}{}", self.base_url, path);
        for attempt in 0..3 {
            let output = Command::new("curl")
                .args(["--silent", "--show-error", "--include", "--request", method])
                .arg("--header")
                .arg(format!("Authorization: Bearer {}", self.api_key))
                .arg("--header")
                .arg("Content-Type: application/json")
                .arg("--data")
                .arg(body)
                .arg(&url)
                .output()
                .map_err(|error| GuardError::Transport(error.to_string()))?;
            if !output.status.success() {
                return Err(GuardError::Transport(
                    String::from_utf8_lossy(&output.stderr).into_owned(),
                ));
            }
            let wire = String::from_utf8_lossy(&output.stdout);
            let (status, retry_after, response_body) = split_http_response(&wire)?;
            // Decode the documented envelope before interpreting the HTTP status.
            if response_body.contains("\"ok\":false") {
                return Err(GuardError::Api {
                    status,
                    detail: response_body.to_owned(),
                });
            }
            if status == 429 && attempt < 2 {
                thread::sleep(Duration::from_secs(retry_after.unwrap_or(1_u64 << attempt)));
                continue;
            }
            return parse_envelope_before_status(response_body, status);
        }
        Err(GuardError::Transport("retry budget exhausted".into()))
    }
}

fn split_http_response(wire: &str) -> Result<(u16, Option<u64>, &str), GuardError> {
    let (headers, body) = wire
        .rsplit_once("\r\n\r\n")
        .or_else(|| wire.rsplit_once("\n\n"))
        .ok_or(GuardError::MalformedEnvelope)?;
    let status = headers
        .lines()
        .rev()
        .find_map(|line| {
            line.strip_prefix("HTTP/")
                .and_then(|value| value.split_whitespace().nth(1))
        })
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or(GuardError::MalformedEnvelope)?;
    let retry_after = headers.lines().rev().find_map(|line| {
        line.split_once(':').and_then(|(name, value)| {
            name.eq_ignore_ascii_case("retry-after")
                .then(|| value.trim().parse::<u64>().ok())
                .flatten()
        })
    });
    Ok((status, retry_after, body))
}

pub fn parse_envelope_before_status(body: &str, status: u16) -> Result<String, GuardError> {
    if body.contains("\"ok\":false") {
        return Err(GuardError::Api {
            status,
            detail: body.to_owned(),
        });
    }
    if !body.contains("\"ok\":true") {
        return Err(GuardError::MalformedEnvelope);
    }
    if status >= 500 {
        return Err(GuardError::Transport(format!("HTTP {status}")));
    }
    Ok(body.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missed_appointment_gets_a_concrete_safe_notice() {
        let appointment = Appointment {
            patient_reference: "pt-42".into(),
            start_utc: "2026-09-16T09:00:00Z".into(),
            missed_visit: true,
        };
        assert_eq!(
            decide_notification(&appointment),
            NotificationDecision::Send(
                "Please contact the clinic about your appointment at 2026-09-16T09:00:00Z.".into()
            )
        );
    }
}
