# read-structure

<p align="center">
  <a href="https://github.com/fulcrumgenomics/read-structure/actions?query=workflow%3ACheck"><img src="https://github.com/fulcrumgenomics/read-structure/actions/workflows/build_and_test.yml/badge.svg" alt="Build Status"></a>
  <img src="https://img.shields.io/crates/l/read_structure.svg" alt="license">
  <a href="https://crates.io/crates/read-structure"><img src="https://img.shields.io/crates/v/read-structure.svg?colorB=319e8c" alt="Version info"></a><br>
</p>

Read structures is a library for working with strings that describe how the bases in a sequencing run should be allocated into logical reads.

<p>
<a href float="left"="https://fulcrumgenomics.com"><img src=".github/logos/fulcrumgenomics.svg" alt="Fulcrum Genomics" height="100"/></a>
</p>

[Visit us at Fulcrum Genomics](www.fulcrumgenomics.com) to learn more about how we can power your Bioinformatics with read-structure and beyond.

<a href="mailto:contact@fulcrumgenomics.com?subject=[GitHub inquiry]"><img src="https://img.shields.io/badge/Email_us-brightgreen.svg?&style=for-the-badge&logo=gmail&logoColor=white"/></a>
<a href="https://www.fulcrumgenomics.com"><img src="https://img.shields.io/badge/Visit_Us-blue.svg?&style=for-the-badge&logo=wordpress&logoColor=white"/></a>


Each read structure is made up of one or more read segments which are in turn a segment type.

For more details see [here](https://github.com/fulcrumgenomics/fgbio/wiki/Read-Structures)

## Documentation and Examples

Please see the generated [Rust Docs](https://docs.rs/read_structure).

## How to use in your project

Add the following to your `Cargo.toml` dependencies section, updating the version number as needed.

```toml
[dependencies]
read-structure = "*"
```

## How to build and test locally

Assuming you have cloned the repo and are in the top level:

```bash
cargo test
```

## How to publish

This assumes that you have installed `cargo-release` via `cargo install cargo-release` and have set up credentials with `crates.io`.

```bash
cargo release <patch|minor|major>
```
