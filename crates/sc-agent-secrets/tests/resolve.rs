/// In-memory environment map for tests.
///
/// [`Debug`] prints variable names and omits values.
#[cfg(test)]
#[derive(Clone, Default)]
pub struct MapEnv {
    vars: BTreeMap<String, String>,
}

#[cfg(test)]
impl MapEnv {
    /// Builds a map from `(name, value)` pairs.
    #[must_use]
    pub fn with<K, V, I>(pairs: I) -> Self
    where
        K: Into<String>,
        V: Into<String>,
        I: IntoIterator<Item = (K, V)>,
    {
        Self {
            vars: pairs
                .into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        }
    }
}

#[cfg(test)]
impl fmt::Debug for MapEnv {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MapEnv")
            .field("names", &self.vars.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
impl EnvSource for MapEnv {
    fn var(&self, name: &str) -> Option<String> {
        self.vars.get(name).cloned()
    }
}
/// In-memory keyring for tests.
///
/// Entries are visible only for the account given at construction. [`Debug`]
/// prints the account and service names and omits secrets.
#[cfg(test)]
#[derive(Clone)]
pub struct MapKeyring {
    account: String,
    entries: BTreeMap<String, String>,
}

#[cfg(test)]
impl MapKeyring {
    /// Builds a store whose secrets are returned only for `account`.
    #[must_use]
    pub fn new<S, V, I>(account: impl Into<String>, entries: I) -> Self
    where
        S: Into<String>,
        V: Into<String>,
        I: IntoIterator<Item = (S, V)>,
    {
        Self {
            account: account.into(),
            entries: entries
                .into_iter()
                .map(|(service, secret)| (service.into(), secret.into()))
                .collect(),
        }
    }
}

#[cfg(test)]
impl fmt::Debug for MapKeyring {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MapKeyring")
            .field("account", &self.account)
            .field("services", &self.entries.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
impl KeyringSource for MapKeyring {
    fn password(&self, service: &str, account: &str) -> Result<Option<String>, JevError> {
        if account != self.account {
            return Ok(None);
        }
        Ok(self.entries.get(service).cloned())
    }
}
