use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Lamport timestamp for message ordering
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LamportTimestamp {
    /// The logical clock value
    pub counter: u64,
    /// The node/user ID for tie-breaking
    pub node_id: Uuid,
}

impl LamportTimestamp {
    pub fn new(counter: u64, node_id: Uuid) -> Self {
        Self { counter, node_id }
    }

    pub fn initial(node_id: Uuid) -> Self {
        Self {
            counter: 0,
            node_id,
        }
    }

    pub fn increment(&self) -> Self {
        Self {
            counter: self.counter + 1,
            node_id: self.node_id,
        }
    }

    pub fn update(&self, received: &LamportTimestamp) -> Self {
        Self {
            counter: std::cmp::max(self.counter, received.counter) + 1,
            node_id: self.node_id,
        }
    }
}

/// Message operation for CRDT sync
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MessageOperation {
    pub op_type: MessageOpType,
    pub timestamp: LamportTimestamp,
    pub message_id: Uuid,
    pub conversation_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageOpType {
    Send {
        sender_id: Uuid,
        content: String,
        message_type: String,
    },
    Edit {
        new_content: String,
    },
    Delete,
    Read {
        reader_id: Uuid,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEntry {
    pub id: Uuid,
    pub sender_id: Uuid,
    pub content: String,
    pub message_type: String,
    pub timestamp: LamportTimestamp,
    pub is_deleted: bool,
    pub read_by: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageState {
    pub messages: Vec<MessageEntry>,
    pub current_timestamp: LamportTimestamp,
}

impl MessageState {
    pub fn new(node_id: Uuid) -> Self {
        Self {
            messages: Vec::new(),
            current_timestamp: LamportTimestamp::initial(node_id),
        }
    }

    pub fn apply(&mut self, op: MessageOperation) {
        self.current_timestamp = self.current_timestamp.update(&op.timestamp);

        match op.op_type {
            MessageOpType::Send {
                sender_id,
                content,
                message_type,
            } => {
                let entry = MessageEntry {
                    id: op.message_id,
                    sender_id,
                    content,
                    message_type,
                    timestamp: op.timestamp,
                    is_deleted: false,
                    read_by: Vec::new(),
                };
                let pos = self
                    .messages
                    .binary_search_by(|m| m.timestamp.cmp(&op.timestamp))
                    .unwrap_or_else(|p| p);
                self.messages.insert(pos, entry);
            }
            MessageOpType::Edit { new_content } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.id == op.message_id) {
                    msg.content = new_content;
                }
            }
            MessageOpType::Delete => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.id == op.message_id) {
                    msg.is_deleted = true;
                }
            }
            MessageOpType::Read { reader_id } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.id == op.message_id) {
                    if !msg.read_by.contains(&reader_id) {
                        msg.read_by.push(reader_id);
                    }
                }
            }
        }
    }
}
