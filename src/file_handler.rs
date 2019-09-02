// Copyright 2017 Kisio Digital and/or its affiliates.
//
// This program is free software: you can redistribute it and/or
// modify it under the terms of the GNU General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see
// <http://www.gnu.org/licenses/>.

use crate::Result;
use failure::{format_err, ResultExt};
use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};

pub trait FileHandler
where
    Self: std::marker::Sized,
{
    type Reader: std::io::Read;

    fn get_file_if_exists(self, name: &str) -> Result<(Option<Self::Reader>, PathBuf)>;

    fn get_file(self, name: &str) -> Result<(Self::Reader, PathBuf)> {
        let (reader, path) = self.get_file_if_exists(name)?;
        Ok((
            reader.ok_or_else(|| format_err!("file {:?} not found", path))?,
            path,
        ))
    }
}

/// PathFileHandler is used to read files for a directory
pub struct PathFileHandler {
    base_path: PathBuf,
}

impl PathFileHandler {
    pub fn new(path: PathBuf) -> Self {
        PathFileHandler { base_path: path }
    }
}

impl<'a> FileHandler for &'a mut PathFileHandler {
    type Reader = File;
    fn get_file_if_exists(self, name: &str) -> Result<(Option<Self::Reader>, PathBuf)> {
        let f = self.base_path.join(name);
        if f.exists() {
            Ok((Some(File::open(&f).with_context(ctx_from_path!(&f))?), f))
        } else {
            Ok((None, f))
        }
    }
}

/// ZipHandler is a wrapper around a ZipArchive
/// It provides a way to access the archive's file by their names
///
/// Unlike ZipArchive, it gives access to a file by it's name not regarding it's path in the ZipArchive
/// It thus cannot be correct if there are 2 files with the same name in the archive,
/// but for transport data if will make it possible to handle a zip with a sub directory
pub struct ZipHandler<R: std::io::Seek + std::io::Read> {
    archive: zip::ZipArchive<R>,
    archive_path: PathBuf,
    index_by_name: BTreeMap<String, usize>,
}

impl<R> ZipHandler<R>
where
    R: std::io::Seek + std::io::Read,
{
    pub fn new<P: AsRef<Path>>(r: R, path: P) -> Result<Self> {
        let mut archive = zip::ZipArchive::new(r)?;
        Ok(ZipHandler {
            index_by_name: Self::files_by_name(&mut archive),
            archive,
            archive_path: path.as_ref().to_path_buf(),
        })
    }

    fn files_by_name(archive: &mut zip::ZipArchive<R>) -> BTreeMap<String, usize> {
        (0..archive.len())
            .filter_map(|i| {
                let file = archive.by_index(i).ok()?;
                // we get the name of the file, not regarding it's patch in the ZipArchive
                let real_name = Path::new(file.name()).file_name()?;
                let real_name: String = real_name.to_str()?.into();
                Some((real_name, i))
            })
            .collect()
    }
}

impl<'a, R> FileHandler for &'a mut ZipHandler<R>
where
    R: std::io::Seek + std::io::Read,
{
    type Reader = zip::read::ZipFile<'a>;
    fn get_file_if_exists(self, name: &str) -> Result<(Option<Self::Reader>, PathBuf)> {
        let p = self.archive_path.join(name);
        match self.index_by_name.get(name) {
            None => Ok((None, p)),
            Some(i) => Ok((Some(self.archive.by_index(*i)?), p)),
        }
    }
}
