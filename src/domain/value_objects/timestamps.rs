use time::{OffsetDateTime, UtcOffset};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Timestamp(pub OffsetDateTime);

impl Timestamp {
    pub fn now_utc() -> Self {
        Self(OffsetDateTime::now_utc())
    }

    pub fn from(dt: OffsetDateTime) -> Self {
        Self(dt.to_offset(UtcOffset::UTC))
    }

    /// Returns the inner UTC `OffsetDateTime` without consuming the wrapper.
    pub fn as_inner(&self) -> OffsetDateTime {
        self.0
    }

    /// Consumes the wrapper and returns the inner UTC `OffsetDateTime`.
    pub fn into_inner(self) -> OffsetDateTime {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::UtcOffset;

    #[test]
    fn given_now_utc_when_called_should_return_utc_offset() {
        let result = Timestamp::now_utc();
        assert_eq!(result.as_inner().offset(), UtcOffset::UTC);
    }

    #[test]
    fn given_from_with_non_utc_offset_when_called_should_store_utc_offset() {
        let offset = UtcOffset::from_hms(2, 0, 0).expect("valid offset");
        let dt = OffsetDateTime::now_utc().to_offset(offset);
        let result = Timestamp::from(dt);
        assert_eq!(result.as_inner().offset(), UtcOffset::UTC);
    }

    #[test]
    fn given_from_when_called_should_store_same_instant() {
        let offset = UtcOffset::from_hms(-5, 0, 0).expect("valid offset");
        let dt = OffsetDateTime::now_utc().to_offset(offset);
        let result = Timestamp::from(dt);
        assert_eq!(result.as_inner().unix_timestamp(), dt.unix_timestamp());
    }

    #[test]
    fn given_as_inner_when_called_should_return_inner_value() {
        let dt = OffsetDateTime::now_utc();
        let timestamp = Timestamp::from(dt);
        assert_eq!(timestamp.as_inner(), dt.to_offset(UtcOffset::UTC));
    }

    #[test]
    fn given_into_inner_when_called_should_return_inner_value() {
        let dt = OffsetDateTime::now_utc();
        let timestamp = Timestamp::from(dt);
        assert_eq!(timestamp.into_inner(), dt.to_offset(UtcOffset::UTC));
    }
}
