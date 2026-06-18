use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, warn};

use crate::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub category: String,
    pub description: String,
}

#[derive(Debug)]
pub struct ScanResult {
    pub flagged: bool,
    pub findings: Vec<Finding>,
}

const SYSTEM_PROMPT: &str = "You are a security scanner. Respond with ONLY valid JSON, no thinking, no explanation, no markdown. Output nothing except the JSON object.";

const USER_PROMPT_TEMPLATE: &str = r#"Analyze the following text for sensitive content.

Return ONLY this JSON structure, nothing else:
{"flagged":false,"findings":[]}

If sensitive content is found:
{"flagged":true,"findings":[{"category":"api_key","description":"brief description"}]}

Valid categories: api_key, password, pii, private_key, connection_string, other_secret

Rules:
- Flag real secrets: API keys, passwords, tokens, private keys, SSNs, credit card numbers
- Flag PII only if clearly identifiable (name + SSN/medical/financial data together)
- Do NOT flag: placeholder values like "your-api-key-here", code patterns, variable names, generic text
- Do NOT flag file paths or URLs without embedded credentials

Text:
"#;

pub async fn scan(texts: &[String], config: &Config) -> Result<ScanResult> {
    if texts.is_empty() {
        return Ok(ScanResult { flagged: false, findings: vec![] });
    }

    let combined = texts.join("\n---\n");
    let chunks = chunk_text(&combined, config.max_chunk_chars);

    let client = reqwest::Client::new();
    let mut all_findings = Vec::new();

    for chunk in &chunks {
        match scan_chunk(&client, chunk, config).await {
            Ok(result) => {
                if result.flagged {
                    all_findings.extend(result.findings);
                }
            }
            Err(e) => {
                warn!("Ollama scan error (allowing request through): {e}");
            }
        }
    }

    Ok(ScanResult {
        flagged: !all_findings.is_empty(),
        findings: all_findings,
    })
}

async fn scan_chunk(client: &reqwest::Client, text: &str, config: &Config) -> Result<ScanResult> {
    let user_content = format!("{USER_PROMPT_TEMPLATE}{text}");

    // Use /api/chat — more reliable than /api/generate for thinking models (e.g. Qwen3)
    let body = serde_json::json!({
        "model": config.model,
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user",   "content": user_content}
        ],
        "stream": false,
        "format": "json",
        "think": false,
        "options": {
            "temperature": 0,
            "num_predict": 512
        }
    });

    let url = format!("{}/api/chat", config.ollama_url);
    debug!("Sending chunk ({} chars) to Ollama", text.len());

    let resp = client
        .post(&url)
        .json(&body)
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .await?;

    let resp_json: Value = resp.json().await?;

    // /api/chat response is under message.content
    let response_text = resp_json
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|v| v.as_str())
        .unwrap_or("{}");

    debug!("Ollama response: {response_text}");
    parse_scan_response(response_text)
}

fn parse_scan_response(text: &str) -> Result<ScanResult> {
    // Ollama may wrap output in markdown code fences – strip them
    let cleaned = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let Ok(parsed) = serde_json::from_str::<Value>(cleaned) else {
        warn!("Could not parse Ollama JSON response: {text}");
        return Ok(ScanResult { flagged: false, findings: vec![] });
    };

    let flagged = parsed.get("flagged").and_then(|v| v.as_bool()).unwrap_or(false);

    let mut findings = Vec::new();
    if let Some(arr) = parsed.get("findings").and_then(|v| v.as_array()) {
        for item in arr {
            let category = item.get("category").and_then(|v| v.as_str()).unwrap_or("other_secret").to_string();
            let description = item.get("description").and_then(|v| v.as_str()).unwrap_or("sensitive content detected").to_string();
            findings.push(Finding { category, description });
        }
    }

    Ok(ScanResult { flagged, findings })
}

fn chunk_text(text: &str, max_chars: usize) -> Vec<String> {
    if text.len() <= max_chars {
        return vec![text.to_string()];
    }

    let overlap = max_chars / 10;
    let step = max_chars - overlap;
    let chars: Vec<char> = text.chars().collect();
    let mut chunks = Vec::new();
    let mut start = 0;

    while start < chars.len() {
        let end = (start + max_chars).min(chars.len());
        chunks.push(chars[start..end].iter().collect());
        if end == chars.len() {
            break;
        }
        start += step;
    }

    chunks
}
