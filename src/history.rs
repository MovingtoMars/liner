use super::*;

use std::collections::{vec_deque, VecDeque};
use std::io::{BufRead, BufReader, Error, ErrorKind};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::iter::IntoIterator;
use std::ops::Index;
use std::ops::IndexMut;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::thread::{sleep, spawn, JoinHandle};
use std::time::Duration;

use bytecount::count;

const DEFAULT_MAX_SIZE: usize = 1000;

/// Structure encapsulating command history
pub struct History {
    // TODO: this should eventually be private
    /// Vector of buffers to store history in
    pub buffers: VecDeque<Buffer>,
    /// Store a filename to save history into; if None don't save history
    file_name: Option<String>,
    /// Maximal number of buffers stored in the memory
    /// TODO: just make this public?
    max_size: usize,
    /// Maximal number of lines stored in the file
    // TODO: just make this public?
    max_file_size: Arc<AtomicUsize>,
    /// Handle to the background thread managing writes to the history file
    bg_handle: Option<JoinHandle<()>>,
    /// Signals the background thread to stop when dropping the struct
    bg_stop: Arc<AtomicBool>,
    /// Sends commands to write to the history file
    sender: Sender<(Buffer, String)>,

    // TODO set from environment variable?
    pub append_duplicate_entries: bool,
}

impl History {
    /// It's important to execute this function before exiting your program, as it will
    /// ensure that all history data has been written to the disk.
    pub fn commit_history(&mut self) {
        // Signal the background thread to stop
        self.bg_stop.store(true, Ordering::Relaxed);
        // Wait for the background thread to stop
        if let Some(handle) = self.bg_handle.take() {
            let _ = handle.join();
        }
    }

    /// Create new History structure.
    pub fn new() -> History {
        let max_file_size = Arc::new(AtomicUsize::new(DEFAULT_MAX_SIZE));
        let bg_stop = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = channel();

        let stop_signal = bg_stop.clone();
        let max_size = max_file_size.clone();
        History {
            buffers: VecDeque::with_capacity(DEFAULT_MAX_SIZE),
            file_name: None,
            sender: sender,
            bg_handle: Some(spawn(move || {
                while !stop_signal.load(Ordering::Relaxed) {
                    if let Ok((command, filepath)) = receiver.try_recv() {
                        let max_file_size = max_size.load(Ordering::Relaxed);
                        let _ = write_to_disk(max_file_size, &command, &filepath);
                    }
                    sleep(Duration::from_millis(100));
                }

                // Deplete the receiver of commands to write, before exiting the thread.
                while let Ok((command, filepath)) = receiver.try_recv() {
                    let max_file_size = max_size.load(Ordering::Relaxed);
                    let _ = write_to_disk(max_file_size, &command, &filepath);
                }
            })),
            bg_stop: bg_stop,
            max_size: DEFAULT_MAX_SIZE,
            max_file_size: max_file_size,
            append_duplicate_entries: false,
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
        self.file_name.as_ref().map(|name| {
            let _ = self.sender.send((new_item.clone(), name.to_owned()));
        });

        // buffers[0] is the oldest entry
        // the new entry goes to the end
        if !self.append_duplicate_entries
            && self.buffers.back().map(|b| b.to_string()) == Some(new_item.to_string())
        {
            return Ok(());
        }

        self.buffers.push_back(new_item);
        while self.buffers.len() > self.max_size {
            self.buffers.pop_front();
        }
        Ok(())
    }

    /// Go through the history and try to find a buffer which starts the same as the new buffer
    /// given to this function as argument.
    pub fn get_newest_match<'a, 'b>(
        &'a self,
        curr_position: Option<usize>,
        new_buff: &'b Buffer,
    ) -> Option<&'a Buffer> {
        let pos = curr_position.unwrap_or(self.buffers.len());
        for iter in (0..pos).rev() {
            if let Some(tested) = self.buffers.get(iter) {
                if tested.starts_with(new_buff) {
                    return self.buffers.get(iter);
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
        self.max_file_size.store(size, Ordering::Relaxed);
    }

    /// Load history from given file name
    pub fn load_history(&mut self) -> io::Result<()> {
        let file_name = match self.file_name.clone() {
            Some(name) => name,
            None => {
                return Err(Error::new(
                    ErrorKind::Other,
                    "Liner: file name not specified",
                ))
            }
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

    fn buffers_ref(&self) -> &VecDeque<Buffer> {
        &self.buffers
    }
}

impl<'a> IntoIterator for &'a History {
    type Item = &'a Buffer;
    type IntoIter = vec_deque::Iter<'a, Buffer>;

    fn into_iter(self) -> Self::IntoIter {
        self.buffers_ref().into_iter()
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

/// Perform write operation. If the history file does not exist, it will be created.
/// This function is not part of the public interface.
/// XXX: include more information in the file (like fish does)
fn write_to_disk(max_file_size: usize, new_item: &Buffer, file_name: &str) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(file_name)?;

    // The metadata contains the length of the file
    let file_length = file.metadata().ok().map_or(0u64, |m| m.len());

    {
        // Count number of entries in file
        let mut num_stored = 0;

        // 4K byte buffer for reading chunks of the file at once.
        let mut buffer = [0; 4096];

        // Find the total number of commands in the file
        loop {
            // Read 4K of bytes all at once into the buffer.
            let read = file.read(&mut buffer)?;
            // If EOF is found, don't seek at all.
            if read == 0 {
                break;
            }
            // Count the number of commands that were found in the current buffer.
            let cmds_read = count(&buffer[0..read], b'\n');
            num_stored += cmds_read;
        }

        // Find how many bytes we need to move backwards
        // in the file to remove all the old commands.
        if num_stored >= max_file_size {
            let mut total_read = 0u64;
            let mut move_dist = 0u64;
            file.seek(SeekFrom::Start(0))?;

            let mut eread = 0;
            loop {
                // Read 4K of bytes all at once into the buffer.
                let read = file.read(&mut buffer)?;
                // If EOF is found, don't seek at all.
                if read == 0 {
                    break;
                }
                // Count the number of commands that were found in the current buffer
                let cmds_read = count(&buffer[0..read], b'\n');

                if eread + cmds_read >= num_stored - max_file_size {
                    for &byte in buffer[0..read].iter() {
                        total_read += 1;
                        if byte == b'\n' {
                            if eread == num_stored - max_file_size {
                                move_dist = total_read;
                                break;
                            }
                            eread += 1;
                        }
                    }
                    break;
                } else {
                    total_read += read as u64;
                    eread += cmds_read;
                }
            }


            // Move it all back
            move_file_contents_backward(&mut file, move_dist)?;
        }
    };


    // Seek to end for appending
    try!(file.seek(SeekFrom::End(0)));
    // Write the command to the history file.
    try!(file.write_all(String::from(new_item.clone()).as_bytes()));
    try!(file.write_all(b"\n"));
    file.flush()?;

    Ok(())
}

fn move_file_contents_backward(file: &mut File, distance: u64) -> io::Result<()> {
    let mut total_read = 0;
    let mut buffer = [0u8; 4096];

    file.seek(SeekFrom::Start(distance))?;
    
    loop {
        // Read 4K of bytes all at once into the buffer.
        let read = file.read(&mut buffer)?;
        total_read += read as u64;
        // If EOF is found, don't seek at all.
        if read == 0 {
            break;
        }

        file.seek(SeekFrom::Current(-(read as i64 + distance as i64)))?;


        file.write_all(&buffer[..read])?;
        file.seek(SeekFrom::Current(distance as i64))?;
    }

    file.set_len(total_read)?;
    file.flush()?;

    Ok(())
}
