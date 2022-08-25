# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://gitlab.com/Schmiddiii/flare/-/compare/0.3.1...master
[0.3.1]: https://gitlab.com/Schmiddiii/flare/-/compare/0.3.0...0.3.1
[0.3.0]: https://gitlab.com/Schmiddiii/flare/-/compare/0.2.1...0.3.0
[0.2.1]: https://gitlab.com/Schmiddiii/flare/-/compare/0.2.0...0.2.1
[0.2.0]: https://gitlab.com/Schmiddiii/flare/-/compare/0.1.5...0.2.0
[0.1.5]: https://gitlab.com/Schmiddiii/flare/-/compare/0.1.4...0.1.5
[0.1.4]: https://gitlab.com/Schmiddiii/flare/-/compare/0.1.3...0.1.4

[#4]: https://gitlab.com/Schmiddiii/flare/-/issues/4
[#5]: https://gitlab.com/Schmiddiii/flare/-/issues/5
