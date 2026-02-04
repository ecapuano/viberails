use std::io::Write;

use tempfile::NamedTempFile;

use crate::providers::extract_last_response_from_transcript;

#[test]
fn test_extract_response_from_transcript_with_text() {
    let mut file = NamedTempFile::new().unwrap();

    // Write a sample transcript with an assistant message containing text
    writeln!(
        file,
        r#"{{"type":"user","message":{{"role":"user","content":"Hello"}}}}"#
    )
    .unwrap();
    writeln!(
        file,
        r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"Hello! How can I help you today?"}}]}}}}"#
    )
    .unwrap();

    let result = extract_last_response_from_transcript(file.path());
    assert_eq!(result, Some("Hello! How can I help you today?".to_string()));
}

#[test]
fn test_extract_response_from_transcript_multiple_messages() {
    let mut file = NamedTempFile::new().unwrap();

    // Write multiple assistant messages - should return the last one
    writeln!(
        file,
        r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"First response"}}]}}}}"#
    )
    .unwrap();
    writeln!(
        file,
        r#"{{"type":"user","message":{{"role":"user","content":"Follow up"}}}}"#
    )
    .unwrap();
    writeln!(
        file,
        r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"Second response"}}]}}}}"#
    )
    .unwrap();

    let result = extract_last_response_from_transcript(file.path());
    assert_eq!(result, Some("Second response".to_string()));
}

#[test]
fn test_extract_response_from_transcript_with_thinking() {
    let mut file = NamedTempFile::new().unwrap();

    // Write an assistant message with thinking and text
    writeln!(
        file,
        r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"thinking","thinking":"Let me think..."}},{{"type":"text","text":"Here is my answer"}}]}}}}"#
    )
    .unwrap();

    let result = extract_last_response_from_transcript(file.path());
    // Should only include text, not thinking
    assert_eq!(result, Some("Here is my answer".to_string()));
}

#[test]
fn test_extract_response_from_transcript_no_text() {
    let mut file = NamedTempFile::new().unwrap();

    // Write an assistant message with only tool_use (no text)
    writeln!(
        file,
        r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"tool_use","id":"123","name":"Bash"}}]}}}}"#
    )
    .unwrap();

    let result = extract_last_response_from_transcript(file.path());
    assert_eq!(result, None);
}

#[test]
fn test_extract_response_from_transcript_no_assistant_message() {
    let mut file = NamedTempFile::new().unwrap();

    // Write only user messages
    writeln!(
        file,
        r#"{{"type":"user","message":{{"role":"user","content":"Hello"}}}}"#
    )
    .unwrap();

    let result = extract_last_response_from_transcript(file.path());
    assert_eq!(result, None);
}

#[test]
fn test_extract_response_from_transcript_empty_file() {
    let file = NamedTempFile::new().unwrap();

    let result = extract_last_response_from_transcript(file.path());
    assert_eq!(result, None);
}

#[test]
fn test_extract_response_from_transcript_multiple_text_parts() {
    let mut file = NamedTempFile::new().unwrap();

    // Write an assistant message with multiple text parts
    writeln!(
        file,
        r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"Part 1"}},{{"type":"text","text":"Part 2"}}]}}}}"#
    )
    .unwrap();

    let result = extract_last_response_from_transcript(file.path());
    assert_eq!(result, Some("Part 1\nPart 2".to_string()));
}

#[test]
fn test_extract_response_nonexistent_file() {
    use std::path::Path;
    let result = extract_last_response_from_transcript(Path::new("/nonexistent/path/file.jsonl"));
    assert_eq!(result, None);
}
