//! CCSDS Packet Implementation
//! https://public.ccsds.org/Pubs/133x0b2e1.pdf

use packed_struct::prelude::{
    PackedStruct,
    Integer,
    PrimitiveEnum_u8,
    packed_bits::Bits
};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum CcsdsError {
    #[error("cannot have multiple primary headers")]
    DuplicatePrimaryHeader,

    #[error("this action would add more data than allowed")]
    ExceedsMaxDataLength,

    #[error("the header must be added before data fields")]
    MissingHeader,

    #[error("the secondary header must precede user data")]
    SecondaryHeaderAfterUserData,

    #[error("a secondary header or user data field (or both) must be present")]
    MissingSecondaryHeaderAndUserData,

    #[error("sequence count exceeds 14-bit value")]
    ExceedsSequenceCountMax,

    #[error("version exceeds 3-bit value")]
    ExceedsPrimaryVersionMax,

    #[error("apid exceeds 11-bit value")]
    ExceedsApidMax,

    #[error("buffer must equal or exceed 7 bytes (minimum ccsds packet length)")]
    InsufficientData,

    #[error("failed to unpack the header section from a byte array")]
    HeaderUnpackFailed,

    #[error("failed to unpack the data section from a byte array")]
    DataUnpackFailed,

    #[error("failed to pack the header section into a byte array")]
    HeaderPackFailed
}

/// APID is an 11-bit field
const APID_MAX: u16 = 0x07FF;

/// Idle APID is all ones, max
pub const APID_IDLE: u16 = APID_MAX;

/// Sequence Count Field Max (14-bit field), 16383
const SEQ_COUNT_MAX: u16 = 0x3FFF;

/// Packet Version Number Maximum Value (3-bit field)
const PVN_MAX: u8 = 0b111;

/// Max number of bytes
/// 3.2.2.1 of https://public.ccsds.org/Pubs/133x0b2e1.pdf
const PACKET_LEN_MAX: usize = 65542;
const PACKET_LEN_MIN: usize = 7; // Header + 1 data byte

/// Max size of header
const HEADER_LEN: usize = 6;

/// Max length of data field
const DATA_BYTE_LEN_MAX: usize = PACKET_LEN_MAX - HEADER_LEN; 

/// Packet Type (1-bit): 0 for Telemetry, 1 for Command
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum PacketType {
    Telemetry = 0,
    Command   = 1
}

/// Secondary Header Presence (1-bit): 1 if present
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum SecondaryHeaderFlag {
    Absent = 0,
    Present = 1
}

/// Type of Packet Relative to Sequence
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum SequenceFlag {
    Continued = 0b00,
    Beginning = 0b01,
    End = 0b10,
    Unsegmented = 0b11
}

#[derive(PackedStruct, Debug, Clone, PartialEq)]
#[packed_struct(bit_numbering="msb0")]
pub struct Identification {

    /// Packet Version Number (Mandatory)
    #[packed_field(bits="0..=2")]
    version: Integer<u8, Bits::<3>>,

    /// Telemetry or Command (Mandatory)
    #[packed_field(bits="3..=3", ty="enum")]
    packet_type: PacketType,

    /// Indicates the presence of a secondary header (Mandatory)
    #[packed_field(bits="4..=4", ty="enum")]
    secondary_header_flag: SecondaryHeaderFlag,

    /// Indicates the user or purpose of the packet
    /// These codes can be unique to the organization
    // Mandatory
    #[packed_field(bits="5..=15", endian="msb")]
    apid: Integer<u16, Bits::<11>>,
}

impl Identification {
    pub fn new(
        version: u8,
        packet_type: PacketType,
        secondary_header_flag: SecondaryHeaderFlag,
        apid: u16
    ) -> Result<Identification, CcsdsError> {
        if version > PVN_MAX {
            return Err(CcsdsError::ExceedsPrimaryVersionMax)
        }

        if apid > APID_MAX {
            return Err(CcsdsError::ExceedsApidMax)
        }

        Ok(Identification {
            version: version.into(),
            packet_type,
            secondary_header_flag,
            apid: apid.into()
        })
    }

    pub fn get_secondary_header_flag(&self) -> SecondaryHeaderFlag {
        self.secondary_header_flag
    }

    pub(super) fn set_secondary_header_flag(&mut self) {
        self.secondary_header_flag = SecondaryHeaderFlag::Present;
    }
}


#[derive(PackedStruct, Debug, Clone, PartialEq)]
#[packed_struct(bit_numbering="msb0")]
pub struct SequenceControl {

    #[packed_field(bits="0..=1", ty="enum")]
    flag: SequenceFlag,

    /// The packet number in the sequence
    #[packed_field(bits="2..=15", endian="msb")]
    count: Integer<u16, Bits::<14>>,
}

impl SequenceControl {
    pub fn new(
        flag: SequenceFlag,
        count: u16
    ) -> Result<SequenceControl, CcsdsError> {
        if count > SEQ_COUNT_MAX {
            return Err(CcsdsError::ExceedsSequenceCountMax)
        }

        Ok(
            SequenceControl {
                flag,
                count: count.into()
            }
        )
    }
}


/// CCSDS Primary Header
/// See 4.1.3 of https://public.ccsds.org/Pubs/133x0b2e1.pdf
#[derive(PackedStruct, Debug, Clone, PartialEq)]
#[packed_struct(bit_numbering="msb0")]
pub struct Header {
    /// Identification Section, 2 Octets
    #[packed_field(element_size_bytes="2")]
    pub identification: Identification,

    /// Sequence Control Section, 2 Octets
    #[packed_field(element_size_bytes="2")]
    pub sequence_control: SequenceControl,

    /// Length of coming data (in bytes) - 1
    #[packed_field(endian="msb")]
    data_len_bytes: u16
}

impl Header {
    pub fn new(
        version: u8,
        packet_type: PacketType,
        secondary_header_flag: SecondaryHeaderFlag,
        apid: u16,
        sequence_flag: SequenceFlag,
        sequence_count: u16,
    ) -> Result<Header, CcsdsError> {
        let id_section = Identification::new(
            version,
            packet_type,
            secondary_header_flag,
            apid
        )?;

        let seq_section = SequenceControl::new(
            sequence_flag,
            sequence_count
        )?;

        Ok(Self::from_sections(id_section, seq_section, 0))
    }

    pub fn from_sections(
        id_section: Identification,
        sequence_control_section: SequenceControl,
        data_length: u16
    ) -> Header {
        Header {
            identification: id_section,
            sequence_control: sequence_control_section,
            data_len_bytes: data_length
        }
    }

    pub(super) fn add_data_length(&mut self, n_bytes: usize) -> bool {
        let total: usize = (self.data_len_bytes as usize) + n_bytes;
        if total > (u16::MAX as usize) {
            return false;
        }

        if total > DATA_BYTE_LEN_MAX {
            return false;
        }

        self.data_len_bytes = total as u16;
        true
    }

    pub fn data_len_bytes(&self) -> u16 {
        self.data_len_bytes
    }

    pub(super) fn clear_data(&mut self) {
        self.data_len_bytes = 0;
    }
}

#[derive(Debug)]
pub struct CcsdsPacket {
    header: Header,
    data: Vec<u8>
}

impl CcsdsPacket {
    pub fn builder() -> CcsdsBuilder {
        CcsdsBuilder::default()
    }

    pub fn header_ref(&self) -> &Header {
        &self.header
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, CcsdsError> {
        if data.len() < PACKET_LEN_MIN {
            return Err(CcsdsError::InsufficientData);
        }
        
        let Ok(t) = <&[u8; HEADER_LEN]>::try_from(&data[..HEADER_LEN]) else {
            return Err(CcsdsError::HeaderUnpackFailed);
        };

        let Ok(header) = Header::unpack(t) else {
            return Err(CcsdsError::HeaderUnpackFailed);
        };


        let packet = CcsdsPacket {
            header,
            data: data[HEADER_LEN..].to_vec()
        };

        // if header.data_len_bytes() != packet.data.len() - 1 {
        //     return Err(CcsdsError::DataLengthFieldDNEDataLength)
        // }

        Ok(packet)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, CcsdsError> {
        let Ok(packed_header) = self.header.pack() else {
            return Err(CcsdsError::HeaderPackFailed);
        };

        let mut ret = packed_header.to_vec();
        ret.append(&mut self.data.clone());

        Ok(ret)
    }
}

#[derive(Debug)]
pub struct CcsdsBuilder {
    header: Option<Header>,
    has_secondary_header: bool,
    has_user_data: bool,
    data: Vec<u8>
}

impl CcsdsBuilder {
    pub fn default() -> Self {
        CcsdsBuilder {
            header: None,
            has_secondary_header: false,
            has_user_data: false,
            data: vec![]
        }
    }

    pub fn has_secondary_header(&self) -> bool {
        self.has_secondary_header
    }

    pub fn with_header(mut self, header: &Header) -> Result<CcsdsBuilder, CcsdsError> {
        if self.header.is_some() {
            return Err(CcsdsError::DuplicatePrimaryHeader);
        }

        let mut hdr = header.clone();
        hdr.clear_data();
        self.header = Some(hdr);

        Ok(self)
    }

    pub fn with_secondary_header(mut self, data: &[u8]) -> Result<CcsdsBuilder, CcsdsError> {
        if self.has_user_data {
            return Err(CcsdsError::SecondaryHeaderAfterUserData);
        }

        let Some(header) = self.header.as_mut() else {
            return Err(CcsdsError::MissingHeader)
        };

        if header.add_data_length(data.len()) {
            self.data.extend(data.iter());
            self.has_secondary_header = true;
            header.identification.set_secondary_header_flag();
            Ok(self)
        } else {
            Err(CcsdsError::ExceedsMaxDataLength)
        }
    }

    pub fn with_user_data(mut self, data: &[u8]) -> Result<CcsdsBuilder, CcsdsError> {
        let Some(header) = self.header.as_mut() else {
            return Err(CcsdsError::MissingHeader)
        };
        
        if header.add_data_length(data.len()) {
            self.data.extend(data.iter());
            self.has_user_data = true;
            Ok(self)
        } else {
            Err(CcsdsError::ExceedsMaxDataLength)
        }
    }

    pub fn build(&mut self) -> Result<CcsdsPacket, CcsdsError> {
        let Some(header) = self.header.as_mut() else {
            return Err(CcsdsError::MissingHeader)
        };

        if !self.has_secondary_header && !self.has_user_data {
            return Err(CcsdsError::MissingSecondaryHeaderAndUserData)
        }

        // Pad with 1-zeroed out byte if no data
        if self.data.is_empty() {
            let pad = [0x00];
            if header.add_data_length(pad.len()) {
                self.data.push(0x00); // Zeroed-out byte
            } else {
                return Err(CcsdsError::ExceedsMaxDataLength)
            }
        }

        // 4.1.3.5.2 https://public.ccsds.org/Pubs/133x0b2e1.pdf
        // data length should be one octet/byte fewer than actual data length of
        //  packet data field
        header.data_len_bytes -= 1;

        Ok(CcsdsPacket {
            header: header.clone(),
            data: self.data.clone()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ut_header_valid_max() {
        Header::new(
            0b111, // version too high
            PacketType::Command,
            SecondaryHeaderFlag::Present,
            APID_MAX, // apid
            SequenceFlag::Unsegmented,
            SEQ_COUNT_MAX, // sequence count
        ).unwrap();
    }

    #[test]
    fn ut_header_valid_min() {
        Header::new(
            0b000, // version too high
            PacketType::Telemetry,
            SecondaryHeaderFlag::Absent,
            0, // apid
            SequenceFlag::Unsegmented,
            0, // sequence count
        ).unwrap();
    }

    #[test]
    fn ut_header_invalid_version() {
        let header = Header::new(
            0b111 + 1, // version too high
            PacketType::Telemetry,
            SecondaryHeaderFlag::Absent,
            0, // apid
            SequenceFlag::Unsegmented,
            0, // sequence count
        );

        assert_eq!(header.unwrap_err(), CcsdsError::ExceedsPrimaryVersionMax);
    }

    #[test]
    fn ut_header_invalid_seq_count() {
        let header = Header::new(
            0b000,
            PacketType::Telemetry,
            SecondaryHeaderFlag::Absent,
            0, // apid
            SequenceFlag::Unsegmented,
            SEQ_COUNT_MAX + 1, // sequence count too high!
        );

        assert_eq!(header.unwrap_err(), CcsdsError::ExceedsSequenceCountMax);
    }

    #[test]
    fn ut_header_invalid_apid() {
        let header = Header::new(
            0, // version
            PacketType::Telemetry,
            SecondaryHeaderFlag::Absent,
            APID_MAX + 1, // apid too high!
            SequenceFlag::Unsegmented,
            0, // sequence count
        );

        assert_eq!(header.unwrap_err(), CcsdsError::ExceedsApidMax);
    }

    #[test]
    /// secondary_header_flag should set to "Present"
    ///  if secondary_header is added through builder
    fn ut_builder_auto_toggle_secondary_header_flag() -> Result<(), CcsdsError> {
        let header = Header::new(
            0b1, // version
            PacketType::Telemetry,
            SecondaryHeaderFlag::Absent, // Set to ABSENT!
            0, // apid
            SequenceFlag::Unsegmented,
            0, // sequence count
        )?;

        let second_header: [u8; 3] = [0x10, 0x20, 0x30];

        // Add a secondary header despite absence
        let packet = CcsdsPacket::builder()
            .with_header(&header)?
            .with_secondary_header(&second_header)?
            .build()?;
        
        assert_eq!(
            packet.header_ref().identification.get_secondary_header_flag(),
            SecondaryHeaderFlag::Present
        );

        Ok(())
    }

    #[test]
    /// The CCSDS Packet MUST have either
    ///  a secondary header or a user data field, or both.
    fn ut_builder_2hdr_or_user_data() -> Result<(), CcsdsError> {
        let header = Header::new(
            0b1, // version
            PacketType::Telemetry,
            SecondaryHeaderFlag::Absent,
            0, // apid
            SequenceFlag::Unsegmented,
            0, // sequence count
        )?;

        let packet = CcsdsPacket::builder()
            .with_header(&header)?
            .build();
        
        assert_eq!(packet.unwrap_err(), CcsdsError::MissingSecondaryHeaderAndUserData);

        Ok(())
    }

    #[test]
    fn ut_builder_zero_data_length() -> Result<(), CcsdsError> {
        let header = Header::new(
            0b1, // version
            PacketType::Telemetry,
            SecondaryHeaderFlag::Present,
            0, // apid
            SequenceFlag::Unsegmented,
            0, // sequence count
        )?;

        let data = vec![0; 10];

        // Give arbitrary number of data bytes
        let packet = CcsdsPacket::builder()
            .with_header(&header)?
            .with_secondary_header(&data)? // succeeds
            .build()?;

        // data_len - 1: 4.1.3.5.2 https://public.ccsds.org/Pubs/133x0b2e1.pdf
        assert_eq!(packet.header_ref().data_len_bytes(), data.len() as u16 - 1);

        // Try again with different number of bytes
        let arb = vec![0; data.len() - 1];
        let packet = CcsdsPacket::builder()
            .with_header(&header)?
            .with_secondary_header(&arb)? // arbitrary bytes instead
            .build()?;
        
        // The data length of the CcsdsPacket header should be less than before
        // data_len - 1: 4.1.3.5.2 https://public.ccsds.org/Pubs/133x0b2e1.pdf
        assert_eq!(packet.header_ref().data_len_bytes(), arb.len() as u16 - 1);

        Ok(())
    }

    #[test]
    fn ut_builder_pad_data() -> Result<(), CcsdsError> {
        let header = Header::new(
            0b1, // version
            PacketType::Telemetry,
            SecondaryHeaderFlag::Present,
            0, // apid
            SequenceFlag::Unsegmented,
            0, // sequence count
        )?;

        // Give no data
        let mut bytes = CcsdsPacket::builder()
            .with_header(&header)?
            .with_secondary_header(&[])?
            .build()?
            .to_bytes()?;

        // Should pad with one zeroed-out byte
        assert_eq!(bytes.len(), HEADER_LEN + 1);
        assert_eq!(bytes.pop().unwrap(), 0x0);

        Ok(())
    }

    #[test]
    fn ut_builder_secondary_header_after_user_data() -> Result<(), CcsdsError> {
        let header = Header::new(
            0b1, // version
            PacketType::Telemetry,
            SecondaryHeaderFlag::Present,
            0, // apid
            SequenceFlag::Unsegmented,
            0, // sequence count
        )?;

        // Give no data
        let builder = CcsdsPacket::builder()
            .with_header(&header)?
            .with_user_data(&[])?
            .with_secondary_header(&[]);
        
        // can't add secondary header after user_data
        assert_eq!(builder.unwrap_err(), CcsdsError::SecondaryHeaderAfterUserData);
        
        Ok(())
    }

    #[test]
    fn ut_builder_max_data() -> Result<(), CcsdsError> {
        let header = Header::new(
            0b1, // version
            PacketType::Telemetry,
            SecondaryHeaderFlag::Present,
            0, // apid
            SequenceFlag::Unsegmented,
            0, // sequence count
        )?;

        let data = vec![0; DATA_BYTE_LEN_MAX];

        // Test secondary header
        CcsdsPacket::builder()
                .with_header(&header)?
                .with_secondary_header(&data[..data.len() - 1])? // succeeds
                .build()
                .unwrap();
        
        let builder = CcsdsPacket::builder()
                .with_header(&header)?
                .with_secondary_header(&data[..data.len()]);
        assert_eq!(builder.unwrap_err(), CcsdsError::ExceedsMaxDataLength);

        // Test User Data
        CcsdsPacket::builder()
                .with_header(&header)?
                .with_user_data(&data[..data.len() - 1])? // succeeds
                .build()
                .unwrap();

        let builder = CcsdsPacket::builder()
                .with_header(&header)?
                .with_user_data(&data[..data.len()]);
        assert_eq!(builder.unwrap_err(), CcsdsError::ExceedsMaxDataLength);

        Ok(())
    }

    #[test]
    fn ut_builder_too_much_data() -> Result<(), CcsdsError> {
        let header = Header::new(
            0b1, // version
            PacketType::Telemetry,
            SecondaryHeaderFlag::Present,
            0, // apid
            SequenceFlag::Unsegmented,
            0, // sequence count
        )?;

        let data = vec![0; DATA_BYTE_LEN_MAX - 1];

        let builder = CcsdsPacket::builder()
                .with_header(&header)?
                .with_secondary_header(&data)? // succeeds
                .with_user_data(&data[..=1]); // add one more than max, fails        
        assert_eq!(builder.unwrap_err(), CcsdsError::ExceedsMaxDataLength);

        Ok(())
    }

    #[test]
    fn ut_builder_duplicate_header() -> Result<(), CcsdsError> {
        let header = Header::new(
            0b1, // version
            PacketType::Telemetry,
            SecondaryHeaderFlag::Present,
            0, // apid
            SequenceFlag::Unsegmented,
            0, // sequence count
        )?;

        let builder = CcsdsPacket::builder()
                .with_header(&header)?
                .with_header(&header);      
        assert_eq!(builder.unwrap_err(), CcsdsError::DuplicatePrimaryHeader);

        Ok(())
    }

    #[test]
    fn ut_builder_to_from_bytes() -> Result<(), CcsdsError> {
        let mut header = Header::new(
            0b1, // version
            PacketType::Telemetry,
            SecondaryHeaderFlag::Present,
            0, // apid
            SequenceFlag::Unsegmented,
            0, // sequence count
        )?;

        let data: Vec<u8> = 0xDEADBEEF_u32.to_be_bytes().to_vec();

        // To Bytes
        let bytes: Vec<u8> = CcsdsPacket::builder()
            .with_header(&header)?
            .with_secondary_header(&data)? // succeeds
            .build()?
            .to_bytes()?;

        assert_eq!(bytes.len(), HEADER_LEN + data.len());

        // From Bytes
        let packet = CcsdsPacket::from_bytes(&bytes)?;

        // header should update to add data.len() - 1 bytes to data_length field
        // data_len value should be one less than actual number of bytes in packet
        //  data field
        // 4.1.3.5.2 https://public.ccsds.org/Pubs/133x0b2e1.pdf
        header.data_len_bytes = data.len() as u16 - 1;
        assert_eq!(packet.header, header);
        assert_eq!(packet.data, data);

        Ok(())
    }
}
