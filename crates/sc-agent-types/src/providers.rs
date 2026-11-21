//! The fixed provider and model tables (REQ-SAG-0101..0103).

use crate::Error;

/// The request shape a provider speaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wire {
    /// The Jev decision API (TypeSafe and local runners).
    SystemOne,
    /// Anthropic Messages.
    Messages,
    /// OpenAI chat/completions and compatible resellers.
    Chat,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Provider {
    pub name: &'static str,
    pub base_url: &'static str,
    pub key_var: &'static str,
    pub wire: Wire,
    /// Model families served (`jev-`, `claude-`, `gpt-`); empty passes any id through.
    pub serves: &'static [&'static str],
    /// Resellers name models `<vendor>/<id>`.
    pub reseller: bool,
}

/// In ranking order.
pub const PROVIDERS: [Provider; 6] = [
    Provider {
        name: "typesafe",
        base_url: "https://api.typesafe.ai",
        key_var: "TYPESAFE_API_KEY",
        wire: Wire::SystemOne,
        serves: &["jev-"],
        reseller: false,
    },
    Provider {
        name: "anthropic",
        base_url: "https://api.anthropic.com/v1",
        key_var: "ANTHROPIC_API_KEY",
        wire: Wire::Messages,
        serves: &["claude-"],
        reseller: false,
    },
    Provider {
        name: "openai",
        base_url: "https://api.openai.com/v1",
        key_var: "OPENAI_API_KEY",
        wire: Wire::Chat,
        serves: &["gpt-"],
        reseller: false,
    },
    Provider {
        name: "openrouter",
        base_url: "https://openrouter.ai/api/v1",
        key_var: "OPENROUTER_API_KEY",
        wire: Wire::Chat,
        serves: &["claude-", "gpt-"],
        reseller: true,
    },
    Provider {
        name: "together",
        base_url: "https://api.together.ai/v1",
        key_var: "TOGETHER_API_KEY",
        wire: Wire::Chat,
        serves: &["claude-", "gpt-"],
        reseller: true,
    },
    Provider {
        name: "nous",
        base_url: "https://inference-api.nousresearch.com/v1",
        key_var: "NOUS_API_KEY",
        wire: Wire::Chat,
        serves: &[],
        reseller: false,
    },
];

/// (short name, model id, release date): the newest row of a short name is its default.
pub const MODELS: [(&str, &str, &str); 10] = [
    ("jev", "jev-1.13.0", "2026-09-15"),
    ("jev", "jev-1.14.0", "2026-10-14"),
    ("haiku", "claude-haiku-5-5", "2026-10-07"),
    ("sonnet", "claude-sonnet-5-5", "2026-09-28"),
    ("opus", "claude-opus-5-5", "2026-09-22"),
    ("fable", "claude-fable-5-2", "2026-11-09"),
    ("luna", "gpt-6-luna", "2026-09-22"),
    ("terra", "gpt-6.1-terra", "2026-10-26"),
    ("sol", "gpt-6.1-sol", "2026-09-29"),
    ("astra", "gpt-6-astra", "2026-09-03"),
];

/// A short name's newest model id; any other text is taken as a model id.
pub fn model_id(model: &str) -> &str {
    MODELS
        .iter()
        .filter(|(short, _, _)| *short == model)
        .max_by_key(|(_, _, released)| *released)
        .map_or(model, |(_, id, _)| *id)
}

impl Provider {
    pub fn by_name(name: &str) -> Result<&'static Self, Error> {
        PROVIDERS
            .iter()
            .find(|p| p.name == name)
            .ok_or_else(|| Error::Config(format!("unknown provider {name}")))
    }

    pub fn serves(&self, id: &str) -> bool {
        self.serves.is_empty() || self.serves.iter().any(|family| id.starts_with(family))
    }

    /// This provider's spelling of a catalog id.
    pub fn wire_id(&self, id: &str) -> String {
        match (self.reseller, id.strip_prefix("claude-")) {
            (false, _) => id.to_owned(),
            (true, Some(_)) => format!("anthropic/{}", dotted(id)),
            (true, None) => format!("openai/{id}"),
        }
    }

    /// `model` (short name or id) on `provider`, or on the first provider in ranking order
    /// that serves it and `has_key`. Returns the provider and its model id.
    pub fn choose(
        model: &str,
        provider: Option<&str>,
        has_key: impl Fn(&Provider) -> bool,
    ) -> Result<(&'static Self, String), Error> {
        let id = model_id(model);
        let chosen = match provider {
            Some(name) => Self::by_name(name)?,
            None => PROVIDERS
                .iter()
                .filter(|p| !p.serves.is_empty() && p.serves(id))
                .find(|p| has_key(p))
                .ok_or_else(|| Error::Config(format!("no provider with a key serves {model}")))?,
        };
        Ok((chosen, chosen.wire_id(id)))
    }
}

/// `claude-haiku-5-5` → `claude-haiku-5.5`.
fn dotted(id: &str) -> String {
    match id.rsplit_once('-') {
        Some((stem, minor))
            if minor.chars().all(|c| c.is_ascii_digit())
                && stem.ends_with(|c: char| c.is_ascii_digit()) =>
        {
            format!("{stem}.{minor}")
        }
        _ => id.to_owned(),
    }
}
