//! The structure for the file should:
//! 1. Have a limited byte size for the uncompressed data
//! 2. (Can be implemented last) Compress the file in a streaming fashion before writing it to disk
//! 3. If the max amount of lines is reached wrap around to avoid shifting every line to insert a
//! last one maintaining a ring-buffer-like structure.
// FIXME: File does not have clear seperators for lines, which will be a problem
// when reading the file.
use serde::{Deserialize, Serialize};
use std::cmp::max;
use std::fs::OpenOptions;
use std::io::{BufRead, Read, Seek, Write};
use std::path::PathBuf;
use zstd::{Decoder, Encoder};

/// The maximum amount of bytes to be saved in the file. For now 2GB
const MAX_BYTES: usize = 1024 * 1024 * 1024 * 2;

struct Index {
    index: usize,
    max_index: usize,
}

impl Index {
    /// Increment the index and update the max index if the index exceeds it.
    ///
    /// Returns true if the index wraps around to 0.
    fn increment(&mut self, exceeds_max_bytes: bool) -> bool {
        if exceeds_max_bytes && self.index == self.max_index {
            self.index = 0;
        } else {
            self.index += 1;
        }
        self.max_index = max(self.index, self.max_index);
        exceeds_max_bytes && self.index == 0
    }
}

struct CompressedWriter {
    pub file_handle: std::fs::File,
    /// The index used to keep track of the current line as well as the max amount of lines.
    /// Essentially when the max amount of bytes is reached the index will wrap around to 0
    /// and start overwriting the lines from the beginning.
    /// This is to avoid shifting every line to insert a new one.
    pub index: Index,
    pub current_bytes: u64,
    pub max_bytes: u64,
}

impl CompressedWriter {
    pub fn new(file_path: PathBuf) -> Self {
        CompressedWriter {
            file_handle: OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(file_path)
                .unwrap(),
            index: Index {
                index: 0,
                max_index: 0,
            },
            current_bytes: 0,
            max_bytes: MAX_BYTES as u64,
        }
    }
    // Problems: No seperator inbetween lines, recalculating lines would have to be based on these seperators
    fn push_line(&mut self, data: impl Serialize) {
        let serialized = postcard::to_allocvec(&data).unwrap();

        if self.current_bytes + (serialized.len() as u64) < self.max_bytes {
            self.current_bytes += serialized.len() as u64;
            // Write the data to the file
            // FIXME: Remove unwrap
            self.index.increment(false);
        } else {
            // Try to replace a line
            if self.index.increment(true) {
                // If the index wraps around to 0, seek to the beginning of the file
                self.file_handle.seek(std::io::SeekFrom::Start(0)).unwrap();
                // TODO: How do we recalculate the bytes here.
            }
            // Read the current line to find out how many bytes we replace.
            // FIXME: We need multiple files in order to achieve this, such that one file can be
            // replaced by another
        }
        self.file_handle.write_all(&serialized).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Seek, Write};
    use tempfile::tempfile;

    #[test]
    fn test_write_to_tempfile() {
        let mut file = tempfile().unwrap();
        let content = include_str!("test.str");
        let compressed = zstd::stream::encode_all(content.as_bytes(), 3).unwrap();
        println!(
            "Uncompressed size: {} Compressed size: {}",
            content.len(),
            compressed.len()
        );
        file.write_all(&compressed).unwrap();
        let mut file_content = Vec::new();
        file.rewind().unwrap();
        zstd::Encoder::file.read_to_end(&mut file_content).unwrap();

        let decompressed = zstd::stream::decode_all(&*file_content).unwrap();
        assert_eq!(decompressed, content.as_bytes())
    }
}
