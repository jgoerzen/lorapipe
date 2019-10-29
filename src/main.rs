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
    
    /// Radio initialization command file
    #[structopt(long, parse(from_os_str))]
    initfile: Option<PathBuf>,

    #[structopt(parse(from_os_str))]
    /// Serial port to use to communicate with radio
    port: PathBuf,

    #[structopt(subcommand)]
    cmd: Command
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Pipe data across raios
    Pipe {
        /// Maximum frame size sent to radio
        #[structopt(long, default_value = "99")]
        maxpacketsize: usize,
    },
    /// Transmit ping requests
    Ping,
    /// Receive ping requests and transmit pongs
    Pong,
}

fn main() {
    let opt = Opt::from_args();
    println!("{:?}", opt);

    if opt.debug {
        WriteLogger::init(LevelFilter::Trace, Config::default(), io::stderr()).expect("Failed to init log");
    }
    info!("lora starting");

    let loraser = ser::LoraSer::new(opt.port).expect("Failed to initialize serial port");
    let (mut ls, radioreceiver) = lorastik::LoraStik::new(loraser, opt.readqual);
    ls.radiocfg(opt.initfile).expect("Failed to configure radio");

    let mut ls2 = ls.clone();
    thread::spawn(move || ls2.readerthread().expect("Failure in readerthread"));

    match opt.cmd {
        Command::Pipe{ maxpacketsize } => {
            thread::spawn(move || pipe::stdintolora(&mut ls, maxpacketsize).expect("Failure in stdintolora"));
            pipe::loratostdout(radioreceiver).expect("Failure in loratostdout");
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
