//! Read Segments
//!
//! Type [`ReadSegment`] encapsulates all the information about a segment within
//! a [`crate::read_structure::ReadStructure`]. A segment can either have a definite length, in which case
//! length must be `Some(usize)`, or an indefinite length (can be any length, 1 or more)
//! in which case length must be `None`.

use std::{convert::TryFrom, io::Read};

use crate::{segment_type::SegmentType, ReadStructure, ReadStructureError};

/// A character that can be put in place of a number in a read structure to mean "1 or more bases".
pub const ANY_LENGTH_BYTE: u8 = b'+';

/// Defined for efficiency, same as [`ANY_LENGTH_BYTE`].
pub const ANY_LENGTH_BYTE_SLICE: &[u8] = b"+";

/// A string that can be put in place of a number in a read structure to mean "1 or more bases".
pub const ANY_LENGTH_STR: &str = "+";

/// The read segment describing a given kind ([`SegmentType`]), optional length, and offset of the
/// bases within a [`crate::read_structure::ReadStructure`].
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ReadSegment {
    /// The offset in the read if the segment belongs to a read structure
    pub(crate) offset: usize,
    /// The optional length of this segment
    pub length: Option<usize>,
    /// The segment type
    pub kind: SegmentType,
}

impl ReadSegment {
    /// Extract the bases corresponding to this [`ReadSegment`] from a slice.
    ///
    /// # Errors
    ///
    /// - If the segment does not fall wholely within the slice.
    pub fn extract_bases<'a, B>(&self, bases: &'a [B]) -> Result<&'a [B], ReadStructureError> {
        let end = self.calculate_end(bases)?;
        Ok(&bases[self.offset..end])
    }

    /// Extract the bases and corresponding quals to this [`ReadSegment`] from a slice.
    ///
    /// # Errors
    ///
    /// - If the segment does not fall wholely within the slice.
    /// - If the bases and quals lengths are not equal.
    pub fn extract_bases_and_quals<'a, B, Q>(
        &self,
        bases: &'a [B],
        quals: &'a [Q],
    ) -> Result<(&'a [B], &'a [Q]), ReadStructureError> {
        if bases.len() != quals.len() {
            return Err(ReadStructureError::MismatchingBasesAndQualsLen {
                bases_len: bases.len(),
                quals_len: quals.len(),
            });
        }
        let end = self.calculate_end(bases)?;
        Ok((&bases[self.offset..end], &quals[self.offset..end]))
    }

    /// Returns the length of the read segment.
    pub fn length(&self) -> Option<usize> {
        self.length
    }

    /// Returns true if the read segment has a length defined (i.e. not `None`)
    pub fn has_length(&self) -> bool {
        self.length.is_some()
    }

    /// Returns the end position for the segment for the given read.
    ///
    /// # Errors
    ///
    /// Errors if the read ends before the segment starts.
    #[inline]
    fn calculate_end<T>(&self, bases: &[T]) -> Result<usize, ReadStructureError> {
        if bases.len() < self.offset {
            return Err(ReadStructureError::ReadEndsBeforeSegment(*self));
        }
        if let Some(l) = self.length {
            if bases.len() < self.offset + l {
                return Err(ReadStructureError::ReadEndsAfterSegment(*self));
            }
            Ok(self.offset + l)
        } else {
            Ok(bases.len())
        }
    }

    /// Clone the read segment but with an updated end. If the new end is before
    /// the current offset, the read segment will have no length defined.
    /// Otherwise, the new length will be reduced based on the offset (`end - offset`).
    fn clone_with_new_end(&self, end: usize) -> Self {
        let option_new_length = if self.offset >= end { None } else { Some(end - self.offset) };
        if option_new_length == self.length {
            *self
        } else {
            Self { offset: self.offset, length: option_new_length, kind: self.kind }
        }
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
    use crate::read_segment::ReadSegment;
    use crate::read_segment::{ANY_LENGTH_BYTE, ANY_LENGTH_STR};
    use crate::segment_type::SegmentType;
    use bstr::B;
    use std::convert::TryFrom;
    use std::str::FromStr;
    use strum::IntoEnumIterator;

    #[test]
    fn test_read_segment_length() {
        let seg_fixed_length =
            ReadSegment { offset: 0, length: Some(10), kind: SegmentType::Template };
        assert_eq!(seg_fixed_length.length().unwrap(), 10);
        assert!(seg_fixed_length.has_length());
        assert_eq!(seg_fixed_length.length().unwrap(), 10);
        let seg_no_length = ReadSegment { offset: 0, length: None, kind: SegmentType::Template };
        assert!(!seg_no_length.has_length());
    }

    #[test]
    #[should_panic]
    fn test_read_segment_fixed_length_panic() {
        let seg_no_length = ReadSegment { offset: 0, length: None, kind: SegmentType::Template };
        seg_no_length.length().unwrap();
    }

    #[test]
    fn test_read_segment_to_string() {
        for tpe in SegmentType::iter() {
            let seg_fixed_length = ReadSegment { offset: 0, length: Some(10), kind: tpe };
            assert_eq!(seg_fixed_length.to_string(), format!("10{}", tpe.value()));
            let seg_no_length = ReadSegment { offset: 0, length: None, kind: tpe };
            assert_eq!(seg_no_length.to_string(), format!("{}{}", ANY_LENGTH_STR, tpe.value()));
        }
    }

    #[test]
    fn test_read_segment_clone_with_new_end() {
        let seg_fixed_length =
            ReadSegment { offset: 2, length: Some(10), kind: SegmentType::Template };
        assert_eq!(seg_fixed_length.clone_with_new_end(10).length().unwrap(), 8);
        assert_eq!(seg_fixed_length.clone_with_new_end(8).length().unwrap(), 6);
        assert_eq!(seg_fixed_length.clone_with_new_end(2).length(), None);
        assert_eq!(seg_fixed_length.clone_with_new_end(1).length(), None);
        let seg_no_length = ReadSegment { offset: 2, length: None, kind: SegmentType::Template };
        assert_eq!(seg_no_length.clone_with_new_end(10).length().unwrap(), 8);
        assert_eq!(seg_no_length.clone_with_new_end(8).length().unwrap(), 6);
        assert_eq!(seg_no_length.clone_with_new_end(2).length(), None);
        assert_eq!(seg_no_length.clone_with_new_end(1).length(), None);
    }

    #[test]
    fn test_extract_bases() {
        let seg = ReadSegment { offset: 2, length: Some(3), kind: SegmentType::MolecularBarcode };
        assert_eq!(seg.extract_bases(B("GATTACA")).unwrap(), b"TTA");
    }

    #[test]
    fn test_extract_bases_and_quals() {
        let seg = ReadSegment { offset: 2, length: Some(3), kind: SegmentType::MolecularBarcode };
        let sub = seg.extract_bases_and_quals(B("GATTACA"), B("1234567")).unwrap();
        assert_eq!(sub.0, B("TTA"));
        assert_eq!(sub.1, B("345"));
    }

    #[test]
    fn test_read_segment_from_str() {
        assert_eq!(
            ReadSegment::from_str("+T").unwrap(),
            ReadSegment { offset: 0, length: None, kind: SegmentType::Template }
        );
        assert_eq!(
            ReadSegment::from_str("10S").unwrap(),
            ReadSegment { offset: 0, length: Some(10), kind: SegmentType::Skip }
        );
    }
}
