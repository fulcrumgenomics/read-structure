//! Segment Types
//!
//! Type [`SegmentType`] represents the types of segments that can show
//! up in a read structure ([`crate::read_structure::ReadStructure`]: trait.ReadStructure).

use std::{convert::TryFrom, mem};

use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::ReadStructureError;

/// The `SegmentType` type. See [the module level documentation](self) for more.
#[non_exhaustive]
#[derive(Debug, Copy, Clone, EnumIter, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum SegmentType {
    /// Template: the bases in the segment are reads of template (e.g. genomic dna, rna, etc.)
    Template = b'T',
    /// Sample Barcode: the bases in the segment are an index sequence used to identify the sample being sequenced
    SampleBarcode = b'B',
    /// Molecular Barcode: the bases in the segment are an index sequence used to identify the unique source molecule being sequence (i.e. a UMI)
    MolecularBarcode = b'M',
    /// Skip: the bases in the segment should be skipped or ignored, for example if they are monotemplate sequence generated by the library preparation
    Skip = b'S',
}

impl SegmentType {
    /// Returns the character representation of this segment type.
    pub fn value(&self) -> char {
        let value = *self as u8;
        value as char
    }
}

impl TryFrom<char> for SegmentType {
    type Error = ReadStructureError;

    /// Returns the segment type given the character representation.
    ///
    /// # Errors
    ///
    /// - If `SegmentType` not valid
    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            'T' => Ok(SegmentType::Template),
            'B' => Ok(SegmentType::SampleBarcode),
            'M' => Ok(SegmentType::MolecularBarcode),
            'S' => Ok(SegmentType::Skip),
            _ => Err(ReadStructureError::ReadSegmentTypeInvalid(value)),
        }
    }
}

impl TryFrom<u8> for SegmentType {
    type Error = ReadStructureError;

    /// Returns the segment type given the byte representation.
    ///
    /// # Errors
    ///
    /// - If `SegmentType` not valid
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::try_from(value as char)
    }
}

#[cfg(test)]
mod test {
    use std::convert::TryFrom;

    use crate::{segment_type::SegmentType, ReadStructureError};
    use strum::IntoEnumIterator;

    #[test]
    fn test_segment_type_round_trip() -> Result<(), ReadStructureError> {
        assert_eq!(SegmentType::iter().len(), 4);
        for tpe in SegmentType::iter() {
            assert_eq!(SegmentType::try_from(tpe.value())?, tpe);
        }
        Ok(())
    }

    #[test]
    fn test_invalid_segment_type() {
        assert!(SegmentType::try_from(b'G').is_err());
    }
}
