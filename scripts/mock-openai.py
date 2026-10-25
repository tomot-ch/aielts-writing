#!/usr/bin/env python3
"""Local synthetic OpenAI fixture; never use as a real model provider.

--record captures JSON request bodies (no Authorization header) for acceptance.
"""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import argparse
import json
import re
import threading
import uuid

LOCK = threading.RLock()
RESPONSES = {}
RECORD_PATH = None
FAULT_ONCE = None
FAULT_TRIGGERED = False
ANNOTATED = False


def text_of(value):
    if isinstance(value, str):
        return value
    if isinstance(value, list):
        return "\n".join(text_of(v) for v in value)
    if isinstance(value, dict):
        return text_of(value.get("content", value.get("text", "")))
    return ""


def reply_for(messages):
    query = text_of(messages[-1]) if messages else ""
    full = text_of(messages)
    if "Return only the exact secret token" in query:
        found = re.search(r"Remember this secret token for the next turn: (context-[a-f0-9-]+)", full)
        return found.group(1) if found else "NO_MEMORY"
    if "Remember this secret token" in query:
        return "OK"
    if "WRITING_OK" in query:
        return "WRITING_OK"
    if "four digits" in query:
        return "2468"
    if "准确转录题目" in query:
        return "图表显示多个类别随时间变化；请对照原图确认细节。"
    if "归纳共性问题" in full:
        return "# 总体分析\n## 共性问题\n连接词使用过于单一。\n## 练习建议\n每周练习比较段落。"
    if ANNOTATED and "annotated-review-v1" in full and not any(m.get("role") == "assistant" for m in messages):
        from pathlib import Path
        fixture = json.loads((Path(__file__).resolve().parents[1] / "tests/fixtures/annotated-review.json").read_text())
        return json.dumps(fixture["response"], ensure_ascii=False)
    marker = "ALPHA" if "SYNTHETIC_ALPHA" in full else "BETA" if "SYNTHETIC_BETA" in full else "作文"
    return f"# {marker} 写作分析\n## 任务回应\n论证需要具体例子。\n## 问题归纳\n主题句和证据之间缺少清晰连接。"


class Handler(BaseHTTPRequestHandler):
    def respond(self, status, body):
        raw = json.dumps(body, ensure_ascii=False).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)

    def do_GET(self):
        if self.path == "/v1/models":
            self.respond(200, {"data": [{"id": "fixture-model"}, {"id": "fixture-stateless"}]})
        else:
            self.respond(404, {"error": {"code": "not_found"}})

    def do_DELETE(self):
        if self.path.startswith("/v1/responses/"):
            with LOCK:
                RESPONSES.pop(self.path.rsplit("/", 1)[-1], None)
            self.respond(200, {"deleted": True})
        else:
            self.respond(404, {"error": {"code": "not_found"}})

    def do_POST(self):
        global FAULT_TRIGGERED
        if self.path not in ("/v1/chat/completions", "/v1/responses"):
            self.respond(404, {"error": {"code": "not_found"}})
            return
        size = int(self.headers.get("Content-Length", "0"))
        if size > 10_000_000:
            self.respond(413, {"error": {"code": "too_large"}})
            return
        try:
            body = json.loads(self.rfile.read(size))
            responses = self.path.endswith("/responses")
            current = body.get("input", []) if responses else body["messages"]
            if isinstance(current, str):
                current = [{"role": "user", "content": current}]
            previous = body.get("previous_response_id")
            with LOCK:
                if RECORD_PATH:
                    with open(RECORD_PATH, "a") as output:
                        output.write(json.dumps({"path": self.path, "body": body}, ensure_ascii=False) + "\n")
                history = []
                if previous and body.get("model") != "fixture-stateless":
                    if previous not in RESPONSES:
                        self.respond(404, {"error": {"code": "response_not_found", "param": "previous_response_id"}})
                        return
                    history = RESPONSES[previous]
                messages = history + current
                result = reply_for(messages)
                if responses:
                    response_id = "resp-" + uuid.uuid4().hex
                    if body.get("store") and body.get("model") != "fixture-stateless":
                        RESPONSES[response_id] = messages + [{"role": "assistant", "content": result}]
                    response = {"id": response_id, "status": "completed", "output": [{"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": result}]}], "usage": {"input_tokens": len(text_of(messages)), "output_tokens": len(result)}}
                else:
                    response = {"choices": [{"finish_reason": "stop", "message": {"role": "assistant", "content": result}}]}
            # Optional acceptance fault: the model has processed/stored this turn,
            # but the caller receives a 500 and must treat the outcome as unknown.
            if FAULT_ONCE and FAULT_ONCE in text_of(current) and not FAULT_TRIGGERED:
                FAULT_TRIGGERED = True
                self.respond(500, {"error": {"code": "internal_server_error"}})
                return
            self.respond(200, response)
            print(json.dumps({"event": "request", "path": self.path, "model": body.get("model"), "messages": len(current), "continuing": bool(previous), "store": body.get("store", False)}, ensure_ascii=False), flush=True)
        except (ValueError, KeyError, TypeError):
            self.respond(400, {"error": {"code": "invalid_request"}})

    def log_message(self, *args):
        pass


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=18765)
    parser.add_argument("--record")
    parser.add_argument("--fault-once", help="simulate one uncertain result for this synthetic marker")
    parser.add_argument("--annotations", action="store_true", help="return the isolated annotated-review fixture")
    args = parser.parse_args()
    ANNOTATED = args.annotations
    RECORD_PATH = args.record
    FAULT_ONCE = args.fault_once
    print(f"Synthetic fixture listening on http://127.0.0.1:{args.port}/v1", flush=True)
    ThreadingHTTPServer(("127.0.0.1", args.port), Handler).serve_forever()
