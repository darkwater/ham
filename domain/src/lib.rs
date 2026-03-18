pub mod errors;
pub mod events;
pub mod tag_values;
pub mod types;

pub use errors::ValidationError;
pub use events::{
    apply_event, DomainState, Event, EventApplyError, EventType, IdempotencyComparison,
    MutationSpec,
};
pub use tag_values::{validate_tag_value, StoredTagValue};
pub use types::FieldType;

pub fn domain_ready() -> bool {
    true
}
