//! Nonsecret named defaults. A profile never grants Discord access.
use super::profile_update_guard::ProfileUpdateGuard;
use crate::paths::AppHome;
use crate::paths::Profile;
use eyre::Context;
use eyre::Result;
use eyre::bail;
use facet::Facet;
use std::io::Write as _;

#[derive(Facet, Clone, Debug, PartialEq, Eq)]
#[facet(rename_all = "kebab-case")]
pub struct Profiles {
    pub version: u32,
    pub active: Option<String>,
    pub profiles: Vec<Profile>,
}

impl Default for Profiles {
    fn default() -> Self {
        Self {
            version: 1,
            active: None,
            profiles: Vec::new(),
        }
    }
}

impl Profiles {
    /// # Errors
    /// Returns an error for invalid configuration, rejected scope or a failed operation.
    pub fn load(home: &AppHome) -> Result<Self> {
        let path = home.file_path("profiles.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(&path).wrap_err("Failed to read nonsecret profiles")?;
        let value: Self = facet_json::from_str(&text).wrap_err("Invalid profiles.json")?;
        value.validate()?;
        Ok(value)
    }

    /// Apply a delta to the latest document under an exclusive interprocess guard.
    /// # Errors
    /// Rejects competing/stale pending writes, invalid data and failed I/O.
    /// A rejected delta leaves the previous document unchanged.
    pub fn update(home: &AppHome, edit: impl FnOnce(&mut Self) -> Result<()>) -> Result<Self> {
        home.ensure_dir()?;
        let pending = home.file_path("profiles.json.pending");
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&pending)
            .wrap_err(
                "Cannot update profiles.json; another writer or a stale pending file exists",
            )?;
        let mut guard = ProfileUpdateGuard {
            pending,
            file: Some(file),
            committed: false,
        };
        // Acquisition precedes loading: a later caller cannot publish an old snapshot.
        let mut profiles = Self::load(home)?;
        edit(&mut profiles)?;
        profiles.validate()?;
        let text = facet_json::to_string_pretty(&profiles)?;
        let staged = guard
            .file
            .as_mut()
            .ok_or_else(|| eyre::eyre!("Missing profile update file"))?;
        staged.write_all(text.as_bytes())?;
        staged.sync_all()?;
        drop(guard.file.take());
        std::fs::rename(&guard.pending, home.file_path("profiles.json"))
            .wrap_err("Failed to replace profiles.json")?;
        guard.committed = true;
        Ok(profiles)
    }
    /// # Errors
    ///
    /// Returns an error for invalid configuration, rejected scope or a failed operation.
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 {
            bail!("Unsupported profile schema version {}", self.version);
        }
        let mut names = std::collections::HashSet::new();
        for p in &self.profiles {
            validate_name(&p.name)?;
            if let Some(broker) = &p.broker {
                broker.validate()?;
            }
            if p.guild_id == 0 || p.channel_id == Some(0) {
                bail!("Profile IDs must be nonzero");
            }
            if !names.insert(&p.name) {
                bail!("Duplicate profile name {}", p.name);
            }
        }
        if let Some(active) = &self.active {
            self.named(active)?;
        }
        Ok(())
    }

    /// # Errors
    ///
    /// Returns an error for invalid configuration, rejected scope or a failed operation.
    pub fn named(&self, name: &str) -> Result<&Profile> {
        self.profiles
            .iter()
            .find(|p| p.name == name)
            .ok_or_else(|| eyre::eyre!("Unknown profile {name}"))
    }

    /// # Errors
    ///
    /// Returns an error for invalid configuration, rejected scope or a failed operation.
    pub fn selected(&self, explicit: Option<&str>) -> Result<Option<&Profile>> {
        explicit
            .or(self.active.as_deref())
            .map(|name| self.named(name))
            .transpose()
    }

    /// # Errors
    ///
    /// Returns an error for invalid configuration, rejected scope or a failed operation.
    pub fn set(&mut self, mut profile: Profile) -> Result<()> {
        validate_name(&profile.name)?;
        if profile.guild_id == 0 || profile.channel_id == Some(0) {
            bail!("Profile IDs must be nonzero");
        }
        if let Some(existing) = self.profiles.iter_mut().find(|p| p.name == profile.name) {
            if profile.broker.is_none() {
                profile.broker.clone_from(&existing.broker);
            }
            *existing = profile;
        } else {
            self.profiles.push(profile);
        }
        Ok(())
    }

    /// Remove only the default selection; named profiles remain available explicitly.
    pub fn clear_active(&mut self) {
        self.active = None;
    }
    /// # Errors
    ///
    /// Returns an error for invalid configuration, rejected scope or a failed operation.
    pub fn activate(&mut self, name: &str) -> Result<()> {
        self.named(name)?;
        self.active = Some(name.to_owned());
        Ok(())
    }
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        bail!("Profile names must contain 1-64 ASCII letters, digits, '-' or '_'");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn profile(name: &str, guild_id: u64, channel_id: Option<u64>) -> Profile {
        Profile {
            name: name.into(),
            guild_id,
            channel_id,
            broker: None,
        }
    }
    #[test]
    fn persisted_selection_and_explicit_override_do_not_copy_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let home = AppHome(dir.path().to_owned());
        Profiles::update(&home, |profiles| {
            profiles.set(profile("sfm", 1, Some(2)))?;
            profiles.set(profile("other", 3, None))?;
            profiles.activate("sfm")
        })
        .unwrap();
        let loaded = Profiles::load(&home).unwrap();
        assert_eq!(loaded.selected(None).unwrap().unwrap().guild_id, 1);
        assert_eq!(loaded.selected(Some("other")).unwrap().unwrap().guild_id, 3);
        let error = loaded.selected(Some("missing")).unwrap_err();
        assert!(error.to_string().contains("Unknown profile"));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        Profiles::update(&home, |profiles| profiles.activate("other")).unwrap();
        assert_eq!(
            Profiles::load(&home).unwrap().active.as_deref(),
            Some("other")
        );
    }
    #[test]
    fn corrupt_unknown_and_ambiguous_profiles_fail_closed() {
        let mut profiles = Profiles::default();
        let error = profiles.set(profile("../escape", 1, None)).unwrap_err();
        assert!(error.to_string().contains("Profile names"));
        let error = profiles.set(profile("zero", 0, None)).unwrap_err();
        assert!(error.to_string().contains("nonzero"));
        let error = profiles.activate("missing").unwrap_err();
        assert!(error.to_string().contains("Unknown profile"));
        profiles.version = 2;
        let error = profiles.validate().unwrap_err();
        assert!(error.to_string().contains("version"));
        let dir = tempfile::tempdir().unwrap();
        let home = AppHome(dir.path().to_owned());
        std::fs::write(home.file_path("profiles.json"), "broken").unwrap();
        let error = Profiles::load(&home).unwrap_err();
        assert!(error.to_string().contains("Invalid profiles.json"));
    }
    #[test]
    fn competing_writer_does_not_replace_existing_profiles() {
        let dir = tempfile::tempdir().unwrap();
        let home = AppHome(dir.path().to_owned());
        Profiles::update(&home, |profiles| profiles.set(profile("sfm", 1, None))).unwrap();
        std::fs::write(home.file_path("profiles.json.pending"), "another writer").unwrap();
        let error = Profiles::update(&home, |profiles| profiles.activate("sfm")).unwrap_err();
        assert!(error.to_string().contains("another writer"));
        assert!(Profiles::load(&home).unwrap().active.is_none());
        assert_eq!(
            std::fs::read_to_string(home.file_path("profiles.json.pending")).unwrap(),
            "another writer"
        );
    }
    #[test]
    fn sequential_deltas_refresh_stale_views_and_preserve_active_selection() {
        let dir = tempfile::tempdir().unwrap();
        let home = AppHome(dir.path().to_owned());
        Profiles::update(&home, |profiles| profiles.set(profile("alpha", 1, None))).unwrap();
        // Two callers observe the same old document before making later edits.
        let stale_a = Profiles::load(&home).unwrap();
        let stale_b = Profiles::load(&home).unwrap();
        Profiles::update(&home, |profiles| {
            profiles.set(profile("beta", 2, None))?;
            profiles.activate("beta")
        })
        .unwrap();
        Profiles::update(&home, |profiles| {
            assert_eq!(profiles.active.as_deref(), Some("beta"));
            assert_eq!(profiles.named("beta").unwrap().guild_id, 2);
            profiles.set(profile("gamma", 3, None))
        })
        .unwrap();
        assert_eq!(stale_a.profiles.len(), 1);
        assert_eq!(stale_b.profiles.len(), 1);
        let latest = Profiles::load(&home).unwrap();
        assert_eq!(latest.profiles.len(), 3);
        assert_eq!(latest.active.as_deref(), Some("beta"));
    }
    #[test]
    fn overlapping_edit_is_rejected_before_loading_then_retry_sees_committed_state() {
        let dir = tempfile::tempdir().unwrap();
        let home = AppHome(dir.path().to_owned());
        Profiles::update(&home, |profiles| profiles.set(profile("alpha", 1, None))).unwrap();
        let (loaded_tx, loaded_rx) = std::sync::mpsc::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        let writer_home = home.clone();
        let writer = std::thread::spawn(move || {
            Profiles::update(&writer_home, |profiles| {
                loaded_tx.send(()).unwrap();
                resume_rx
                    .recv_timeout(std::time::Duration::from_secs(10))
                    .unwrap();
                profiles.set(profile("beta", 2, None))?;
                profiles.activate("beta")
            })
        });
        loaded_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        let mut edit_called = false;
        let rejected = Profiles::update(&home, |profiles| {
            edit_called = true;
            profiles.set(profile("gamma", 3, None))
        });
        assert!(!edit_called);
        let error = rejected.unwrap_err();
        assert!(error.to_string().contains("another writer"));
        resume_tx.send(()).unwrap();
        writer.join().unwrap().unwrap();
        Profiles::update(&home, |profiles| {
            assert_eq!(profiles.active.as_deref(), Some("beta"));
            profiles.set(profile("gamma", 3, None))
        })
        .unwrap();
        let latest = Profiles::load(&home).unwrap();
        assert_eq!(latest.profiles.len(), 3);
        assert_eq!(latest.active.as_deref(), Some("beta"));
        assert!(!home.file_path("profiles.json.pending").exists());
    }
    #[test]
    fn clearing_selection_persists_none_without_deleting_named_profiles() {
        let dir = tempfile::tempdir().unwrap();
        let home = AppHome(dir.path().to_owned());
        Profiles::update(&home, |profiles| {
            profiles.set(profile("sfm", 10, Some(20)))?;
            profiles.activate("sfm")
        })
        .unwrap();
        crate::cli::profile::clear::ProfileClearArgs
            .invoke(&home)
            .unwrap();
        let stored = Profiles::load(&home).unwrap();
        assert!(stored.active.is_none());
        assert!(stored.selected(None).unwrap().is_none());
        assert_eq!(
            stored.selected(Some("sfm")).unwrap().unwrap().channel_id,
            Some(20)
        );
        crate::cli::profile::clear::ProfileClearArgs
            .invoke(&home)
            .unwrap();
        assert_eq!(Profiles::load(&home).unwrap().profiles.len(), 1);
    }
}
