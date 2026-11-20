//! Turning bare teardown.exe offsets into "inside function X".
//!
//! A teardown.exe's exception table (.pdata, not encrypted on disk) gives each offset's enclosing
//! function. Names come from per-build tables in server/names/<exe>-<PE timestamp>.tsv (compiled
//! in through name_tables.rs, which tools/port_names.py regenerates). A table is used only for
//! the exact exe and build it was made for; any other build gets function starts only.
//!
//! The first table (teardown.exe 0x6ae468bd) is Teardown-PhysX's reverse-engineered list
//! (https://github.com/slopkhayzo/Teardown-PhysX, MIT licence); the others are ported from it.

use crate::name_tables::TABLES;
use std::path::Path;

pub struct Exe {
    pub stamp: u32,
    funcs: Vec<(u32, u32)>, // begin, end RVAs, sorted
    names: Vec<(u32, String, bool)>, // rva, name, matched unchanged (false: ported to changed code)
    pub table: Option<&'static str>,
}

fn u16_at(d: &[u8], o: usize) -> Option<u32> {
    d.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as u32)
}
fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    d.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// The table for an exe file name and build, if there is one.
fn table_for(file: &str, stamp: u32) -> Option<&'static str> {
    TABLES.iter().find(|t| t.0.eq_ignore_ascii_case(file) && t.1 == stamp).map(|t| t.2)
}

/// Builds that have a table: "teardown.exe 0x6ae468bd", ...
pub fn known_builds() -> Vec<String> {
    TABLES.iter().map(|t| format!("{} {:#010x}", t.0, t.1)).collect()
}

pub fn load(path: &Path) -> Option<Exe> {
    let d = std::fs::read(path).ok()?;
    let pe = u32_at(&d, 0x3c)? as usize;
    let stamp = u32_at(&d, pe + 8)?;
    let nsec = u16_at(&d, pe + 6)? as usize;
    let opt_size = u16_at(&d, pe + 20)? as usize;
    let opt = pe + 24;
    let (exc_rva, exc_size) = (u32_at(&d, opt + 112 + 3 * 8)?, u32_at(&d, opt + 116 + 3 * 8)?);
    let mut file_off = None;
    for i in 0..nsec {
        let s = opt + opt_size + i * 40;
        let (vsize, va, rsize, rptr) = (u32_at(&d, s + 8)?, u32_at(&d, s + 12)?, u32_at(&d, s + 16)?, u32_at(&d, s + 20)?);
        if exc_rva >= va && exc_rva < va + vsize.max(rsize) {
            file_off = Some((rptr + exc_rva - va) as usize);
        }
    }
    let base = file_off?;
    let mut funcs: Vec<(u32, u32)> = (0..exc_size as usize / 12)
        .filter_map(|i| Some((u32_at(&d, base + i * 12)?, u32_at(&d, base + i * 12 + 4)?)))
        .collect();
    funcs.sort();
    // A crash report's copy is named teardown.exe whichever exe crashed; try the file's own name first.
    let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let table = table_for(&file, stamp).or_else(|| table_for("teardown.exe", stamp)).or_else(|| table_for("teardown_modtest.exe", stamp));
    let mut names: Vec<(u32, String, bool)> = table
        .map(|t| {
            t.lines()
                .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
                .filter_map(|l| {
                    let mut p = l.split('\t');
                    let rva = u32::from_str_radix(p.next()?.trim_start_matches("0x"), 16).ok()?;
                    let name = p.next()?.to_string();
                    let same = p.next().map(|h| h == "same").unwrap_or(true);
                    Some((rva, name, same))
                })
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    Some(Exe { stamp, funcs, names, table })
}

impl Exe {
    /// "in ShapeInterpTransform+0x1a", "in fn 0x2dd20", or "" outside every function.
    pub fn describe(&self, rva: u32) -> String {
        let i = self.funcs.partition_point(|f| f.0 <= rva);
        if i == 0 || rva >= self.funcs[i - 1].1 {
            return String::new();
        }
        let begin = self.funcs[i - 1].0;
        let j = self.names.partition_point(|n| n.0 <= begin);
        if j > 0 {
            let (start, name, same) = &self.names[j - 1];
            // A function's later chunks have their own table entries: allow a short distance.
            if *start == begin || begin - start < 0x40 {
                let chunk = if *start == begin { "" } else { ", a later chunk" };
                let ported = if *same { "" } else { ", name ported to changed code" };
                return format!("in {name}+{:#x}{chunk}{ported}", rva - start);
            }
        }
        format!("in fn {begin:#x}")
    }
}
