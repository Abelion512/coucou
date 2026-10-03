//! The PermissionRequest decision JSON, shared by both platform halves of the
//! relay. It used to live in main.rs; extracting it keeps the wire format Claude
//! Code expects in exactly one place while `win.rs` / `unix.rs` stay about
//! transports only.

use serde_json::{json, Value};

/// The documented PermissionRequest output. Anything we do not recognise prints
/// nothing at all rather than guessing — silence is the safe answer.
/// See https://code.claude.com/docs/en/hooks
pub fn decision_json(decision: &str) -> Option<String> {
    let behavior = match decision.trim() {
        // "always" still answers a plain allow; remembering it is the island's
        // business, not Claude Code's.
        "allow" | "always" => r#"{"behavior":"allow"}"#.to_string(),
        "deny" => r#"{"behavior":"deny","message":"Denied from Coucou"}"#.to_string(),
        _ => return None,
    };
    Some(format!(
        r#"{{"hookSpecificOutput":{{"hookEventName":"PermissionRequest","decision":{behavior}}}}}"#
    ))
}

/// The documented PreToolUse answer for `AskUserQuestion`: allow the call and
/// hand the choices back inside `updatedInput`, keyed by the question text.
///
/// `reply` is what the island wrote on the socket — `{"decision":"answer",
/// "answers":{…}}`. Anything else (including `ask`, which is how the island
/// says "answer this one in the terminal") prints nothing, and Claude Code
/// re-asks the question itself.
pub fn ask_answer_json(reply: &str, questions: &Value) -> Option<String> {
    let parsed: Value = serde_json::from_str(reply).ok()?;
    if parsed.get("decision")?.as_str()? != "answer" {
        return None;
    }
    let answers = parsed.get("answers")?.clone();
    if !answers.is_object() || answers.as_object()?.is_empty() {
        return None;
    }
    let out = json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "allow",
            "updatedInput": { "questions": questions, "answers": answers },
        }
    });
    out.get("hookSpecificOutput")
        .and_then(|v| v.get("updatedInput"))
        .filter(|v| v.get("answers").is_some())
        .map(|_| out.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn decision_json_matches_the_documented_shape() {
        assert_eq!(
            decision_json("allow").unwrap(),
            r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}"#
        );
        assert_eq!(
            decision_json("deny").unwrap(),
            r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny","message":"Denied from Coucou"}}}"#
        );
        // "always" is an island concept; Claude Code just gets an allow.
        assert!(decision_json("always").unwrap().contains(r#""behavior":"allow""#));
    }

    #[test]
    fn anything_unrecognised_prints_nothing() {
        assert!(decision_json("").is_none());
        assert!(decision_json("maybe").is_none());
        // The shape the app used to send must not be mistaken for a decision.
        assert!(decision_json(r#"{"permissionDecision":"allow"}"#).is_none());
    }

    #[test]
    fn an_answer_comes_back_as_an_allowed_call_with_updated_input() {
        let questions = json!([{ "question": "Deploy?", "options": [{ "label": "yes" }] }]);
        let out = ask_answer_json(
            r#"{"decision":"answer","answers":{"Deploy?":"yes"}}"#,
            &questions,
        )
        .expect("an answer should print");
        let parsed: Value = serde_json::from_str(&out).unwrap();
        let specific = &parsed["hookSpecificOutput"];
        assert_eq!(specific["hookEventName"], "PreToolUse");
        assert_eq!(specific["permissionDecision"], "allow");
        assert_eq!(specific["updatedInput"]["answers"]["Deploy?"], "yes");
        assert_eq!(specific["updatedInput"]["questions"][0]["question"], "Deploy?");
    }

    #[test]
    fn a_multi_select_answer_keeps_its_array() {
        let questions = json!([{ "question": "Pick" }]);
        let out = ask_answer_json(
            r#"{"decision":"answer","answers":{"Pick":["a","b"]}}"#,
            &questions,
        )
        .unwrap();
        let parsed: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["hookSpecificOutput"]["updatedInput"]["answers"]["Pick"], json!(["a", "b"]));
    }

    #[test]
    fn ask_and_silence_both_print_nothing() {
        let questions = json!([{ "question": "Deploy?" }]);
        // "Reply in terminal" and a dropped connection must leave Claude Code to
        // ask the question itself, not print a half-answer.
        assert!(ask_answer_json(r#"{"decision":"ask"}"#, &questions).is_none());
        assert!(ask_answer_json("", &questions).is_none());
        assert!(ask_answer_json("allow", &questions).is_none());
        assert!(ask_answer_json(r#"{"decision":"answer","answers":{}}"#, &questions).is_none());
    }
}
