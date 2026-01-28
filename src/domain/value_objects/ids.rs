use uuid::Uuid;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
        pub struct $name(pub u128);

        impl $name {
            #[inline]
            pub fn new() -> Self {
                Self(Uuid::new_v4().as_u128())
            }
        }
    };
}

id_type!(JobId);
id_type!(EventId);
id_type!(ReportId);
id_type!(ClientId);

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! id_unique_test {
        ($name:ident, $test_name:ident) => {
            #[test]
            fn $test_name() {
                let result = $name::new();
                assert_ne!(result.0, $name::new().0)
            }
        };
    }

    id_unique_test!(JobId, given_new_job_id_when_generated_should_be_unique);
    id_unique_test!(EventId, given_new_event_id_when_generated_should_be_unique);
    id_unique_test!(
        ReportId,
        given_new_report_id_when_generated_should_be_unique
    );
    id_unique_test!(
        ClientId,
        given_new_client_id_when_generated_should_be_unique
    );
}
