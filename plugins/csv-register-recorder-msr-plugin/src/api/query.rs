use std::num::NonZeroUsize;

use msr_core::storage::RecordPreludeFilter;

use crate::ResultSender;

use super::{Config, RegisterGroupConfig, RegisterGroupId, Status, StoredRegisterRecord};

#[derive(Debug, Clone)]
pub struct RecentRecordsRequest {
    pub limit: NonZeroUsize,
}

#[derive(Debug, Clone)]
pub struct FilterRecordsRequest {
    pub limit: NonZeroUsize,
    pub filter: RecordPreludeFilter,
}

#[derive(Debug, Clone, Default)]
pub struct StatusRequest {
    pub with_register_groups: bool,
    pub with_storage_statistics: bool,
}

#[derive(Debug)]
pub enum Query {
    Config(ResultSender<Config>),
    RegisterGroupConfig(ResultSender<Option<RegisterGroupConfig>>, RegisterGroupId),
    Status(ResultSender<Status>, StatusRequest),
    RecentRecords(
        ResultSender<Vec<StoredRegisterRecord>>,
        RegisterGroupId,
        RecentRecordsRequest,
    ),
    FilterRecords(
        ResultSender<Vec<StoredRegisterRecord>>,
        RegisterGroupId,
        FilterRecordsRequest,
    ),
}
