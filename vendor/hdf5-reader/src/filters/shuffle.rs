/// Unshuffle bytes — reverses the HDF5 shuffle filter.
///
/// The shuffle filter rearranges bytes so that byte 0 of all elements comes first,
/// then byte 1 of all elements, etc. This improves compression of numeric arrays
/// where adjacent elements often have similar high bytes.
///
/// On read, we need to unshuffle: given the shuffled bytes, reconstruct the
/// original element order.
pub fn unshuffle(data: &[u8], element_size: usize) -> Vec<u8> {
    if element_size <= 1 || data.is_empty() {
        return data.to_vec();
    }

    let n_elements = data.len() / element_size;
    if n_elements == 0 {
        return data.to_vec();
    }

    let mut output = vec![0u8; data.len()];

    for byte_idx in 0..element_size {
        let src_start = byte_idx * n_elements;
        for elem in 0..n_elements {
            if src_start + elem < data.len() && elem * element_size + byte_idx < output.len() {
                output[elem * element_size + byte_idx] = data[src_start + elem];
            }
        }
    }

    // Copy any trailing bytes that don't form a complete element
    let complete = n_elements * element_size;
    if complete < data.len() {
        output[complete..].copy_from_slice(&data[complete..]);
    }

    output
}

/// Shuffle bytes — for testing purposes.
#[cfg(test)]
fn shuffle(data: &[u8], element_size: usize) -> Vec<u8> {
    if element_size <= 1 || data.is_empty() {
        return data.to_vec();
    }

    let n_elements = data.len() / element_size;
    let mut output = vec![0u8; data.len()];

    for byte_idx in 0..element_size {
        let dst_start = byte_idx * n_elements;
        for elem in 0..n_elements {
            output[dst_start + elem] = data[elem * element_size + byte_idx];
        }
    }

    let complete = n_elements * element_size;
    if complete < data.len() {
        output[complete..].copy_from_slice(&data[complete..]);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_4byte() {
        let original: Vec<u8> = vec![
            0x01, 0x02, 0x03, 0x04, // element 0
            0x05, 0x06, 0x07, 0x08, // element 1
            0x09, 0x0A, 0x0B, 0x0C, // element 2
        ];
        let shuffled = shuffle(&original, 4);
        // After shuffle: [0x01,0x05,0x09, 0x02,0x06,0x0A, 0x03,0x07,0x0B, 0x04,0x08,0x0C]
        assert_eq!(
            shuffled,
            vec![0x01, 0x05, 0x09, 0x02, 0x06, 0x0A, 0x03, 0x07, 0x0B, 0x04, 0x08, 0x0C]
        );

        let unshuffled = unshuffle(&shuffled, 4);
        assert_eq!(unshuffled, original);
    }

    #[test]
    fn single_element() {
        let data = vec![0x01, 0x02, 0x03, 0x04];
        let result = unshuffle(&data, 4);
        assert_eq!(result, data);
    }

    #[test]
    fn element_size_1() {
        let data = vec![0x01, 0x02, 0x03];
        let result = unshuffle(&data, 1);
        assert_eq!(result, data);
    }
}
