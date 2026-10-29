//! HDF5 Filter Pipeline message (type 0x000B).
//!
//! Describes the sequence of filters applied to chunked data (e.g. deflate,
//! shuffle, fletcher32). Each filter has an ID, optional name, and optional
//! client data parameters.

use crate::error::{Error, Result};
use crate::io::Cursor;
pub use hdf5_core::{
    FilterDescription, FilterPipelineMessage, FILTER_DEFLATE, FILTER_FLETCHER32, FILTER_NBIT,
    FILTER_SCALEOFFSET, FILTER_SHUFFLE, FILTER_SZIP,
};

/// Parse a filter pipeline message.
pub fn parse(
    cursor: &mut Cursor<'_>,
    _offset_size: u8,
    _length_size: u8,
    msg_size: usize,
) -> Result<FilterPipelineMessage> {
    let start = cursor.position();
    let version = cursor.read_u8()?;

    match version {
        1 => parse_v1(cursor, start, msg_size),
        2 => parse_v2(cursor, start, msg_size),
        v => Err(Error::UnsupportedFilterPipelineVersion(v)),
    }
}

fn parse_v1(cursor: &mut Cursor<'_>, start: u64, msg_size: usize) -> Result<FilterPipelineMessage> {
    let n_filters = cursor.read_u8()? as usize;
    let _reserved = cursor.read_bytes(6)?;

    let mut filters = Vec::with_capacity(n_filters);
    for _ in 0..n_filters {
        let id = cursor.read_u16_le()?;
        let name_len = cursor.read_u16_le()? as usize;
        let _flags = cursor.read_u16_le()?;
        let n_client_data = cursor.read_u16_le()? as usize;

        let name = if name_len > 0 {
            let s = cursor.read_fixed_string(name_len)?;
            // Pad to 8-byte boundary
            let padded = (name_len + 7) & !7;
            if padded > name_len {
                cursor.skip(padded - name_len)?;
            }
            Some(s)
        } else {
            None
        };

        let mut client_data = Vec::with_capacity(n_client_data);
        for _ in 0..n_client_data {
            client_data.push(cursor.read_u32_le()?);
        }
        // Pad client data to even count (v1 requires padding to 8 bytes)
        if n_client_data % 2 != 0 {
            cursor.skip(4)?;
        }

        filters.push(FilterDescription {
            id,
            name,
            client_data,
        });
    }

    let consumed = (cursor.position() - start) as usize;
    if consumed < msg_size {
        cursor.skip(msg_size - consumed)?;
    }

    Ok(FilterPipelineMessage { filters })
}

fn parse_v2(cursor: &mut Cursor<'_>, start: u64, msg_size: usize) -> Result<FilterPipelineMessage> {
    let n_filters = cursor.read_u8()? as usize;

    let mut filters = Vec::with_capacity(n_filters);
    for _ in 0..n_filters {
        let id = cursor.read_u16_le()?;

        // In v2, user-defined filters (id >= 256) carry a name length field
        // before flags. The actual name bytes come after n_client_data.
        let name_len = if id >= 256 {
            cursor.read_u16_le()? as usize
        } else {
            0
        };

        let _flags = cursor.read_u16_le()?;
        let n_client_data = cursor.read_u16_le()? as usize;

        // Name bytes (only for user-defined filters)
        let name = if name_len > 0 {
            Some(cursor.read_fixed_string(name_len)?)
        } else {
            None
        };

        let mut client_data = Vec::with_capacity(n_client_data);
        for _ in 0..n_client_data {
            client_data.push(cursor.read_u32_le()?);
        }

        filters.push(FilterDescription {
            id,
            name,
            client_data,
        });
    }

    let consumed = (cursor.position() - start) as usize;
    if consumed < msg_size {
        cursor.skip(msg_size - consumed)?;
    }

    Ok(FilterPipelineMessage { filters })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_v2_deflate() {
        let mut data = vec![
            0x02, // version 2
            0x01, // 1 filter
        ];
        // Filter: deflate (id=1)
        data.extend_from_slice(&1u16.to_le_bytes()); // id
                                                     // Well-known (id < 256), so no name_len field
        data.extend_from_slice(&0u16.to_le_bytes()); // flags
        data.extend_from_slice(&1u16.to_le_bytes()); // 1 client data value
        data.extend_from_slice(&6u32.to_le_bytes()); // compression level = 6

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, 8, 8, data.len()).unwrap();
        assert_eq!(msg.filters.len(), 1);
        assert_eq!(msg.filters[0].id, FILTER_DEFLATE);
        assert!(msg.filters[0].name.is_none());
        assert_eq!(msg.filters[0].client_data, vec![6]);
    }
}
