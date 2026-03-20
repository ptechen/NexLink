use nexlink_core::{Attachment, EventEnvelope, MessageInboundPayload, MessageOutboundPayload};

use crate::message_model::{
    connector_inbound_event, connector_outbound_event, inbound_message_event,
    outbound_message_event,
};

#[derive(Debug, Clone, Default)]
pub struct ConnectorEnvelopeBuilder {
    pub source_peer: Option<String>,
    pub target_peer: Option<String>,
    pub channel: String,
}

impl ConnectorEnvelopeBuilder {
    pub fn new(channel: impl Into<String>) -> Self {
        Self {
            source_peer: None,
            target_peer: None,
            channel: channel.into(),
        }
    }

    pub fn source_peer(mut self, peer: impl Into<String>) -> Self {
        self.source_peer = Some(peer.into());
        self
    }

    pub fn target_peer(mut self, peer: impl Into<String>) -> Self {
        self.target_peer = Some(peer.into());
        self
    }

    pub fn inbound_payload(
        &self,
        event_id: impl Into<String>,
        session_key: impl Into<String>,
        payload: MessageInboundPayload,
    ) -> EventEnvelope {
        inbound_message_event(
            event_id,
            self.source_peer.clone(),
            self.target_peer.clone(),
            session_key,
            self.channel.clone(),
            payload,
        )
    }

    pub fn inbound_connector(
        &self,
        event_id: impl Into<String>,
        session_key: impl Into<String>,
        message_id: impl Into<String>,
        sender_id: impl Into<String>,
        text: Option<String>,
        attachments: Vec<Attachment>,
        metadata: serde_json::Value,
    ) -> EventEnvelope {
        connector_inbound_event(
            event_id,
            self.source_peer.clone(),
            self.target_peer.clone(),
            session_key,
            self.channel.clone(),
            message_id,
            sender_id,
            text,
            attachments,
            metadata,
        )
    }

    pub fn outbound_payload(
        &self,
        event_id: impl Into<String>,
        session_key: impl Into<String>,
        payload: MessageOutboundPayload,
    ) -> EventEnvelope {
        outbound_message_event(
            event_id,
            self.source_peer.clone(),
            self.target_peer.clone(),
            session_key,
            self.channel.clone(),
            payload,
        )
    }

    pub fn outbound_connector(
        &self,
        event_id: impl Into<String>,
        session_key: impl Into<String>,
        reply_to: Option<String>,
        text: Option<String>,
        attachments: Vec<Attachment>,
        metadata: serde_json::Value,
    ) -> EventEnvelope {
        connector_outbound_event(
            event_id,
            self.source_peer.clone(),
            self.target_peer.clone(),
            session_key,
            self.channel.clone(),
            reply_to,
            text,
            attachments,
            metadata,
        )
    }
}
