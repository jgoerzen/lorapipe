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
use crossbeam_channel;
use crossbeam_channel::select;
use hex;
use std::thread;

pub fn mkerror(msg: &str) -> Error {
    Error::new(ErrorKind::Other, msg)
}

#[derive(PartialEq, Debug)]
enum LState {
    ReadRx,
    Command,
}

pub struct LoraStik {
    ser: LoraSer,
    readerlinesrx: crossbeam_channel::Receiver<String>,
    writerlinestx: crossbeam_channel::Sender<String>,     // prevents data races
    readeroutput: crossbeam_channel::Sender<Vec<u8>>,
    txblockstx: crossbeam_channel::Sender<Vec<u8>>,
    txblocksrx: crossbeam_channel::Receiver<Vec<u8>>
}

/// Utility to read the response from initialization
fn initresp(rx: crossbeam_channel::Receiver<String>) -> io::Result<()> {
    let line = rx.recv().unwrap();
    if line == "invalid_param" {
        Err(mkerror("Bad response from radio during initialization"))
    } else {
        Ok(())
    }
}

fn readerlinesthread(ser: LoraSer, tx: crossbeam_channel::Sender<String>) {
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

fn writerlinesthread(ser: LoraSer, rx: crossbeam_channel::Receiver<String>) {
    loop {
        let line = rx.recv().unwrap();
        ser.writeln(line).expect("Error writing line");
    }
}

/// Assert that a given response didn't indicate an EOF, and that it
/// matches the given text.  Return an IOError if either of these
/// conditions aren't met.  The response type is as given by
/// ['ser::LoraSer::readln'].
pub fn assert_response(resp: String, expected: String) -> io::Result<()> {
    if resp == expected {
        Ok(())
    } else {
        Err(mkerror("Unexpected response"))
    }
}

impl LoraStik {
    pub fn new(ser: LoraSer) -> (LoraStik, crossbeam_channel::Receiver<Vec<u8>>) {
        let (readerlinestx, readerlinesrx) = mpsc::channel();
        let (writerlinestx, writerlinesrx) = mpsc::sync_channel(0);
        let (txblockstx, txblocksrx) = mpsc::channel();

        thread::spawn(move || readerlinesthread(ser, readerlinestx));
        thread::spawn(move || writerlinesthread(ser, writerlinestx));
        
        (LoraStik { ser, readercmdrx, readercmdtx, readeroutput, readerlinesrx, writerlinestx, txblockstx, txblocksrx}, readeroutputreader)
    }

    pub fn radiocfg(&mut self) -> io::Result<()> {
        let f = fs::File::open("init.txt")?;
        let reader = BufReader::new(f);

        for line in reader.lines() {
            let line = line?;
            if line.len() > 0 {
                self.writerlinestx.send(line).unwrap();
                initresp(self.readerlinesrx)?;
            }
        }
        Ok(())
    }

    pub fn readerthread(&mut self) -> io::Result<()> {
        loop {
            // Do we have anything to send?
            let r = self.txblocksrx.try_recv();
            match r {
                Ok(data) => {
                    dosend(self.writerlinestx, data);
                    continue;
                },
                Err(e) => {
                    if e.is_disconnected() {
                        // other threads crashed
                        Err(e).unwrap();
                    }
                    // Otherwise - nothing to write, go on through.
                }
            }
                
            self.writerlinestx.send(String::from("radio rx 0")).unwrap();
            let response = self.readerlinesrx.recv().unwrap();
            assert_response(response, "ok")?;

            // Now we wait for either a write request or data.

            select! {
                recv(self.readerlinesrx) -> msg => {
                    let msg = msg.unwrap();
                    if msg.starts_with("radio_rx ") {
                        if let Ok(decoded) = hex::decode(&msg.as_bytes()[10..]) {
                            self.readeroutput.send(decoded).unwrap();
                        } else {
                            return Err(mkerror("Error with hex decoding"));
                        }
                    },
                },
                
                recv(self.txblocksrx) -> msg => {
                    let msg = msg.unwrap();
                    
                    // We have something to send.  First, we have to stop the receiver (rxstop).
                    self.writerlinestx.send(String::from("radio rxstop"))?;
                    let mut checkresp = self.readerlinesrx.recv().unwrap();
                    if checkresp.starts_with("radio_rx ") {
                        // We had a race.  A packet was coming in.  Decode and deal with it,
                        // then look for the 'ok' from rxstop.
                        if let Ok(decoded) = hex::decode(&checkresp.as_bytes()[10..]) {
                            self.readeroutput.send(decoded).unwrap();
                            checkresp = self.readerlinesrx.recv().unwrap();
                        } else {
                            return Err(mkerror("Error with hex decoding"));
                        }
                    }
                    
                    // Now, checkresp should hold 'ok'.
                    assert_response(checkkresp, String::from("ok"))?;
                    
                    // Now, send the mesage.
                    let txstr = String::from("radio tx ");
                    let hexstr = hex::encode(msg);
                    txstr.push_str(&hexstr);
                    self.writerlinestx.send(txstr);
                    
                    // We get two responses from this.
                    let resp = self.readerlinesrx.recv().unwrap();
                    assert_response(resp, String::from("ok"))?;
                    
                    // Second.
                    let resp = self.readerlinesrx.recv().unwrap();
                    assert_response(resp, String::from("radio_tx_ok"));
                }
            }
        }
    }

    pub fn transmit(&mut self, data: Vec<u8>)  {
        self.txblockstx.send(data).unwrap();
    }
}


