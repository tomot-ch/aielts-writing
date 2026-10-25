use super::OpenAiGateway;
use crate::{context::*, ConfigRef};
use serde_json::{json, Value};

pub(super) fn output_budget(base: &reqwest::Url) -> u64 {
    if base.host_str() == Some("api.deepseek.com") { 16384 } else { 8192 }
}

pub(super) fn endpoint(c: &ConfigRef) -> Result<String, GatewayError> {
    let base = reqwest::Url::parse(&c.base_url)
        .map_err(|_| GatewayError::new(GatewayErrorKind::Rejected, "API 地址无效"))?;
    if !matches!(base.scheme(), "https" | "http")
        || (base.scheme() == "http"
            && !matches!(base.host_str(), Some("localhost" | "127.0.0.1" | "::1")))
        || base.host_str().is_none()
        || !base.username().is_empty()
        || base.password().is_some()
        || base.query().is_some()
        || base.fragment().is_some()
    {
        return Err(GatewayError::new(
            GatewayErrorKind::Rejected,
            "API 地址须为不含凭据或参数的 HTTP(S) 地址",
        ));
    }
    let base = c.base_url.trim_end_matches('/');
    let root = base
        .strip_suffix("/chat/completions")
        .or_else(|| base.strip_suffix("/responses"))
        .unwrap_or(base);
    Ok(format!(
        "{root}/{}",
        if protocol(c) == "responses" {
            "responses"
        } else {
            "chat/completions"
        }
    ))
}

fn responses_body(c: &ConfigRef, request: &GenerationRequest, budget: u64) -> Value {
    let instructions = request
        .messages
        .iter()
        .filter(|m| matches!(m["role"].as_str(), Some("system" | "developer")))
        .filter_map(|m| m["content"].as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    let input: Vec<Value> = request.messages.iter()
        .filter(|m| !matches!(m["role"].as_str(), Some("system" | "developer")))
        .map(|m| {
            let mut result = m.clone();
            if let Some(parts) = m["content"].as_array() {
                result["content"] = Value::Array(parts.iter().map(|p| match p["type"].as_str() {
                    Some("text") => json!({"type":if m["role"]=="assistant" {"output_text"}else{"input_text"},"text":p["text"]}),
                    Some("image_url") => {
                        let mut part = json!({"type":"input_image","image_url":p["image_url"]["url"]});
                        if let Some(detail) = p["image_url"].get("detail") { part["detail"] = detail.clone(); }
                        part
                    }
                    _ => p.clone(),
                }).collect());
            }
            result
        }).collect();
    let mut body = json!({"model":c.model_name,"input":input,"store":request.store,"stream":false,"max_output_tokens":budget});
    if !instructions.is_empty() {
        body["instructions"] = json!(instructions);
    }
    if let Some(id) = &request.previous_response_id {
        body["previous_response_id"] = json!(id);
    }
    body
}

fn http_error(status: u16, body: &Value, continuing: bool) -> GatewayError {
    let error = &body["error"];
    let code = error["code"]
        .as_str()
        .or_else(|| error["type"].as_str())
        .unwrap_or("")
        .to_lowercase();
    let param = error["param"].as_str().unwrap_or("");
    let detail = error["message"].as_str().unwrap_or("").to_lowercase();
    if matches!(status, 400 | 404)
        && continuing
        && (matches!(
            code.as_str(),
            "previous_response_not_found"
                | "response_not_found"
                | "response_expired"
                | "invalid_previous_response_id"
        ) || (param == "previous_response_id"
            && ["not found", "expired", "does not exist"]
                .iter()
                .any(|s| detail.contains(s))))
    {
        return GatewayError::new(
            GatewayErrorKind::RemoteStateMissing,
            "服务端上下文已过期或不存在",
        );
    }
    if status == 400
        && (matches!(code.as_str(), "unsupported_parameter" | "unsupported_value")
            && matches!(param, "previous_response_id" | "store" | "conversation"))
    {
        return GatewayError::new(
            GatewayErrorKind::UnsupportedContinuation,
            "接口不支持服务端上下文参数",
        );
    }
    let (kind, message) = match status {
        401 | 403 => (
            GatewayErrorKind::Unauthorized,
            "模型接口拒绝访问，请检查 API 密钥与权限",
        ),
        429 => (
            GatewayErrorKind::RateLimited,
            "模型接口限流或额度不足，请稍后重试或检查额度",
        ),
        500..=599 => (
            GatewayErrorKind::OutcomeUnknown,
            "模型服务暂时异常，本次调用结果未确认；请勿连续重复发送",
        ),
        _ if code.contains("context_length") || code.contains("context_window") => (
            GatewayErrorKind::ContextExceeded,
            "模型上下文超限，请调整预算或新建对话；历史未删除",
        ),
        _ => (
            GatewayErrorKind::Rejected,
            "模型接口拒绝请求，请检查地址、模型和接口类型",
        ),
    };
    GatewayError::new(kind, format!("{message}（HTTP {status}）"))
}

impl OpenAiGateway {
    pub(super) async fn request(
        &self,
        c: &ConfigRef,
        key: &str,
        request: GenerationRequest,
    ) -> Result<GenerationResult, GatewayError> {
        let url = endpoint(c)?;
        let responses = protocol(c) == "responses";
        if !responses && (request.store || request.previous_response_id.is_some()) {
            return Err(GatewayError::new(
                GatewayErrorKind::UnsupportedContinuation,
                "普通聊天接口需要本地传入历史",
            ));
        }
        // Avoid the former 2048-token cap, especially with thinking models.
        let initial_budget = reqwest::Url::parse(&url)
            .as_ref().map(output_budget).unwrap_or(8192);
        let mut budget = initial_budget;
        loop {
            let body = if responses {
                responses_body(c, &request, budget)
            } else {
                json!({"model":c.model_name,"messages":request.messages,"stream":false,"max_tokens":budget})
            };
            let r = self
                .client_for(&url)
                .post(&url)
                .bearer_auth(key)
                .json(&body)
                .send()
                .await
                .map_err(|e| {
                    if e.is_connect() {
                        GatewayError::new(
                            GatewayErrorKind::Rejected,
                            "无法连接模型接口，请检查地址与网络",
                        )
                    } else {
                        GatewayError::new(
                            GatewayErrorKind::OutcomeUnknown,
                            "请求中断或超时，模型是否已处理尚未确认，请勿连续重复发送",
                        )
                    }
                })?;
            let status = r.status().as_u16();
            let provider_request_id = r
                .headers()
                .get("x-request-id")
                .or_else(|| r.headers().get("x-tt-logid"))
                .and_then(|s| s.to_str().ok())
                .map(str::to_owned);
            let bytes = r.bytes().await.map_err(|_| {
                GatewayError::new(
                    GatewayErrorKind::OutcomeUnknown,
                    "接收回复中断，本次结果未确认",
                )
            })?;
            let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            if !(200..300).contains(&status) {
                return Err(http_error(
                    status,
                    &body,
                    request.previous_response_id.is_some(),
                ));
            }
            if responses {
                if body["status"] == "incomplete" {
                    return Err(GatewayError::new(
                        GatewayErrorKind::ContextExceeded,
                        "模型输出未完成；未归档不完整回复，请缩小问题范围或调整模型",
                    ));
                }
                if body["status"] != "completed" {
                    return Err(GatewayError::new(
                        GatewayErrorKind::OutcomeUnknown,
                        "模型未返回可确认的完成状态；未重复发送",
                    ));
                }
                let output = body["output"].as_array().cloned().unwrap_or_default();
                let text = output
                    .iter()
                    .filter(|i| i["type"] == "message" && i["role"] == "assistant")
                    .flat_map(|i| i["content"].as_array().into_iter().flatten())
                    .filter(|p| p["type"] == "output_text")
                    .filter_map(|p| p["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                let response_id = body["id"]
                    .as_str()
                    .filter(|id| !id.is_empty())
                    .map(str::to_owned);
                if text.trim().is_empty() || (request.store && response_id.is_none()) {
                    return Err(GatewayError::new(
                        GatewayErrorKind::OutcomeUnknown,
                        "回复缺少文字或服务端接续标识；未确认结果，未自动重发",
                    ));
                }
                return Ok(GenerationResult {
                    text,
                    response_id,
                    output_items: output,
                    usage: body.get("usage").cloned(),
                    provider_request_id,
                });
            }
            if body
                .pointer("/choices/0/finish_reason")
                .and_then(Value::as_str)
                == Some("length")
            {
                // Only stateless Chat requests retry a known, completed truncation once.
                if budget == initial_budget {
                    budget *= 2;
                    continue;
                }
                return Err(GatewayError::new(
                    GatewayErrorKind::ContextExceeded,
                    format!("模型在 {budget} tokens 的回复预算内仍未完成，已自动扩大预算重试一次。请缩小提问范围或换用其他模型；未归档不完整报告"),
                ));
            }
            let text = body
                .pointer("/choices/0/message/content")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| {
                    GatewayError::new(GatewayErrorKind::InvalidResponse, "模型响应缺少文字内容")
                })?;
            return Ok(GenerationResult {
                text: text.into(),
                response_id: None,
                output_items: vec![],
                usage: body.get("usage").cloned(),
                provider_request_id,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::ModelGateway;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{Arc, Mutex},
    };

    fn server(
        replies: Vec<(u16, Value)>,
    ) -> (String, Arc<Mutex<Vec<Value>>>, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}/v1", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(vec![]));
        let capture = seen.clone();
        let handle = std::thread::spawn(move || {
            for (status, body) in replies {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let mut all = vec![];
                let mut chunk = [0u8; 4096];
                let header_end = loop {
                    let n = stream.read(&mut chunk).unwrap();
                    assert!(n > 0);
                    all.extend_from_slice(&chunk[..n]);
                    if let Some(i) = all.windows(4).position(|w| w == b"\r\n\r\n") {
                        break i + 4;
                    }
                };
                let headers = String::from_utf8_lossy(&all[..header_end]);
                let length = headers
                    .lines()
                    .find_map(|l| {
                        l.to_lowercase()
                            .strip_prefix("content-length:")
                            .map(|s| s.trim().parse::<usize>().unwrap())
                    })
                    .unwrap();
                let path = headers
                    .lines()
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .nth(1)
                    .unwrap()
                    .to_string();
                while all.len() < header_end + length {
                    let n = stream.read(&mut chunk).unwrap();
                    assert!(n > 0);
                    all.extend_from_slice(&chunk[..n]);
                }
                let mut request: Value =
                    serde_json::from_slice(&all[header_end..header_end + length]).unwrap();
                request["captured_path"] = json!(path);
                capture.lock().unwrap().push(request);
                let body = serde_json::to_vec(&body).unwrap();
                write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nX-Request-Id: fixture-request\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
                stream.write_all(&body).unwrap();
            }
        });
        (address, seen, handle)
    }
    fn config(base_url: String, protocol_name: &str) -> ConfigRef {
        ConfigRef {
            schema_version: 1,
            id: "config-test".into(),
            name: "test".into(),
            base_url,
            model_name: "fixture-model".into(),
            context_limit: 32000,
            secret_ref: "key-test".into(),
            text_capable: true,
            vision_capable: true,
            tested_at: "2026-10-20".into(),
            api_protocol: Some(protocol_name.into()),
            context_policy: Some("auto".into()),
            context_capability: None,
        }
    }
    fn response(id: &str) -> Value {
        json!({"id":id,"status":"completed","output":[
            {"type":"reasoning","summary":[{"type":"summary_text","text":"not the visible answer"}]},
            {"type":"message","role":"assistant","content":[{"type":"output_text","text":"visible reply"}]}],
            "usage":{"input_tokens":8,"output_tokens":3}})
    }
    #[tokio::test]
    async fn response_wire_format_converts_images_and_only_sends_incremental_input() {
        let (url, seen, handle) = server(vec![
            (200, response("resp-first")),
            (200, response("resp-second")),
        ]);
        let c = config(format!("{url}/responses"), "responses");
        let gateway = OpenAiGateway::default();
        let first = gateway.generate(&c, "synthetic-key", GenerationRequest {
            messages: vec![json!({"role":"system","content":"teacher instructions"}), json!({"role":"user","content":[{"type":"text","text":"original essay"},{"type":"image_url","image_url":{"url":"data:image/png;base64,abcd"}}]})],
            previous_response_id: None, store: true,
        }).await.unwrap();
        assert_eq!(first.text, "visible reply");
        assert_eq!(first.output_items.len(), 2);
        assert_eq!(
            first.provider_request_id.as_deref(),
            Some("fixture-request")
        );
        let second = gateway
            .generate(
                &c,
                "synthetic-key",
                GenerationRequest {
                    messages: vec![
                        json!({"role":"system","content":"teacher instructions"}),
                        json!({"role":"user","content":"follow-up only"}),
                    ],
                    previous_response_id: first.response_id,
                    store: true,
                },
            )
            .await
            .unwrap();
        assert_eq!(second.response_id.as_deref(), Some("resp-second"));
        handle.join().unwrap();
        let requests = seen.lock().unwrap();
        assert_eq!(requests[0]["captured_path"], "/v1/responses");
        assert_eq!(requests[0]["instructions"], "teacher instructions");
        assert_eq!(requests[0]["input"][0]["content"][0]["type"], "input_text");
        assert_eq!(requests[0]["input"][0]["content"][1]["type"], "input_image");
        assert_eq!(requests[1]["previous_response_id"], "resp-first");
        assert_eq!(requests[1]["input"].as_array().unwrap().len(), 1);
        assert!(!requests[1].to_string().contains("original essay"));
    }
    #[tokio::test]
    async fn stateless_responses_do_not_request_storage_and_missing_ids_are_not_success() {
        let mut missing = response("unused");
        missing.as_object_mut().unwrap().remove("id");
        let (url, seen, handle) = server(vec![(200, response("resp-local")), (200, missing)]);
        let c = config(url, "responses");
        let gateway = OpenAiGateway::default();
        gateway
            .generate(
                &c,
                "test",
                GenerationRequest::stateless(vec![
                    json!({"role":"user","content":"local history"}),
                ]),
            )
            .await
            .unwrap();
        let error = gateway
            .generate(
                &c,
                "test",
                GenerationRequest {
                    messages: vec![json!({"role":"user","content":"new"})],
                    previous_response_id: None,
                    store: true,
                },
            )
            .await
            .unwrap_err();
        assert_eq!(error.kind, GatewayErrorKind::OutcomeUnknown);
        handle.join().unwrap();
        assert_eq!(seen.lock().unwrap()[0]["store"], false);
    }
    #[tokio::test]
    async fn structured_remote_expiration_is_distinct_from_unauthorized_and_rate_limit() {
        for (status, body, expected) in [
            (
                400,
                json!({"error":{"code":"response_expired","param":"previous_response_id"}}),
                GatewayErrorKind::RemoteStateMissing,
            ),
            (
                401,
                json!({"error":{"message":"sensitive provider detail"}}),
                GatewayErrorKind::Unauthorized,
            ),
            (
                429,
                json!({"error":{"code":"rate_limit"}}),
                GatewayErrorKind::RateLimited,
            ),
            (
                404,
                json!({"error":{"code":"not_found"}}),
                GatewayErrorKind::Rejected,
            ),
        ] {
            let (url, seen, handle) = server(vec![(status, body)]);
            let error = OpenAiGateway::default()
                .generate(
                    &config(url, "responses"),
                    "test",
                    GenerationRequest {
                        messages: vec![],
                        previous_response_id: Some("old-id".into()),
                        store: true,
                    },
                )
                .await
                .unwrap_err();
            assert_eq!(error.kind, expected);
            assert!(!error.message.contains("sensitive provider detail"));
            handle.join().unwrap();
            assert_eq!(seen.lock().unwrap().len(), 1);
        }
    }
    #[tokio::test]
    async fn legacy_chat_uses_bounded_output_retry_and_never_returns_reasoning_as_answer() {
        let (url, seen, handle) = server(vec![
            (
                200,
                json!({"choices":[{"finish_reason":"length","message":{"content":"partial","reasoning_content":"thinking"}}]}),
            ),
            (
                200,
                json!({"choices":[{"finish_reason":"stop","message":{"content":"complete answer","reasoning_content":"thinking"}}]}),
            ),
        ]);
        let result = OpenAiGateway::default()
            .generate(
                &config(url, "chat_completions"),
                "test",
                GenerationRequest::stateless(vec![json!({"role":"user","content":"essay"})]),
            )
            .await
            .unwrap();
        assert_eq!(result.text, "complete answer");
        handle.join().unwrap();
        let requests = seen.lock().unwrap();
        assert_eq!(requests[0]["captured_path"], "/v1/chat/completions");
        assert_eq!(requests[0]["max_tokens"], 8192);
        assert_eq!(requests[1]["max_tokens"], 16384);
    }
}
