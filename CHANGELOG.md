# [Unreleased (still)]

* Display membership status when updated
* Join a room you have been invited to
* Client sends read receipts to server
* Display when messages have been read
* Leave a room by pressing Delete key (this should probably be a Ctrl-some key deal...)
* Specify homeserver to join on start up (before the login screen)
  * Simply run `rumatui [HOMESERVER]`, defaults to `http://matrix.org`
* `ErrorWidget` displays errors, now a wrapper `Error` type is needed to show something useful

# [Unreleased] pre 0.1.0

Improvements:

* Receive and display messages
  * formatted messages display as rendered markdown
* Send messages
  * local echo has been removed
  * Send textbox grows as more lines of text are added
* Selectable rooms list
  * change rooms using the arrow keys, making this clickable may be difficult
* Login widget is click/arrow key navigable
  * hides password
* TODO work on error widget for displaying recoverable errors
