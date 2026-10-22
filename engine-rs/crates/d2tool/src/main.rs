//! d2tool — Diablo 2 MPQ command-line utility.
//!
//! ```text
//! d2tool extract <mpq> <name> <out>   extract <name> from <mpq> to <out>
//! d2tool probe   <mpq> <name>         archive/block info for <name> (hit/miss)
//! d2tool sha     <mpq> <name>         SHA-256 of the extracted <name>
//! ```
//!
//! All file I/O lives here; the `d2mpq` library itself is IO-free so it
//! builds unchanged for `wasm32-unknown-unknown`.

use std::fmt::Write as _;
use std::fs;
use std::process::ExitCode;

use d2mpq::{Archive, D2Error};
use sha2::{Digest, Sha256};

const USAGE: &str = "\
d2tool — Diablo 2 MPQ utility

USAGE:
    d2tool extract <mpq> <name> <out>   extract <name> from <mpq> to file <out>
    d2tool probe   <mpq> <name>         hit/miss + block info for <name>
    d2tool sha     <mpq> <name>         SHA-256 of the extracted <name>

Exit codes: 0 success, 1 probe miss, 2 error.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("d2tool: {err}");
            ExitCode::from(2)
        }
    }
}

fn run(args: &[String]) -> Result<ExitCode, D2Error> {
    match args {
        [cmd, mpq, name, out] if cmd == "extract" => cmd_extract(mpq, name, out),
        [cmd, mpq, name] if cmd == "probe" => cmd_probe(mpq, name),
        [cmd, mpq, name] if cmd == "sha" => cmd_sha(mpq, name),
        _ => {
            eprintln!("{USAGE}");
            Ok(ExitCode::from(2))
        }
    }
}

fn open_archive(path: &str) -> Result<Archive, D2Error> {
    let raw = fs::read(path).map_err(|e| D2Error::Io(format!("{path}: {e}")))?;
    Archive::open(raw)
}

fn cmd_extract(mpq: &str, name: &str, out: &str) -> Result<ExitCode, D2Error> {
    let archive = open_archive(mpq)?;
    let bytes = archive.read(name)?;
    fs::write(out, &bytes).map_err(|e| D2Error::Io(format!("{out}: {e}")))?;
    println!("{name}: {} bytes -> {out}", bytes.len());
    Ok(ExitCode::SUCCESS)
}

fn cmd_probe(mpq: &str, name: &str) -> Result<ExitCode, D2Error> {
    let archive = open_archive(mpq)?;
    println!(
        "archive v{} sector={} hash={} block={}",
        archive.version(),
        archive.sector_size(),
        archive.hash_table_len(),
        archive.block_table_len()
    );
    match archive.block_info(name) {
        Some(info) => {
            println!("HIT {name}");
            println!(
                "  index={} offset=0x{:08x} csize={} fsize={} flags=0x{:08x} [{}]",
                info.index,
                info.offset,
                info.csize,
                info.fsize,
                info.flags,
                flags_str(info.flags)
            );
            Ok(ExitCode::SUCCESS)
        }
        None => {
            println!("MISS {name}");
            Ok(ExitCode::from(1))
        }
    }
}

fn cmd_sha(mpq: &str, name: &str) -> Result<ExitCode, D2Error> {
    let archive = open_archive(mpq)?;
    let bytes = archive.read(name)?;
    let digest = Sha256::digest(&bytes);
    println!("{}  {name} ({} bytes)", hex(&digest), bytes.len());
    Ok(ExitCode::SUCCESS)
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// Human-readable flag names for `probe`.
fn flags_str(flags: u32) -> String {
    const NAMES: &[(u32, &str)] = &[
        (d2mpq::FLAG_EXISTS, "EXISTS"),
        (d2mpq::FLAG_SINGLE_UNIT, "SINGLE_UNIT"),
        (d2mpq::FLAG_DELETE, "DELETE"),
        (d2mpq::FLAG_SECTOR_CRC, "SECTOR_CRC"),
        (d2mpq::FLAG_PATCH, "PATCH"),
        (d2mpq::FLAG_ENCRYPTED, "ENCRYPTED"),
        (d2mpq::FLAG_KEY_FIX, "FIX_KEY"),
        (d2mpq::FLAG_IMPLODE, "IMPLODE"),
        (d2mpq::FLAG_COMPRESS, "COMPRESS"),
    ];
    let mut parts: Vec<&str> = NAMES
        .iter()
        .filter(|(bit, _)| flags & bit != 0)
        .map(|(_, name)| *name)
        .collect();
    if parts.is_empty() {
        parts.push("-");
    }
    parts.join("|")
}
