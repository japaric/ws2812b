#![no_std]

extern crate byteorder;

use byteorder::{ByteOrder, LittleEndian};

/// LED driver state
#[derive(Clone, Copy, Debug)]
pub struct State {
    pub context_switches: u16,
    pub frames: u8,
    pub sleep_cycles: u32,
    pub snapshot: u32,
}

/// Byte used for frame synchronization
pub const HEAD: u8 = 0xAA;
pub const TAIL: u8 = 0x55;

impl State {
    /// Binary deserializes a `buffer` into `State`
    ///
    /// Note that the input buffer doesn't include the `SYNC_BYTE`
    pub fn deserialize(buffer: &[u8; 11]) -> Self {
        State {
            snapshot: LittleEndian::read_u32(&buffer[..4]),
            sleep_cycles: LittleEndian::read_u32(&buffer[4..8]),
            context_switches: LittleEndian::read_u16(&buffer[8..10]),
            frames: buffer[10],
        }
    }

    /// Binary serializes `State` into a `buffer`
    pub fn serialize(&self, buffer: &mut [u8; 13]) {
        buffer[0] = HEAD;
        LittleEndian::write_u32(&mut buffer[1..5], self.snapshot);
        LittleEndian::write_u32(&mut buffer[5..9], self.sleep_cycles);
        LittleEndian::write_u16(&mut buffer[9..11], self.context_switches);
        buffer[11] = self.frames;
        buffer[12] = TAIL;
    }
}
