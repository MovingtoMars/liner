use super::*;

use std::collections::VecDeque;
use std::io::{BufReader, BufRead, Error, ErrorKind};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::ops::Index;
use std::ops::IndexMut;

const DEFAULT_MAX_SIZE: usize = 1000;

/// Structure encapsulating command history
pub struct History {
    // TODO: this should eventually be private
    /// Vector of buffers to store history in
    pub buffers: VecDeque<Buffer>,
    // TODO: Do we need this here? Ion can take care of this.
    // pub previous_status: i32,
    /// Store a filename to save history into; if None don't save history
    file_name: Option<String>,
    /// Maximal number of buffers stored in the memory
    max_size: usize,
    /// Maximal number of lines stored in the file
    max_file_size: usize,
}

impl History {
    /// Create new History structure.
    pub fn new() -> History {
        History {
            buffers: VecDeque::with_capacity(DEFAULT_MAX_SIZE),
            file_name: None,
            max_size: DEFAULT_MAX_SIZE,
            max_file_size: DEFAULT_MAX_SIZE,
        }
    }

    /// Number of items in history.
    pub fn len(&self) -> usize {
        self.buffers.len()
    }

    /// Add a command to the history buffer and remove the oldest commands when the max history
    /// size has been met. If writing to the disk is enabled, this function will be used for
    /// logging history to the designated history file.
    pub fn push(&mut self, new_item: Buffer) -> io::Result<()> {

        let mut ret = Ok(());
        self.file_name.as_ref().map(|name| {
            ret = self.write_to_disk(&new_item, name);
        });

        // buffers[0] is the oldest entry
        // the new entry goes to the end
        self.buffers.push_back(new_item);
        while self.buffers.len() > self.max_size {
            self.buffers.pop_front();
        }
        ret
    }

    pub fn get_first_match(&self, curr_position: Option<usize>, new_buff: &Buffer) -> Option<&Buffer> {
        let pos = curr_position.unwrap_or(self.buffers.len());
        for iter in (0..pos).rev() {
            if let Some(tested) = self.buffers.get(iter) {
                if tested.is_match(new_buff) {
                    return self.buffers.get(iter)
                }
            }
        }
        None
    }

    /// Get the history file name.
    pub fn file_name(&self) -> Option<&str> {
        match self.file_name {
            Some(ref s) => Some(&s[..]),
            None => None,
        }
    }

    /// Set history file name. At the same time enable history.
    pub fn set_file_name(&mut self, name: Option<String>) {
        self.file_name = name;
    }

    /// Set maximal number of buffers stored in memory
    pub fn set_max_size(&mut self, size: usize) {
        self.max_size = size;
    }

    /// Set maximal number of entries in history file
    pub fn set_max_file_size(&mut self, size: usize) {
        self.max_file_size = size;
    }

    /// Load history from given file name
    pub fn load_history(&mut self) -> io::Result<()> {
        let file_name = match self.file_name.clone() {
            Some(name) => name,
            None => return Err(Error::new(ErrorKind::Other, "Liner: file name not specified")),
        };
        let file = try!(OpenOptions::new().read(true).open(file_name));
        let reader = BufReader::new(file);
        for line in reader.lines() {
            match line {
                Ok(line) => self.buffers.push_back(Buffer::from(line)),
                Err(_) => break,
            }
        }
        Ok(())
    }

    /// Perform write operation. If the history file does not exist, it will be created.
    /// This function is not part of the public interface.
    /// XXX: include more information in the file (like fish does)
    fn write_to_disk(&self, new_item: &Buffer, file_name: &String) -> io::Result<()> {
        let ret = match OpenOptions::new().read(true).write(true).create(true).open(file_name) {
            Ok(mut file) => {
                // Determine the number of commands stored and the file length.
                let (file_length, commands_stored) = {
                    let mut commands_stored = 0;
                    let mut file_length = 0;
                    let file = File::open(file_name).unwrap();
                    for byte in file.bytes() {
                        if byte.unwrap_or(b' ') == b'\n' {
                            commands_stored += 1;
                        }
                        file_length += 1;
                    }
                    (file_length, commands_stored)
                };

                // If the max history file size has been reached, truncate the file so that only
                // N amount of commands are listed. To truncate the file, the seek point will be
                // discovered by counting the number of bytes until N newlines have been found and
                // then the file will be seeked to that point, copying all data after and rewriting
                // the file with the first N lines removed.
                if commands_stored >= self.max_file_size {
                    let seek_point = {
                        let commands_to_delete = commands_stored - self.max_file_size + 1;
                        let mut matched = 0;
                        let mut bytes = 0;
                        let file = File::open(file_name).unwrap();
                        for byte in file.bytes() {
                            if byte.unwrap_or(b' ') == b'\n' {
                                matched += 1;
                            }
                            bytes += 1;
                            if matched == commands_to_delete {
                                break;
                            }
                        }
                        bytes as u64
                    };

                    try!(file.seek(SeekFrom::Start(seek_point)));
                    let mut buffer: Vec<u8> = Vec::with_capacity(file_length - seek_point as usize);
                    try!(file.read_to_end(&mut buffer));
                    try!(file.set_len(0));
                    try!(io::copy(&mut buffer.as_slice(), &mut file));

                }

                // Seek to end for appending
                try!(file.seek(SeekFrom::End(0)));
                // Write the command to the history file.
                try!(file.write_all(String::from(new_item.clone()).as_bytes()));
                try!(file.write(b"\n"));

                Ok(())
            }
            Err(message) => Err(message),
        };
        ret
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
