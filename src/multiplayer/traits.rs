use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Core trait for all networked messages.
/// Bridges Bevy events with custom serialization requirements.
pub trait Message: Event + Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static {}

/// Automatically implement Message for any type that satisfies the bounds.
impl<T: Event + Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static> Message for T {}

/// System parameter for writing messages (aliased to EventWriter in 0.18).
pub type MessageWriter<'w, T> = EventWriter<'w, T>;

/// System parameter for reading messages (aliased to EventReader in 0.18).
pub type MessageReader<'w, 's, T> = EventReader<'w, 's, T>;

/// Extension trait for App to register custom braid messages.
pub trait AddMessage {
    fn add_message<T: Message>(&mut self) -> &mut Self;
}

impl AddMessage for App {
    fn add_message<T: Message>(&mut self) -> &mut Self {
        self.init_resource::<bevy::prelude::Events<T>>();
        self
    }
}
