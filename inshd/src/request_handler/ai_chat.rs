//! Handles AI chats.
use anthropic_sdk::Anthropic;
use anthropic_sdk::{MessageCreateBuilder, MessageStreamEvent, MessageStream};
use anthropic_sdk::ContentBlockDelta::TextDelta;
use futures::StreamExt;
use typed_builder::TypedBuilder;

use insh_api::{AIChatRequestParams, ResponseParamsAndLast, ResponseParams, AIChatResponseParams};
use config::Config;

lazy_static::lazy_static! {
    static ref RUNTIME: tokio::runtime::Runtime = 
        tokio::runtime::Runtime::new().unwrap();
}

#[derive(TypedBuilder)]
pub struct AIChat {
    /// Configuration.
    config: Config,
    /// The token stream.
    stream: Option<MessageStream>,
    /// If the chat is done.
    done: bool,
}

impl AIChat {
    /// Return an AI chat.
    pub fn new(_params: &AIChatRequestParams, config: Config) -> Self {
        Self {
            config: config,
            stream: None,
            done: false,
        }
    }
}

impl Iterator for AIChat {
    type Item = ResponseParamsAndLast;

    fn next(&mut self) -> Option<ResponseParamsAndLast> {
        if self.done {
            return None;
        }

        let token: String = match self.config.ai().token() {
            Some(token) => token.into()
            None => {
                return Some(ResponseParamsAndLast::builder()
                    .response_params(
                        ResponseParams::AIChat(
                            AIChatResponseParams::builder()
                                .result(AIChatError::TokenNotSet)
                                .build()
                        )
                    )
                    .last(true)
                    .build()
                )
            }
        }

        let client = Anthropic::new(self.config.ai().token()).unwrap();

        return RUNTIME.block_on(async {
            // Get or create the message stream.
            let stream: &mut MessageStream = match &mut self.stream {
                Some(stream) => stream,
                None => {
                    let stream = client.messages()
                        .stream(
                            MessageCreateBuilder::new("claude-3-5-sonnet-latest", 1024)
                                .user("Tell me a story")
                                .build()
                        )
                        .await.unwrap();
                    self.stream = Some(stream);
                    self.stream.as_mut().unwrap()
                }
            };

            loop {
                // Get the next event.
                let event: MessageStreamEvent = stream.next().await.unwrap().unwrap();

                // Handle the next event from the message stream.
                match event {
                    MessageStreamEvent::ContentBlockDelta{ delta, .. } => {
                        match delta {
                            TextDelta{text} => {
                                return Some(ResponseParamsAndLast::builder()
                                    .response_params(
                                        ResponseParams::AIChat(
                                            AIChatResponseParams::builder()
                                                .text(text)
                                                .build()
                                        )
                                    )
                                    .last(true)
                                    .build()
                                )
                            }
                            _ => {}
                        }
                    }
                    MessageStreamEvent::MessageStop => {
                        log::info!("AI chat complete.");
                        self.done = true;
                        return Some(ResponseParamsAndLast::builder()
                            .response_params(
                                ResponseParams::AIChat(
                                    AIChatResponseParams::builder().build()
                                )
                            )
                            .last(true)
                            .build()
                        )
                    },
                    _ => {}
                }
            }
        });
    }
}
