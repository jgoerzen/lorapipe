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

use std::io;
use serialport::prelude::*;
use std::io::{BufReader, BufRead, Write};
use log::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone)]
pub struct LoraSer {
    // BufReader can't be cloned.  Sigh.
    pub br: Arc<Mutex<BufReader<Box<dyn SerialPort>>>>,
    pub swrite: Arc<Mutex<Box<dyn SerialPort>>>,
    pub portname: String
}

impl LoraSer {

    /// Initialize the serial system, configuring the port.
    pub fn new(portname: &str) -> io::Result<LoraSer> {
        let settings = SerialPortSettings {
            baud_rate: 57600,
            data_bits: DataBits::Eight,
            flow_control: FlowControl::None,
            parity: Parity::None,
            stop_bits: StopBits::One,
            timeout: Duration::new(60 * 60 * 24 * 365 * 20, 0),
        };
        let readport = serialport::open_with_settings(portname, &settings)?;
        let writeport = readport.try_clone()?;
        
        Ok(LoraSer {br: Arc::new(Mutex::new(BufReader::new(readport))),
                    swrite: Arc::new(Mutex::new(writeport)),
                    portname: String::from(portname)})
    }

    /// Read a line from the port.  Return it with EOL characters removed.
    /// None if EOF reached.
    pub fn readln(&mut self) -> io::Result<Option<String>> {
        let mut buf = String::new();
        let size = self.br.lock().unwrap().read_line(&mut buf)?;
        if size == 0 {
            debug!("{}: Received EOF from serial port", self.portname); 
            Ok(None)
        } else {
            let buf = String::from(buf.trim());
            trace!("{} SERIN: {}", self.portname, buf);
            Ok(Some(buf))
        }
    }

    /// Transmits a command with terminating EOL characters
    pub fn writeln(&mut self, mut data: String) -> io::Result<()> {
        trace!("{} SEROUT: {}", self.portname, data);
        data.push_str("\r\n");
        // Give the receiver a chance to process
        self.swrite.lock().unwrap().write_all(data.as_bytes())?;
        self.swrite.lock().unwrap().flush()
    }
}


