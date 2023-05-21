//! Symbol resolution of PIDs to commands and instruction pointers to function names.
use crate::error::TauphiError;
use std::fs;
use std::io::{BufRead, BufReader, Error};
use std::path::PathBuf;

/// File (or its region) mapped in memory of another process.
#[derive(Debug)]
pub struct MappedRegion {
    /// Pathname to the mapped file.
    pub file: PathBuf,
    /// Offset in process memory space at which the file is mapped.
    pub begin: usize,
    /// Ending offset.
    pub end: usize,
    /// Starting offset of the file's region which is mapped.
    pub offset: usize,
}

/// Useful information about a single process during sampling.
#[derive(Debug)]
pub struct PIDInfo {
    /// Process ID
    pub pid: u32,
    /// Process cmdline, the process might have modified this as desired.
    pub cmdline: String,
    /// Files mapped into this process's memory space.
    ///
    /// Includes the process's executable itself and any loaded shared libs.
    ///
    /// This is used to map instruction pointers to functions/lines.
    pub mapped_regions: Vec<MappedRegion>,
}

/// Parse a single line in /proc/<pid>/maps.
fn parse_map_line(line: Result<String, Error>) -> Result<MappedRegion, TauphiError> {
    let line = line?;
    let mut it = line.splitn(6, ' ');
    let range = it.next().ok_or(TauphiError::InvalidPIDMapsFromat)?;
    let _perms = it.next().ok_or(TauphiError::InvalidPIDMapsFromat)?;
    let offset = it.next().ok_or(TauphiError::InvalidPIDMapsFromat)?;
    let _dev = it.next().ok_or(TauphiError::InvalidPIDMapsFromat)?;
    let _inode = it.next().ok_or(TauphiError::InvalidPIDMapsFromat)?;
    let path = it.next().ok_or(TauphiError::InvalidPIDMapsFromat)?;
    let path = PathBuf::from(path.trim());

    let (begin, end) = range
        .split_once('-')
        .ok_or(TauphiError::InvalidPIDMapsFromat)?;
    let begin = usize::from_str_radix(begin, 16).map_err(|_| TauphiError::InvalidPIDMapsFromat)?;
    let end = usize::from_str_radix(end, 16).map_err(|_| TauphiError::InvalidPIDMapsFromat)?;

    let offset =
        usize::from_str_radix(offset, 16).map_err(|_| TauphiError::InvalidPIDMapsFromat)?;

    Ok(MappedRegion {
        begin,
        end,
        offset,
        file: path,
    })
}

impl PIDInfo {
    /// Collect information about `pid` process.
    pub fn new(pid: u32) -> Result<Self, TauphiError> {
        let cmdline = fs::read_to_string(format!("/proc/{pid}/cmdline"))?;
        let maps = fs::File::open(format!("/proc/{pid}/maps"))?;
        let regions: Result<Vec<MappedRegion>, TauphiError> =
            BufReader::new(maps).lines().map(parse_map_line).collect();
        Ok(PIDInfo {
            pid,
            cmdline,
            mapped_regions: regions?,
        })
    }
}
