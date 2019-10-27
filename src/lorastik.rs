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
use std::mem;

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
    readeroutput: crossbeam_channel::Sender<Vec<u8>>,
    txblockstx: crossbeam_channel::Sender<Vec<u8>>,
    txblocksrx: crossbeam_channel::Receiver<Vec<u8>>
}

fn readerlinesthread(ser: &mut LoraSer, tx: crossbeam_channel::Sender<String>) {
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
    pub fn new(mut ser: LoraSer) -> (LoraStik, crossbeam_channel::Receiver<Vec<u8>>) {
        let (readerlinestx, readerlinesrx) = crossbeam_channel::unbounded();
        let (txblockstx, txblocksrx) = crossbeam_channel::unbounded();
        let (readeroutput, readeroutputreader) = crossbeam_channel::unbounded();

        thread::spawn(move || readerlinesthread(&mut ser, readerlinestx));
        
        (LoraStik { ser, readeroutput, readerlinesrx, txblockstx, txblocksrx}, readeroutputreader)
    }

    /// Utility to read the response from initialization
    fn initresp(&mut self) -> io::Result<()> {
        let line = self.readerlinesrx.recv().unwrap();
        if line == "invalid_param" {
            Err(mkerror("Bad response from radio during initialization"))
        } else {
            Ok(())
        }
    }

    pub fn radiocfg(&mut self) -> io::Result<()> {
        let f = fs::File::open("init.txt")?;
        let reader = BufReader::new(f);

        for line in reader.lines() {
            let line = line?;
            if line.len() > 0 {
                self.ser.writeln(line)?;
                self.initresp()?;
            }
        }
        Ok(())
    }

    /// Utililty function to handle actual sending.  Assumes radio is idle.
    fn dosend(&mut self, data: Vec<u8>) -> io::Result<()> {
        // Now, send the mesage.
        let mut txstr = String::from("radio tx ");
        let hexstr = hex::encode(data);
        txstr.push_str(&hexstr);
        self.ser.writeln(txstr)?;
        
        // We get two responses from this.
        let resp = self.readerlinesrx.recv().unwrap();
        assert_response(resp, String::from("ok"))?;
        
        // Second.
        let resp = self.readerlinesrx.recv().unwrap();
        assert_response(resp, String::from("radio_tx_ok"))?;

        Ok(())
    }
    
    pub fn readerthread(&mut self) -> io::Result<()> {
        loop {
            // Do we have anything to send?  Check at the top and keep checking
            // here so we send as much as possible before going back into read
            // mode.
            let r = self.txblocksrx.try_recv();
            match r {
                Ok(data) => {
                    self.dosend(data)?;
                    continue;
                },
                Err(e) => {
                    if e.is_disconnected() {
                        // other threads crashed
                        r.unwrap();
                    }
                    // Otherwise - nothing to write, go on through.
                }
            }

            // Enter read mode
            
            self.ser.writeln(String::from("radio rx 0"))?;
            let response = self.readerlinesrx.recv().unwrap();
            assert_response(response, String::from("ok"))?;

            // Now we wait for either a write request or data.

            let mut sel = crossbeam_channel::Select::new();
            let readeridx = sel.recv(&self.readerlinesrx);
            let blocksidx = sel.recv(&self.txblocksrx);
            match sel.ready() {
                i if i == readeridx => {
                    // We have data coming in from the radio.
                    
                    let msg = self.readerlinesrx.recv().unwrap();
                    if msg.starts_with("radio_rx ") {
                        if let Ok(decoded) = hex::decode(&msg.as_bytes()[10..]) {
                            self.readeroutput.send(decoded).unwrap();
                        } else {
                            return Err(mkerror("Error with hex decoding"));
                        }
                    } else if msg == String::from("radio_err") {
                        // time out.  Harmless.
                        continue;
                    }
                },
                i if i == blocksidx => {
                    // We have something to send.  Stop the receiver and then go
                    // back to the top of the loop to handle it.

                    self.ser.writeln(String::from("radio rxstop"))?;
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
                    assert_response(checkresp, String::from("ok"))?;
                    
                },
                _ => panic!("Invalid response from sel.ready()"),
            }
        }
    }

    pub fn transmit(&mut self, data: Vec<u8>)  {
        self.txblockstx.send(data).unwrap();
    }
}


