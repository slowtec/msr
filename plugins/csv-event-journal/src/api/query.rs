use std::num::NonZeroUsize;

use msr_core::event_journal::{RecordFilter, StoredRecord};

use crate::ResultSender;

use super::{Config, Status};

#[derive(Debug)]
pub enum Query {
    Config(ResultSender<Config>),
    Status(ResultSender<Status>, StatusRequest),
    RecentRecords(ResultSender<Vec<StoredRecord>>, RecentRecordsRequest),
    FilterRecords(ResultSender<Vec<StoredRecord>>, FilterRecordsRequest),
}

#[derive(Debug, Clone)]
pub struct StatusRequest {
    pub with_storage_statistics: bool,
}

#[derive(Debug, Clone)]
pub struct RecentRecordsRequest {
    pub limit: NonZeroUsize,
}

#[derive(Debug, Clone)]
pub struct FilterRecordsRequest {
    pub limit: NonZeroUsize,
    pub filter: RecordFilter,
}
