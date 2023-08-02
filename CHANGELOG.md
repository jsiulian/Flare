# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.9.1] - 2023-08-02

### Added

- Notifications on reactions received (configurable).
- Setting to turn off sending a message when the enter-key is pressed. 
- Ability to submit captcha requests.

### Changed

- Text of the quote in the reply message is now truncated to 2 lines.
- Moved the download and open attachments buttons to the popover.
- Don't hard-code the font size for texts.

### Fixed

- General UI fixes.

### Chores

- Update all dependencies. This especially includes presage/libsignal-service-rs backends becoming post-quantum secure.

## [0.9.0] - 2023-07-10

### Changed

- Added support for voice messages/audio files.
- Ported the ui to Blueprint.
- New UI improvements to the feed list. This also now respects the accent colors set in GNOME.
- Reworked the popover menu for messages.
- Updated to GTK 4.10 with many UI improvements and minor fixes.
- Message popups are now opened with right-click or long-press (touch only).

### Fixed

- Issue where special characters in replied messages were displayed incorrectly.
- Attachments with very long names not being ellipsized.
- Messages with only attachment not persisting reactions.

## [0.8.2] - 2023-05-29

### Added

- A "pick emoji" button for the text entry.
- Support for receiving mentions.
- Smaller UI improvements.

### Fixed

- Avatar of contacts with special characters not displaying the initials label.
- No profile names were shown in notifications.
- Removing expiration timer for conversations with expire messages.

## [0.8.1] - 2023-05-07

### Added

- Button to show all channels.
- Button to scroll down in the messages view.
- Keyboard shortcut "Ctrl+Q" to close window.

### Changed

- Only show non-empty channels by default.
- UI changes to differentiate own messages from others.

## [0.8.0] - 2023-05-03

### Changed

- Rewrote the message list to be a ListView.
- Slight fix for the icon.

### Fixed

- Switches in the settings having inconsistent activation.
- Possibly fixed session corruptions upstream.
- Don't ignore emoji remove reactions.
- Fixed sending wrong reaction with complex emojis.
- Fixed groups not working (upstream).

## [0.7.2] - 2023-04-08

### Added

- Message deletion.

### Changed

- Updated application icon and emblem.
- Don't receive messages from blocked contacts anymore.

## [0.7.1] - 2023-04-01

### HOTFIX

- Fix outgoing 1-to-1 messages stored incorrectly.

## [0.7.0] - 2023-03-29

### BREAKING

- Due to a new storage for data, a relink is required on first startup

### Added

- Integration with feedbackd.
- Showing profile names.
- Copy action in message menu.
- Initial channel information dialog with identity reset.
- Show offline status in GUI.

### Changed

- Notifications for group messages now display the name of the group instead of just the name of the sender.
- Messages must now be successfully sent before displaying in a chat.
- UI got revamped.

### Fixed

- Fixed not setting revision for groups.
- Prevent sending messages while offline.

### Chores

- Updated all dependencies.

## [0.6.0] - 2023-01-13

### Added

- Ability to receive messages in the background.

### Fixed

- Fixed restarting the backend after suspend.
- Fixed grammar error in the project description.

## [0.5.7] - 2022-12-17

### Fixed

- Fixed application freeze when there are many initial messages
- Fixed incorrect ordering of new messages

## [0.5.6] - 2022-12-03

### Fixed

- Fixed memory leak.
- Fixed displayed reply message not clearing attachments when changing reply.
- Fixed crash when searching non-existing channel.

## [0.5.5] - 2022-11-16

### HOTFIX

- Fix linking not working 

## [0.5.4] - 2022-11-15

### Changed

- Slight UI update to channel list

### Fixed

- Crash when clicking on call message
- URLs containing "&" not being displayed
- Fixed some (hopefully all) duplicate messages
- Fixed receiving messages from certain groups

## [0.5.3] - 2022-10-27

### HOTFIX

- Updated certificate of Signal servers (upstream, [#32])

## [0.5.2] - 2022-10-16

### Added

- Pasting files and images into the text entry ([#19])
- Unlink without deleting messages
- Messages and notifications for calls

### Fixed

- Fixed showing messages with "&", "<" or ">"

### Chores

- Refactored GTK properties
- Greatly refactored messages

## [0.5.1] - 2022-10-10

### Added

- Clickable links in text messages. ([#24])
- Open image in default program. ([#22])
- Contributing guidelines

### Changed

- Show backend thread panics.
- UI improvements for the channel list. ([#20])
- Display user initials in profile pictures. ([#26])

### Fixed

- Fixed duplicate message receiving.
- Wrapping for extremely long words.

## [0.5.0] - 2022-10-05

### Added

- Immediately sync contacts after linking, without application restart required.
- Show UUID for unknown contacts (without name, phone numbers) instead of empty string.
- Notification support

### Changed

- Many UI updates ([#9], [#10], [#11], [#13])
- Use new libadwaita widgets (about dialog, message dialog, entry row)
- New message storage backend

### Fixed

- Wrongly associated stored messages from other devices ([#12])
- Possibly fixed [#4]
- (Upstream) Fixed duplicate messages to other third-party Signal clients

### BREAKING

- Due to changes in the storage backend, your previously stored messages will be lost.

## [0.4.1] - 2022-09-20

### Added

- Button in the link window to copy link URL to clipboard ([#6])
- Group storage
- Search for the channel view ([#7])

### Fixed

- Maybe fixed rare crash in the backend thread
- Fixed messages sent to contacts being wrongly stored to "Note to self" ([#8])

## [0.4.0] - 2022-09-16

### Added

- Accessibility (no idea how good it is)
- Message storage
- Prevented sending empty messages
- Display the time the message was sent

### Fixed

- Attachment list not hiding after sending message
- Minor glib warning

## [0.3.3] - 2022-09-05

### Added

- Keyboard shortcuts

### Changed

- Small UI updates
- Moved from Entry to TextView which provides:
    - Multi-Line editing
    - Line Wrapping

### Fixed

- Fixed screenshot dummy

## [0.3.2] - 2022-08-30

### Fixed

- Revert fix for [#4] which fixes a crash when no default collection is set up.

## [0.3.1] - 2022-08-25

### Fixed

- Gschema not being installed

## [0.3.0] - 2022-08-25

### Added

- Settings dialog.
- Configurable device name.
- Basic video attachment support.
- Lazy-loading of attachments including configuration on what to load when viewed and what to load on click.

### Fixed

- Crash on startup when presage failed.
- Not unlocking the default libsecret collection at startup, see [#4].
    - Update: Actually not fixed, even made worse.

## [0.2.1] - 2022-08-15

### Added

- Section in the README on installation

### Changed

- Small UI change with messages
- Link to signal-notify renamed to messenger-notify

## Fixed

- Link-Window never popping up, see [#5].
- Unlink not clearing all the data

## [0.2.0] - 2022-08-07

### Added

- Weblate for translations.
- Ability to unlink device.
- Receive non-image attachments.
- Send non-image attachments.
- Display sender of last message in channel overview.

### Fixed

- Another attempt at fixing the application freeze. Pretty sure it should work this time.

### Internationalization

- New Norwegian Bokmål (thanks [@kingu](https://gitlab.com/kingu))
- New French (thanks [@rene-coty](https://gitlab.com/rene-coty))
- Modified German (thanks [@gastornis](https://gitlab.com/gastornis))
- Updated all the translations

## [0.1.5] - 2022-07-23
### Added

- Upload attachments (currently image-only).
- Download attachments.
- Remove reply message before sending a message.
- Remove attachments before sending a message.
- Improved UI for replies in the chat.

### Fixed

- Reply message not being cleared when sending a message.
- Reply message not being cleared when switching channels.
- Replies not showing correctly.
- Another attempt at fixing application freeze.
- Non-adaptive UI in the error dialog.

### Development

- Fixed cargo clippy.

## [0.1.4] - 2022-07-16
### Added

- A few new GitLab templates for issues (e.g. for feature requests).
- CHANGELOG

### Fixed

- Issue where messages sent from other device will land in the "Note to self" disregarding the recipient.
- Contacts with unknown name were displayed as "Note to self" in the channel list. Now their phone numbers are displayed.
- Maybe fix a crash where the application did not respond any more. (Update: The patch did not fix this issue. Still investigating)
- Maybe fix a crash when the sled database was locked causing the window not to open.

### Development

- Refactored that external classes should not access `Manager::internal`.
- Fixed cargo clippy.
- Updated presage to official repository.

[Unreleased]: https://gitlab.com/Schmiddiii/flare/-/compare/0.9.1...master
[0.9.1]: https://gitlab.com/Schmiddiii/flare/-/compare/0.9.0...0.9.1
[0.9.0]: https://gitlab.com/Schmiddiii/flare/-/compare/0.8.2...0.9.0
[0.8.2]: https://gitlab.com/Schmiddiii/flare/-/compare/0.8.1...0.8.2
[0.8.1]: https://gitlab.com/Schmiddiii/flare/-/compare/0.8.0...0.8.1
[0.8.0]: https://gitlab.com/Schmiddiii/flare/-/compare/0.7.2...0.8.0
[0.7.2]: https://gitlab.com/Schmiddiii/flare/-/compare/0.7.1...0.7.2
[0.7.1]: https://gitlab.com/Schmiddiii/flare/-/compare/0.7.0...0.7.1
[0.7.0]: https://gitlab.com/Schmiddiii/flare/-/compare/0.6.0...0.7.0
[0.6.0]: https://gitlab.com/Schmiddiii/flare/-/compare/0.5.6...0.6.0
[0.5.6]: https://gitlab.com/Schmiddiii/flare/-/compare/0.5.5...0.5.6
[0.5.5]: https://gitlab.com/Schmiddiii/flare/-/compare/0.5.4...0.5.5
[0.5.4]: https://gitlab.com/Schmiddiii/flare/-/compare/0.5.3...0.5.4
[0.5.3]: https://gitlab.com/Schmiddiii/flare/-/compare/0.5.2...0.5.3
[0.5.2]: https://gitlab.com/Schmiddiii/flare/-/compare/0.5.1...0.5.2
[0.5.1]: https://gitlab.com/Schmiddiii/flare/-/compare/0.5.0...0.5.1
[0.5.0]: https://gitlab.com/Schmiddiii/flare/-/compare/0.4.1...0.5.0
[0.4.1]: https://gitlab.com/Schmiddiii/flare/-/compare/0.4.0...0.4.1
[0.4.0]: https://gitlab.com/Schmiddiii/flare/-/compare/0.3.3...0.4.0
[0.3.3]: https://gitlab.com/Schmiddiii/flare/-/compare/0.3.2...0.3.3
[0.3.2]: https://gitlab.com/Schmiddiii/flare/-/compare/0.3.1...0.3.2
[0.3.1]: https://gitlab.com/Schmiddiii/flare/-/compare/0.3.0...0.3.1
[0.3.0]: https://gitlab.com/Schmiddiii/flare/-/compare/0.2.1...0.3.0
[0.2.1]: https://gitlab.com/Schmiddiii/flare/-/compare/0.2.0...0.2.1
[0.2.0]: https://gitlab.com/Schmiddiii/flare/-/compare/0.1.5...0.2.0
[0.1.5]: https://gitlab.com/Schmiddiii/flare/-/compare/0.1.4...0.1.5
[0.1.4]: https://gitlab.com/Schmiddiii/flare/-/compare/0.1.3...0.1.4

[#4]: https://gitlab.com/Schmiddiii/flare/-/issues/4
[#5]: https://gitlab.com/Schmiddiii/flare/-/issues/5
[#6]: https://gitlab.com/Schmiddiii/flare/-/issues/6
[#7]: https://gitlab.com/Schmiddiii/flare/-/issues/7
[#8]: https://gitlab.com/Schmiddiii/flare/-/issues/8
[#9]: https://gitlab.com/Schmiddiii/flare/-/issues/9
[#10]: https://gitlab.com/Schmiddiii/flare/-/issues/10
[#11]: https://gitlab.com/Schmiddiii/flare/-/issues/11
[#12]: https://gitlab.com/Schmiddiii/flare/-/issues/12
[#13]: https://gitlab.com/Schmiddiii/flare/-/issues/13
[#14]: https://gitlab.com/Schmiddiii/flare/-/issues/14
[#15]: https://gitlab.com/Schmiddiii/flare/-/issues/15
[#16]: https://gitlab.com/Schmiddiii/flare/-/issues/16
[#17]: https://gitlab.com/Schmiddiii/flare/-/issues/17
[#18]: https://gitlab.com/Schmiddiii/flare/-/issues/18
[#19]: https://gitlab.com/Schmiddiii/flare/-/issues/19
[#20]: https://gitlab.com/Schmiddiii/flare/-/issues/20
[#21]: https://gitlab.com/Schmiddiii/flare/-/issues/21
[#22]: https://gitlab.com/Schmiddiii/flare/-/issues/22
[#23]: https://gitlab.com/Schmiddiii/flare/-/issues/23
[#24]: https://gitlab.com/Schmiddiii/flare/-/issues/24
[#25]: https://gitlab.com/Schmiddiii/flare/-/issues/25
[#26]: https://gitlab.com/Schmiddiii/flare/-/issues/26
[#27]: https://gitlab.com/Schmiddiii/flare/-/issues/27
[#28]: https://gitlab.com/Schmiddiii/flare/-/issues/28
[#29]: https://gitlab.com/Schmiddiii/flare/-/issues/29
[#30]: https://gitlab.com/Schmiddiii/flare/-/issues/30
[#31]: https://gitlab.com/Schmiddiii/flare/-/issues/31
[#32]: https://gitlab.com/Schmiddiii/flare/-/issues/32
