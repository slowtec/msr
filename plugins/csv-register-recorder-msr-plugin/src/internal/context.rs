use std::{
    collections::{hash_map::Entry, HashMap},
    fmt, fs,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    time::SystemTime,
};

use msr_core::{
    register::{
        recorder::{
            csv::FileRecordStorage as CsvFileRecordStorage, RecordPrelude, RecordStorage as _,
            StoredRecordPrelude as StoredRegisterRecordPrelude,
        },
        Index as RegisterIndex,
    },
    storage::{
        RecordPreludeFilter, RecordStorageBase, Result as StorageResult, StorageConfig,
        StorageStatus,
    },
    time::SystemTimeInstant,
    ScalarType, ScalarValue,
};

use crate::{
    api::{
        ObservedRegisterValues, RegisterGroupId, RegisterRecord, RegisterType, StoredRegisterRecord,
    },
    Error, Result,
};

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct PartitionId(String);

impl PartitionId {
    pub fn encode(s: &str) -> Self {
        Self(bs58::encode(s).into_string())
    }
}

impl AsRef<str> for PartitionId {
    fn as_ref(&self) -> &str {
        let Self(inner) = &self;
        inner
    }
}

impl fmt::Display for PartitionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_ref())
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RegisterGroupConfig {
    pub registers: Vec<(RegisterIndex, RegisterType)>,
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum State {
    Inactive,
    Active,
}

#[derive(Debug, Clone)]
pub struct Status {
    pub state: State,
    pub register_groups: Option<HashMap<RegisterGroupId, RegisterGroupStatus>>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Config {
    pub default_storage: StorageConfig,
    pub register_groups: HashMap<RegisterGroupId, RegisterGroupConfig>,
}

pub struct Context {
    data_path: PathBuf, // immutable

    file_name_prefix: String,

    config: Config,

    state: State,

    register_groups: HashMap<RegisterGroupId, RegisterGroupContext>,

    event_cb: Box<dyn ContextEventCallback + Send>,
}

pub trait ContextEventCallback {
    fn data_directory_created(&self, register_group_id: &RegisterGroupId, data_dir: &Path);
}

struct RegisterGroupContext {
    storage: CsvFileRecordStorage,
}

#[derive(Debug, Clone)]
pub struct RegisterGroupStatus {
    pub storage: StorageStatus,
}

fn partition_id_as_path(partition_id: &PartitionId) -> &Path {
    let path = Path::new(partition_id.as_ref());
    debug_assert!(!path.has_root());
    debug_assert!(path.is_relative());
    debug_assert!(path.components().count() == 1);
    path
}

impl RegisterGroupContext {
    fn try_new(
        register_group_id: &RegisterGroupId,
        data_path: &Path,
        file_name_prefix: String,
        config: RegisterGroupConfig,
        event_cb: &dyn ContextEventCallback,
    ) -> Result<Self> {
        let mut data_path = PathBuf::from(data_path);
        let partition_id = PartitionId::encode(register_group_id.as_ref());
        let id_path = partition_id_as_path(&partition_id);
        data_path.push(id_path);
        if !data_path.is_dir() {
            log::info!("Creating non-existent directory {}", data_path.display());
            fs::create_dir_all(&data_path)?;
            event_cb.data_directory_created(register_group_id, &data_path);
        }
        let storage = CsvFileRecordStorage::try_new(
            config.storage,
            data_path,
            file_name_prefix,
            config.registers,
        )
        .map_err(anyhow::Error::from)?;
        let context = Self { storage };
        Ok(context)
    }

    fn status(&mut self, with_storage_statistics: bool) -> StorageResult<RegisterGroupStatus> {
        let storage_statistics = if with_storage_statistics {
            Some(self.storage.report_statistics()?)
        } else {
            None
        };
        let storage = StorageStatus {
            descriptor: self.storage.descriptor().clone(),
            statistics: storage_statistics,
        };
        Ok(RegisterGroupStatus { storage })
    }
}

pub trait RecordPreludeGenerator {
    fn generate_prelude(&self) -> Result<(SystemTimeInstant, RecordPrelude)>;
}

#[derive(Debug)]
struct DefaultRecordPreludeGenerator;

impl RecordPreludeGenerator for DefaultRecordPreludeGenerator {
    fn generate_prelude(&self) -> Result<(SystemTimeInstant, RecordPrelude)> {
        Ok((SystemTimeInstant::now(), Default::default()))
    }
}

pub trait RecordRepo {
    fn append_record(&mut self, record: RegisterRecord) -> Result<()>;

    fn recent_records(&self, limit: NonZeroUsize) -> Result<Vec<StoredRegisterRecord>>;

    fn filter_records(
        &self,
        limit: NonZeroUsize,
        filter: RecordPreludeFilter,
    ) -> Result<Vec<StoredRegisterRecord>>;

    fn total_record_count(&self) -> usize;
}

fn create_register_group_contexts(
    data_path: &Path,
    file_name_prefix: String,
    register_group_configs: HashMap<RegisterGroupId, RegisterGroupConfig>,
    event_cb: &dyn ContextEventCallback,
) -> Result<HashMap<RegisterGroupId, RegisterGroupContext>> {
    let mut register_group_contexts = HashMap::with_capacity(register_group_configs.len());
    for (register_group_id, register_group_config) in register_group_configs {
        let register_group_context = RegisterGroupContext::try_new(
            &register_group_id,
            data_path,
            file_name_prefix.clone(),
            register_group_config.clone(),
            event_cb,
        )?;
        register_group_contexts.insert(register_group_id, register_group_context);
    }
    Ok(register_group_contexts)
}

impl Context {
    pub fn try_new(
        data_path: PathBuf,
        file_name_prefix: String,
        initial_config: Config,
        initial_state: State,
        event_cb: Box<dyn ContextEventCallback + Send>,
    ) -> Result<Self> {
        let register_groups = create_register_group_contexts(
            &data_path,
            file_name_prefix.clone(),
            initial_config.register_groups.clone(),
            &*event_cb,
        )?;
        Ok(Self {
            data_path,
            file_name_prefix,
            config: initial_config,
            state: initial_state,
            register_groups,
            event_cb,
        })
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn register_group_config(&self, id: &RegisterGroupId) -> Option<&RegisterGroupConfig> {
        self.config.register_groups.get(id)
    }

    pub fn status(
        &mut self,
        with_register_groups: bool,
        with_storage_statistics: bool,
    ) -> Result<Status> {
        let state = self.state();
        let register_groups = if with_register_groups {
            let mut register_groups = HashMap::with_capacity(self.register_groups.len());
            for (id, context) in &mut self.register_groups {
                let status = context
                    .status(with_storage_statistics)
                    .map_err(Error::MsrStorage)?;
                register_groups.insert(id.clone(), status);
            }
            Some(register_groups)
        } else {
            None
        };
        Ok(Status {
            state,
            register_groups,
        })
    }

    pub fn recent_records(
        &mut self,
        register_group_id: &RegisterGroupId,
        limit: NonZeroUsize,
    ) -> Result<Vec<StoredRegisterRecord>> {
        let context = self
            .register_groups
            .get_mut(register_group_id)
            .ok_or(Error::RegisterGroupUnknown)?;
        Ok(context.storage.recent_records(limit)?)
    }

    pub fn filter_records(
        &mut self,
        register_group_id: &RegisterGroupId,
        limit: NonZeroUsize,
        filter: &RecordPreludeFilter,
    ) -> Result<Vec<StoredRegisterRecord>> {
        let context = self
            .register_groups
            .get_mut(register_group_id)
            .ok_or(Error::RegisterGroupUnknown)?;
        Ok(context.storage.filter_records(limit, filter)?)
    }

    /// Switch the current configuration
    ///
    /// Returns the previous configuration.
    pub fn replace_config(&mut self, new_config: Config) -> Result<Config> {
        if self.config == new_config {
            return Ok(new_config);
        }
        log::debug!(
            "Replacing configuration: {:?} -> {:?}",
            self.config,
            new_config
        );
        let new_register_groups = create_register_group_contexts(
            &self.data_path,
            self.file_name_prefix.clone(),
            new_config.register_groups.clone(),
            &*self.event_cb,
        )?;
        // Replace atomically
        self.register_groups = new_register_groups;
        Ok(std::mem::replace(&mut self.config, new_config))
    }

    /// Switch the current configuration of a single register group
    ///
    /// Returns the previous configuration.
    pub fn replace_register_group_config(
        &mut self,
        register_group_id: RegisterGroupId,
        new_config: RegisterGroupConfig,
    ) -> Result<Option<RegisterGroupConfig>> {
        let entry = self.config.register_groups.entry(register_group_id);
        match entry {
            Entry::Vacant(vacant) => {
                let register_group_id = vacant.key().clone();
                log::debug!(
                    "Configuring register group {}: {:?}",
                    register_group_id,
                    new_config
                );
                let register_group_context = RegisterGroupContext::try_new(
                    &register_group_id,
                    &self.data_path,
                    self.file_name_prefix.clone(),
                    new_config.clone(),
                    &*self.event_cb,
                )?;
                self.register_groups
                    .insert(register_group_id, register_group_context);
                vacant.insert(new_config);
                Ok(None)
            }
            Entry::Occupied(mut occupied) => {
                if occupied.get() == &new_config {
                    return Ok(Some(new_config));
                }
                let register_group_id = occupied.key().clone();
                log::debug!(
                    "Replacing configuration of register group {}: {:?} -> {:?}",
                    register_group_id,
                    occupied.get(),
                    new_config
                );
                let register_group_context = RegisterGroupContext::try_new(
                    &register_group_id,
                    &self.data_path,
                    self.file_name_prefix.clone(),
                    new_config.clone(),
                    &*self.event_cb,
                )?;
                self.register_groups
                    .insert(register_group_id, register_group_context);
                let old_config = std::mem::replace(occupied.get_mut(), new_config);
                Ok(Some(old_config))
            }
        }
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

    pub fn record_observed_register_group_values(
        &mut self,
        register_group_id: &RegisterGroupId,
        observed_register_values: ObservedRegisterValues,
    ) -> Result<Option<StoredRegisterRecordPrelude>> {
        match self.state {
            State::Inactive => {
                log::debug!(
                    "Discarding new observation for register group {} while inactive: {:?}",
                    register_group_id,
                    observed_register_values
                );
                Ok(None)
            }
            State::Active => {
                if let Some(config) = self.config.register_groups.get(register_group_id) {
                    let expected_register_count = config.registers.len();
                    let actual_register_count = observed_register_values.register_values.len();
                    if expected_register_count != actual_register_count {
                        log::warn!(
                            "Mismatching number of register values in observation for group {}: expected = {}, actual = {}",
                            register_group_id,
                            expected_register_count,
                            actual_register_count);
                        return Err(Error::DataFormatInvalid);
                    }
                    for ((register_index, expected_type), actual_type) in
                        config.registers.iter().zip(
                            observed_register_values
                                .register_values
                                .iter()
                                .map(|v| v.as_ref().map(|v| v.to_type())),
                        )
                    {
                        if let Some(actual_type) = actual_type {
                            if *expected_type != actual_type {
                                log::warn!(
                                    "Mismatching register type for register {} in observation for group {}: expected = {}, actual = {}",
                                    register_index,
                                    register_group_id,
                                    expected_type,
                                    actual_type);
                            }
                        }
                    }
                } else {
                    log::warn!(
                        "Missing configuration for register group {} - rejecting observation",
                        register_group_id
                    );
                    return Err(Error::RegisterGroupUnknown);
                }
                let context = self
                    .register_groups
                    .get_mut(register_group_id)
                    .ok_or(Error::RegisterGroupUnknown)?;
                DefaultRecordPreludeGenerator.generate_prelude().and_then(
                    |(created_at, prelude)| {
                        let new_record = RegisterRecord {
                            prelude,
                            observation: observed_register_values,
                        };
                        log::debug!(
                            "Recording new observation for register group {}: {:?}",
                            register_group_id,
                            new_record
                        );
                        let prelude = context.storage.append_record(&created_at, new_record)?;
                        Ok(Some(prelude))
                    },
                )
            }
        }
    }

    // FIXME: Replace with an integration test
    #[allow(clippy::panic_in_result_fn)] // just a test
    pub fn smoke_test(&mut self) -> Result<()> {
        let register_group_id = RegisterGroupId::from_value("smoke-test-register-group".into());
        let register_group_config = RegisterGroupConfig {
            registers: vec![
                (
                    RegisterIndex::new(1),
                    RegisterType::Scalar(ScalarType::Bool),
                ),
                (RegisterIndex::new(2), RegisterType::Scalar(ScalarType::I64)),
                (RegisterIndex::new(3), RegisterType::Scalar(ScalarType::U64)),
                (RegisterIndex::new(4), RegisterType::Scalar(ScalarType::F64)),
                (RegisterIndex::new(5), RegisterType::String),
            ],
            storage: self.config.default_storage.clone(),
        };
        let orig_config =
            self.replace_register_group_config(register_group_id.clone(), register_group_config)?;
        let recorded_observations = vec![
            ObservedRegisterValues {
                observed_at: SystemTime::now(),
                register_values: vec![
                    None,
                    Some(ScalarValue::I64(0).into()),
                    Some(ScalarValue::U64(0).into()),
                    Some(ScalarValue::F64(0.0).into()),
                    None,
                ],
            },
            ObservedRegisterValues {
                observed_at: SystemTime::now(),
                register_values: vec![
                    Some(ScalarValue::Bool(false).into()),
                    Some(ScalarValue::I64(-1).into()),
                    Some(ScalarValue::U64(1).into()),
                    Some(ScalarValue::F64(-1.125).into()),
                    Some("Hello".to_owned().into()),
                ],
            },
            ObservedRegisterValues {
                observed_at: SystemTime::now(),
                register_values: vec![
                    Some(ScalarValue::Bool(true).into()),
                    Some(ScalarValue::I64(1).into()),
                    None,
                    Some(ScalarValue::F64(1.125).into()),
                    Some(", world!".to_owned().into()),
                ],
            },
            ObservedRegisterValues {
                observed_at: SystemTime::now(),
                register_values: vec![None, None, None, None, None],
            },
        ];
        for observation in &recorded_observations {
            self.record_observed_register_group_values(&register_group_id, observation.clone())?;
        }
        let recent_records = self.recent_records(
            &register_group_id,
            NonZeroUsize::new(recorded_observations.len()).unwrap(),
        )?;
        assert_eq!(recent_records.len(), recorded_observations.len());
        log::info!(
            "Smoke test recorded observations: {:?}",
            recorded_observations
        );
        log::info!("Smoke test records: {:?}", recent_records);
        // Restore configuration
        if let Some(orig_config) = orig_config {
            self.replace_register_group_config(register_group_id, orig_config)?;
        }
        Ok(())
    }
}
