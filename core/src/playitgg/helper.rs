/*
Copyright 2022 Developed Methods LLC

Redistribution and use in source and binary forms, with or without modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following disclaimer in the documentation and/or other materials provided with the distribution.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
*/

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use playit_agent_core::agent_control::errors::SetupError;
use playit_agent_core::network::origin_lookup::OriginLookup;
use playit_agent_core::network::tcp::tcp_settings::TcpSettings;
use playit_agent_core::network::udp::udp_settings::UdpSettings;
use playit_agent_core::playit_agent::{PlayitAgent, PlayitAgentSettings};

use crate::event_broadcaster::EventBroadcaster;
use crate::events::{CausedBy, Event, EventInner, PlayitggRunnerEvent, PlayitggRunnerEventInner};
use crate::types::Snowflake;

/// Runs a playit.gg agent and reports its lifecycle through Lodestone events.
///
/// The control channel, TCP and UDP forwarding are handled by playit-agent-core's
/// `PlayitAgent`; tunnels are routed to their origin through the given `OriginLookup`.
pub struct TunnelRunner {
    agent: PlayitAgent,
}

impl TunnelRunner {
    pub async fn new(
        api_url: String,
        secret_key: String,
        lookup: Arc<OriginLookup>,
    ) -> Result<Self, SetupError> {
        let agent = PlayitAgent::new(
            PlayitAgentSettings {
                api_url,
                secret_key,
                tcp_settings: TcpSettings::default(),
                udp_settings: UdpSettings::default(),
            },
            lookup,
        )
        .await?;

        Ok(TunnelRunner { agent })
    }

    pub fn keep_running(&self) -> Arc<AtomicBool> {
        self.agent.keep_running()
    }

    pub async fn run(self, event_broadcaster: EventBroadcaster, broadcast: bool) {
        if broadcast {
            event_broadcaster.send(Event {
                event_inner: EventInner::PlayitggRunnerEvent(PlayitggRunnerEvent {
                    playitgg_runner_event_inner: PlayitggRunnerEventInner::RunnerLoading,
                }),
                snowflake: Snowflake::default(),
                details: "Starting runner".to_string(),
                caused_by: CausedBy::System,
            });
            event_broadcaster.send(Event {
                event_inner: EventInner::PlayitggRunnerEvent(PlayitggRunnerEvent {
                    playitgg_runner_event_inner: PlayitggRunnerEventInner::RunnerStarted,
                }),
                snowflake: Snowflake::default(),
                details: "Started".to_string(),
                caused_by: CausedBy::System,
            });
        }

        // Runs until `keep_running` is set to false.
        self.agent.run().await;
    }
}
