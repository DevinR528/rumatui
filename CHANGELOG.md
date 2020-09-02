# [Unreleased]

* Fix performance regression caused by `widgets::utils::markdown_to_terminal`
* Ctrl-k room filtering (Thanks to [zwieberl](https://github.com/zwieberl))
* Tab selects next text box (same as down arrow)

# [0.1.19]

* Update matrix-rust-sdk to a version (rev 037d62b) that uses ruma mono repo (rev 848b225)
* Default to `https` over `http`
* Fix device ID generation on every login
  * rumatui keeps track of each device's device_id
* Add ability to log to `~/.rumatui/log.json` using `RUST_LOG` env var or `-v/--verbose` cli arguments
* Add memory for send message textbox
  * When switching rooms whatever has been typed for that room will be kept when the user returns

# [0.1.17]

* Ignore all `widgets::ctrl_chars` tests for Nix packaging
* Use matrix-org/matrix-rust-sdk at master for sdk dependency

# [0.1.16]

* Add help output `rumatui [-h/--help]`
* Add license files to the release package

# Release 0.1.15

### Bug Fixes

* Remove http proxy left in from testing

## 0.1.14

* Room search is now available
  * Public rooms can be joined from the room search window 
* A user can register from the new register window
  * This features complete User Interactive Authentication by opening a web browser
* Message edits are shown
  * When markdown is part of the message they are properly formatted
* Reactions display under the respective message
* Redaction events are handled for reactions (emoji) and messages
* Update dependency
  * `muncher` 0.6.0 -> 0.6.1
  
* Note: the above features are only for displaying received events
  `rumatui` can not yet send these events

### Bug Fixes

* Send read receipts to mark the correct read message (it was sending random event ids)
* Send `read_marker` events instead of `read_receipt`

# Pre-release

## 0.1.13-alpha

* Errors are now displayed with more helpful messages
  * Using internal Error type instead of `anyhow::Error`
* Send a message with Ctrl-s
* Update dependencies
  * `mdcat` 0.15 -> 0.18.2
  * `serde` 1.0.111 -> 1.0.111
  * `regex` 1.3.7 -> 1.3.9
  * `tokio` 0.2.20 -> 0.2.21

## 0.1.12-alpha
* Display membership status when updated
* Join a room you are invited to
* Client sends read receipts to server
* Display when messages have been read
* Leave a room by pressing Delete key (this should probably be a Ctrl-some key deal...)
* Specify homeserver to join on startup (before the login screen)
  * Simply run `rumatui [HOMESERVER]`, defaults to "http://matrix.org"
* Displays errors, albeit not very helpful or specific
* Receive and display messages
  * formatted messages display as rendered markdown
* Send messages
  * local echo is removed
  * Send textbox grows as more lines of text are added
* Selectable rooms list
  * change rooms using the arrow keys, making this clickable may be difficult
* Login widget is click/arrow key navigable
  * hides password
