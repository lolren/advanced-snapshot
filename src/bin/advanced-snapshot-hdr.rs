// SPDX-License-Identifier: GPL-3.0-or-later

#[path = "../hdr.rs"]
mod hdr;

use std::path::PathBuf;

fn usage() -> ! {
    eprintln!(
        "usage: advanced-snapshot-hdr --output OUTPUT --input DARK --input BASE --input BRIGHT"
    );
    std::process::exit(2);
}

fn main() {
    let mut args = std::env::args_os().skip(1);
    let mut output = None;
    let mut inputs = Vec::new();

    while let Some(argument) = args.next() {
        match argument.to_str() {
            Some("--output") => output = args.next().map(PathBuf::from),
            Some("--input") => {
                if let Some(path) = args.next() {
                    inputs.push(PathBuf::from(path));
                } else {
                    usage();
                }
            }
            _ => usage(),
        }
    }

    let Some(output) = output else {
        usage();
    };
    if inputs.len() != 3 {
        usage();
    }

    if let Err(error) = hdr::merge_hdr_files(&inputs, &output) {
        eprintln!("advanced-snapshot-hdr: {error:#}");
        std::process::exit(1);
    }
}
