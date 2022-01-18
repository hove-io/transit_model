//! Provides an easy way to access directory or flat zip archive
use crate::Result;
use anyhow::{anyhow, Context};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek},
    path::{Path, PathBuf},
};

/// Allows files in a directory or ZipArchive to be read either
pub trait FileHandler
where
    Self: std::marker::Sized,
{
    /// Reader
    type Reader: Read;

    /// Return a file if exist
    fn get_file_if_exists(self, name: &str) -> Result<(Option<Self::Reader>, PathBuf)>;

    /// Return a file or an error if not exist
    fn get_file(self, name: &str) -> Result<(Self::Reader, PathBuf)> {
        let (reader, path) = self.get_file_if_exists(name)?;
        Ok((
            reader.ok_or_else(|| anyhow!("file {:?} not found", path))?,
            path,
        ))
    }

    /// Allows to have nicer error messages
    fn source_name(&self) -> &str;
}

/// PathFileHandler is used to read files for a directory
pub struct PathFileHandler<P: AsRef<Path>> {
    base_path: P,
}

impl<P: AsRef<Path>> PathFileHandler<P> {
    /// Constructs a new PathFileHandler
    pub fn new(path: P) -> Self {
        PathFileHandler { base_path: path }
    }
}

impl<'a, P: AsRef<Path>> FileHandler for &'a mut PathFileHandler<P> {
    type Reader = File;
    fn get_file_if_exists(self, name: &str) -> Result<(Option<Self::Reader>, PathBuf)> {
        let f = self.base_path.as_ref().join(name);
        if f.exists() {
            Ok((
                Some(File::open(&f).with_context(|| format!("Error reading {:?}", &f))?),
                f,
            ))
        } else {
            Ok((None, f))
        }
    }
    fn source_name(&self) -> &str {
        self.base_path.as_ref().to_str().unwrap_or_else(|| {
            panic!(
                "the path '{:?}' should be valid UTF-8",
                self.base_path.as_ref()
            )
        })
    }
}

/// ZipHandler is a wrapper around a ZipArchive
/// It provides a way to access the archive's file by their names
///
/// Unlike ZipArchive, it gives access to a file by its name not regarding its path in the ZipArchive
/// It thus cannot be correct if there are 2 files with the same name in the archive,
/// but for transport data if will make it possible to handle a zip with a sub directory
pub struct ZipHandler<R: Seek + Read> {
    archive: zip::ZipArchive<R>,
    archive_path: PathBuf,
    index_by_name: BTreeMap<String, usize>,
}

impl<R> ZipHandler<R>
where
    R: Seek + Read,
{
    pub(crate) fn new<P: AsRef<Path>>(r: R, path: P) -> Result<Self> {
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
                // we get the name of the file, not regarding its path in the ZipArchive
                let real_name = Path::new(file.name()).file_name()?;
                let real_name: String = real_name.to_str()?.into();
                Some((real_name, i))
            })
            .collect()
    }
}

impl<'a, R> FileHandler for &'a mut ZipHandler<R>
where
    R: Seek + Read,
{
    type Reader = zip::read::ZipFile<'a>;
    fn get_file_if_exists(self, name: &str) -> Result<(Option<Self::Reader>, PathBuf)> {
        let p = self.archive_path.join(name);
        match self.index_by_name.get(name) {
            None => Ok((None, p)),
            Some(i) => Ok((Some(self.archive.by_index(*i)?), p)),
        }
    }
    fn source_name(&self) -> &str {
        self.archive_path
            .to_str()
            .unwrap_or_else(|| panic!("the path '{:?}' should be valid UTF-8", self.archive_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::io::Read;

    #[test]
    fn path_file_handler() {
        let mut file_handler = PathFileHandler::new(PathBuf::from("tests/fixtures/file-handler"));

        let (mut hello, _) = file_handler.get_file("hello.txt").unwrap();
        let mut hello_str = String::new();
        hello.read_to_string(&mut hello_str).unwrap();
        assert_eq!("hello\n", hello_str);

        let (mut world, _) = file_handler.get_file("folder/world.txt").unwrap();
        let mut world_str = String::new();
        world.read_to_string(&mut world_str).unwrap();
        assert_eq!("world\n", world_str);
    }

    #[test]
    fn zip_file_handler() {
        let p = "tests/fixtures/file-handler.zip";
        let reader = File::open(p).unwrap();
        let mut file_handler = ZipHandler::new(reader, p).unwrap();

        {
            let (mut hello, _) = file_handler.get_file("hello.txt").unwrap();
            let mut hello_str = String::new();
            hello.read_to_string(&mut hello_str).unwrap();
            assert_eq!("hello\n", hello_str);
        }

        {
            let (mut world, _) = file_handler.get_file("world.txt").unwrap();
            let mut world_str = String::new();
            world.read_to_string(&mut world_str).unwrap();
            assert_eq!("world\n", world_str);
        }
    }
}
