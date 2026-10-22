//! mpq_extract — extract one file from an MPQ archive with the repaired
//! reader (raw-last-sector rule, `archive.rs::plain_len`).
//!
//! ```text
//! mpq_extract <archive.mpq> <internal\name> <out.file>
//! ```
//!
//! Exit 1 with `NOT FOUND` when the name hashes to no live block. The Python
//! reference (`tools/mpq_mini.py`) mishandles some PKWARE-DCL streams on
//! character DCCs (`bad lit flag`); this reader is authoritative.

fn die(msg: impl std::fmt::Display) -> ! {
    eprintln!("mpq_extract: {msg}");
    std::process::exit(1);
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.len() != 3 {
        die("usage: mpq_extract <archive.mpq> <internal\\name> <out.file>");
    }
    let raw = std::fs::read(&argv[0]).unwrap_or_else(|e| die(format!("{}: {e}", argv[0])));
    let archive = d2mpq::Archive::open(raw).unwrap_or_else(|e| die(format!("{}: {e}", argv[0])));
    match archive.read(&argv[1]) {
        Ok(data) => {
            std::fs::write(&argv[2], &data).unwrap_or_else(|e| die(format!("{}: {e}", argv[2])));
            println!("{}: {} B -> {}", argv[1], data.len(), argv[2]);
        }
        Err(e) => {
            println!("NOT FOUND {} ({e})", argv[1]);
            std::process::exit(1);
        }
    }
}
