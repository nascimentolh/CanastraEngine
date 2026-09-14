//! `canastra`: inspection and migration tool for a Lineage 2 High Five client.

mod migrate;
mod texture;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use l2_crypto::Scheme;
use l2_dat::Value;
use ue2_package::{ObjectRef, Package, TAG};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

const USAGE: &str = "usage:
  canastra decrypt <file> <output>   write the decrypted file
  canastra package <file>            list a package's exports
  canastra dat <file>                decode a .dat table and print its first record
  canastra texture <package> <object> <output.png>
                                     decode one texture, e.g. `L2UI_CH3.utx Button.Btn1_normal`
  canastra scan <client-root>        decrypt and parse every file and texture, report failures
  canastra migrate <client-root> <server-stats-dir> [<output.cana>]
                                     convert items, skills and npcs into game data";

/// What `scan` understood a file to be.
enum Parsed {
    Package { exports: usize, textures: usize, texture_failures: Vec<String> },
    Table { records: usize },
    UnknownTable,
    Opaque,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["decrypt", input, output] => decrypt(Path::new(input), Path::new(output)),
        ["package", input] => package(Path::new(input)),
        ["dat", input] => dat(Path::new(input)),
        ["texture", input, object, output] => texture::export(Path::new(input), object, Path::new(output)),
        ["scan", root] => scan(Path::new(root)),
        ["migrate", client, server, output @ ..] if output.len() <= 1 => {
            migrate::run(Path::new(client), Path::new(server), output.first().map(Path::new))
        }
        _ => Err(USAGE.into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn read_decrypted(path: &Path) -> Result<(Scheme, Vec<u8>)> {
    let data = fs::read(path)?;
    let scheme = l2_crypto::detect(&data)?;
    Ok((scheme, l2_crypto::decrypt(&data, path)?.into_owned()))
}

fn decrypt(input: &Path, output: &Path) -> Result {
    let (scheme, plain) = read_decrypted(input)?;
    fs::write(output, &plain)?;
    println!("{scheme:?}: wrote {} bytes to {}", plain.len(), output.display());
    Ok(())
}

fn package(input: &Path) -> Result {
    let (scheme, plain) = read_decrypted(input)?;
    let package = Package::parse(&plain)?;
    println!(
        "{scheme:?} package v{} licensee {} | {} names, {} imports, {} exports",
        package.version,
        package.licensee,
        package.names().len(),
        package.imports().len(),
        package.exports().len()
    );
    for (index, export) in package.exports().iter().enumerate() {
        println!(
            "{index:>6}  {:<24} {:<60} {:>10} @ {:#x}",
            package.class_name(export),
            package.object_path(ObjectRef::Export(index)),
            export.serial_size,
            export.serial_offset
        );
    }
    Ok(())
}

fn dat(input: &Path) -> Result {
    let (_, plain) = read_decrypted(input)?;
    let name = file_name(input);
    let table = l2_dat_h5::table(name).ok_or_else(|| format!("no H5 layout for {name}"))?;
    let records = l2_dat::decode(table, &plain)?;
    let items = records_of(&records);
    println!("{}: {} records", table.name, items.len());
    if let Some(first) = items.first() {
        println!("{first:#?}");
    }
    Ok(())
}

fn scan(root: &Path) -> Result {
    let mut files = Vec::new();
    collect_files(root, &mut files)?;

    let mut schemes = BTreeMap::<Scheme, usize>::new();
    let (mut packages, mut exports, mut tables, mut records) = (0, 0, 0, 0);
    let (mut textures, mut texture_failures) = (0, 0);
    let mut unknown_tables = Vec::new();
    let mut failures = 0;
    for path in &files {
        let outcome = read_decrypted(path).and_then(|(scheme, plain)| {
            let name = file_name(path);
            let parsed = if plain.starts_with(&TAG.to_le_bytes()) {
                let package = Package::parse(&plain)?;
                let (textures, texture_failures) = texture::check_all(&package, &plain);
                Parsed::Package { exports: package.exports().len(), textures, texture_failures }
            } else if let Some(table) = l2_dat_h5::table(name) {
                Parsed::Table { records: records_of(&l2_dat::decode(table, &plain)?).len() }
            } else if name.to_ascii_lowercase().ends_with(".dat") {
                Parsed::UnknownTable
            } else {
                Parsed::Opaque
            };
            Ok((scheme, parsed))
        });
        match outcome {
            Ok((scheme, parsed)) => {
                *schemes.entry(scheme).or_default() += 1;
                match parsed {
                    Parsed::Package { exports: n, textures: decoded, texture_failures: failed } => {
                        (packages, exports, textures) = (packages + 1, exports + n, textures + decoded);
                        texture_failures += failed.len();
                        for failure in failed {
                            eprintln!("FAIL {}: {failure}", path.display());
                        }
                    }
                    Parsed::Table { records: n } => (tables, records) = (tables + 1, records + n),
                    Parsed::UnknownTable => unknown_tables.push(path.display().to_string()),
                    Parsed::Opaque => {}
                }
            }
            Err(error) => {
                failures += 1;
                eprintln!("FAIL {}: {error}", path.display());
            }
        }
    }

    println!("{} files: {schemes:?}", files.len());
    println!("{packages} packages parsed, {exports} exports");
    println!("{textures} textures decoded, {texture_failures} failed");
    println!("{tables} tables decoded, {records} records");
    println!("{} .dat files without a layout: {unknown_tables:#?}", unknown_tables.len());
    println!("{failures} failures");
    match failures + texture_failures {
        0 => Ok(()),
        n => Err(format!("{n} failure(s)").into()),
    }
}

fn file_name(path: &Path) -> &str {
    path.file_name().and_then(|name| name.to_str()).unwrap_or_default()
}

/// Every H5 table is a single top-level list of records.
fn records_of(table: &l2_dat::Record) -> &[Value] {
    table
        .iter()
        .find_map(|(_, value)| match value {
            Value::List(items) => Some(items.as_slice()),
            _ => None,
        })
        .unwrap_or_default()
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_files(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}
