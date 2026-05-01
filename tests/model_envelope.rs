//! Tests for SpanKindClass::from_name. LLR:
//! - Span name classified into kind class

use ghcp_mon::model::SpanKindClass;

#[test]
fn from_name_classifies_invoke_agent_exact_and_prefix() {
    assert_eq!(SpanKindClass::from_name("invoke_agent"), SpanKindClass::InvokeAgent);
    assert_eq!(SpanKindClass::from_name("invoke_agent foo"), SpanKindClass::InvokeAgent);
}

#[test]
fn from_name_classifies_chat_prefix() {
    assert_eq!(SpanKindClass::from_name("chat"), SpanKindClass::Chat);
    assert_eq!(SpanKindClass::from_name("chat gpt-5.4"), SpanKindClass::Chat);
}

#[test]
fn from_name_classifies_execute_tool_prefix() {
    assert_eq!(SpanKindClass::from_name("execute_tool"), SpanKindClass::ExecuteTool);
    assert_eq!(SpanKindClass::from_name("execute_tool bash"), SpanKindClass::ExecuteTool);
}

#[test]
fn from_name_classifies_external_tool_prefix() {
    assert_eq!(SpanKindClass::from_name("external_tool"), SpanKindClass::ExternalTool);
    assert_eq!(SpanKindClass::from_name("external_tool foo"), SpanKindClass::ExternalTool);
}

#[test]
fn from_name_classifies_unknown_as_other() {
    assert_eq!(SpanKindClass::from_name(""), SpanKindClass::Other);
    assert_eq!(SpanKindClass::from_name("something"), SpanKindClass::Other);
    assert_eq!(SpanKindClass::from_name("invoke_agentXYZ"), SpanKindClass::Other);
}
