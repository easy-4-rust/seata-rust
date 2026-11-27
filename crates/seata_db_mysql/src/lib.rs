//! MySQL AT数据源；真实事务、元数据及镜像按WP05顺序实施。
mod cache;
mod connection_proxy;
mod current_table_constraints;
mod data_compare_utils;
mod data_source_manager;
mod data_source_proxy;
mod db_error;
mod dialect_escape_rules;
mod dm;
mod exec;
mod java_compare_field;
mod java_compare_records;
mod java_compare_temporal;
mod java_compare_value;
mod java_tc_mysql_lock_collation;
mod java_tc_mysql_lock_limits;
mod kingbase;
mod mariadb;
mod my_sql_launcher_claim_error;
mod my_sql_launcher_snapshot;
mod my_sql_launcher_store;
mod my_sql_transferred_launcher;
mod mysql;
mod mysql_at_registration_checks;
#[cfg(feature = "experimental_changed_commit")]
mod mysql_changed_commit_error;
#[cfg(feature = "experimental_changed_commit")]
mod mysql_commit_attempt;
#[cfg(feature = "experimental_changed_commit")]
mod mysql_commit_attempt_store;
mod mysql_commit_check_error;
mod mysql_commit_checked_transaction;
#[cfg(feature = "experimental_changed_commit")]
mod mysql_commit_recovery_error;
#[cfg(feature = "experimental_changed_commit")]
mod mysql_commit_recovery_outcome;
mod mysql_failure_report_guard;
#[cfg(feature = "experimental_changed_commit")]
mod mysql_first_phase_report_error;
mod mysql_image_reader;
#[cfg(test)]
mod mysql_loopback_fixture;
mod mysql_no_change_commit_error;
mod mysql_options;
#[cfg(test)]
mod mysql_options_java_tc_profile_tests;
#[cfg(feature = "experimental_changed_commit")]
mod mysql_phase_one_failure_report_error;
mod mysql_prepare_error;
#[cfg(all(test, feature = "experimental_changed_commit"))]
mod mysql_prepare_error_tests;
#[cfg(feature = "experimental_changed_commit")]
mod mysql_prepare_failure_report_error;
#[cfg(feature = "experimental_changed_commit")]
mod mysql_prepare_reported_error;
mod mysql_prepared_transaction;
#[cfg(feature = "experimental_changed_commit")]
mod mysql_recovered_report_error;
mod mysql_recovery_error;
mod mysql_resource_binding;
mod mysql_transaction_preparer;
mod oceanbase;
mod oracle;
mod oscar;
mod polardbx;
mod postgresql;
mod recovery_action;
mod recovery_schema;
mod registration_origin;
mod rm_branch_record;
mod rm_branch_store;
mod sqlserver;
mod undo;
mod undo_fence_error;
mod undo_rollback_attempt;
#[cfg(test)]
mod undo_rollback_attempt_tests;
mod undo_rollback_error;
mod undo_rollback_observation;
mod util;
pub use connection_proxy::MySqlLocalTransaction;
pub use data_compare_utils::{DataCompareUtils, JavaCompareRowMap};
pub use data_source_manager::MySqlDataSourceManager;
pub use data_source_proxy::MySqlDataSource;
pub use db_error::DbError;
pub use dm::DmEscapeHandler;
pub use java_compare_field::JavaCompareField;
pub use java_compare_records::{JavaCompareRecords, JavaCompareRecordsMut};
pub use java_compare_temporal::JavaCompareTemporal;
pub use java_compare_value::JavaCompareValue;
pub use kingbase::KingbaseEscapeHandler;
pub use mariadb::MariadbEscapeHandler;
pub use my_sql_launcher_claim_error::MySqlLauncherClaimError;
pub use my_sql_launcher_snapshot::MySqlLauncherSnapshot;
pub use my_sql_launcher_store::MySqlLauncherStore;
pub use my_sql_transferred_launcher::MySqlTransferredLauncher;
pub use mysql::{MySQLEscapeHandler, UndoFenceObservation};
#[cfg(feature = "experimental_changed_commit")]
pub use mysql_changed_commit_error::MySqlChangedCommitError;
#[cfg(feature = "experimental_changed_commit")]
pub use mysql_commit_attempt::MySqlCommitAttempt;
pub use mysql_commit_check_error::MySqlCommitCheckError;
pub use mysql_commit_checked_transaction::MySqlCommitCheckedTransaction;
#[cfg(feature = "experimental_changed_commit")]
pub use mysql_commit_recovery_error::MySqlCommitRecoveryError;
#[cfg(feature = "experimental_changed_commit")]
pub use mysql_commit_recovery_outcome::MySqlCommitRecoveryOutcome;
pub use mysql_failure_report_guard::MySqlFailureReportGuard;
#[cfg(feature = "experimental_changed_commit")]
pub use mysql_first_phase_report_error::MySqlFirstPhaseReportError;
pub use mysql_no_change_commit_error::MySqlNoChangeCommitError;
pub use mysql_options::MySqlOptions;
#[cfg(feature = "experimental_changed_commit")]
pub use mysql_phase_one_failure_report_error::MySqlPhaseOneFailureReportError;
pub use mysql_prepare_error::MySqlPrepareError;
#[cfg(feature = "experimental_changed_commit")]
pub use mysql_prepare_failure_report_error::MySqlPrepareFailureReportError;
#[cfg(feature = "experimental_changed_commit")]
pub use mysql_prepare_reported_error::MySqlPrepareReportedError;
pub use mysql_prepared_transaction::MySqlPreparedTransaction;
#[cfg(feature = "experimental_changed_commit")]
pub use mysql_recovered_report_error::MySqlRecoveredReportError;
pub use mysql_recovery_error::MySqlRecoveryError;
pub use oceanbase::OceanBaseEscapeHandler;
pub use oracle::OracleEscapeHandler;
pub use oscar::OscarEscapeHandler;
pub use polardbx::PolarDBXEscapeHandler;
pub use postgresql::PostgresqlEscapeHandler;
pub use sqlserver::SqlServerEscapeHandler;
pub use undo_fence_error::UndoFenceError;
pub use undo_rollback_error::UndoRollbackError;
pub use undo_rollback_observation::UndoRollbackObservation;
pub use util::{
    JdbcConnection, JdbcDriverLoader, JdbcResourceInitialization, JdbcUtils, JdbcXaConnection,
};

mod my_sql_control_event;
mod my_sql_control_guard;
mod my_sql_control_metrics;
mod my_sql_recovery_metrics_sampler;
mod my_sql_recovery_operation;
pub use my_sql_control_metrics::MySqlControlMetrics;
pub use my_sql_recovery_metrics_sampler::MySqlRecoveryMetricsSampler;
pub use my_sql_recovery_operation::MySqlRecoveryOperation;

#[cfg(test)]
mod my_sql_undo_metrics_sampler_tests;

mod my_sql_undo_event;
mod my_sql_undo_guard;
mod my_sql_undo_metrics;
mod my_sql_undo_metrics_sampler;
mod my_sql_undo_operation;
pub use my_sql_undo_metrics::MySqlUndoMetrics;
pub use my_sql_undo_metrics_sampler::MySqlUndoMetricsSampler;
pub use my_sql_undo_operation::MySqlUndoOperation;

mod my_sql_phase_one_guard;
mod my_sql_phase_one_metrics;
mod my_sql_phase_one_metrics_sampler;
mod my_sql_phase_one_operation;
mod my_sql_phase_one_outcome;
pub use my_sql_phase_one_metrics::MySqlPhaseOneMetrics;
pub use my_sql_phase_one_metrics_sampler::MySqlPhaseOneMetricsSampler;
pub use my_sql_phase_one_operation::MySqlPhaseOneOperation;
pub use my_sql_phase_one_outcome::MySqlPhaseOneOutcome;

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[path = "../tests/support/java_checksum_events.rs"]
mod test_event_capture;

#[cfg(feature = "xa")]
mod mysql_xa_error;
#[cfg(feature = "xa")]
pub use mysql_xa_error::MySqlXaError;
#[cfg(feature = "xa")]
mod mysql_xa_data_source;
#[cfg(feature = "xa")]
pub use mysql_xa_data_source::MySqlXaDataSource;
#[cfg(feature = "xa")]
mod mysql_xa_transaction;
#[cfg(feature = "xa")]
pub use mysql_xa_transaction::MySqlXaTransaction;
#[cfg(feature = "xa")]
mod mysql_xa_table;

#[cfg(feature = "xa")]
mod mysql_xa_branch_record;

#[cfg(feature = "xa")]
mod mysql_xa_branch_store;

#[cfg(feature = "xa")]
mod mysql_xa_enlist_error;

#[cfg(feature = "xa")]
mod mysql_xa_resource_manager;

#[cfg(feature = "xa")]
mod mysql_xa_enlisted_transaction;
#[cfg(feature = "xa")]
pub use mysql_xa_enlist_error::MySqlXaEnlistError;
#[cfg(feature = "xa")]
pub use mysql_xa_enlisted_transaction::MySqlXaEnlistedTransaction;
#[cfg(feature = "xa")]
pub use mysql_xa_resource_manager::MySqlXaResourceManager;

#[cfg(feature = "xa")]
mod mysql_xa_commit_witness;
