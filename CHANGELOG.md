# [Unreleased] pre 0.1.0

## 0.1.14

* Message edits
* Reactions
* Redactions

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
* Join a room you have been invited to
* Client sends read receipts to server
* Display when messages have been read
* Leave a room by pressing Delete key (this should probably be a Ctrl-some key deal...)
* Specify homeserver to join on start up (before the login screen)
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
