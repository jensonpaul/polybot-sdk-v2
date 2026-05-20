
pub mod config;
pub mod connection;
pub mod error;
pub mod traits;

pub use connection::ConnectionManager;
#[expect(
    clippy::module_name_repetitions,
    reason = "WsError includes module name for clarity when used outside this module"
)]
pub use error::WsError;
pub use traits::*;
