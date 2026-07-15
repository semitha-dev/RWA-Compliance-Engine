pub mod force_transfer;
pub mod freeze_investor_account;
pub mod initialize_config;
pub mod manage_jurisdictions;
pub mod revoke_investor;
pub mod set_paused;
pub mod thaw_investor_account;
pub mod transfer_authority;
pub mod upsert_investor;

// Each module's `handler` free function is intentionally NOT glob-exported here:
// every instruction module defines a function named `handler`, so a wildcard
// re-export would collide. `lib.rs` calls `module::handler(...)` explicitly.
// Only the `Accounts` context structs (unique names) are re-exported for use
// in `#[derive(Accounts)]` return types on the program's instruction functions.
pub use force_transfer::ForceTransfer;
pub use freeze_investor_account::FreezeInvestorAccount;
pub use initialize_config::InitializeConfig;
pub use manage_jurisdictions::ManageJurisdictions;
pub use revoke_investor::RevokeInvestor;
pub use set_paused::SetPaused;
pub use thaw_investor_account::ThawInvestorAccount;
pub use transfer_authority::{AcceptAuthorityTransfer, InitiateAuthorityTransfer};
pub use upsert_investor::UpsertInvestor;
