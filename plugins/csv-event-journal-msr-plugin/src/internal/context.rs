use std::{num::NonZeroUsize, path::PathBuf, result::Result as StdResult};

use msr_core::{
    csv_event_journal::{
        CsvFileRecordStorage, DefaultRecordPreludeGenerator, Entry, Record, RecordFilter,
        RecordPreludeGenerator, RecordStorage, Result, Severity, StoredRecord, StoredRecordPrelude,
    },
    storage::{RecordStorageBase as _, RecordStorageWrite as _, StorageConfig, StorageStatus},
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum State {
    Inactive,
    Active,
}

#[derive(Debug, Clone)]
pub struct Status {
    pub state: State,
    pub storage: StorageStatus,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Config {
    pub severity_threshold: Severity,
    pub storage: StorageConfig,
}

pub struct Context {
    config: Config,

    state: State,

    storage: CsvFileRecordStorage,
}

#[derive(Debug)]
pub struct EntryRecorded(pub StoredRecordPrelude);

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum EntryNotRecorded {
    Inactive,
    SeverityBelowThreshold,
}

pub type RecordEntryOutcome = StdResult<EntryRecorded, EntryNotRecorded>;

impl Context {
    pub fn try_new(
        data_dir: PathBuf,
        initial_config: Config,
        initial_state: State,
    ) -> Result<Self> {
        let storage = CsvFileRecordStorage::try_new(initial_config.storage.clone(), data_dir)?;
        Ok(Self {
            config: initial_config,
            state: initial_state,
            storage,
        })
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn status(&mut self, with_storage_statistics: bool) -> Result<Status> {
        let storage_statistics = if with_storage_statistics {
            Some(self.storage.report_statistics()?)
        } else {
            None
        };
        let storage = StorageStatus {
            descriptor: self.storage.descriptor().clone(),
            statistics: storage_statistics,
        };
        Ok(Status {
            state: self.state(),
            storage,
        })
    }

    pub fn recent_records(&mut self, limit: NonZeroUsize) -> Result<Vec<StoredRecord>> {
        self.storage.recent_records(limit)
    }

    pub fn filter_records(
        &mut self,
        limit: NonZeroUsize,
        filter: RecordFilter,
    ) -> Result<Vec<StoredRecord>> {
        self.storage.filter_records(limit, filter)
    }

    /// Switch the current configuration
    ///
    /// Returns the previous configuration.
    pub fn replace_config(&mut self, new_config: Config) -> Result<Config> {
        if self.config == new_config {
            return Ok(new_config);
        }
        log::debug!("Replacing config: {:?} -> {:?}", self.config, new_config);
        self.storage.replace_config(new_config.storage.clone());
        Ok(std::mem::replace(&mut self.config, new_config))
    }

    /// Switch the current state
    ///
    /// Returns the previous state.
    pub fn switch_state(&mut self, new_state: State) -> Result<State> {
        if self.state == new_state {
            return Ok(new_state);
        }
        log::debug!("Switching state: {:?} -> {:?}", self.state, new_state);
        Ok(std::mem::replace(&mut self.state, new_state))
    }

    pub fn record_entry(&mut self, new_entry: Entry) -> Result<RecordEntryOutcome> {
        match self.state {
            State::Inactive => {
                log::debug!("Discarding new entry while inactive: {:?}", new_entry);
                Ok(Err(EntryNotRecorded::Inactive))
            }
            State::Active => {
                if new_entry.severity < self.config.severity_threshold {
                    log::debug!(
                        "Discarding new entry below severity threshold: {:?}",
                        new_entry
                    );
                    return Ok(Err(EntryNotRecorded::SeverityBelowThreshold));
                }
                DefaultRecordPreludeGenerator
                    .generate_prelude()
                    .map(|(created_at, prelude)| {
                        (
                            created_at,
                            Record {
                                prelude,
                                entry: new_entry,
                            },
                        )
                    })
                    .and_then(|(created_at, recorded_entry)| {
                        log::debug!("Recording entry: {:?}", recorded_entry);
                        let prelude = StoredRecordPrelude {
                            id: recorded_entry.prelude.id.clone(),
                            created_at: created_at.system_time(),
                        };
                        self.storage
                            .append_record(&created_at, recorded_entry)
                            .map(|_created_at_offset| Ok(EntryRecorded(prelude)))
                            .map_err(msr_core::csv_event_journal::Error::Storage)
                    })
            }
        }
    }
}
