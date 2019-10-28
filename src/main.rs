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

use std::env;
use std::process::exit;
use simplelog::*;
use std::io;
use log::*;
use std::thread;

mod ser;
mod lorastik;
mod pipe;
mod ping;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        println!("Syntax: lora command portname");
        println!("Valid commands are: pipe ping pong kiss");
        exit(255);
    }

    WriteLogger::init(LevelFilter::Trace, Config::default(), io::stderr()).expect("Failed to init log");
    info!("lora starting");

    let loraser = ser::LoraSer::new(&args[2]).expect("Failed to initialize serial port");
    let (mut ls, radioreceiver) = lorastik::LoraStik::new(loraser, true);
    ls.radiocfg().expect("Failed to configure radio");

    let mut ls2 = ls.clone();
    thread::spawn(move || ls2.readerthread().expect("Failure in readerthread"));

    if args[1] == String::from("pipe") {
        thread::spawn(move || pipe::stdintolora(&mut ls).expect("Failure in stdintolora"));
        pipe::loratostdout(radioreceiver).expect("Failure in loratostdout");
    } else if args[1] == String::from("ping") {
        thread::spawn(move || ping::genpings(&mut ls).expect("Failure in genpings"));
        pipe::loratostdout(radioreceiver).expect("Failure in loratostdout");
    } else if args[1] == String::from("pong") {
        ping::pong(&mut ls, radioreceiver).expect("Failure in loratostdout");
    }
    
}
