pub mod account;
pub mod java_manager;
pub mod modpacks;
pub mod play;
pub mod server;
pub mod settings;

pub use account::{AccountScreen, AccountUpdate, Message as AccountMessage};
pub use java_manager::{JavaManagerScreen, Message as JavaManagerMessage};
pub use modpacks::{Message as ModpacksMessage, ModpacksScreen};
pub use play::{Message as PlayMessage, PlayScreen};
pub use server::{Message as ServerMessage, ServerScreen};
pub use settings::{Message as SettingsMessage, SettingsScreen};
pub mod instances;
pub use instances::{InstancesScreen, Message as InstancesMessage};
