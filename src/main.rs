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
    let (mut ls, mut radioreceiver) = lorastik::LoraStik::new(loraser);
    ls.radiocfg().expect("Failed to configure radio");

    thread::spawn(move || pipe::loratostdout(radioreceiver));
    pipe::stdintolora(&mut ls).expect("Failure in stdintolora");
    
}
