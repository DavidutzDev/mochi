//! What a module offers others, declared up front through
//! [`crate::Module::contributions`].

use mochi_protocol::Contribution;
use serde_json::Value;

/// A [`Contribution`] before the daemon fills in the module.
#[derive(Debug, Clone, PartialEq)]
pub struct ContributionSpec {
    pub target: String,
    pub kind: String,
    pub id: String,
    pub view: String,
    pub title: String,
    pub icon: Option<String>,
    pub order: i32,
    pub options: Value,
}

impl ContributionSpec {
    /// Offers `view` to the `target` module as a `kind`, like a hub `page`.
    pub fn new(
        target: impl Into<String>,
        kind: impl Into<String>,
        id: impl Into<String>,
        view: impl Into<String>,
        title: impl Into<String>,
    ) -> Self {
        Self {
            target: target.into(),
            kind: kind.into(),
            id: id.into(),
            view: view.into(),
            title: title.into(),
            icon: None,
            order: 0,
            options: Value::Null,
        }
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    /// Anything else the target reads, like `{"span": 2}` for a hub card.
    pub fn options(mut self, options: Value) -> Self {
        self.options = options;
        self
    }

    pub fn into_contribution(self, module: &str) -> Contribution {
        Contribution {
            module: module.to_owned(),
            target: self.target,
            kind: self.kind,
            id: self.id,
            view: self.view,
            title: self.title,
            icon: self.icon,
            order: self.order,
            options: self.options,
        }
    }
}
