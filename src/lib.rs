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
//! use std::convert::TryFrom;
//! use std::str::FromStr;
//! use read_structure::{
//!     ReadStructure,
//!     SegmentType,
//! };
//! let read_sequence = b"\
//!     AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAGGGGGGGGCCCCCCCCTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT"
//! .to_vec();
//! let kind_of_interest = SegmentType::Template;
//! let rs = ReadStructure::from_str("76T8B8B76T").unwrap();
//!
//! let mut sections = vec![];
//! for segment in rs.segments_by_type(kind_of_interest) {
//!     sections.push(segment.extract_bases(read_sequence.as_slice()).unwrap())
//! }
//! assert_eq!(sections, vec![
//!     b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
//!     b"TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT"
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

    #[error("Read structure contains a non-terminal segment that has an indefinite length: {0}")]
    ReadStructureNonTerminalIndefiniteLengthReadSegment(ReadSegment),

    #[error("Read ends before start of segment: {0}")]
    ReadEndsBeforeSegment(ReadSegment),

    #[error("Read ends before end of segment: {0}")]
    ReadEndsAfterSegment(ReadSegment),

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
