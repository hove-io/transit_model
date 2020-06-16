// Copyright (C) 2017 Kisio Digital and/or its affiliates.
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

//! Helpers to create a report for faillible processes.
use serde::Serialize;

/// Each report record will be categorized with a type implementing this
/// `ReportCategory` trait.
pub trait ReportCategory: Serialize + PartialEq {}

/// Type of the report
#[derive(Debug, Serialize, PartialEq)]
pub(crate) enum TransferReportCategory {
    IntraIgnored,
    InterIgnored,
    OnNonExistentStop,
    OnUnreferencedStop,
    AlreadyDeclared,
}

impl ReportCategory for TransferReportCategory {}

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
}

impl<R: ReportCategory> Default for Report<R> {
    fn default() -> Self {
        Report {
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

impl<R: ReportCategory> Report<R> {
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
}
