//! [`Catalog`], the built-in plus global-config models, and [`ModelChoice`].

use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use crate::model::{
    CLAUDE_FABLE_5_2, CLAUDE_HAIKU_5_5, CLAUDE_OPUS_5_5, CLAUDE_SONNET_5_5, GPT_6_1_SOL,
    GPT_6_1_TERRA, GPT_6_ASTRA, GPT_6_LUNA, JEV_1_13_0,
};
use crate::{MODELS, ModelDescriptor, ModelId, ReleaseDate, ShortName, TypesError};

/// The model catalog: the built-in [`MODELS`] rows plus the rows of the global config.
///
/// Every line always has a latest model, so [`Self::resolve`] cannot fail. Per line the newest
/// release date wins; on an equal date a global-config model wins over the one it meets. This
/// type reads no files: the config loader passes each row to [`Self::with_config_model`].
///
/// # Examples
///
/// ```
/// use sc_agent_types::{Catalog, ModelChoice, ShortName};
///
/// let catalog = Catalog::builtin()
///     .with_config_model("claude-haiku-6-0".parse().unwrap(), ShortName::Haiku, "2027-01-01".parse().unwrap())
///     .unwrap();
/// let choice: ModelChoice = "haiku".parse().unwrap();
/// assert_eq!(catalog.resolve(&choice).as_str(), "claude-haiku-6-0");
/// assert_eq!(catalog.line_of("claude-haiku-5-5"), Some(ShortName::Haiku));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    latest: Latest,
    lines: BTreeMap<ModelId, ShortName>,
}

/// The newest model of a line and its release date.
type Slot = (ModelId, ReleaseDate);

/// One slot per [`ShortName`], so every line always has a model (total by construction).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Latest {
    jev: Slot,
    haiku: Slot,
    sonnet: Slot,
    opus: Slot,
    fable: Slot,
    luna: Slot,
    terra: Slot,
    sol: Slot,
    astra: Slot,
}

impl Latest {
    fn slot(&self, line: ShortName) -> &Slot {
        match line {
            ShortName::Jev => &self.jev,
            ShortName::Haiku => &self.haiku,
            ShortName::Sonnet => &self.sonnet,
            ShortName::Opus => &self.opus,
            ShortName::Fable => &self.fable,
            ShortName::Luna => &self.luna,
            ShortName::Terra => &self.terra,
            ShortName::Sol => &self.sol,
            ShortName::Astra => &self.astra,
        }
    }

    fn slot_mut(&mut self, line: ShortName) -> &mut Slot {
        match line {
            ShortName::Jev => &mut self.jev,
            ShortName::Haiku => &mut self.haiku,
            ShortName::Sonnet => &mut self.sonnet,
            ShortName::Opus => &mut self.opus,
            ShortName::Fable => &mut self.fable,
            ShortName::Luna => &mut self.luna,
            ShortName::Terra => &mut self.terra,
            ShortName::Sol => &mut self.sol,
            ShortName::Astra => &mut self.astra,
        }
    }
}

fn seed(row: &ModelDescriptor) -> Slot {
    (row.model().clone(), row.released())
}

impl Catalog {
    /// The catalog of the built-in [`MODELS`] rows: the newest row per line is its latest.
    #[must_use]
    pub fn builtin() -> Self {
        let mut catalog = Self {
            latest: Latest {
                jev: seed(&JEV_1_13_0),
                haiku: seed(&CLAUDE_HAIKU_5_5),
                sonnet: seed(&CLAUDE_SONNET_5_5),
                opus: seed(&CLAUDE_OPUS_5_5),
                fable: seed(&CLAUDE_FABLE_5_2),
                luna: seed(&GPT_6_LUNA),
                terra: seed(&GPT_6_1_TERRA),
                sol: seed(&GPT_6_1_SOL),
                astra: seed(&GPT_6_ASTRA),
            },
            lines: BTreeMap::new(),
        };
        for row in MODELS {
            catalog.lines.insert(row.model().clone(), row.line());
            let slot = catalog.latest.slot_mut(row.line());
            if row.released() > slot.1 {
                *slot = seed(row);
            }
        }
        catalog
    }

    /// Adds one global-config model to `line`.
    ///
    /// It becomes the line's latest when `released` is later than or equal to the current
    /// latest's date (on an equal date the config model wins). Offering an id again under its
    /// own line is allowed.
    ///
    /// # Errors
    ///
    /// [`TypesError::ModelOnTwoLines`] when `model` already belongs to another line.
    pub fn with_config_model(
        mut self,
        model: ModelId,
        line: ShortName,
        released: ReleaseDate,
    ) -> Result<Self, TypesError> {
        if let Some(&other_line) = self.lines.get(&model)
            && other_line != line
        {
            return Err(TypesError::ModelOnTwoLines {
                model,
                line,
                other_line,
            });
        }
        self.lines.insert(model.clone(), line);
        let slot = self.latest.slot_mut(line);
        if released >= slot.1 {
            *slot = (model, released);
        }
        Ok(self)
    }

    /// The model a choice names: the line's latest for [`ModelChoice::Latest`], the id itself
    /// for [`ModelChoice::Version`] (unlisted ids included).
    #[must_use]
    pub fn resolve(&self, choice: &ModelChoice) -> ModelId {
        match choice {
            ModelChoice::Latest(line) => self.latest.slot(*line).0.clone(),
            ModelChoice::Version(model) => model.clone(),
        }
    }

    /// The line a catalog model belongs to, or `None` for an id the catalog does not list.
    ///
    /// Takes a `&ModelId` or a plain `&str`, so a provider-listed id needs no parse.
    #[must_use]
    pub fn line_of<Q>(&self, model: &Q) -> Option<ShortName>
    where
        ModelId: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.lines.get(model).copied()
    }
}

/// How a caller names a model: a line's latest, or one specific version.
///
/// # Examples
///
/// ```
/// use sc_agent_types::{ModelChoice, ShortName};
///
/// assert_eq!("sol".parse::<ModelChoice>().unwrap(), ModelChoice::Latest(ShortName::Sol));
/// assert!(matches!("gpt-6.1-sol".parse::<ModelChoice>().unwrap(), ModelChoice::Version(_)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelChoice {
    /// The newest model of a line.
    Latest(ShortName),
    /// One model id, used as given.
    Version(ModelId),
}

impl FromStr for ModelChoice {
    type Err = TypesError;

    /// A short name when the text is one, else a model id.
    ///
    /// # Errors
    ///
    /// [`TypesError::InvalidModelId`] when the text is neither a short name nor a valid id.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.parse::<ShortName>() {
            Ok(line) => Ok(Self::Latest(line)),
            Err(_) => text.parse().map(Self::Version),
        }
    }
}

impl fmt::Display for ModelChoice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Latest(line) => fmt::Display::fmt(line, formatter),
            Self::Version(model) => fmt::Display::fmt(model, formatter),
        }
    }
}
