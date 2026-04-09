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

/// Each report record will be categorized with a type implementing this
/// `ReportCategory` trait.
pub trait ReportCategory: Serialize + PartialEq {}

/// A report record.
#[derive(Debug, Serialize, PartialEq)]
struct ReportRow<R: ReportCategory> {
    category: R,
    message: String,
}

/// An report is a list of report records with 2 levels of recording: warnings
/// and errors.
#[derive(Debug, Serialize)]
pub struct Report<R: ReportCategory> {
    errors: Vec<ReportRow<R>>,
    warnings: Vec<ReportRow<R>>,
    infos: Vec<ReportRow<R>>,
}

impl<R: ReportCategory> Default for Report<R> {
    fn default() -> Self {
        Report {
            errors: Vec::new(),
            warnings: Vec::new(),
            infos: Vec::new(),
        }
    }
}

impl<R: ReportCategory> Report<R> {
    /// Add an error report record.
    pub fn add_error(&mut self, error: String, error_type: R) {
        let report_row = ReportRow {
            category: error_type,
            message: error,
        };
        if !self.errors.contains(&report_row) {
            self.errors.push(report_row);
        }
    }
    /// Add a warning report record.
    pub fn add_warning(&mut self, warning: String, warning_type: R) {
        let report_row = ReportRow {
            category: warning_type,
            message: warning,
        };
        if !self.warnings.contains(&report_row) {
            self.warnings.push(report_row);
        }
    }
    /// Add an info report record.
    pub fn add_info(&mut self, info: String, info_type: R) {
        let report_row = ReportRow {
            category: info_type,
            message: info,
        };
        if !self.infos.contains(&report_row) {
            self.infos.push(report_row);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Debug, Serialize, PartialEq, Clone)]
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
        assert_eq!(report.infos[0].message, "info message");
        assert_eq!(report.infos[0].category, TestCategory::TypeA);
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
        assert_eq!(report.warnings[0].message, "warning message");
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
        assert_eq!(report.errors[0].message, "error message");
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
