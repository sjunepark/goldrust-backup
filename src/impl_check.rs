/// Assert that a type implements all common traits.
/// The common traits are
/// Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Display, Default, Send, Sync, Serialize, DeserializeOwned.
/// (DeserializeOwned is used instead of Deserialize because of lifetime issues, and Deserialize<'static> auto-implements DeserializeOwned.)
#[macro_export]
macro_rules! assert_impl_commons {
    ($type:ty) => {
        static_assertions::assert_impl_all!($type: Clone, Eq, PartialEq, Ord, PartialOrd, std::fmt::Debug, std::fmt::Display, Default, Send, Sync, serde::Serialize, serde::de::DeserializeOwned);
    };
}

#[macro_export]
macro_rules! assert_impl_commons_without_default {
    ($type:ty) => {
        static_assertions::assert_impl_all!($type: Clone, Eq, PartialEq, Ord, PartialOrd, std::fmt::Debug, std::fmt::Display, Send, Sync, serde::Serialize, serde::de::DeserializeOwned);
    };
}

#[cfg(test)]
mod tests {
    use derive_more::Display;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Display, Serialize, Deserialize)]
    struct CommStruct {
        field: String,
    }

    impl Default for CommStruct {
        fn default() -> Self {
            Self {
                field: "default".to_string(),
            }
        }
    }

    #[test]
    fn test_assert_impl_commons() {
        assert_impl_commons!(CommStruct);
    }

    #[test]
    fn test_assert_impl_commons_without_default() {
        assert_impl_commons_without_default!(CommStruct);
    }
}
