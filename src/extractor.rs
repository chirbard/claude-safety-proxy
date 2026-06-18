use serde_json::Value;

pub fn extract_text(body: &[u8]) -> Vec<String> {
    let Ok(root) = serde_json::from_slice::<Value>(body) else {
        return vec![];
    };

    let mut texts = Vec::new();

    if let Some(system) = root.get("system") {
        collect_content(system, &mut texts);
    }

    if let Some(messages) = root.get("messages").and_then(|v| v.as_array()) {
        for msg in messages {
            if let Some(content) = msg.get("content") {
                collect_content(content, &mut texts);
            }
        }
    }

    // tool results embedded in messages are already covered above via content blocks;
    // also pull top-level tool inputs if present
    if let Some(tools) = root.get("tools").and_then(|v| v.as_array()) {
        for tool in tools {
            if let Some(desc) = tool.get("description").and_then(|v| v.as_str()) {
                texts.push(desc.to_string());
            }
        }
    }

    texts
}

fn collect_content(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(s) => {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                out.push(trimmed.to_string());
            }
        }
        Value::Array(arr) => {
            for item in arr {
                collect_content_block(item, out);
            }
        }
        _ => {}
    }
}

fn collect_content_block(block: &Value, out: &mut Vec<String>) {
    let kind = block.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match kind {
        "text" => {
            if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    out.push(trimmed.to_string());
                }
            }
        }
        "tool_result" => {
            if let Some(content) = block.get("content") {
                collect_content(content, out);
            }
        }
        "tool_use" => {
            if let Some(input) = block.get("input") {
                out.push(input.to_string());
            }
        }
        _ => {
            // unknown block type – include any string fields
            if let Some(obj) = block.as_object() {
                for (_, v) in obj {
                    if let Some(s) = v.as_str() {
                        let trimmed = s.trim();
                        if !trimmed.is_empty() {
                            out.push(trimmed.to_string());
                        }
                    }
                }
            }
        }
    }
}
