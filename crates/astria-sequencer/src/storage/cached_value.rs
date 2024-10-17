#[derive(Clone, Debug)]
pub(super) enum CachedValue {
    /// Was either not in the on-disk storage, or is due to be deleted from there.
    Absent,
    /// Is either present in the on-disk storage, or is due to be added there.
    Stored { serialized_value: Vec<u8> },
}

impl From<Option<Vec<u8>>> for CachedValue {
    fn from(value: Option<Vec<u8>>) -> Self {
        match value {
            None => CachedValue::Absent,
            Some(serialized_value) => CachedValue::Stored {
                serialized_value,
            },
        }
    }
}

impl From<CachedValue> for Option<Vec<u8>> {
    fn from(value: CachedValue) -> Self {
        match value {
            CachedValue::Absent => None,
            CachedValue::Stored {
                serialized_value,
            } => Some(serialized_value),
        }
    }
}
