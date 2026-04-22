//! Read Segments
//!
//! Type [`ReadSegment`] encapsulates all the information about a segment within
//! a [`crate::read_structure::ReadStructure`]. A segment can either have a definite length, in which case
//! length must be `Some(usize)`, or an indefinite length (can be any length, 1 or more)
//! in which case length must be `None`.

use crate::{ReadStructure, ReadStructureError, segment_type::SegmentType};

/// A character that can be put in place of a number in a read structure to mean "1 or more bases".
pub const ANY_LENGTH_BYTE: u8 = b'+';

/// Defined for efficiency, same as [`ANY_LENGTH_BYTE`].
pub const ANY_LENGTH_BYTE_SLICE: &[u8] = b"+";

/// A string that can be put in place of a number in a read structure to mean "1 or more bases".
pub const ANY_LENGTH_STR: &str = "+";

/// A single segment of a read structure: a segment type and an optional fixed length.
///
/// Segments are typically obtained by parsing a [`ReadStructure`]; the positions of the
/// segment's bases within a read are tracked by the enclosing [`ReadStructure`], not
/// here.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ReadSegment {
    /// The optional length of this segment. `None` means "the rest of the read" — the
    /// indefinite-length (`+`) segment. At most one segment per read structure may be
    /// indefinite.
    pub length: Option<usize>,
    /// The segment type.
    pub kind: SegmentType,
}

impl ReadSegment {
    /// Returns the length of the read segment, or `None` for an indefinite-length (`+`) segment.
    pub fn length(&self) -> Option<usize> {
        self.length
    }

    /// Returns true if the read segment has a defined length (i.e. is not `+`).
    pub fn has_length(&self) -> bool {
        self.length.is_some()
    }
}

impl std::str::FromStr for ReadSegment {
    type Err = ReadStructureError;

    /// Builds a [`ReadSegment`] from a string representation.  The character representation
    /// of [`SegmentType`] must be the last character, while the leading character(s) either
    /// a non-zero integer, or the any-length character.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the string was too short, if the length could not be parsed, or if
    /// the segment type could not be recognized.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let rs = ReadStructure::from_str(s)?;
        if rs.number_of_segments() == 1 {
            // Unwrap is safe since we checked the length
            Ok(rs.first().copied().unwrap())
        } else {
            Err(ReadStructureError::ReadSegmentMultipleSegments(s.to_owned()))
        }
    }
}

impl std::fmt::Display for ReadSegment {
    /// Formats the [`ReadSegment`] as a string.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.length {
            Some(l) => write!(f, "{}", l),
            None => write!(f, "{}", ANY_LENGTH_STR),
        }?;
        write!(f, "{}", self.kind.value())
    }
}

#[cfg(test)]
mod test {
    use crate::read_segment::ANY_LENGTH_STR;
    use crate::read_segment::ReadSegment;
    use crate::segment_type::SegmentType;
    use std::str::FromStr;
    use strum::IntoEnumIterator;

    #[test]
    fn test_read_segment_length() {
        let seg_fixed_length = ReadSegment { length: Some(10), kind: SegmentType::Template };
        assert_eq!(seg_fixed_length.length().unwrap(), 10);
        assert!(seg_fixed_length.has_length());
        let seg_no_length = ReadSegment { length: None, kind: SegmentType::Template };
        assert!(!seg_no_length.has_length());
    }

    #[test]
    #[should_panic]
    fn test_read_segment_fixed_length_panic() {
        let seg_no_length = ReadSegment { length: None, kind: SegmentType::Template };
        seg_no_length.length().unwrap();
    }

    #[test]
    fn test_read_segment_to_string() {
        for tpe in SegmentType::iter() {
            let seg_fixed_length = ReadSegment { length: Some(10), kind: tpe };
            assert_eq!(seg_fixed_length.to_string(), format!("10{}", tpe.value()));
            let seg_no_length = ReadSegment { length: None, kind: tpe };
            assert_eq!(seg_no_length.to_string(), format!("{}{}", ANY_LENGTH_STR, tpe.value()));
        }
    }

    #[test]
    fn test_read_segment_from_str() {
        assert_eq!(
            ReadSegment::from_str("+T").unwrap(),
            ReadSegment { length: None, kind: SegmentType::Template }
        );
        assert_eq!(
            ReadSegment::from_str("10S").unwrap(),
            ReadSegment { length: Some(10), kind: SegmentType::Skip }
        );
    }
}
