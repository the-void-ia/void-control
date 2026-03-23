#[cfg(feature = "serde")]
use std::collections::{BTreeMap, HashMap};

#[cfg(feature = "serde")]
use serde_json::Value;

#[cfg(feature = "serde")]
use super::types::{
    CandidateInbox, CommunicationIntent, CommunicationIntentAudience, InboxEntry, InboxSnapshot,
    RoutedMessage, RoutedMessageStatus,
};

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
