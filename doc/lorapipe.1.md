% LORAPIPE(1) John Goerzen | lorapipe Manual
% John Goerzen
% October 2019

# NAME

lorapipe - Transfer data and run a network over LoRa long-range radios

# SYNOPSIS

**lorapipe** [ *OPTIONS* ] **PORT** **COMMAND** [ *command_options* ]

# OVERVIEW

**lorapipe** is designed to integrate LoRa long-range radios into a
Unix/Linux system.  In particular, lorapipe can:

- Bidirectionally pipe data across a LoRa radio system
- Do an RF ping and report signal strength at each end
- Operate an AX.25 network using LoRa, and atop it, TCP/IP

# HARDWARE REQUIREMENTS

**lorapipe** is designed to run with a Microchip RN2903/RN2483 as
implemented by LoStik.

Drivers for other hardware may be added in the future.

The Microchip firmware must be upgraded to 1.0.5 before running with
**lorapipe**.  Previous versions lacked the `radio rxstop` command,
which is a severe limitation when receiving multiple packets rapidly.

See the documents tab for the
[RN2093](https://www.microchip.com/wwwproducts/en/RN2903) and the
[firmware upgrade
guide](https://www.pocketmagic.net/rn2483-rn2903-firmware-upgrade-guide/) -
note that the upgrade part is really finicky and you need the "offset" file.

# PROTOCOL

The **lorapipe pipe** command is the primary one of interest here.  It
will receive data on stdin, break it up into LoRa-sized packets (see
**--maxpacketsize**), and transmit it across the radio.  It also will
receive data from the radio channel and send it to stdout.  No attempt
at encryption or authentication is made; all packets successfully
decoded will be sent to stdout.  Authentication and filtering is left
to other layers of the stack atop **lorapipe**.

A thin layer atop **lorapipe pipe** is **lorapipe kiss**, which
implements the AX.25 KISS protocol.  It transmits each KISS frame it
receives as a LoRa frame, and vice-versa.  It performs rudimentary
checking to ensure it is receiving valid KISS data, and will not pass
anything else to stdout.  This support can be used to build a TCP/IP
network atop LoRa as will be shown below.  Encryption and
authentication could be added atop this by using tools such as OpenVPN
or SSH.

**lorapipe** provides only the guarantees that LoRa itself does: that
raw LoRa frames which are decoded are intact, but not all frames will
be received.  It is somewhat akin to UDP in this sense.  Protocols
such as UUCP, ZModem, or TCP can be layered atop **lorapipe** to
transform this into a "reliable" connection.

## Broadcast Use and Separate Frequencies

It is quite possible to use **lorapipe** to broadcast data to multiple
listeners; an unlimited number of systems can run **lorapipe pipe**
to receive data, and as long as there is nothing on stdin, they will
happily decode data received over the air without transmitting
anything.

Separate communication channels may be easily achieved by selecting
separate radio frequencies.

## Collision Mitigation

**lorapipe** cannot provide collision detection or avoidance, though
it does impliement a collision mitigation strategy as described below.

As LoRa radios are half-duplex (they cannot receive while
transmitting), this poses challenges for quite a few applications that
expect full-duplex communication or something like it.  In testing, a
particular problem was observed with protocols that use transmission
windows and send data in packets.  These protocols send ACKs after a
successful packet transmission, which frequently collided with the
next packet transmitted from the other radio.  This caused serious
performance degredations, and for some protocols, complete failure.

There is no carrier detect signal from the LoRa radio.  Therefore, a
turn-based mechanism is implemented; with each frame transmitted, a
byte is prepended indicating whether the sender has more data in queue
to transmit or not.  The sender will continue transmitting until its
transmit buffer is empty.  When that condition is reached, the
other end will begin transmitting whatever is in its queue.  This
enables protocols such as UUCP "g" and UUCP "i" to work quite well.

A potential complication could arise if the "last" packet from the
transmitter never arrives at the receiver; the receiver might
therefore never take a turn to transmit.  To guard against this
possibility, there is a timer, and after receiving no packets for a
certain amount of time, the receiver will assume it is acceptable to
transmit.  This timeout is set by the **--eotwait** option and
defaults to 1000ms (1 second).

The signal about whether or not data remains in the queue takes the
form of a single byte prepended to every frame.  It is 0x00 if no data
will follow immediately, and 0x01 if data exists in the transmitters
queue which will be sent immediately.  The receiving side processes
this byte and strips it off before handing the data to the
application.  This byte is, however, visible under **--debug** mode,
so you can observe the protocol at this low level.

# RADIO PARAMETERS AND INITIALIZATION

The Microchip command reference, available at
<http://ww1.microchip.com/downloads/en/DeviceDoc/40001811A.pdf>,
describes the parameters available for the radio.  A LoRa data rate
calculator is available at
<https://www.rfwireless-world.com/calculators/LoRa-Data-Rate-Calculator.html>
to give you a rough sense of the speed of different parameters.  In
general, by sacrificing speed, you can increase range and robustness
of the signal.  The default initialization uses fairly slow and
high-range settings:

```
sys get ver
mac reset
mac pause
radio get mod
radio get freq
radio get pwr
radio get sf
radio get bw
radio get cr
radio get wdt
radio set pwr 20
radio set sf sf12
radio set bw 125
radio set cr 4/5
radio set wdt 60000
```

The `get` commands will cause the pre-initialization settings to be
output to stderr if `--debug` is used.  A maximum speed init would
look like this:

```
sys get ver
mac reset
mac pause
radio get mod
radio get freq
radio get pwr
radio get sf
radio get bw
radio get cr
radio get wdt
radio set pwr 20
radio set sf sf7
radio set bw 500
radio set cr 4/5
radio set wdt 60000
```

You can craft your own parameters and pass them in with `--initfile`
to customize the performance of your RF link.

A particular hint: if `--debug` shows `radio_err` after a `radio rx 0`
command, the radio is seeing carrier but is getting CRC errors
decoding packets.  Increasing the code rate with `radio set cr` to a
higher value such as `4/6` or even `4/8` will increase the FEC
redundancy and enable it to decode some of those packets.  Increasing
code rate will not help if there is complete silence from the radio
during a transmission; for those situations, try decreasing bandwidth
or increasing the spreading factor.  Note that coderate `4/5` to the
radio is the same as `1` to the calculator, while `4/8` is the same as
`4`.

# PROTOCOL HINTS

Although **lorapipe pipe** doesn't guarantee it preserves application
framing, in many cases it does.  For applications that have their own
framing, it is highly desirable to set their frame size to be less
than the **--maxpacketsize** setting.  This will
reduce the amount of data that would have to be retransmitted due to
lost frames.

As speed decreases, packet size should as well.

# APPLICATION HINTS

## SOCAT

The **socat**(1) program can be particularly helpful; it can gateway TCP
ports and various other sorts of things into **lorapipe**.  This is
helpful if the **lorapipe** system is across a network from the system
you wish to run an application on.  **ssh**(1) can also be useful for
this purpose.

A basic command might be like this:

```
socat TCP-LISTEN:12345 EXEC:'lorapipe /dev/ttyUSB0 pipe,pty,rawer'
```

Some systems might require disabling buffering in some situations, or
using a pty.  In those instances, something like this may be in order:

```
socat TCP-LISTEN:10104 EXEC:'stdbuf -i0 -o0 -e0 lorapipe /dev/ttyUSB4 pipe,pty,rawer'
```

## UUCP

For UUCP, I recommend protocol `i` with the default window-size
setting.  Use as large of a packet size as you can; for slow links,
perhaps 32, up to around 100 for fast, high-quality links.  (LoRa seems to
not do well with packets above 100 bytes).

Protocol `g` (or `G` with a smaller packet size) can also work, but
won't work as well.

Make sure to specify `half-duplex true` in `/etc/uucp/port`.

Here is an example of settings in `sys`:
```
protocol i
protocol-parameter i packet-size 90
protocol-parameter i timeout 30
chat-timeout 60
```

Note that UUCP protocol i adds 10 bytes of overhead per packet, so
this is designed to work with the default recommended packet size of
100.

Then in `/etc/uucp/port`:

```
half-duplex true
reliable false
```

## YMODEM (and generic example of bidirectional pipe)

ZModem makes a poor fit for LoRa because its smallest block size is
1K.  YModem, however, uses a 128-byte block size.  Here's an example
of how to make it work.  Let's say we want to transmit /bin/true over
the radio.  We could run this:

```
socat EXEC:'sz --ymodem /bin/true' EXEC:'lorapipe /dev/ttyUSB0 pipe,pty,rawer'
```

And on the receiving end:

```
socat EXEC:'rz --ymodem' EXEC:'lorapipe /dev/ttyUSB0 pipe,pty,rawer'
```

This approach can also be used with many other programs.  For
instance, `uucico -l` for UUCP logins.

## KERMIT

Using the C-kermit distribution (**apt-get install ckermit**), you can
configure for **lorapipe** like this:

```
set receive packet-length 90
set send packet-length 90
set duplex half
set window 2
set receive timeout 10
set send timeout 10
```

Then, on one side, run:

```
pipe lorapipe /dev/ttyUSB0 pipe
Ctrl-\ c
server
```

And on the other:

```
pipe lorapipe /dev/ttyUSB0 pipe
Ctrl-\ c
```

Now you can do things like `rdir` (to see ls from the remote), `get`,
`put`, etc.

## DEBUGGING WITH CU

To interact directly with the modem, something like this will work:

```
cu -h --line /dev/ttyUSB0 -s 57600 -e -o -f --nostop
```

# RUNNING TCP/IP OVER LORAPIPE WITH AX.25

The AX.25 protocol was initially designed to be used for amateur radio
purposes.  As the original amateur radio systems have a number of
properties in common with LoRa, it makes a reasonable way to run a
TCP/IP stack atop LoRa.  **lorapipe** supports it via the [KISS
protocol](http://www.ax25.net/kiss.aspx), which is similar to PPP for
AX.25.

We cannot use PPP directly over LoRa because PPP assumes a reliable,
full-duplex connection.  KISS does not.

These instructions assume Debian or Raspbian.  Other operating systems
may be different.

First, install the AX.25 tools: `apt-get install ax25-tools
ax25-apps socat`.

Now, edit `/etc/ax25/axports` and add a line such as:

```
lora    NODE1           1200    70      2       lorapipe radio
```

This defines a port named **lora**, with fake "callsign" **NODE1**,
speed 1200 (which is ignored), maximum packet length 70, and
window 2.  Keep the packet length less than the **--maxpacketsize**.
It is possible that KISS frames may expand due to escaping;
**lorapipe** will fragment them in this case, but it is best to keep
this size significantly less than the **lorapipe** max packet size to
avoid fragmentation as much as possible.  On other machines, give them
unique callsigns (NODE2 or FOO1 or whatever you like).

Now, start KISS:

```
kissattach /dev/ptmx lora 192.168.2.2
AX.25 port lora bound to device ax0
Awaiting client connects on
/dev/pts/7
```

That IP address was made up; you can use any RFC1918 IP address here;
just make sure they're different on each node.

It says to connect to /dev/pts/7, so we'll do just that:

```
socat /dev/pts/7,rawer \
  EXEC:'lorapipe /dev/ttyUSB0 kiss,pty,rawer'
```

Now, assume you connected a second machine to 192.168.2.3, you should
be able to ping and talk back and forth between them.  Standard
commands will work at this stage.  You may wish to adjust the packet
size in /etc/axports up from 70.

# INSTALLATION

**lorapipe** is a Rust program and can be built by running **`cargo
build --release`**.  The executable will then be placed in
**target/release/lorapipe**. Rust can be easily installed from
<https://www.rust-lang.org/>. 

# INVOCATION

Every invocation of **lorapipe** requires at least the name of a
serial port (for instance, **/dev/ttyUSB0**) and a subcommand to run.

# GLOBAL OPTIONS

These options may be specified for any command, and must be given
before the port and command on the command line.

**-d**, **--debug**
:  Activate debug mode.  Details of program operation will be sent to
   stderr.
   
**-h**, **--help**
:  Display brief help on program operation.

**--readqual**
:  Attempt to read and log information about the RF quality of
   incoming packets after each successful packet received.  There are
   some corner cases where this is not possible.  The details will be
   logged with **lorapipe**'s logging facility, and are therefore only
   visible if **--debug** is also used.
   
**-V**, **--version**
:  Display the version number of **lorapipe**.

**--eotwait** *TIME*
:  The amount of time in milliseconds to wait after receiving a packet
   that indicates more are coming before giving up on receiving an
   additional packet and proceeding to transmit.  Ideally this would
   be at least the amount of time it takes to transmit 2 packets.
   Default: 1000.
   
**--initfile** *FILE*
:  A file listing commands to send to the radio to initialize it.
   If not given, a default set will be used.
   
**--txwait** *TIME*
:  Amount of time in milliseconds to pause before transmitting each
   packet.  Due to processing delays on the receiving end, packets
   cannot be transmitted immediately back to back.  Increase this if
   you are seeing frequent receive errors for back-to-back packets,
   which may be indicative of a late listen.  Experimentation has
   shown that a value of 120 is needed for very large packets, and is
   the default.  You may be able to use 50ms or less if you are
   sending small packets.  In my testing, with 100-byte packets, 
   a txwait of 50 was generally sufficient.

**--maxpacketsize** *BYTES*
:  The maximum frame size, in the range of 10 - 250.  The actual frame
   transmitted over the air will be one byte larger due to
   **lorapipe** collision mitigation as described above.
   Experimentation myself, and reports from others, suggests that LoRa
   works best when this is 100 or less.  Valid only for **kiss** and
   **pipe** commands; ignored for all others.

*PORT*
:  The name of the serial port to which the radio is attached.

*COMMAND*
:  The subcommand which will be executed.

# SUBCOMMANDS

## lorapipe ... pipe

The **pipe** subcommand is the main workhorse of the application and
is described extensively above.

## lorapipe ... ping

The **ping** subcommand will transmit a simple line of text every 10
seconds including an increasing counter.  It can be displayed at the
other end with **lorapipe ... pipe** or reflected with **lorapipe
... pong**.

## lorapipe ... pong

The **pong** subcommand receives packets and crafts a reply.  It is
intended to be used with **lorapipe ... ping**.  Its replies include
the signal quality SNR and RSSI if available.

# AUTHOR

John Goerzen <jgoerzen@complete.org>

# COPYRIGHT AND LICENSE

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
