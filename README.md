# RumaTui
![rumatui-logo](https://github.com/DevinR528/RumaTui/blob/master/resources/small_logo.gif)
## A command-line Matrix client.
[![Build Status](https://travis-ci.com/DevinR528/rumatui.svg?branch=master)](https://travis-ci.com/DevinR528/rumatui)
[![Latest Version](https://img.shields.io/crates/v/rumatui.svg)](https://crates.io/crates/rumatui)
[![#rumatui](https://img.shields.io/badge/matrix-%23rumatui-purple?style=flat-square)](https://matrix.to/#/#rumatui:matrix.org)

Work In Progress. A Matrix client written using [tui.rs](https://github.com/fdehau/tui-rs) and [matrix-rust-sdk](https://github.com/matrix-org/matrix-rust-sdk) to provide a clickable cli to chat.

![rumatui-demo](https://github.com/DevinR528/rumatui/blob/master/resources/rumatui-notice.gif)

This project is still very much a work in progress. Please file issues, but I will preemptively say the error messages need work, and the code needs to be refactored to be a little more reader-friendly. Thanks for giving it a go!

# Install
For the latest and greatest
```bash
cargo install --git https://github.com/DevinR528/rumatui
```

Or the slightly safer approach but with fewer features (see [CHANGELOG](https://github.com/DevinR528/rumatui/blob/master/CHANGELOG.md#0113-alpha))
```bash
cargo install rumatui --version 0.1.13-alpha
```

# Run
```bash
rumatui [HOMESERVER | OPTIONS]
```
It can be run [torified](https://gitlab.torproject.org/legacy/trac/-/wikis/doc/TorifyHOWTO) with [torsocks](https://gitlab.torproject.org/legacy/trac/-/wikis/doc/torsocks):
```
torsocks rumatui [HOMESERVER | OPTIONS]
```
### Options
  * -h or --help Prints help information
  * -v or -verbose Will create a log of the session at '~/.rumatui/logs.json'

If no `homeserver` is specified, matrix.org is used.

# Use

Most of `rumatui` is click-able however, there are a few buttons that can be used (this is a terminal after all).

* Esc will exit `rumatui`
* Up/down arrow toggles login/register selected text box
* Enter still works for all buttons except the decline/accept invite
* Ctrl-s sends a message
* Delete leaves and forgets the selected room
* Left/right arrows, while at the login window, toggles login/register window
* Left arrow, while at the main chat window, brings up the room search window
* Enter, while in the room search window, starts the search
* Ctrl-d, while a room is selected in the room search window, joins the room

#### License
<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
</sub>
