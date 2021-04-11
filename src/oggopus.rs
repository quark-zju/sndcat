// https://wiki.xiph.org/OggOpus

use crate::mixer::StreamInfo;

pub struct Header {
    pub info: StreamInfo,
}

impl Header {
    /// The first page.
    pub fn serialize_head(&self) -> anyhow::Result<Vec<u8>> {
        let mut out = Vec::with_capacity(19);
        //      0                   1                   2                   3
        //  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
        // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        // |       'O'     |      'p'      |     'u'       |     's'       |
        // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        // |       'H'     |       'e'     |     'a'       |     'd'       |
        // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        // |  version = 1  | channel count |           pre-skip            |
        // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        // |                original input sample rate in Hz               |
        // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        // |    output gain Q7.8 in dB     |  channel map  |               |
        // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+               :
        // |                                                               |
        // :          optional channel mapping table...                    :
        // |                                                               |
        // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        let header = b"OpusHead";
        let version: u8 = 1;
        let channel_count: u8 = self.info.channels as u8;
        let pre_skip = 0u16;
        let sample_rate: u32 = self.info.sample_rate;
        let output_gain: u16 = 0; // Q7.8! Be aware of non-zero values.
        let channel_map: u8 = 0;
        anyhow::ensure!(self.info.channels <= 2);
        out.extend_from_slice(header);
        out.push(version);
        out.extend_from_slice(&channel_count.to_le_bytes());
        out.extend_from_slice(&pre_skip.to_le_bytes());
        out.extend_from_slice(&sample_rate.to_le_bytes());
        out.extend_from_slice(&output_gain.to_le_bytes());
        out.push(channel_map);
        assert_eq!(out.len(), 19);
        Ok(out)
    }

    /// The second page.
    pub fn serialize_tags(&self) -> anyhow::Result<Vec<u8>> {
        let mut out = Vec::with_capacity(32);
        // - 8 byte 'OpusTags' magic signature (64 bits)
        // - The remaining data follows the vorbis-comment header design used in OggVorbis (without the "framing-bit"), OggTheora, and Speex:
        //  * Vendor string (always present).
        //  ** 4-byte little-endian length field, followed by length bytes of UTF-8 vendor string.
        //  * TAG=value metadata strings (zero or more).
        //  ** 4-byte little-endian string count.
        //  ** Count strings consisting of 4-byte little-endian length and length bytes of UTF-8 string in "tag=value" form.
        let header = b"OpusTags";
        let vendor = b"sndcat";
        let vendor_len: u32 = vendor.len() as _;
        out.extend_from_slice(header);
        out.extend_from_slice(&vendor_len.to_le_bytes());
        out.extend_from_slice(vendor);
        Ok(out)
    }
}
