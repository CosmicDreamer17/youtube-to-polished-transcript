use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Speaker {
    pub label: String,
    pub display_name: String,
    pub is_primary: bool,
    pub id: String,
}

impl Speaker {
    pub fn new(label: &str, display_name: &str, is_primary: bool) -> Self {
        Self {
            label: label.to_string(),
            display_name: display_name.to_string(),
            is_primary,
            id: Uuid::new_v4().to_string(),
        }
    }

    pub fn name(&self) -> &str {
        if self.display_name.is_empty() {
            &self.label
        } else {
            &self.display_name
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_returns_display_name_when_set() {
        let s = Speaker::new("Speaker A", "Alice", false);
        assert_eq!(s.name(), "Alice");
    }

    #[test]
    fn name_falls_back_to_label() {
        let s = Speaker::new("Speaker A", "", false);
        assert_eq!(s.name(), "Speaker A");
    }
}
