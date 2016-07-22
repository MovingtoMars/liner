use super::*;

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::ops::Index;
use std::ops::IndexMut;

/// Structure encapsulating command history
pub struct History {
    // TODO: this should eventually be private
    /// Vector of buffers to store history in
    pub buffers:    VecDeque<Buffer>,
    // TODO: Do we need this here? Ion can take care of this.
    //pub previous_status: i32,
    enabled:        bool,
    file_name:      String,
    max_size:       usize,
}

impl History {

    /// Create new History structure.
    pub fn new() -> History {
        History {
            buffers: VecDeque::with_capacity(1000),
            enabled: false,
            file_name: "".to_string(),
            max_size: 1000,
        }
    }

    /// Number of items in history.
    pub fn len(&self) -> usize {
        self.buffers.len()
    }

    /// Add a command to the history buffer and remove the oldest commands when the max history
    /// size has been met.
    pub fn push(&mut self, new_item: Buffer) {

        if self.enabled == true {
            self.write_to_disk(&new_item);
        }

        self.buffers.truncate(self.max_size-1); // Make room for the new item
        self.buffers.push_front(new_item)
    }

    /// Set history file name. At the same time enable history.
    pub fn set_file_name(&mut self, name: String) {
        self.file_name = name;
        self.enabled = true;
        // TODO: load history from this file
    }

    /// Set maximum number of items in history.
    pub fn set_max_size(&mut self, size: usize) {
        self.max_size = size;
    }

    /// If writing to the disk is enabled, this function will be used for logging history to the
    /// designated history file. If the history file does not exist, it will be created.
    fn write_to_disk(&self, new_item: &Buffer) {
        match OpenOptions::new().read(true).write(true).create(true).open(&self.file_name) {
            Ok(mut file) => {
                // Determine the number of commands stored and the file length.
                let (file_length, commands_stored) = {
                    let mut commands_stored = 0;
                    let mut file_length = 0;
                    let file = File::open(&self.file_name).unwrap();
                    for byte in file.bytes() {
                        if byte.unwrap_or(b' ') == b'\n' { commands_stored += 1; }
                        file_length += 1;
                    }
                    (file_length, commands_stored)
                };

                // If the max history file size has been reached, truncate the file so that only
                // N amount of commands are listed. To truncate the file, the seek point will be
                // discovered by counting the number of bytes until N newlines have been found and
                // then the file will be seeked to that point, copying all data after and rewriting
                // the file with the first N lines removed.
                if commands_stored >= self.max_size {
                    let seek_point = {
                        let commands_to_delete = commands_stored - self.max_size + 1;
                        let mut matched = 0;
                        let mut bytes = 0;
                        let file = File::open(&self.file_name).unwrap();
                        for byte in file.bytes() {
                            if byte.unwrap_or(b' ') == b'\n' { matched += 1; }
                            bytes += 1;
                            if matched == commands_to_delete { break }
                        }
                        bytes as u64
                    };


                    if let Err(message) = file.seek(SeekFrom::Start(seek_point)) {
                        println!("ion: unable to seek in history file: {}", message);
                    }

                    let mut buffer: Vec<u8> = Vec::with_capacity(file_length - seek_point as usize);
                    if let Err(message) = file.read_to_end(&mut buffer) {
                        println!("ion: unable to buffer history file: {}", message);
                    }

                    if let Err(message) = file.set_len(0) {
                        println!("ion: unable to truncate history file: {}", message);
                    }

                    if let Err(message) = io::copy(&mut buffer.as_slice(), &mut file) {
                        println!("ion: unable to write to history file: {}", message);
                    }
                }

                // Seek to end for appending
                if let Err(message) = file.seek(SeekFrom::End(0)) {
                    println!("ion: unable to seek in history file: {}", message);
                }

                // Write the command to the history file.
                if let Err(message) = file.write_all(String::from(new_item.clone()).as_bytes()) {
                    println!("ion: unable to write to history file: {}", message);
                }
                if let Err(message) = file.write(b"\n") {
                    println!("ion: unable to write to history file: {}", message);
                }
            }
            Err(message) => println!("ion: error opening file: {}", message)
        }
    }

}

impl Index<usize> for History {
    type Output = Buffer;

    fn index(&self, index: usize) -> &Buffer {
        &self.buffers[index]
    }
}

impl IndexMut<usize> for History {
    fn index_mut(&mut self, index: usize) -> &mut Buffer {
        &mut self.buffers[index]
    }
}