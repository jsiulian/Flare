# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Immediately sync contacts after linking, without application restart required.
- Show UUID for unknown contacts (without name, phone numbers) instead of empty string.
- Notification support

### Changed

- Many UI updates ([#9], [#10], [#11], [#13])
- Use new libadwaita widgets (about dialog, message dialog, entry row)
- New message storage backend

## Fixed

- Wrongly associated stored messages from other devices ([#12])
- Possibly fixed [#4]

### BREAKING

- Due to changes in the storage backend, your previously stored messages will be lost.

## [0.4.1]

### Added

- Button in the link window to copy link URL to clipboard ([#6])
- Group storage
- Search for the channel view ([#7])

### Fixed

- Maybe fixed rare crash in the backend thread
- Fixed messages sent to contacts being wrongly stored to "Note to self" ([#8])

## [0.4.0]

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

[Unreleased]: https://gitlab.com/Schmiddiii/flare/-/compare/0.4.1...master
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
