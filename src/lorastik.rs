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
use hex;
use std::thread;
use std::time::{Duration, Instant};
use format_escape_default::format_escape_default;

/** The amount of time to pause before transmitting a packet.  The
main purpose of this is to give the othe rradio a chance to finish
decoding the previous packet, send it to the OS, and re-enter RX mode.
A secondary purpose is to give the duplex logic a chance to see if
anything else is coming in.  Given in ms.
*/
const WAIT_BEFORE_TX_MILLIS: u64 = 50;

/** The amount of time to wait before transmitting after receiving a
packet that indicated more data was forthcoming.  The purpose of this is
to compensate for a situation in which the "last" incoming packet was lost,
to prevent the receiver from waiting forever for more packets before
transmitting.  Given in ms. */
const TX_PREVENTION_TIMEOUT_MILLIS: u64 = 1000;

pub fn mkerror(msg: &str) -> Error {
    Error::new(ErrorKind::Other, msg)
}

/// Received frames.  The option is populated only if
/// readqual is true, and reflects the SNR and RSSI of the
/// received packet.
#[derive(Clone, Debug, PartialEq)]
pub struct ReceivedFrames(pub Vec<u8>, pub Option<(String, String)>);

#[derive(Clone)]
pub struct LoraStik {
    ser: LoraSer,

    // Lines coming from the radio
    readerlinesrx: crossbeam_channel::Receiver<String>,

    // Frames going to the app
    readeroutput: crossbeam_channel::Sender<ReceivedFrames>,

    // Blocks to transmit
    txblockstx: crossbeam_channel::Sender<Vec<u8>>,
    txblocksrx: crossbeam_channel::Receiver<Vec<u8>>,

    // Whether or not to read quality data from the radio
    readqual: bool,

    // Whether we must delay before transmit.  The Instant
    // reflects the moment when the delay should end.
    txdelay: Option<Instant>,

    // The wait before transmitting.  Initialized from
    // [`WAIT_BEFORE_TX_MILLIS`].
    wait_before_tx: Duration,
    
    // The transmit prevention timeout.  Initialized from
    // [`TX_PREVENTION_TIMEOUT_MILLIS`].
    tx_prevention_timeout: Duration,
}

/// Reads the lines from the radio and sends them down the channel to
/// the processing bits.
fn readerlinesthread(mut ser: LoraSer, tx: crossbeam_channel::Sender<String>) {
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
        Err(mkerror(&format!("Unexpected response: got {}, expected {}", resp, expected)))
    }
}

impl LoraStik {
    /// Creates a new LoraStik.  Returns an instance to be used for sending,
    /// as well as a separate receiver to be used in a separate thread to handle
    /// incoming frames.  The bool specifies whether or not to read the quality
    /// parameters after a read.
    pub fn new(ser: LoraSer, readqual: bool) -> (LoraStik, crossbeam_channel::Receiver<ReceivedFrames>) {
        let (readerlinestx, readerlinesrx) = crossbeam_channel::unbounded();
        let (txblockstx, txblocksrx) = crossbeam_channel::unbounded();
        let (readeroutput, readeroutputreader) = crossbeam_channel::unbounded();

        let ser2 = ser.clone();
        
        thread::spawn(move || readerlinesthread(ser2, readerlinestx));
        
        (LoraStik { readqual, ser, readeroutput, readerlinesrx, txblockstx, txblocksrx,
                    txdelay: None,
                    wait_before_tx: Duration::from_millis(WAIT_BEFORE_TX_MILLIS),
                    tx_prevention_timeout: Duration::from_millis(TX_PREVENTION_TIMEOUT_MILLIS)}, readeroutputreader)
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
        // First, send it an invalid command.  Then, consume everything it sends back
        self.ser.writeln(String::from("INVALIDCOMMAND"))?;

        // Give it a chance to do its thing.
        thread::sleep(Duration::from_secs(1));

        // Consume all data.
        while let Ok(_) = self.readerlinesrx.try_recv() {
        }
                         
        debug!("Configuring radio");
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
        // Give receiver a change to process.
        thread::sleep(self.wait_before_tx);

        let mut flag: u8 = 0;
        if !self.txblocksrx.is_empty() {
            flag = 1;
        }

        // Now, send the mesage.
        let txstr = format!("radio tx {}{}", hex::encode([flag]), hex::encode(data));

        self.ser.writeln(txstr)?;
        
        // We get two responses from this.... though sometimes a lingering radio_err also.
        let mut resp = self.readerlinesrx.recv().unwrap();
        if resp == String::from("radio_err") {
            resp = self.readerlinesrx.recv().unwrap();
        }
        assert_response(resp, String::from("ok"))?;
        
        // Second.
        self.readerlinesrx.recv().unwrap();  // normally radio_tx_ok

        Ok(())
    }

    // Receive a message from the incoming radio channel and process it.
    fn handlerx(&mut self, msg: String, readqual: bool) -> io::Result<()> {
        if msg.starts_with("radio_rx ") {
            if let Ok(mut decoded) = hex::decode(&msg.as_bytes()[10..]) {
                trace!("DECODED: {}", format_escape_default(&decoded));
                let radioqual = if readqual {
                    self.ser.writeln(String::from("radio get snr"))?;
                    let snr = self.readerlinesrx.recv().unwrap();
                    self.ser.writeln(String::from("radio get rssi"))?;
                    let rssi = self.readerlinesrx.recv().unwrap();
                    Some((snr, rssi))
                } else {
                    None
                };

                let flag = decoded.remove(0);  // Remove the flag from the vec
                if flag == 1 {
                    // More data is coming
                    self.txdelay = Some(Instant::now() + self.tx_prevention_timeout);
                } else {
                    self.txdelay = None;
                }

                self.readeroutput.send(ReceivedFrames(decoded, radioqual)).unwrap();
            } else {
                return Err(mkerror("Error with hex decoding"));
            }
        }

        // Might get radio_err here.  That's harmless.
        Ok(())
    }

    // Whether or not a txdelay prevents transmit at this time.  None if
    // we are cleared to transmit; Some(Duration) gives the amount of time
    // we'd have to wait otherwise.
    fn txdelayrequired(&mut self) -> Option<Duration> {
        match self.txdelay {
            None => None,
            Some(delayend) => {
                let now = Instant::now();
                if now >= delayend {
                    // We're past the delay.  Clear it and return.
                    self.txdelay = None;
                    None
                } else {
                    // Indicate we're still blocked.
                    Some(delayend - now)
                }
            }
        }
    }

    fn enterrxmode(&mut self) -> io::Result<()> {
        // Enter read mode
        
        self.ser.writeln(String::from("radio rx 0"))?;
        let mut response = self.readerlinesrx.recv().unwrap();
        
        // For some reason, sometimes we get a radio_err here, then an OK.  Ignore it.
        if response == String::from("radio_err") {
            response = self.readerlinesrx.recv().unwrap();
        }
        assert_response(response, String::from("ok"))?;
        Ok(())
    }

    fn rxstop(&mut self) -> io::Result<()> {
        self.ser.writeln(String::from("radio rxstop"))?;
        let checkresp = self.readerlinesrx.recv().unwrap();
        if checkresp.starts_with("radio_rx ") {
            // We had a race.  A packet was coming in.  Decode and deal with it,
            // then look for the 'ok' from rxstop.  We can't try to read the quality in
            // this scenario.
            self.handlerx(checkresp, false)?;
            self.readerlinesrx.recv().unwrap();  // used to pop this into checkresp, but no need now.
        }
        
        // Now, checkresp should hold 'ok'.
        //  It might not be; I sometimes see radio_err here.  it's OK too.
        // assert_response(checkresp, String::from("ok"))?;
        Ok(())
    }
    
    pub fn readerthread(&mut self) -> io::Result<()> {
        loop {
            // First, check to see if we're allowed to transmit.  If not, just
            // try to read and ignore all else.
            if let Some(delayamt) = self.txdelayrequired() {
                // We can't transmit yet.  Just read, but with a time box.
                self.enterrxmode()?;
                let res = self.readerlinesrx.recv_timeout(delayamt);
                match res {
                    Ok(msg) => {
                        self.handlerx(msg, self.readqual)?;
                        continue;
                    },
                    Err(e) => {
                        if e.is_timeout() {
                            self.txdelay = None;
                            // Now we can fall through to the rest of the logic - already in read mode.
                        } else {
                            res.unwrap(); // disconnected - crash
                        }
                    }
                }
            } else {
                // We are allowed to transmit.
                
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

                self.enterrxmode()?;
            }

            // At this point, we're in rx mode with no timeout.
            // Now we wait for either a write request or data.

            let mut sel = crossbeam_channel::Select::new();
            let readeridx = sel.recv(&self.readerlinesrx);
            let blocksidx = sel.recv(&self.txblocksrx);
            match sel.ready() {
                i if i == readeridx => {
                    // We have data coming in from the radio.
                    let msg = self.readerlinesrx.recv().unwrap();
                    self.handlerx(msg, self.readqual)?;
                },
                i if i == blocksidx => {
                    // We have something to send.  Stop the receiver and then go
                    // back to the top of the loop to handle it.

                    self.rxstop()?;
                    
                },
                _ => panic!("Invalid response from sel.ready()"),
            }
        }
    }

    pub fn transmit(&mut self, data: &[u8])  {
        self.txblockstx.send(data.to_vec()).unwrap();
    }
}


