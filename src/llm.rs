//! Optional local LLM hook (Ollama HTTP API on 127.0.0.1:11434 by default).
//! Used only to write news story bodies. Runs on the local GPU when the
//! Ollama server is configured for it (works on Linux; on FreeBSD/MidnightBSD
//! Ollama also supports Vulkan). No cloud API is ever contacted; every call
//! has a short timeout and any failure falls back to procedural text.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Llm {
    pub model: String,
    pub addr: String,
}

impl Llm {
    /// Returns Some if a local Ollama server answers on the configured address.
    pub fn probe() -> Option<Llm> {
        let addr = std::env::var("WSC_OLLAMA_ADDR").unwrap_or_else(|_| "127.0.0.1:11434".to_string());
        let model = std::env::var("WSC_MODEL").unwrap_or_else(|_| "llama3.1:latest".to_string());
        let llm = Llm { model, addr };
        match llm.http("GET", "/api/tags", None, 2) {
            Some(body) if body.contains("models") => Some(llm),
            _ => None,
        }
    }

    fn connect(&self, timeout: Duration) -> Option<TcpStream> {
        let sock = self.addr.to_socket_addrs().ok()?.next()?;
        let s = TcpStream::connect_timeout(&sock, timeout).ok()?;
        s.set_read_timeout(Some(timeout)).ok()?;
        s.set_write_timeout(Some(timeout)).ok()?;
        Some(s)
    }

    fn http(&self, method: &str, path: &str, body: Option<String>, timeout_secs: u64) -> Option<String> {
        let mut s = self.connect(Duration::from_secs(timeout_secs))?;
        let body = body.unwrap_or_default();
        let req = format!(
            "{} {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            method,
            path,
            self.addr,
            body.len(),
            body
        );
        s.write_all(req.as_bytes()).ok()?;
        let mut out = Vec::new();
        s.read_to_end(&mut out).ok()?;
        let text = String::from_utf8_lossy(&out);
        let body_start = text.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
        Some(text[body_start..].to_string())
    }

    /// Generic completion. Returns the response text or None on any failure.
    pub fn complete(&self, prompt: &str, max_tokens: u32) -> Option<String> {
        let payload = serde_json::json!({
            "model": self.model,
            "prompt": prompt,
            "stream": false,
            "options": { "num_predict": max_tokens, "temperature": 0.8 }
        });
        let body = self.http("POST", "/api/generate", Some(payload.to_string()), 15)?;
        let v: serde_json::Value = serde_json::from_str(&body).ok()?;
        let text = v.get("response")?.as_str()?.trim().to_string();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    /// Write a punchy 1-2 sentence body for a news item.
    pub fn news_body(&self, headline: &str, facts: &str) -> Option<String> {
        let prompt = format!(
            "You are a financial newsroom writer. Rewrite the story below in 1-2 punchy sentences (max 45 words). Keep the facts exactly; do not add advice or disclaimers. Output only the story text.\nHEADLINE: {}\nFACTS: {}",
            headline, facts
        );
        let text = self.complete(&prompt, 80)?;
        let clean = text.lines().next().unwrap_or("").trim().to_string();
        if clean.len() > 220 {
            Some(clean[..220].to_string())
        } else if clean.is_empty() {
            None
        } else {
            Some(clean)
        }
    }
}
