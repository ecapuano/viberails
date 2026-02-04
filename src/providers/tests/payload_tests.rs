//! Tests for payload structure logging and tool use detection.
//!
//! These tests verify that the payload classification works correctly
//! for different provider formats (Claude Code, OpenClaw, Cursor, etc.)

use serde_json::json;

use crate::providers::{LLmProviderTrait, claudecode::ClaudeCode};

/// Helper to create a Claude provider for testing is_tool_use
fn test_provider() -> ClaudeCode {
    ClaudeCode::with_custom_path("/usr/bin/test").unwrap()
}

// =============================================================================
// Tool Use Detection Tests
// =============================================================================

#[test]
fn test_is_tool_use_detects_tool_input() {
    let provider = test_provider();
    let payload = json!({
        "tool_input": {"command": "ls -la"},
        "tool_name": "Bash"
    });

    assert!(provider.is_tool_use(&payload));
}

#[test]
fn test_is_tool_use_detects_tool_name_only() {
    let provider = test_provider();
    let payload = json!({
        "tool_name": "Read"
    });

    assert!(provider.is_tool_use(&payload));
}

#[test]
fn test_is_tool_use_detects_tool_use_id() {
    let provider = test_provider();
    let payload = json!({
        "tool_use_id": "toolu_01ABC123"
    });

    assert!(provider.is_tool_use(&payload));
}

#[test]
fn test_is_tool_use_detects_openclaw_tool_name() {
    // OpenClaw uses "toolName" (camelCase) instead of "tool_name"
    let provider = test_provider();
    let payload = json!({
        "toolName": "execute_command",
        "params": {"cmd": "echo hello"}
    });

    assert!(provider.is_tool_use(&payload));
}

#[test]
fn test_is_tool_use_rejects_prompt_event() {
    let provider = test_provider();
    let payload = json!({
        "content": "Hello, can you help me?",
        "role": "user"
    });

    assert!(!provider.is_tool_use(&payload));
}

#[test]
fn test_is_tool_use_rejects_message_event() {
    let provider = test_provider();
    let payload = json!({
        "message": "User prompt text",
        "session_id": "abc123"
    });

    assert!(!provider.is_tool_use(&payload));
}

#[test]
fn test_is_tool_use_rejects_params_only() {
    // "params" alone is too generic and shouldn't trigger tool_use detection
    let provider = test_provider();
    let payload = json!({
        "params": {"key": "value"},
        "eventType": "some_event"
    });

    assert!(!provider.is_tool_use(&payload));
}

#[test]
fn test_is_tool_use_rejects_empty_object() {
    let provider = test_provider();
    let payload = json!({});

    assert!(!provider.is_tool_use(&payload));
}

#[test]
fn test_is_tool_use_handles_nested_tool_fields() {
    // Tool fields nested inside other objects shouldn't trigger detection
    let provider = test_provider();
    let payload = json!({
        "data": {
            "tool_name": "Bash"
        }
    });

    // Should NOT detect - tool_name is nested, not at top level
    assert!(!provider.is_tool_use(&payload));
}

// =============================================================================
// Payload Classification Tests (Claude Code format)
// =============================================================================

#[test]
fn test_claude_code_tool_use_payload() {
    let provider = test_provider();

    // Real Claude Code PreToolUse payload format
    let payload = json!({
        "tool_name": "Bash",
        "tool_input": {
            "command": "cargo build"
        },
        "tool_use_id": "toolu_01XYZ789",
        "session_id": "session-abc-123"
    });

    assert!(provider.is_tool_use(&payload));
}

#[test]
fn test_claude_code_prompt_payload() {
    let provider = test_provider();

    // Real Claude Code UserPromptSubmit payload format
    let payload = json!({
        "prompt": "Please help me write a function",
        "session_id": "session-abc-123"
    });

    assert!(!provider.is_tool_use(&payload));
}

// =============================================================================
// Payload Classification Tests (OpenClaw format)
// =============================================================================

#[test]
fn test_openclaw_before_tool_call_payload() {
    let provider = test_provider();

    // OpenClaw before_tool_call event format
    let payload = json!({
        "eventType": "before_tool_call",
        "toolName": "execute_shell",
        "params": {
            "command": "npm install"
        },
        "agentId": "agent-123",
        "sessionKey": "session-456"
    });

    assert!(provider.is_tool_use(&payload));
}

#[test]
fn test_openclaw_message_received_payload() {
    let provider = test_provider();

    // OpenClaw message_received event format
    let payload = json!({
        "eventType": "message_received",
        "content": "Can you help me debug this?",
        "role": "user",
        "timestamp": "2024-01-15T10:30:00Z",
        "agentId": "agent-123"
    });

    assert!(!provider.is_tool_use(&payload));
}

#[test]
fn test_openclaw_message_sent_payload() {
    let provider = test_provider();

    // OpenClaw message_sent event format (LLM response)
    let payload = json!({
        "eventType": "message_sent",
        "content": "I'll help you debug that issue.",
        "role": "assistant",
        "timestamp": "2024-01-15T10:30:05Z"
    });

    assert!(!provider.is_tool_use(&payload));
}

// =============================================================================
// Payload Classification Tests (Codex format)
// =============================================================================

#[test]
fn test_codex_notification_payload() {
    let provider = test_provider();

    // Codex notify event (agent-turn-complete style)
    let payload = json!({
        "type": "agent-turn-complete",
        "data": {
            "turn_id": "turn-123",
            "result": "success"
        }
    });

    // Codex notifications are never tool_use events
    assert!(!provider.is_tool_use(&payload));
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn test_payload_with_null_tool_name() {
    let provider = test_provider();
    let payload = json!({
        "tool_name": null
    });

    // null value should still be detected as having the key
    assert!(provider.is_tool_use(&payload));
}

#[test]
fn test_payload_with_empty_tool_name() {
    let provider = test_provider();
    let payload = json!({
        "tool_name": ""
    });

    // Empty string should still be detected
    assert!(provider.is_tool_use(&payload));
}

#[test]
fn test_payload_array_not_object() {
    let provider = test_provider();
    let payload = json!(["tool_name", "Bash"]);

    // Arrays should not match
    assert!(!provider.is_tool_use(&payload));
}

#[test]
fn test_payload_primitive_not_object() {
    let provider = test_provider();
    let payload = json!("tool_name");

    // Primitives should not match
    assert!(!provider.is_tool_use(&payload));
}
