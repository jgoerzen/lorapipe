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
use std::thread;
use std::time::Duration;

const INTERVAL: u64 = 5;

pub fn genpings(ls: &mut LoraStik) -> io::Result<()> {
    let mut counter: u64 = 1;
    loop {
        let sendstr = format!("Ping {}", counter);
        println!("SEND: {}", sendstr);
        ls.transmit(&sendstr.as_bytes());
        thread::sleep(Duration::from_secs(INTERVAL));
        counter += 1;
    }
}

/// Reply to pings
pub fn pong(ls: &mut LoraStik, receiver: crossbeam_channel::Receiver<ReceivedFrames>) -> io::Result<()> {
    let mut stdout = io::stdout();

    loop {
        let data = receiver.recv().unwrap();
        let resp = format!("Pong {}, {:?}", String::from_utf8_lossy(&data.0), data.1);
        println!("SEND: {}", resp);
        ls.transmit(resp.as_bytes());
    }
}

