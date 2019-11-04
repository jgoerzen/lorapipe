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

use simplelog::*;
use std::io;
use log::*;
use std::thread;

mod ser;
mod lorastik;
mod pipe;
mod ping;
mod kiss;

use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "lorapipe", about = "Tools for LoRa radios", author = "John Goerzen <jgoerzen@complete.org>")]
struct Opt {
    /// Activate debug mode
    // short and long flags (-d, --debug) will be deduced from the field's name
    #[structopt(short, long)]
    debug: bool,

    /// Read and log quality data after receiving packets
    #[structopt(long)]
    readqual: bool,

    /// Pack as many bytes as possible into each TX frame, regardless of original framing
    #[structopt(long)]
    pack: bool,
    
    /// Radio initialization command file
    #[structopt(long, parse(from_os_str))]
    initfile: Option<PathBuf>,

    /// Maximum frame size sent to radio [10..250] (valid only for ping and kiss)
    #[structopt(long, default_value = "100")]
    maxpacketsize: usize,

    /// Amount of time (ms) to pause before transmitting a packet
    /* The
    main purpose of this is to give the othe rradio a chance to finish
    decoding the previous packet, send it to the OS, and re-enter RX mode.
    A secondary purpose is to give the duplex logic a chance to see if
    anything else is coming in.  Given in ms.
     */
    #[structopt(long, default_value = "120")]
    txwait: u64,

    /// Amount of time (ms) to wait for end-of-transmission signal before transmitting
    /* The amount of time to wait before transmitting after receiving a
    packet that indicated more data was forthcoming.  The purpose of this is
    to compensate for a situation in which the "last" incoming packet was lost,
    to prevent the receiver from waiting forever for more packets before
    transmitting.  Given in ms. */
    #[structopt(long, default_value = "1000")]
    eotwait: u64,
    
    #[structopt(parse(from_os_str))]
    /// Serial port to use to communicate with radio
    port: PathBuf,

    #[structopt(subcommand)]
    cmd: Command
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Pipe data across raios
    Pipe,
    /// Transmit ping requests
    Ping,
    /// Receive ping requests and transmit pongs
    Pong,
    /// Pipe KISS data across the radios
    Kiss,
}

fn main() {
    let opt = Opt::from_args();

    if opt.debug {
        WriteLogger::init(LevelFilter::Trace, Config::default(), io::stderr()).expect("Failed to init log");
    }
    info!("lora starting");

    let maxpacketsize = opt.maxpacketsize;
    
    let loraser = ser::LoraSer::new(opt.port).expect("Failed to initialize serial port");
    let (mut ls, radioreceiver) = lorastik::LoraStik::new(loraser, opt.readqual, opt.txwait, opt.eotwait, maxpacketsize, pack);
    ls.radiocfg(opt.initfile).expect("Failed to configure radio");

    let mut ls2 = ls.clone();
    thread::spawn(move || ls2.mainloop().expect("Failure in readerthread"));

    match opt.cmd {
        Command::Pipe => {
            thread::spawn(move || pipe::stdintolora(&mut ls, maxpacketsize).expect("Failure in stdintolora"));
            pipe::loratostdout(radioreceiver).expect("Failure in loratostdout");
        },
        Command::Kiss => {
            thread::spawn(move || kiss::stdintolorakiss(&mut ls, maxpacketsize).expect("Failure in stdintolorakiss"));
            kiss::loratostdout(radioreceiver).expect("Failure in loratostdout");
        },
        Command::Ping => {
            thread::spawn(move || ping::genpings(&mut ls).expect("Failure in genpings"));
            pipe::loratostdout(radioreceiver).expect("Failure in loratostdout");
        },
        Command::Pong => {
            ping::pong(&mut ls, radioreceiver).expect("Failure in loratostdout");
        }
    }

}
