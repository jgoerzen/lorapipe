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
use std::io::{Read, Write};
use crate::lorastik::{LoraStik, ReceivedFrames};
use crossbeam_channel;

const MAXFRAME: usize = 81;

/// A thread for stdin processing
pub fn stdintolora(ls: &mut LoraStik) -> io::Result<()> {
    let stdin = io::stdin();
    let mut br = io::BufReader::new(stdin);

    let mut buf = vec![0u8; 8192];

    loop {
        let res = br.read(&mut buf)?;
        if res == 0 {
            // EOF
            return Ok(());
        }

        for chunk in buf[0..res].chunks(MAXFRAME) {
            ls.transmit(&chunk);
        }
    }
}

pub fn loratostdout(receiver: crossbeam_channel::Receiver<ReceivedFrames>) -> io::Result<()> {
    let mut stdout = io::stdout();

    loop {
        let data = receiver.recv().unwrap();
        stdout.write_all(&data.0)?;
        stdout.flush()?;
    }
}

