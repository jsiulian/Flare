# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.19.0] - 2026-03-08

### Added

- Startup screen when loading messages.
- Clicking on a notification now brings up the channel the notification came from.

### Removed

- Temporarily disabled registering as a primary device due to it being broken upstream.
  It will be enabled again once upstream fixed the issue.

### Changed

- Decreased message spacing for messages sent in quick succession.
- Improved message loading spinner.
- Ported to the Adwaita shortcuts dialog, instead of the deprecated GTK one.
- Added some padding between attachments and the message label.
- Made the UI for reactions more compact.

## [0.18.8] - 2026-03-03

### HOTFIX

- Fix deadlock receiving messages.

### Changed

- Automatically focus the channel input when activating a channel.

### Fixed

- Reactions not showing up when loading a channel multiple times.
- Sometimes mentions not being displayed in the text.
- Activating channels with a keyboard shortcut not displaying the channel as activated in the channel list.
- Some reactions not being received.

## [0.18.7] - 2026-03-02

### HOTFIX

- Fix receive loop constantly restarting due to errors from the Signal servers.

## [0.18.6] - 2026-03-01

### HOTFIX

- Fix compilation on aarch64.

## [0.18.5] - 2026-03-01

### HOTFIX

- Fix for Flare not starting anymore. This was caused by changes from Signal servers.

## [0.18.4] - 2026-02-18

### HOTFIX

- Potential fix for receiving messages deadlocking at startup sometimes.

## [0.18.3] - 2026-02-13

### HOTFIX

- Fix crash of the backend when there is an error receiving messages.

## [0.18.2] - 2026-02-12

### Chores

- Update backend crates.

## [0.18.1] - 2026-01-21

### HOTFIX

- Fixed errors migrating from 0.17.5 to 0.18.0.

## [0.18.0] - 2026-01-14

### HOTFIX

- Fixed some messages not being received anymore, since some time 2026-01-13.

### Changed

- Enabled the primary device mode in the setup menu.

### Fixed

- Fixed some bugs with the device being in primary mode.

## [0.17.5] - 2025-12-04

### Fix

- Fixed avatars not being properly cached anymore.

## [0.17.4] - 2025-11-30

### HOTFIX

- Fixed unlinking the device not working.
- Fixed the store not actually being encrypted.

## [0.17.3] - 2025-11-12

### HOTFIX

- Fixed captchas for many sent messages.

## [0.17.2] - 2025-10-21

### HOTFIX

- Fixed linking the device.
- Fixed groups not showing up on startup.

### Fixed

- Downloading large/many attachments and switching to another chat would require a long time to load the messages in the new chat.

## [0.17.1] - 2025-10-13

### Fixed

- Implemented preedit, which made sending single-word messages harder if using autocompletion.
- Restarted the backend after the network changed, which could fix issues with sending messages directly after the network changing.

## [0.17.0] - 2025-07-18

### BREAKING

- This release will require a relink of Flare.

### HOTFIX

- Many contacts being displayed only by their phone number or 'Unknown Contact', as well as not having a profile picture set.

## [0.16.3] - 2025-06-03

### Fixed

- Mentions not being highlighted anymore.
- Mentions in a reply message highlighting the quoted message.

## [0.16.2] - 2025-06-01

### Fixed

- Some reactions not being displayed on official Signal applications.

## [0.16.1] - 2025-05-25

### Fixed

- Emoji messages never showing their sender.

## [0.16.0] - 2025-04-22

### Added

- Initial support for receiving stickers.

## [0.15.16] - 2025-04-09

### Fixed

- Images as attachments being cut off in messages.

## [0.15.15] - 2025-04-01

### Fixed

- Some channel names not displaying in the header bar, for example due to an ascii heart.
- Images sent not being downloadable for other people.

## [0.15.14] - 2025-03-09

### Fixed

- Ignore one more networking-related error.
- Don't stop receiving messages after an error occurred when receiving a message.

## [0.15.13] - 2025-03-06

### Fixed

- Fixed the attachment box being massive when attaching an image.
- Crash when starting Flare multiple times.

### Changed

- Let the notification deamon handle feedback.

## [0.15.12] - 2025-02-28

### Fixed

- PNG attachments are sent as files instead of images.

## [0.15.11] - 2025-02-26

### HOTFIX

- Compilation on aarch64.

## [0.15.10] - 2025-02-26

### Fixed

- Hide some more "no internet" errors, as Flare should be able to handle those automatically.

### Chores

- Update all dependencies.

## [0.15.9] - 2025-02-13

### Security

- Updated `libsignal-service-rs` to fix two upstream security issues:
    - [GHSA-r58q-66g9-h6g8](https://github.com/whisperfish/libsignal-service-rs/security/advisories/GHSA-r58q-66g9-h6g8)
    - [GHSA-r58q-66g9-h6g8](https://github.com/whisperfish/libsignal-service-rs/security/advisories/GHSA-hrrc-wpfw-5hj2)

### Fixed

- Compilation without optional `libspelling` dependency.

## [0.15.8] - 2025-01-30

### HOTFIX

- Fix `409` error when linking the device.

## [0.15.7] - 2025-01-16

### HOTFIX

- Fix "Unauthorized" error when sending messages, and not receiving messages anymore.

## [0.15.6] - 2024-11-20

### Fixed

- Fixed contact synchronization not working.

## [0.15.5] - 2024-11-11

### Chores

- Updates to the backend, which should fix a few issues with Signal.

## [0.15.4] - 2024-11-02

### Changed

- Don't focus a few buttons when clicked, in order to keep on-screen keyboard open.
- Add counts to reaction emojis.
- Changed default reaction emoji list to match the one from Signal.

### Chores

- Updates to the backend, which should fix a few issues with Signal.

## [0.15.3] - 2024-10-07

### Chores

- Updated to GNOME 47.
- Improved linked devices window UI.
- Improved channel information UI.

## [0.15.2] - 2024-09-26

### HOTFIX

- Fixed linking errors.

### Fixed

- Metainfo not up-to-date.

## [0.15.1] - 2024-09-13

### Added

- Text entry input hints.

### Fixed

- Potential usage memory improvements.

### Chores

- Update dependencies.
- Add more checks to CI.
- Fixed advisory errors.

## [0.15.0] - 2024-07-22

### HOTFIX

- Linking the device being broken (fixed upstream).

### Added

- Environmental variable to enable primary device mode option.
- Linked devices window on primary device mode.

### Fixes

- No notifications being sent when Flare is in the background for the last active channel.
- Fix not working on systems without login1 D-Bus interface.
- Primary device mode not working.

## [0.14.3] - 2024-04-20

### Fixes

- Metainfo not being translatable.
- Crash when reacting with certain emojis.

## [0.14.2] - 2024-04-02

### Added

- Withdraw notification when message has been read.
 
### Fixed

- Failing to load groups and messages when started offline.
- Improve performance on startup.
- Sometimes the avatar and name showing for own messages.

### Chores

- Cleanup major parts of the code.
- Add developer documentation.

## [0.14.1] - 2024-03-22

### Added

- Revamped channel info dialog.

## [0.14.0] - 2024-03-20

### HOTFIX

- Groups not working for newly linked devices.

### Added

- Dialog for adding a new channel.

### Fixed

- UUID not found issues for some groups.

### Chores

- Updated GTK and libadwaita dependencies to GNOME 46 versions.

## [0.13.0] - 2024-02-25

### Added

- Avatars for contacts and groups.

### Fixed

- Persistent typing messages
- Possible crashes while linking

## [0.12.0] - 2024-02-11

### Added

- Draft message for each chat.
- Display of group descriptions and contact about information.
- Reception of typing indicators (contact-only for now).

### Changed

- The sidebar now highlights the active chat.
- Grab focus of the entry when replying to a message.
- UI improvements to the channel information dialog, which now also displays phone number, disappearing messages timer and description.
- Fix the messages being sent on shift+enter instead of ctrl+enter.

### Fixed

- Random color of note-to-self chat.

## [0.11.2] - 2024-01-04

### HOTFIX

- Backend panic after suspend.

### Fixed

- Use Control instead of Shift for invertet action on the text input.

## [0.11.1] - 2023-12-30

### Fixed

- High memory usage due to having unused emoji pickers.
- High memory usage of the setup window, even if unused.
- Send on Ctrl+Enter when sending on Enter is disabled.
- Long starting times due to formatting of contacts.
- Quitting the application turning off running in the background.

## [0.11.0] - 2023-12-25

### HOTFIX

- Contacts syncing.
- Linking device.

### Added

- A date-divider between messages sent on different days.
- Initial work for usage of Flare as a primary device (not yet enabled by default).
- Menu entry for contact sync.
- Menu entry for stopping Flare, even if it is running in the background.
- An improved setup window, which introduces Flare, gives the option to either link or use Flare as primary device (not yet enabled by default), insert required information and gives further information about Flare, e.g. some known issues or contact information.

### Changed

- Better UI for showing the last message of a channel.
- Only show the message timestamp if it differs from the next message.
- Better UI for the message entry bar.

### Fixed

- Unlinking Flare while running in the background is enabled not bringing up the relink window at next start.

### Chores

- Updated to GTK 4.12 and Libadwaita 1.4 and all other dependencies.

## [0.10.0] - 2023-08-28

### Added

- Deleting messages of a channel locally.
- Blurred placeholders for attachments.

### Changed

- Better UI for linking.
- Better UI for attachments.
- Better UI for message entry.

### Fixed

- Handle libsecret stores that can lock specific items.
- Restrict sending of a file-attachment such that a file-attachment must be sent without any additional attachments (restriction from Signal).

## [0.9.3] - 2023-08-17

### Added

- Support for [libspelling](https://gitlab.gnome.org/chergert/libspelling/).

### Fixed

- Fixed performance issues when loading messages.

### Note to Packagers

- New library required: [gtksourceview5](https://gitlab.gnome.org/GNOME/gtksourceview).
- New library recommended: [libspelling](https://gitlab.gnome.org/chergert/libspelling/).

## [0.9.2] - 2023-08-09

### HOTFIX

- Fix linking being broken.

### Changed

- Updated description and namespace.

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

[Unreleased]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.19.0...master
[0.19.0]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.18.8...0.19.0
[0.18.8]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.18.7...0.18.8
[0.18.7]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.18.6...0.18.7
[0.18.6]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.18.5...0.18.6
[0.18.5]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.18.4...0.18.5
[0.18.4]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.18.3...0.18.4
[0.18.3]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.18.2...0.18.3
[0.18.2]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.18.1...0.18.2
[0.18.1]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.18.0...0.18.1
[0.18.0]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.17.5...0.18.0
[0.17.5]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.17.4...0.17.5
[0.17.4]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.17.3...0.17.4
[0.17.3]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.17.2...0.17.3
[0.17.2]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.17.1...0.17.2
[0.17.1]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.17.0...0.17.1
[0.17.0]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.16.3...0.17.0
[0.16.3]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.16.2...0.16.3
[0.16.2]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.16.1...0.16.2
[0.16.1]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.16.0...0.16.1
[0.16.0]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.15.16...0.16.0
[0.15.16]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.15.15...0.15.16
[0.15.15]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.15.14...0.15.15
[0.15.14]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.15.13...0.15.14
[0.15.13]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.15.12...0.15.13
[0.15.12]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.15.11...0.15.12
[0.15.11]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.15.10...0.15.11
[0.15.10]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.15.9...0.15.10
[0.15.9]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.15.8...0.15.9
[0.15.8]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.15.7...0.15.8
[0.15.7]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.15.6...0.15.7
[0.15.6]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.15.5...0.15.6
[0.15.5]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.15.4...0.15.5
[0.15.4]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.15.3...0.15.4
[0.15.3]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.15.2...0.15.3
[0.15.2]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.15.1...0.15.2
[0.15.1]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.15.0...0.15.1
[0.15.0]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.14.3...0.15.0
[0.14.3]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.14.2...0.14.3
[0.14.2]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.14.1...0.14.2
[0.14.1]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.14.0...0.14.1
[0.14.0]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.13.0...0.14.0
[0.13.0]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.12.0...0.13.0
[0.12.0]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.11.2...0.12.0
[0.11.2]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.11.1...0.11.2
[0.11.1]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.11.0...0.11.1
[0.11.0]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.10.0...0.11.0
[0.10.0]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.9.3...0.10.0
[0.9.3]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.9.2...0.9.3
[0.9.2]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.9.1...0.9.2
[0.9.1]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.9.0...0.9.1
[0.9.0]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.8.2...0.9.0
[0.8.2]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.8.1...0.8.2
[0.8.1]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.8.0...0.8.1
[0.8.0]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.7.2...0.8.0
[0.7.2]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.7.1...0.7.2
[0.7.1]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.7.0...0.7.1
[0.7.0]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.6.0...0.7.0
[0.6.0]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.5.6...0.6.0
[0.5.6]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.5.5...0.5.6
[0.5.5]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.5.4...0.5.5
[0.5.4]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.5.3...0.5.4
[0.5.3]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.5.2...0.5.3
[0.5.2]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.5.1...0.5.2
[0.5.1]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.5.0...0.5.1
[0.5.0]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.4.1...0.5.0
[0.4.1]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.4.0...0.4.1
[0.4.0]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.3.3...0.4.0
[0.3.3]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.3.2...0.3.3
[0.3.2]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.3.1...0.3.2
[0.3.1]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.3.0...0.3.1
[0.3.0]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.2.1...0.3.0
[0.2.1]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.2.0...0.2.1
[0.2.0]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.1.5...0.2.0
[0.1.5]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.1.4...0.1.5
[0.1.4]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/0.1.3...0.1.4

[#4]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/4
[#5]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/5
[#6]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/6
[#7]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/7
[#8]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/8
[#9]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/9
[#10]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/10
[#11]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/11
[#12]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/12
[#13]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/13
[#14]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/14
[#15]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/15
[#16]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/16
[#17]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/17
[#18]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/18
[#19]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/19
[#20]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/20
[#21]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/21
[#22]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/22
[#23]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/23
[#24]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/24
[#25]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/25
[#26]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/26
[#27]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/27
[#28]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/28
[#29]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/29
[#30]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/30
[#31]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/31
[#32]: https://gitlab.com/schmiddi-on-mobile/flare/-/issues/32
