#[cfg(feature = "serde")]
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[cfg(feature = "serde")]
use serde_json::Value;

#[cfg(feature = "serde")]
use super::types::{
    CandidateInbox, CommunicationIntent, CommunicationIntentAudience, CommunicationIntentKind,
    CommunicationIntentPriority, InboxEntry, InboxSnapshot, MessageStats, RoutedMessage,
    RoutedMessageStatus,
};

#[cfg(feature = "serde")]
pub fn merge_and_dedup(
    sidecar_intents: Vec<CommunicationIntent>,
    structured_output_intents: Vec<CommunicationIntent>,
) -> Vec<CommunicationIntent> {
    let mut merged = Vec::with_capacity(sidecar_intents.len() + structured_output_intents.len());
    let mut seen = BTreeSet::new();

    for intent in sidecar_intents
        .into_iter()
        .chain(structured_output_intents.into_iter())
    {
        let dedup_key = message_dedup_key(&intent);
        if seen.insert(dedup_key) {
            merged.push(intent);
        }
    }

    merged
}

#[cfg(feature = "serde")]
pub fn normalize_intents(
    candidate_id: &str,
    iteration: u32,
    intents: &[CommunicationIntent],
) -> (Vec<CommunicationIntent>, usize) {
    let mut valid = Vec::new();
    let mut rejected = 0usize;
    let mut broadcast_count = 0usize;

    for intent in intents {
        if valid.len() >= 3 {
            rejected += 1;
            continue;
        }
        if intent.intent_id.trim().is_empty() || intent.ttl_iterations == 0 {
            rejected += 1;
            continue;
        }
        if !payload_has_summary_text(&intent.payload) {
            rejected += 1;
            continue;
        }
        if matches!(intent.audience, CommunicationIntentAudience::Broadcast) {
            broadcast_count += 1;
            if broadcast_count > 1 {
                rejected += 1;
                continue;
            }
        }

        let mut normalized = intent.clone();
        normalized.from_candidate_id = candidate_id.to_string();
        normalized.iteration = iteration;
        valid.push(normalized);
    }

    (valid, rejected)
}

#[cfg(feature = "serde")]
pub fn route_intents(intents: &[CommunicationIntent]) -> Vec<RoutedMessage> {
    intents
        .iter()
        .map(|intent| {
            let (to, routing_reason) = match intent.audience {
                CommunicationIntentAudience::Leader => {
                    ("leader".to_string(), "leader_feedback_channel".to_string())
                }
                CommunicationIntentAudience::Broadcast => {
                    ("broadcast".to_string(), "broadcast_fanout".to_string())
                }
            };
            RoutedMessage {
                message_id: format!("msg-{}-{}", intent.intent_id, to),
                intent_id: intent.intent_id.clone(),
                to,
                delivery_iteration: intent.iteration + 1,
                routing_reason,
                status: RoutedMessageStatus::Routed,
            }
        })
        .collect()
}

#[cfg(feature = "serde")]
pub fn extract_message_stats(
    intents: &[CommunicationIntent],
    routed_messages: &[RoutedMessage],
    delivery_iteration: u32,
) -> MessageStats {
    let intents_by_id: BTreeMap<_, _> = intents
        .iter()
        .map(|intent| (intent.intent_id.clone(), intent))
        .collect();
    let mut stats = MessageStats {
        iteration: delivery_iteration,
        ..MessageStats::default()
    };
    let mut unique_sources = BTreeSet::new();
    let mut unique_intents = BTreeSet::new();

    for message in routed_messages
        .iter()
        .filter(|message| message.delivery_iteration == delivery_iteration)
    {
        let Some(intent) = intents_by_id.get(&message.intent_id) else {
            continue;
        };

        stats.total_messages += 1;
        unique_intents.insert(intent.intent_id.clone());
        unique_sources.insert(intent.from_candidate_id.clone());

        match message.to.as_str() {
            "leader" => stats.leader_messages += 1,
            "broadcast" => stats.broadcast_messages += 1,
            _ => {}
        }

        match intent.kind {
            CommunicationIntentKind::Proposal => stats.proposal_count += 1,
            CommunicationIntentKind::Signal => stats.signal_count += 1,
            CommunicationIntentKind::Evaluation => stats.evaluation_count += 1,
        }

        match intent.priority {
            CommunicationIntentPriority::High => stats.high_priority_count += 1,
            CommunicationIntentPriority::Normal => stats.normal_priority_count += 1,
            CommunicationIntentPriority::Low => stats.low_priority_count += 1,
        }

        match message.status {
            RoutedMessageStatus::Delivered => stats.delivered_count += 1,
            RoutedMessageStatus::Dropped => stats.dropped_count += 1,
            RoutedMessageStatus::Expired => stats.expired_count += 1,
            RoutedMessageStatus::Routed => {}
        }
    }

    stats.unique_sources = unique_sources.len();
    stats.unique_intent_count = unique_intents.len();
    stats
}

#[cfg(feature = "serde")]
pub fn pending_delivery_messages(
    intents: &[CommunicationIntent],
    messages: &[RoutedMessage],
    delivery_iteration: u32,
) -> Vec<(CommunicationIntent, RoutedMessage)> {
    let intents_by_id: HashMap<_, _> = intents
        .iter()
        .cloned()
        .map(|intent| (intent.intent_id.clone(), intent))
        .collect();
    let mut latest_by_message = BTreeMap::new();
    for message in messages {
        latest_by_message.insert(message.message_id.clone(), message.clone());
    }

    latest_by_message
        .into_values()
        .filter(|message| {
            message.delivery_iteration == delivery_iteration
                && message.status == RoutedMessageStatus::Routed
        })
        .filter_map(|message| {
            let intent = intents_by_id.get(&message.intent_id)?.clone();
            if intent.iteration + intent.ttl_iterations < delivery_iteration {
                return None;
            }
            Some((intent, message))
        })
        .collect()
}

#[cfg(feature = "serde")]
pub fn backlog_from_pending_messages(
    intents: &[CommunicationIntent],
    messages: &[RoutedMessage],
    delivery_iteration: u32,
) -> Vec<String> {
    pending_delivery_messages(intents, messages, delivery_iteration)
        .into_iter()
        .map(|(intent, _)| summary_text(&intent.payload))
        .collect()
}

#[cfg(feature = "serde")]
pub fn materialize_inbox_snapshots(
    execution_id: &str,
    delivery_iteration: u32,
    candidate_inboxes: &[CandidateInbox],
    intents: &[CommunicationIntent],
    messages: &[RoutedMessage],
) -> Vec<(InboxSnapshot, Vec<RoutedMessage>)> {
    let pending = pending_delivery_messages(intents, messages, delivery_iteration);
    if candidate_inboxes.is_empty() {
        return Vec::new();
    }

    let mut snapshots: Vec<_> = candidate_inboxes
        .iter()
        .map(|inbox| InboxSnapshot {
            execution_id: execution_id.to_string(),
            candidate_id: inbox.candidate_id.clone(),
            iteration: delivery_iteration,
            entries: Vec::new(),
        })
        .collect();
    let mut delivered_records = vec![Vec::new(); snapshots.len()];

    for (intent, message) in pending {
        let entry = InboxEntry {
            message_id: message.message_id.clone(),
            intent_id: intent.intent_id.clone(),
            from_candidate_id: intent.from_candidate_id.clone(),
            kind: intent.kind.clone(),
            payload: intent.payload.clone(),
        };
        match message.to.as_str() {
            "broadcast" => {
                for (idx, snapshot) in snapshots.iter_mut().enumerate() {
                    snapshot.entries.push(entry.clone());
                    delivered_records[idx].push(RoutedMessage {
                        status: RoutedMessageStatus::Delivered,
                        ..message.clone()
                    });
                }
            }
            _ => {
                snapshots[0].entries.push(entry);
                delivered_records[0].push(RoutedMessage {
                    status: RoutedMessageStatus::Delivered,
                    ..message
                });
            }
        }
    }

    snapshots.into_iter().zip(delivered_records).collect()
}

#[cfg(feature = "serde")]
pub fn build_candidate_inboxes(
    delivery_iteration: u32,
    candidate_count: usize,
    intents: &[CommunicationIntent],
    messages: &[RoutedMessage],
) -> Vec<CandidateInbox> {
    let mut inboxes: Vec<_> = (0..candidate_count)
        .map(|idx| CandidateInbox::new(&format!("candidate-{}", idx + 1)))
        .collect();
    let pending = pending_delivery_messages(intents, messages, delivery_iteration);

    for (intent, message) in pending {
        let summary = summary_text(&intent.payload);
        match message.to.as_str() {
            "broadcast" => {
                for inbox in &mut inboxes {
                    inbox.messages.push(summary.clone());
                }
            }
            _ => {
                if let Some(first) = inboxes.first_mut() {
                    first.messages.push(summary);
                }
            }
        }
    }

    if inboxes.is_empty() {
        return vec![CandidateInbox::new("candidate-1")];
    }
    inboxes
}

#[cfg(feature = "serde")]
fn payload_has_summary_text(payload: &Value) -> bool {
    payload
        .get("summary_text")
        .and_then(Value::as_str)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

#[cfg(feature = "serde")]
fn summary_text(payload: &Value) -> String {
    payload
        .get("summary_text")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

#[cfg(feature = "serde")]
fn message_dedup_key(intent: &CommunicationIntent) -> String {
    format!(
        "{}|{}|{}",
        normalized_payload_key(&intent.payload),
        audience_key(&intent.audience),
        intent.iteration
    )
}

#[cfg(feature = "serde")]
fn normalized_payload_key(payload: &Value) -> String {
    let mut out = String::new();
    append_normalized_value(payload, &mut out);
    out
}

#[cfg(feature = "serde")]
fn append_normalized_value(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
        Value::Number(value) => out.push_str(&value.to_string()),
        Value::String(value) => out.push_str(
            &serde_json::to_string(value)
                .expect("serialize string value for canonical payload key"),
        ),
        Value::Array(values) => {
            out.push('[');
            for (idx, item) in values.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                append_normalized_value(item, out);
            }
            out.push(']');
        }
        Value::Object(values) => {
            out.push('{');
            let mut keys: Vec<_> = values.keys().collect();
            keys.sort();
            for (idx, key) in keys.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                out.push_str(
                    &serde_json::to_string(key)
                        .expect("serialize object key for canonical payload key"),
                );
                out.push(':');
                append_normalized_value(&values[*key], out);
            }
            out.push('}');
        }
    }
}

#[cfg(feature = "serde")]
fn audience_key(audience: &CommunicationIntentAudience) -> &'static str {
    match audience {
        CommunicationIntentAudience::Leader => "leader",
        CommunicationIntentAudience::Broadcast => "broadcast",
    }
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::{merge_and_dedup, CommunicationIntent, CommunicationIntentAudience};
    use crate::orchestration::{CommunicationIntentKind, CommunicationIntentPriority};
    use serde_json::json;

    fn intent(
        intent_id: &str,
        from_candidate_id: &str,
        iteration: u32,
        audience: CommunicationIntentAudience,
        payload: serde_json::Value,
    ) -> CommunicationIntent {
        CommunicationIntent {
            intent_id: intent_id.to_string(),
            from_candidate_id: from_candidate_id.to_string(),
            iteration,
            kind: CommunicationIntentKind::Proposal,
            audience,
            payload,
            priority: CommunicationIntentPriority::Normal,
            ttl_iterations: 1,
            caused_by: None,
            context: None,
        }
    }

    #[test]
    fn merge_and_dedup_prefers_sidecar_for_exact_duplicate() {
        let sidecar = vec![intent(
            "sidecar-1",
            "candidate-1",
            2,
            CommunicationIntentAudience::Leader,
            json!({
                "summary_text": "keep cache warm",
                "strategy_hint": "cache",
            }),
        )];
        let structured_output = vec![intent(
            "output-1",
            "candidate-1",
            2,
            CommunicationIntentAudience::Leader,
            json!({
                "strategy_hint": "cache",
                "summary_text": "keep cache warm",
            }),
        )];

        let merged = merge_and_dedup(sidecar, structured_output);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].intent_id, "sidecar-1");
    }

    #[test]
    fn merge_and_dedup_keeps_intents_when_audience_or_iteration_differs() {
        let sidecar = vec![
            intent(
                "sidecar-1",
                "candidate-1",
                2,
                CommunicationIntentAudience::Leader,
                json!({
                    "summary_text": "route to leader",
                }),
            ),
            intent(
                "sidecar-2",
                "candidate-1",
                3,
                CommunicationIntentAudience::Broadcast,
                json!({
                    "summary_text": "same content, later iteration",
                }),
            ),
        ];
        let structured_output = vec![
            intent(
                "output-1",
                "candidate-1",
                2,
                CommunicationIntentAudience::Broadcast,
                json!({
                    "summary_text": "route to leader",
                }),
            ),
            intent(
                "output-2",
                "candidate-1",
                4,
                CommunicationIntentAudience::Broadcast,
                json!({
                    "summary_text": "same content, later iteration",
                }),
            ),
        ];

        let merged = merge_and_dedup(sidecar, structured_output);

        assert_eq!(
            merged
                .iter()
                .map(|intent| intent.intent_id.as_str())
                .collect::<Vec<_>>(),
            vec!["sidecar-1", "sidecar-2", "output-1", "output-2"]
        );
    }
}
