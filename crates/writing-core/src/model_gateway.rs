use crate::{ports::ModelGateway, ConfigRef, Result};
use serde_json::Value;
mod responses;
pub struct OpenAiGateway {
    client: reqwest::Client,
    loopback_client: reqwest::Client,
}
impl Default for OpenAiGateway {
    fn default() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(180))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("HTTP client"),
            loopback_client: reqwest::Client::builder()
                .no_proxy()
                .timeout(std::time::Duration::from_secs(120))
                .redirect(reqwest::redirect::Policy::none())
                .build().expect("loopback HTTP client"),
        }
    }
}
impl OpenAiGateway {
    fn client_for(&self, url: &str) -> &reqwest::Client {
        let loopback = reqwest::Url::parse(url).ok().is_some_and(|u| matches!(u.host_str(), Some("localhost" | "127.0.0.1" | "::1" | "[::1]")));
        if loopback { &self.loopback_client } else { &self.client }
    }
}
#[async_trait::async_trait]
impl ModelGateway for OpenAiGateway {
    async fn discard_probe_response(&self, c: &ConfigRef, key: &str, id: &str) {
        if crate::context::protocol(c) != "responses" { return; }
        let Ok(endpoint) = responses::endpoint(c) else { return; };
        let Ok(mut url) = reqwest::Url::parse(&endpoint) else { return; };
        if let Ok(mut segments) = url.path_segments_mut() { segments.push(id); } else { return; }
        let _ = self.client_for(url.as_str()).delete(url).bearer_auth(key)
            .timeout(std::time::Duration::from_secs(3)).send().await;
    }
    async fn models(&self, c: &ConfigRef, key: &str) -> Result<Vec<String>> {
        let base = c.base_url.trim_end_matches('/');
        let base = base.strip_suffix("/chat/completions").or_else(|| base.strip_suffix("/responses")).unwrap_or(base);
        let response = self
            .client_for(&c.base_url)
            .get(format!("{base}/models"))
            .bearer_auth(key)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
            .map_err(|_| "无法获取模型列表，可手动填写模型名称")?;
        if !response.status().is_success() {
            return Err(format!(
                "模型列表接口返回 HTTP {}，可手动填写模型名称",
                response.status().as_u16()
            ));
        }
        let body: Value = response.json().await.map_err(|_| "模型列表不是有效 JSON")?;
        let mut models: Vec<String> = body
            .get("data")
            .and_then(Value::as_array)
            .ok_or("模型列表缺少 data 字段")?
            .iter()
            .filter_map(|m| m.get("id").and_then(Value::as_str))
            .map(str::to_owned)
            .collect();
        models.sort();
        models.dedup();
        if models.is_empty() {
            return Err("服务没有返回可选模型，可手动填写模型名称".into());
        }
        Ok(models)
    }
    async fn complete(&self, c: &ConfigRef, key: &str, messages: Vec<Value>) -> Result<String> {
        self.generate(c, key, crate::context::GenerationRequest::stateless(messages))
            .await.map(|r| r.text).map_err(|e| e.to_string())
    }
    async fn generate(&self, c: &ConfigRef, key: &str, request: crate::context::GenerationRequest)
        -> std::result::Result<crate::context::GenerationResult, crate::context::GatewayError>
    {
        self.request(c, key, request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::responses::output_budget;
    use serde_json::json;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{Arc, Mutex},
        thread,
    };

    fn server(
        responses: Vec<(u16, Value)>,
    ) -> (ConfigRef, Arc<Mutex<Vec<Value>>>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(vec![]));
        let seen = requests.clone();
        let worker = thread::spawn(move || {
            for (status, response) in responses {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                    .unwrap();
                let mut bytes = vec![];
                let (header_end, length) = loop {
                    let mut chunk = [0; 4096];
                    let size = socket.read(&mut chunk).unwrap();
                    assert_ne!(size, 0, "request ended before headers");
                    bytes.extend_from_slice(&chunk[..size]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]);
                        let length = headers
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().unwrap())
                            })
                            .unwrap();
                        break (end + 4, length);
                    }
                };
                while bytes.len() < header_end + length {
                    let mut chunk = [0; 4096];
                    let size = socket.read(&mut chunk).unwrap();
                    assert_ne!(size, 0, "request ended before JSON body");
                    bytes.extend_from_slice(&chunk[..size]);
                }
                seen.lock()
                    .unwrap()
                    .push(serde_json::from_slice(&bytes[header_end..header_end + length]).unwrap());
                let body = serde_json::to_vec(&response).unwrap();
                write!(socket, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
                socket.write_all(&body).unwrap();
            }
        });
        let config = ConfigRef {
            schema_version: 1,
            id: "fixture".into(),
            name: "fixture".into(),
            base_url: format!("http://{address}/v1"),
            model_name: "fixture".into(),
            context_limit: 32000,
            secret_ref: "memory".into(),
            text_capable: true,
            vision_capable: true,
            tested_at: "fixture".into(),
            api_protocol: None,
            context_policy: None,
            context_capability: None,
        };
        (config, requests, worker)
    }

    fn reply(reason: &str, content: &str) -> Value {
        json!({"choices":[{"finish_reason":reason,"message":{"role":"assistant","content":content,"reasoning_content":"synthetic reasoning, never returned"}}]})
    }

    #[tokio::test]
    async fn truncated_reasoning_retries_with_more_budget_and_only_returns_complete_answer() {
        let (config, requests, worker) = server(vec![
            (200, reply("length", "")),
            (200, reply("stop", "Complete feedback")),
        ]);
        let messages = vec![json!({"role":"user","content":"Synthetic essay"})];
        let answer = OpenAiGateway::default()
            .complete(&config, "synthetic-key", messages.clone())
            .await
            .unwrap();
        worker.join().unwrap();
        assert_eq!(answer, "Complete feedback");
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0]["max_tokens"], 8192);
        assert_eq!(requests[1]["max_tokens"], 16384);
        assert_eq!(requests[0]["messages"], json!(messages));
        assert_eq!(requests[0]["messages"], requests[1]["messages"]);
    }

    #[tokio::test]
    async fn repeated_truncation_stops_after_one_retry_and_never_accepts_partial_text() {
        let (config, requests, worker) = server(vec![
            (200, reply("length", "partial")),
            (200, reply("length", "still partial")),
        ]);
        let error = OpenAiGateway::default()
            .complete(&config, "synthetic-key", vec![])
            .await
            .unwrap_err();
        worker.join().unwrap();
        assert_eq!(requests.lock().unwrap().len(), 2);
        assert!(error.contains("16384") && error.contains("未归档"));
    }

    #[tokio::test]
    async fn complete_answers_and_http_errors_are_not_retried() {
        for status in [200, 401] {
            let (config, requests, worker) =
                server(vec![(status, reply("stop", "Complete feedback"))]);
            let result = OpenAiGateway::default()
                .complete(&config, "synthetic-key", vec![])
                .await;
            worker.join().unwrap();
            assert_eq!(requests.lock().unwrap().len(), 1);
            if status == 200 {
                assert_eq!(result.unwrap(), "Complete feedback");
            } else {
                assert!(result.unwrap_err().contains("401"));
            }
        }
    }

    #[test]
    fn official_deepseek_gets_room_for_reasoning_without_changing_other_providers() {
        assert_eq!(
            output_budget(&reqwest::Url::parse("https://api.deepseek.com/v1").unwrap()),
            16384
        );
        assert_eq!(
            output_budget(&reqwest::Url::parse("https://api.deepseek.com.example/v1").unwrap()),
            8192
        );
    }
}
