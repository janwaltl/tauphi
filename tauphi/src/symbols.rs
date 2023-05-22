//! Symbol resolution of PIDs to commands and instruction pointers to function names.
use crate::error::TauphiError;
use std::fs;
use std::io::{BufRead, BufReader, Error};
use std::path::{Path, PathBuf};
use std::process;

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

/// Resolve instruction pointers of a single process into function names, source locations.
///
/// Uses `addr2line` child process to perform the translation.
///
/// # Examples
/// ```no_run
/// use std::path::Path;
/// use symbols::SymbolResolver;
/// let mut resolver = SymbolResolver::new(Path::new("./executable")).unwrap();
///
/// let (function, source) = resolver.resolve(0x118A).unwrap();
///
/// println!("Function {} at line {source}", function, source);
/// ```
pub struct SymbolResolver {
    child: process::Child,
    input: process::ChildStdin,
    output: BufReader<process::ChildStdout>,
}

/// Parse a single line in `/proc/<pid>/maps`.
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

impl SymbolResolver {
    /// Create a new resolver of the given process.
    ///
    /// # Arguments
    /// * `filename` Path to the executable whose symbols to resolve.
    pub fn new(filename: &Path) -> Result<Self, TauphiError> {
        let mut child = process::Command::new("/usr/bin/addr2line")
            .stdin(process::Stdio::piped())
            .stdout(process::Stdio::piped())
            .arg("-ifCe")
            .arg(filename)
            .spawn()?;
        let stdout = child.stdout.take().unwrap();
        let stdout = std::io::BufReader::new(stdout);
        let stdin = child.stdin.take().unwrap();

        Ok(Self {
            child,
            input: stdin,
            output: stdout,
        })
    }

    /// Translate the instruction address in the executable to the function name and source
    /// location.
    ///
    /// # Arguments
    /// `address` Absolute address inside the executable which is translated to the function to
    /// which it belongs.
    pub fn resolve(&mut self, address: usize) -> Result<(String, String), TauphiError> {
        use std::io::Write;
        // Send the address as hex to addr2line.
        writeln!(&mut self.input, "{:#x}", address)?;

        //addr2line outputs two lines, first with the function name, second with the source
        //location.

        let mut function_name = String::new();
        let _ = self.output.read_line(&mut function_name)?;
        function_name.pop(); // Remove the newline.
        let mut source = String::new();
        let _ = self.output.read_line(&mut source)?;
        source.pop(); // Remove the newline.

        Ok((function_name, source))
    }
}

impl Drop for SymbolResolver {
    /// Kill the child and wait until it exits.
    fn drop(&mut self) {
        self.child
            .kill()
            .expect("Symbol resolver process(addr2line) could not be killed.");
        self.child
            .wait()
            .expect("Failed to wait for the child process.");
    }
}
