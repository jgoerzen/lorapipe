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

use serial::prelude::*;
use std::io;
use serial;
use std::io::{BufReader, BufRead, Write};
use log::*;

pub struct LoraSer {
    br: BufReader<serial::SystemPort>,
    portname: String
}

impl LoraSer {

    /// Initialize the serial system, configuring the port.
    pub fn new(portname: &str) -> io::Result<LoraSer> {
        let mut port = serial::open(portname)?;
        port.reconfigure(&|settings| {
            settings.set_baud_rate(serial::Baud57600)?;
            settings.set_char_size(serial::Bits8);
            settings.set_parity(serial::ParityNone);
            settings.set_stop_bits(serial::Stop1);
            settings.set_flow_control(serial::FlowNone);
            Ok(())
        })?;
        Ok(LoraSer {br: BufReader::new(port), portname: String::from(portname)})
    }

    /// Read a line from the port.  Return it with EOL characters removed.
    /// None if EOF reached.
    pub fn readln(&mut self) -> io::Result<Option<String>> {
        let mut buf = String::new();
        let size = self.br.read_line(&mut buf)?;
        if size == 0 {
            debug!("{}: Received EOF from serial port", self.portname); 
            Ok(None)
        } else {
            let last = buf.pop();
            if last == Some('\n') {
                trace!("{} SERIN: {}", self.portname, buf);
                Ok(Some(buf))
            } else {
                debug!("{}: Received input line '{}' without terminating nerline", self.portname, buf);
                None.expect("Input line didn't end with newline")
            }
        }
    }

    /// Transmits a command with terminating EOL characters
    pub fn writeln(&mut self, mut data: String) -> io::Result<()> {
        trace!("{} SEROUT: {}", self.portname, data);
        data.push_str("\r\n");
        let serport = self.br.get_mut();
        serport.write_all(data.as_bytes())
    }
}


