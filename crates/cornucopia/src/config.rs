//! Configuration for Cornucopia.

use std::collections::HashMap;

use serde::Deserialize;

/// Configuration for Cornucopia.
#[derive(Clone, Deserialize, Default, Debug)]
pub struct Config {
    /// Contains a map of what given type should map to.
    pub custom_type_map: HashMap<String, String>,
}
