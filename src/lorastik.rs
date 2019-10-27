/*
    Copyright (C) 2019  John Goerzen <jgoerzen@complete.org

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.

*/

use crate::ser::LoraSer;
use log::*;
use std::fs;
use std::io::{BufRead, BufReader, Error, ErrorKind};
use std::io;
use std::sync::mpsc;
use hex;
use std::thread;

pub fn mkerror(msg: &str) -> Error {
    Error::new(ErrorKind::Other, msg)
}

#[derive(PartialEq, Debug)]
/// Sent down the channel to the reader thread to tell it when to start or stop reading.
enum ReaderCommand {
    StartReading,
    StopReading
}
        
pub struct LoraStik {
    ser: LoraSer,
    readercmdrx: mpsc::Receiver<ReaderCommand>,
    readercmdtx: mpsc::SyncSender<ReaderCommand>,
    readeroutput: mpsc::Sender<Vec<u8>>,
    readerlinesrx: mpsc::Receiver<String>,
}

/// Utility to read the response from initialization
fn initresp(rx: mpsc::Receiver<String>) -> io::Result<()> {
    let line = rx.recv().unwrap();
    if line == "invalid_param" {
        Err(mkerror("Bad response from radio during initialization"))
    } else {
        Ok(())
    }
}

fn readerlinesthread(ser: LoraSer, tx: mpsc::Sender<String>) {
    loop {
        let line = ser.readln().expect("Error reading line");
        if let Some(l) = line {
            tx.send(l).unwrap();
        } else {
            debug!("{}: EOF", ser.portname);
            return;
        }
    }
}


impl LoraStik {
    pub fn new(ser: LoraSer) -> (LoraStik, mpsc::Receiver<Vec<u8>>) {
        let (readercmdtx, readercmdrx) = mpsc::sync_channel(0);
        let (readeroutput, readeroutputreader) = mpsc::channel();
        let (readerlinestx, readerlinesrx) = mpsc::channel();

        thread::spawn(move || readerlinesthread(ser, readerlinestx));
        
        (LoraStik { ser, readercmdrx, readercmdtx, readeroutput, readerlinesrx}, readeroutputreader)
    }

    pub fn radiocfg(&mut self) -> io::Result<()> {
        let f = fs::File::open("init.txt")?;
        let reader = BufReader::new(f);

        for line in reader.lines() {
            let line = line?;
            if line.len() > 0 {
                self.ser.writeln(line)?;
                initresp(self.readerlinesrx)?;
            }
        }
        Ok(())
    }

    pub fn readerthread(&mut self) -> io::Result<()> {
        loop {
            let command = self.readercmdrx.try_recv();
            if command == Ok(ReaderCommand::StartReading) || command == Err(mpsc::TryRecvError::Empty) {
                debug!("{}: Entering RX mode", self.ser.portname);
                self.ser.writeln(String::from("radio rx 0"));
                let response = self.ser.readln()?;
                if response != Some(String::from("ok")) {
                    return Err(mkerror("Unexpected response from radio rx"));
                }

                // Now read the ultimate response from the radio, or 
                let response = self.ser.readln()?;
                match response {
                    Some(r) => 
                        if r.starts_with("radio_rx ") {
                            if let Ok(decoded) = hex::decode(&r.as_bytes()[10..]) {
                                self.readeroutput.send(decoded).unwrap();
                            } else {
                                return Err(mkerror("Error with hex decoding"));
                            }
                        },
                    None => return Err(mkerror("Unexpected EOF in radio_rx"))
                }
            } else if command == Ok(ReaderCommand::StopReading) {
                loop {
                    // Block until we are unblocked.
                    let command = self.readercmdrx.recv().unwrap();
                    if command == ReaderCommand::StartReading {
                        break;
                    }
                }
            } else {
                command.unwrap();
            }
        }
    }

    pb fn transmit(&mut self, Vec<u8>) -> io::Result<()> {
        
    }
}


