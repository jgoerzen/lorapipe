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
use std::io::{BufRead, BufReader};
use std::io;

pub struct LoraStik {
    ser: LoraSer
}

/// Utility to read the response from initialization
fn initresp(ser: &mut LoraSer) -> io::Result<()> {
    let line = ser.readln()?;
    match line {
        Some(x) =>
            if x == "invalid_param" {
                None.expect("Bad response from radio during initialization")
            } else {
                Ok(())
            },
        None => None.expect("Unexpected EOF from radio during initialization"),
    }
}

impl LoraStik {
    pub fn new(ser: LoraSer) -> LoraStik {
        LoraStik { ser }
    }

    pub fn radiocfg(&mut self) -> io::Result<()> {
        let f = fs::File::open("init.txt")?;
        let reader = BufReader::new(f);

        for line in reader.lines() {
            let line = line?;
            if line.len() > 0 {
                self.ser.writeln(line)?;
                initresp(&mut self.ser)?;
            }
        }
        Ok(())
    }
}


