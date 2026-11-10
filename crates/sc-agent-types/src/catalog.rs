#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelAliases(
    BTreeMap<&'static str, Entry>,
    // Every model id the catalog knows (crate rows and models-file rows,
    // not overrides) and the alias it was declared under.
    BTreeMap<String, &'static str>,
);

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    model: ModelId,
    source: ModelSource,
    released: String,
}

impl ModelAliases {
    /// The newest `MODELS` row per alias.
    #[must_use]
    pub fn builtin() -> Self {
        let mut newest: BTreeMap<&'static str, &ModelDescriptor> = BTreeMap::new();
        for row in MODELS {
            let replace = newest
                .get(row.alias)
                .is_none_or(|current| row.released >= current.released);
            if replace {
                newest.insert(row.alias, row);
            }
        }
        let known = MODELS
            .iter()
            .map(|row| (row.model.to_owned(), row.alias))
            .collect();
        Self(
            newest
                .into_iter()
                .map(|(alias, row)| {
                    let entry = Entry {
                        model: row.model(),
                        source: ModelSource::Crate,
                        released: row.released.to_owned(),
                    };
                    (alias, entry)
                })
                .collect(),
            known,
        )
    }

    /// The alias a model id of the catalog was declared under (`jev` for
    /// `jev-1.15.0`), or `None` for an id the catalog does not list.
    ///
    /// The catalog is the [`MODELS`] table plus the rows added with
    /// [`Self::with_file_model`], whether or not a row is the newest of its
    /// alias. Overrides ([`Self::with_override`]) do not add to it.
    #[must_use]
    pub fn alias_of(&self, id: &str) -> Option<&'static str> {
        self.1.get(id).copied()
    }

    /// Every model id of the catalog with its alias, sorted by id.
    #[must_use]
    pub fn known_models(&self) -> Vec<(&str, &'static str)> {
        self.1
            .iter()
            .map(|(id, alias)| (id.as_str(), *alias))
            .collect()
    }

    /// The model `alias` currently names, or `None` for an unknown alias.
    #[must_use]
    pub fn latest(&self, alias: &str) -> Option<ModelId> {
        self.0.get(alias).map(|entry| entry.model.clone())
    }
    pub fn with_file_model(
        mut self,
        id: &str,
        alias: &str,
        released: &str,
    ) -> Result<Self, JevError> {
        let model: ModelId = id.parse().map_err(|error: JevError| {
            JevError::config_invalid(format!("models file: model id {id:?}: {error}"))
        })?;
        check_date(released)?;
        self.check_family(alias, &model)
            .map_err(|error| error.map_detail(|detail| format!("models file: {detail}")))?;
        let valid = self.aliases().join(", ");
        let Some((&name, entry)) = self.0.iter_mut().find(|(name, _)| **name == alias) else {
            return Err(JevError::config_invalid(format!(
                "models file: unknown model alias {alias:?}; valid aliases: {valid}"
            )));
        };
        let newer = entry.source != ModelSource::Override && released >= entry.released.as_str();
        // An id belongs to one alias. Redeclaring it under another would
        // move it away from the backend it selects today.
        if let Some(&declared) = self
            .1
            .get(model.as_str())
            .filter(|declared| **declared != name)
        {
            return Err(JevError::config_invalid(format!(
                "models file: model {:?} is already declared under alias {declared:?}; it cannot also be {alias:?}",
                model.as_str()
            )));
        }
        self.1.insert(model.as_str().to_owned(), name);
        if newer {
            *entry = Entry {
                model,
                source: ModelSource::File,
                released: released.to_owned(),
            };
        }
        Ok(self)
    }
