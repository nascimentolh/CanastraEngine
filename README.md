# CanastraEngine

Lineage 2 High Five client in Rust. It reads assets straight from an existing H5 client install;
the `system` tables (`.dat`) are being migrated to Canastra's own data format.

## Architecture

Dependencies only point downward. Format crates are pure (`&[u8]` in, types out, no IO),
so they can be tested against synthetic bytes and reused by tools.

```
apps/        canastra-cli        IO, filesystem, user-facing commands
crates/      l2-dat              schema-driven decoder for the legacy system/*.dat tables
             ue2-package         UE2 package tables: names, imports, exports, object paths
             ue2-core            bounds-checked binary reader (compact index, FString, UTF-16)
             l2-crypto           Lineage2Ver111 / 121 (XOR) and 413 (RSA + zlib)
```

## Commands

```
cargo run --release -p canastra-cli -- scan    "<H5 client root>"
cargo run --release -p canastra-cli -- package "<file>.utx"
cargo run --release -p canastra-cli -- dat     "<file>.dat"
cargo run --release -p canastra-cli -- decrypt "<file>.dat" out.bin
```

`scan` on a real H5 client: 3020 of 3021 files decrypt, 1244 packages parse, and all 54 `system`
tables decode with every byte accounted for. The one failure (`Animations/RTA63900`) is a
corrupt leftover file.

## Guards

Enable the pre-commit hook once per clone:

```
git config core.hooksPath .githooks
```

It runs `cargo fmt --check`, `cargo clippy -D warnings` and `cargo test`. Lints live in the
workspace `Cargo.toml`: `unsafe` is forbidden, clippy `all` is denied, `pedantic` warns, and
`unwrap`/`panic`/`todo` are denied outside tests, because client files are untrusted input.

## Format notes (verified against the client)

- Header: `Lineage2VerXXX` in UTF-16LE (28 bytes). Optional 20-byte trailer `[0, ?, ?, crc32, 0]`.
- Ver111: XOR `0xAC`. Ver121: XOR with the low byte of the sum of the lower-cased file name.
  On XOR files the trailer is stripped only when its CRC matches (custom encoders omit it).
- Ver413: 128-byte RSA blocks (NCsoft and l2encdec public keys), each `[0,0,0,size,...data]`,
  concatenating to `u32 size` + zlib. Block alignment tells whether the trailer exists.
- Packages: tag `0x9E2A83C1`, versions 117 to 128. An export's offset is present when `SerialSize != 0`.
- Tables: a `u32` record count, records, then the `FString` `SafePackage`. H5 layouts live in
  `crates/l2-dat/src/h5.rs`; `RideData` had no public layout and was derived from the file.
