// Copyright (C) 2017 Hove and/or its affiliates.
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by the
// Free Software Foundation, version 3.

// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.

// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>

//! Helpers to create a report for processes.

use serde::Serialize;
use std::collections::BTreeSet;

/// Each report record will be categorized with a type implementing this
/// `ReportCategory` trait.
pub trait ReportCategory: Serialize + Eq + Ord {}

/// A report record.
#[derive(Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
struct ReportRow<R: ReportCategory> {
    category: R,
    message: String,
}

/// A report is a list of report records with 3 levels of recording.
#[derive(Debug, Serialize)]
pub struct Report<R: ReportCategory> {
    errors: BTreeSet<ReportRow<R>>,
    warnings: BTreeSet<ReportRow<R>>,
    infos: BTreeSet<ReportRow<R>>,
}

impl<R: ReportCategory> Default for Report<R> {
    fn default() -> Self {
        Report {
            errors: BTreeSet::new(),
            warnings: BTreeSet::new(),
            infos: BTreeSet::new(),
        }
    }
}

impl<R: ReportCategory> Report<R> {
    /// Add an error report record.
    pub fn add_error(&mut self, error: String, error_type: R) {
        self.errors.insert(ReportRow {
            category: error_type,
            message: error,
        });
    }
    /// Add a warning report record.
    pub fn add_warning(&mut self, warning: String, warning_type: R) {
        self.warnings.insert(ReportRow {
            category: warning_type,
            message: warning,
        });
    }
    /// Add an info report record.
    pub fn add_info(&mut self, info: String, info_type: R) {
        self.infos.insert(ReportRow {
            category: info_type,
            message: info,
        });
    }
}

/// Report categories for transfer generation and modification.
/// to classify warnings and errors encountered during transfer processing.
#[derive(Debug, Serialize, Eq, PartialEq, Ord, PartialOrd)]
pub enum TransferReportCategory {
    // --- Warnings ---
    /// A transfer was already declared.
    AlreadyDeclared,
    /// A transfer rule was ignored.
    Ignored,
    /// A transfer references an unreferenced stop.
    OnUnreferencedStop,
    /// A transfer references a non-existent stop.
    OnNonExistentStop,

    // --- Infos ---
    /// A transfer was created.
    Created,
    /// A transfer was updated.
    Updated,
}
impl ReportCategory for TransferReportCategory {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Debug, Serialize, PartialEq, Eq, PartialOrd, Ord, Clone)]
    enum TestCategory {
        TypeA,
        TypeB,
    }
    impl ReportCategory for TestCategory {}

    // --- add_info ---

    #[test]
    fn test_add_info_single() {
        let mut report = Report::default();
        report.add_info("info message".to_string(), TestCategory::TypeA);
        assert_eq!(report.infos.len(), 1);
        let row = report.infos.iter().next().unwrap();
        assert_eq!(row.message, "info message");
        assert_eq!(row.category, TestCategory::TypeA);
    }

    #[test]
    fn test_add_info_no_duplicate() {
        let mut report = Report::default();
        report.add_info("repeated".to_string(), TestCategory::TypeA);
        report.add_info("repeated".to_string(), TestCategory::TypeA);
        assert_eq!(report.infos.len(), 1);
    }

    #[test]
    fn test_add_info_same_message_different_category_creates_two_entries() {
        let mut report = Report::default();
        report.add_info("message".to_string(), TestCategory::TypeA);
        report.add_info("message".to_string(), TestCategory::TypeB);
        assert_eq!(report.infos.len(), 2);
    }

    // --- add_warning ---

    #[test]
    fn test_add_warning_single() {
        let mut report = Report::default();
        report.add_warning("warning message".to_string(), TestCategory::TypeA);
        assert_eq!(report.warnings.len(), 1);
        let row = report.warnings.iter().next().unwrap();
        assert_eq!(row.message, "warning message");
    }

    #[test]
    fn test_add_warning_no_duplicate() {
        let mut report = Report::default();
        report.add_warning("repeated".to_string(), TestCategory::TypeB);
        report.add_warning("repeated".to_string(), TestCategory::TypeB);
        assert_eq!(report.warnings.len(), 1);
    }

    // --- add_error ---

    #[test]
    fn test_add_error_single() {
        let mut report = Report::default();
        report.add_error("error message".to_string(), TestCategory::TypeA);
        assert_eq!(report.errors.len(), 1);
        let row = report.errors.iter().next().unwrap();
        assert_eq!(row.message, "error message");
    }

    #[test]
    fn test_add_error_no_duplicate() {
        let mut report = Report::default();
        report.add_error("repeated".to_string(), TestCategory::TypeA);
        report.add_error("repeated".to_string(), TestCategory::TypeA);
        assert_eq!(report.errors.len(), 1);
    }

    // --- Level isolation ---

    #[test]
    fn test_levels_are_isolated() {
        let mut report = Report::default();
        report.add_info("msg".to_string(), TestCategory::TypeA);
        report.add_warning("msg".to_string(), TestCategory::TypeA);
        report.add_error("msg".to_string(), TestCategory::TypeA);

        assert_eq!(report.infos.len(), 1);
        assert_eq!(report.warnings.len(), 1);
        assert_eq!(report.errors.len(), 1);
    }

    // --- Default ---

    #[test]
    fn test_default_is_empty() {
        let report: Report<TestCategory> = Report::default();
        assert!(report.infos.is_empty());
        assert!(report.warnings.is_empty());
        assert!(report.errors.is_empty());
    }
}
