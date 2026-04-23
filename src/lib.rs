//! Read structures is a library for working with strings that describe how the bases in a sequencing run
//! should be allocated into logical reads.
//!
//! Each read structure is made up of one or more read segments which are in turn a segment type.
//!
//! For more details see [here](https://github.com/fulcrumgenomics/fgbio/wiki/Read-Structures)
//!
//! # Example
//!
//! Parsing a complex read structure.
//!
//! ```rust
//! use std::str::FromStr;
//! use read_structure::ReadStructure;
//!
//! let rs = ReadStructure::from_str("76T8B8B76T").unwrap();
//! let templates: String = rs.templates().map(|s| s.to_string()).collect();
//! assert_eq!(templates, "76T76T");
//! ```
//!
//! Extracting segments from an actual read based on the read structure:
//!
//! ```rust
//! use std::str::FromStr;
//! use read_structure::{ReadStructure, SegmentType, SkipHandling};
//!
//! let bases = b"\
//!     AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAGGGGGGGGCCCCCCCCTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT";
//! let quals = &[b'I'; 168][..];
//! let rs = ReadStructure::from_str("76T8B8B76T").unwrap();
//!
//! let templates: Vec<&[u8]> = rs.extract(bases, quals, SkipHandling::Exclude)
//!     .unwrap()
//!     .filter(|(seg, _, _)| seg.kind == SegmentType::Template)
//!     .map(|(_, bases, _)| bases)
//!     .collect();
//!
//! assert_eq!(templates, vec![
//!     &b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"[..],
//!     &b"TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT"[..],
//! ]);
//! ```

#![allow(unused, clippy::must_use_candidate)]
#![allow(dead_code)]

mod read_segment;
mod read_structure;
mod segment_type;

pub use crate::read_structure::*;
pub use read_segment::*;
pub use segment_type::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReadStructureError {
    #[error("Example")]
    Example,

    #[error("Invalid read structure: {0}")]
    InvalidReadStructure(String),

    #[error("Mismatching bases and quals lengths: {bases_len}, {quals_len}")]
    MismatchingBasesAndQualsLen { bases_len: usize, quals_len: usize },

    #[error("Read structure missing length information: {}[{}]{}", .0.prefix, .0.error, .0.suffix)]
    ReadStructureMissingLengthInformation(ErrorMessageParts),

    #[error("Read structure missing operator: {}[{}]{}", .0.prefix, .0.error, .0.suffix)]
    ReadStructureMissingOperator(ErrorMessageParts),

    #[error("Read structure had unknown type: {}[{}]{}", .0.prefix, .0.error, .0.suffix)]
    ReadStructureHadUnknownType(ErrorMessageParts),

    #[error("Read structure contains zero elements")]
    ReadStructureContainsZeroElements,

    #[error("Read structure contains more than one indefinite-length (`+`) segment: {0}")]
    ReadStructureMultipleIndefiniteLengthSegments(ReadSegment),

    /// The read is too short to accommodate every fixed-length segment in the read
    /// structure. `required` is the sum of all fixed segment lengths, plus 1 if the
    /// structure also has an indefinite (`+`) segment (which must be at least one base).
    #[error("Read of length {read_len} is shorter than required minimum {required}")]
    ReadTooShort { read_len: usize, required: usize },

    /// A fixed-length read structure was handed a read of the wrong length. Fixed
    /// structures require exact-length reads; anything longer is almost always a
    /// caller bug (wrong structure for this data, or a stray adapter still attached).
    #[error("Read of length {read_len} does not match fixed structure length {expected}")]
    ReadTooLong { read_len: usize, expected: usize },

    #[error("ReadSegment too short: {0}")]
    ReadSegmentTooShort(String),

    #[error("ReadSegment str contained more than one segment: {0}")]
    ReadSegmentMultipleSegments(String),

    #[error("ReadSegment must have length > 0 or `+`: {}[{}]{}", .0.prefix, .0.error, .0.suffix)]
    ReadSegmentLengthZero(ErrorMessageParts),

    #[error("Invalid SegmentType: {0}")]
    ReadSegmentTypeInvalid(char),

    #[error("Invalid SegmentType: {0}")]
    ReadSegmentTypeStringInvalid(String),
}

/// Helper struct for isolating the erroneous portion of a string.
#[derive(Debug)]
pub struct ErrorMessageParts {
    prefix: String,
    error: String,
    suffix: String,
}

impl ErrorMessageParts {
    fn new(chars: &[char], start: usize, end: usize) -> Self {
        let prefix: String = chars.iter().take(start).collect();
        let error: String = chars.iter().skip(start).take(end - start).collect();
        let suffix: String = if end == chars.len() {
            "".to_string()
        } else {
            chars.iter().skip(end).take(chars.len() - end).collect()
        };
        Self { prefix, error, suffix }
    }
}
