use super::StoredValue;

#[derive(Clone, Debug)]
pub(super) enum CachedValue {
    /// Was either not in the on-disk storage, or is due to be deleted from there.
    Absent,
    /// Is either present in the on-disk storage, or is due to be added there.
    Stored(StoredValue<'static>),
}

impl From<Option<StoredValue>> for CachedValue {
    fn from(value: Option<StoredValue>) -> Self {
        match value {
            None => CachedValue::Absent,
            Some(stored) => CachedValue::Stored(stored),
        }
    }
}

impl From<CachedValue> for Option<StoredValue> {
    fn from(value: CachedValue) -> Self {
        match value {
            CachedValue::Absent => None,
            CachedValue::Stored(stored_value) => Some(stored_value),
        }
    }
}
