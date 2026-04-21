//! Read Structures
//!
//! Type [`ReadStructure`] describes the structure of a given read.  A read
//! contains one or more read segments. A read segment describes a contiguous
//! stretch of bases of the same type (e.g. template bases) of some length and
//! some offset from the start of the read.

use crate::ErrorMessageParts;
use crate::ReadStructureError;
use crate::read_segment;
use crate::read_segment::ANY_LENGTH_BYTE;
use crate::read_segment::ReadSegment;
use crate::segment_type::SegmentType;
use std::convert::TryFrom;
use std::ops::Index;
use std::string;
use std::string::ToString;

/// The read structure composed of one or more [`ReadSegment`]s.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ReadStructure {
    /// The elements that make up the [`ReadStructure`].
    elements: Vec<ReadSegment>,
    /// The combined length of fixed length segments.
    length_of_fixed_segments: usize,
}

impl ReadStructure {
    /// Builds a new [`ReadStructure`] from a vector of [`ReadSegment`]s.
    ///
    /// At most one segment may have an indefinite length. By default that segment must
    /// be the last one. When the `non-terminal-plus` feature is enabled, the
    /// indefinite-length segment is also permitted in a non-terminal position; in that
    /// case the indefinite segment and every segment following it will have their
    /// `extractable` flag set to `false`, and calling [`ReadSegment::extract_bases`] or
    /// [`ReadSegment::extract_bases_and_quals`] on them returns
    /// [`ReadStructureError::SegmentNotExtractable`].
    ///
    /// # Errors
    ///
    /// - Returns `Err` if no elements exist.
    /// - Returns `Err` if more than one segment has an indefinite length.
    /// - Without the `non-terminal-plus` feature, returns `Err` if the single
    ///   indefinite-length segment is not the last segment.
    #[allow(clippy::missing_panics_doc)]
    pub fn new(mut segments: Vec<ReadSegment>) -> Result<Self, ReadStructureError> {
        if segments.is_empty() {
            return Err(ReadStructureError::ReadStructureContainsZeroElements);
        }

        let mut num_indefinite = 0;
        let mut length_of_fixed_segments = 0;
        let mut indefinite_index: Option<usize> = None;
        for (i, s) in segments.iter().enumerate() {
            if let Some(len) = s.length {
                length_of_fixed_segments += len;
            } else {
                num_indefinite += 1;
                if indefinite_index.is_none() {
                    indefinite_index = Some(i);
                }
            }
        }

        if num_indefinite > 1 {
            return Err(ReadStructureError::ReadStructureNonTerminalIndefiniteLengthReadSegment(
                *segments.iter().find(|s| !s.has_length()).unwrap(),
            ));
        }

        #[cfg(not(feature = "non-terminal-plus"))]
        if let Some(idx) = indefinite_index {
            if idx != segments.len() - 1 {
                return Err(
                    ReadStructureError::ReadStructureNonTerminalIndefiniteLengthReadSegment(
                        segments[idx],
                    ),
                );
            }
        }

        let mut off: usize = 0;
        for segment in &mut segments {
            segment.offset = off;
            off += segment.length.unwrap_or(0);
        }

        #[cfg(feature = "non-terminal-plus")]
        if let Some(idx) = indefinite_index {
            if idx != segments.len() - 1 {
                for segment in &mut segments[idx..] {
                    segment.extractable = false;
                }
            }
        }

        Ok(ReadStructure { elements: segments, length_of_fixed_segments })
    }

    /// Returns `true` if the [`ReadStructure`] has a fixed (i.e. non-variable) length,
    /// `false` if any segment has an indefinite length.
    pub fn has_fixed_length(&self) -> bool {
        self.elements.iter().all(ReadSegment::has_length)
    }

    /// Returns the fixed length if there is one.
    pub fn fixed_length(&self) -> Option<usize> {
        if self.has_fixed_length() { Some(self.length_of_fixed_segments) } else { None }
    }

    /// Returns the number of segments in this read structure.
    pub fn number_of_segments(&self) -> usize {
        self.elements.len()
    }

    /// Returns the underlying elements in this read structure.
    pub fn segments(&self) -> &[ReadSegment] {
        &self.elements
    }

    /// Returns an iterator over the read segments
    pub fn iter(&self) -> impl Iterator<Item = &ReadSegment> {
        self.elements.iter()
    }

    /// Returns the [`ReadSegment`]s in this read structure of the given kind.
    pub fn segments_by_type(&self, kind: SegmentType) -> impl Iterator<Item = &ReadSegment> {
        self.elements.iter().filter(move |seg| seg.kind == kind)
    }

    /// Returns the template [`ReadSegment`]s in this read structure
    pub fn templates(&self) -> impl Iterator<Item = &ReadSegment> {
        self.segments_by_type(SegmentType::Template)
    }

    /// Returns the sample barcode [`ReadSegment`]s in this read structure
    pub fn sample_barcodes(&self) -> impl Iterator<Item = &ReadSegment> {
        self.segments_by_type(SegmentType::SampleBarcode)
    }

    /// Returns the molecular barcode [`ReadSegment`]s in this read structure
    pub fn molecular_barcodes(&self) -> impl Iterator<Item = &ReadSegment> {
        self.segments_by_type(SegmentType::MolecularBarcode)
    }

    /// Returns the skip [`ReadSegment`]s in this read structure
    pub fn skips(&self) -> impl Iterator<Item = &ReadSegment> {
        self.segments_by_type(SegmentType::Skip)
    }

    /// Returns the cellular barcode [`ReadSegment`]s in this read structure
    pub fn cellular_barcodes(&self) -> impl Iterator<Item = &ReadSegment> {
        self.segments_by_type(SegmentType::CellularBarcode)
    }

    /// Returns the first [`ReadSegment`] in this read structure
    pub fn first(&self) -> Option<&ReadSegment> {
        self.elements.first()
    }

    /// Returns the last [`ReadSegment`] in this read structure
    pub fn last(&self) -> Option<&ReadSegment> {
        self.elements.last()
    }
}

impl IntoIterator for ReadStructure {
    type Item = ReadSegment;

    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.elements.into_iter()
    }
}

impl Index<usize> for ReadStructure {
    type Output = ReadSegment;

    /// Returns the [`ReadSegment`] at the given index in the read structure.
    fn index(&self, idx: usize) -> &Self::Output {
        &self.elements[idx]
    }
}

impl std::fmt::Display for ReadStructure {
    /// Formats this read structure as a string.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for e in &self.elements {
            write!(f, "{}", e)?;
        }
        Ok(())
    }
}

impl std::str::FromStr for ReadStructure {
    type Err = ReadStructureError;

    /// Returns a new read structure from a string, or `Err` if parsing failed.
    fn from_str(rs: &str) -> Result<Self, Self::Err> {
        let mut offset = 0;
        let mut i = 0;
        let mut segs: Vec<ReadSegment> = Vec::new();
        let chars: Vec<char> = rs.to_uppercase().chars().filter(|c| !c.is_whitespace()).collect();
        while i < chars.len() {
            // Stash the beginning position of our parsing so we can highlight what we're having trouble with
            let parse_i = i;

            // Parse out the length segment which many be 1 or more digits or the AnyLengthChar
            let length = if chars[i] as u8 == ANY_LENGTH_BYTE {
                i += 1;
                None
            } else if chars[i].is_ascii_digit() {
                let mut len: usize = 0;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    // Unwrap is safe since we've checked `is_ascii_digit` already
                    let digit = chars[i].to_digit(10).unwrap() as usize;
                    len = (len * 10) + digit;
                    i += 1;
                }
                Some(len)
            } else {
                return Err(ReadStructureError::ReadStructureMissingLengthInformation(
                    ErrorMessageParts::new(&chars, parse_i, parse_i + 1),
                ));
            };

            // Parse out the operator and make a segment
            if chars.len() == i {
                return Err(ReadStructureError::ReadStructureMissingOperator(
                    ErrorMessageParts::new(&chars, parse_i, i),
                ));
            } else if let Ok(kind) = SegmentType::try_from(chars[i]) {
                if length == Some(0) {
                    return Err(ReadStructureError::ReadSegmentLengthZero(ErrorMessageParts::new(
                        &chars, parse_i, i,
                    )));
                }
                i += 1;
                segs.push(ReadSegment {
                    offset,
                    length,
                    kind,
                    #[cfg(feature = "non-terminal-plus")]
                    extractable: true,
                });
                offset += length.unwrap_or(0);
            } else {
                return Err(ReadStructureError::ReadStructureHadUnknownType(
                    ErrorMessageParts::new(&chars, parse_i, i + 1),
                ));
            }
        }

        ReadStructure::new(segs)
    }
}

impl TryFrom<&[ReadSegment]> for ReadStructure {
    type Error = ReadStructureError;
    /// Builds a new read structure from a slice of elements.
    fn try_from(elements: &[ReadSegment]) -> Result<Self, Self::Error> {
        Self::new(elements.to_vec())
    }
}

#[cfg(test)]
mod test {
    use crate::read_structure::ReadStructure;
    use std::str::FromStr;

    #[test]
    fn test_read_structure_from_str() {
        let rss =
            ["1T", "1B", "1M", "1S", "101T", "5B101T", "123456789T", "10T10B10B10S10M", "5B2C3T"];
        for rs in &rss {
            assert_eq!(ReadStructure::from_str(rs).unwrap().to_string(), *rs);
        }
    }

    #[test]
    fn test_read_structure_from_str_with_whitespace() {
        let rss = ["75T 8B 8B 75T", " 75T 8B 8B\t75T  "];
        for rs in &rss {
            assert_eq!(ReadStructure::from_str(rs).unwrap().to_string(), "75T8B8B75T");
        }
    }

    #[test]
    fn test_read_structure_allow_anylength_char_only_once_and_for_last_segment() {
        assert_eq!(ReadStructure::from_str("5M+T").unwrap().to_string(), "5M+T");
        assert_eq!(ReadStructure::from_str("+M").unwrap().to_string(), "+M");
    }

    macro_rules! test_read_structure_from_str_err {
        ($($name:ident: $value:expr_2021,)*) => {
        $(
            #[test]
            fn $name() {
                 assert!(ReadStructure::from_str($value).is_err());
            }
        )*
        }
    }

    test_read_structure_from_str_err! {
        test_read_structure_allow_any_char_only_once_and_for_last_segment_panic_0: "++M",
        test_read_structure_allow_any_char_only_once_and_for_last_segment_panic_1: "5M++T",
        test_read_structure_allow_any_char_only_once_and_for_last_segment_panic_2: "5M70+T",
        test_read_structure_allow_any_char_only_once_and_for_last_segment_panic_3: "+M+T",
    }

    // With the `non-terminal-plus` feature enabled, this is a legal read structure.
    // Tested for acceptance in the feature-gated test module below.
    #[test]
    #[cfg(not(feature = "non-terminal-plus"))]
    fn test_read_structure_strict_rejects_non_terminal_plus() {
        assert!(ReadStructure::from_str("+M70T").is_err());
    }

    macro_rules! test_read_structure_from_str_invalid {
        ($($name:ident: $value:expr_2021,)*) => {
        $(
            #[test]
            fn $name() {
                let (input, expected) = $value;
                let actual = ReadStructure::from_str(input);
                assert!(actual.unwrap_err().to_string().ends_with(expected));
            }
        )*
        }
    }

    test_read_structure_from_str_invalid! {
        test_read_structure_from_str_invalid_0: ("9R", "[9R]"),
        test_read_structure_from_str_invalid_1: ("T", "[T]"),
        test_read_structure_from_str_invalid_2: ("23TT", "23T[T]"),
        test_read_structure_from_str_invalid_3: ("23T2", "23T[2]"),
        test_read_structure_from_str_invalid_4: ("23T2TT23T", "23T2T[T]23T"),
    }

    #[test]
    fn test_read_structure_collect_segments() {
        let rs = ReadStructure::from_str("10M9T8B7S3C10M9T8B7S2C").unwrap();
        let templates: String = rs.templates().map(|s| s.to_string()).collect();
        assert_eq!(templates, "9T9T");
        let sample_barcodes: String = rs.sample_barcodes().map(|s| s.to_string()).collect();
        assert_eq!(sample_barcodes, "8B8B");
        let molecular_barcodes: String = rs.molecular_barcodes().map(|s| s.to_string()).collect();
        assert_eq!(molecular_barcodes, "10M10M");
        let skips: String = rs.skips().map(|s| s.to_string()).collect();
        assert_eq!(skips, "7S7S");
        let cellular_barcodes: String = rs.cellular_barcodes().map(|s| s.to_string()).collect();
        assert_eq!(cellular_barcodes, "3C2C");
    }

    macro_rules! test_read_structure_length {
        ($($name:ident: $value:expr_2021,)*) => {
        $(
            #[test]
            fn $name() {
                let (input, expected) = $value;
                let actual = ReadStructure::from_str(input).unwrap().number_of_segments();
                assert_eq!(actual, expected);
            }
        )*
        }
    }

    test_read_structure_length! {
        test_read_structure_length_0: ("1T", 1),
        test_read_structure_length_1: ("1B", 1),
        test_read_structure_length_2: ("1M", 1),
        test_read_structure_length_3: ("1S", 1),
        test_read_structure_length_4: ("101T", 1),
        test_read_structure_length_5: ("5B101T", 2),
        test_read_structure_length_6: ("123456789T", 1),
        test_read_structure_length_7: ("10T10B10B10S10M", 5),
    }

    macro_rules! test_read_structure_index {
        ($($name:ident: $value:expr_2021,)*) => {
        $(
            #[test]
            fn $name() {
                let (string, index, exp_string, exp_offset) = $value;
                let read_structure = ReadStructure::from_str(string).unwrap();
                let read_segment = read_structure[index];
                assert_eq!(read_segment.to_string(), exp_string);
                assert_eq!(read_segment.offset, exp_offset);
            }
        )*
        }
    }

    test_read_structure_index! {
        test_read_structure_index_0: ("1T", 0, "1T", 0),
        test_read_structure_index_1: ("1B", 0, "1B", 0),
        test_read_structure_index_2: ("1M", 0, "1M", 0),
        test_read_structure_index_3: ("1S", 0, "1S", 0),
        test_read_structure_index_4: ("101T", 0, "101T", 0),
        test_read_structure_index_5: ("5B101T", 0, "5B", 0),
        test_read_structure_index_6: ("5B101T", 1, "101T", 5),
        test_read_structure_index_7: ("123456789T", 0, "123456789T", 0),
        test_read_structure_index_8: ("10T10B10B10S10M", 0, "10T", 0),
        test_read_structure_index_9: ("10T10B10B10S10M", 1, "10B", 10),
        test_read_structure_index_10: ("10T10B10B10S10M", 2, "10B", 20),
        test_read_structure_index_11: ("10T10B10B10S10M", 3, "10S", 30),
        test_read_structure_index_12: ("10T10B10B10S10M", 4, "10M", 40),
        test_read_structure_index_32: ("10T10B10B10S10C10M", 4, "10C", 40),
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_serde() {
        let rs = ReadStructure::from_str("10T10B10B10S10M").unwrap();
        let rs_json = serde_json::to_string(&rs).unwrap();
        let rs2 = serde_json::from_str(&rs_json).unwrap();
        assert_eq!(rs, rs2);
    }

    // ---- tests covering the `non-terminal-plus` feature ----

    #[cfg(feature = "non-terminal-plus")]
    mod non_terminal_plus {
        use crate::ReadStructureError;
        use crate::read_structure::ReadStructure;
        use std::str::FromStr;

        #[test]
        fn test_accepts_middle_plus() {
            let rs = ReadStructure::from_str("8B+M10T").unwrap();
            assert_eq!(rs.to_string(), "8B+M10T");
            assert_eq!(rs.number_of_segments(), 3);
        }

        #[test]
        fn test_accepts_leading_plus() {
            let rs = ReadStructure::from_str("+B10T").unwrap();
            assert_eq!(rs.to_string(), "+B10T");
            assert_eq!(rs.number_of_segments(), 2);
        }

        #[test]
        fn test_accepts_trailing_plus() {
            // Trailing `+` is still legal (it was before and it is now).
            let rs = ReadStructure::from_str("10T+M").unwrap();
            assert_eq!(rs.to_string(), "10T+M");
        }

        #[test]
        fn test_accepts_middle_plus_between_fixed_runs() {
            let rs = ReadStructure::from_str("10T8B+M10T").unwrap();
            assert_eq!(rs.to_string(), "10T8B+M10T");
            assert_eq!(rs.number_of_segments(), 4);
        }

        #[test]
        fn test_rejects_two_consecutive_plus() {
            assert!(ReadStructure::from_str("++M").is_err());
        }

        #[test]
        fn test_rejects_two_separate_plus() {
            assert!(ReadStructure::from_str("+M+T").is_err());
        }

        #[test]
        fn test_rejects_two_plus_in_longer_structure() {
            assert!(ReadStructure::from_str("5M+T+B").is_err());
        }

        #[test]
        fn test_pre_plus_segments_still_extract() {
            let rs = ReadStructure::from_str("8B+M10T").unwrap();
            let read = b"BBBBBBBBUUUUUUUUUUUUTTTTTTTTTT";
            assert_eq!(rs.segments()[0].extract_bases(read).unwrap(), b"BBBBBBBB");
        }

        #[test]
        fn test_middle_plus_segment_not_extractable() {
            let rs = ReadStructure::from_str("8B+M10T").unwrap();
            let read = b"BBBBBBBBUUUUUUUUUUUUTTTTTTTTTT";
            let err = rs.segments()[1].extract_bases(read).unwrap_err();
            assert!(matches!(err, ReadStructureError::SegmentNotExtractable(_)));
        }

        #[test]
        fn test_post_plus_segment_not_extractable() {
            let rs = ReadStructure::from_str("8B+M10T").unwrap();
            let read = b"BBBBBBBBUUUUUUUUUUUUTTTTTTTTTT";
            let err = rs.segments()[2].extract_bases(read).unwrap_err();
            assert!(matches!(err, ReadStructureError::SegmentNotExtractable(_)));
        }

        #[test]
        fn test_extract_bases_and_quals_errors_on_post_plus() {
            let rs = ReadStructure::from_str("8B+M10T").unwrap();
            let bases = b"BBBBBBBBUUUUUUUUUUUUTTTTTTTTTT";
            let quals = b"##########################????";
            let err = rs.segments()[2].extract_bases_and_quals(bases, quals).unwrap_err();
            assert!(matches!(err, ReadStructureError::SegmentNotExtractable(_)));
        }

        #[test]
        fn test_trailing_plus_still_extractable() {
            // Terminal `+` must remain extractable under the feature.
            let rs = ReadStructure::from_str("10T+M").unwrap();
            let read = b"AAAAAAAAAAGGGGGGGGGG";
            assert_eq!(rs.segments()[1].extract_bases(read).unwrap(), b"GGGGGGGGGG");
        }

        #[test]
        fn test_has_fixed_length_middle_plus() {
            assert!(!ReadStructure::from_str("8B+M10T").unwrap().has_fixed_length());
        }

        #[test]
        fn test_has_fixed_length_no_plus() {
            assert!(ReadStructure::from_str("10T10B").unwrap().has_fixed_length());
        }

        #[test]
        fn test_fixed_length_none_for_middle_plus() {
            assert!(ReadStructure::from_str("8B+M10T").unwrap().fixed_length().is_none());
        }
    }
}
