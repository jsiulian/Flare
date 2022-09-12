# Flare

[![Matrix](https://img.shields.io/badge/Matrix-Join-brightgreen)](https://matrix.to/#/%23flare-signal:matrix.org)
[![Translation status](https://hosted.weblate.org/widgets/flare/-/flare/svg-badge.svg)](https://hosted.weblate.org/engage/flare/)

A unofficial Signal GTK client.

## Screenshot

![Overview](https://gitlab.com/Schmiddiii/flare/-/raw/master/data/screenshots/screenshot.png)

## Installation

<table>
  <tr>
    <td>Flatpak</td>
  </tr>
  <tr>
    <td>
      <a href='https://flathub.org/apps/details/de.schmidhuberj.Flare'><img width='130' alt='Download on Flathub' src='https://flathub.org/assets/badges/flathub-badge-en.png'/></a>
    </td>
  </tr>
</table>

## Features

- Linking device
- Sending a message
- Receiving messages
- Replying to a message
- Reacting to a message
- Attachments
- Message storage
- Encrypted storage (As I am not a security-expert, I do not guarantee anything. Read the `Security`-section)

## Planned Features

- The blocked features listed below once they are ready
- Maybe once mature enough: Primary device

## Not planned features

- Full compatibility with the official Signal products, including:
    - Calling
    - Paying
    - Majority of settings
- Notifications (use something standalone for that, e.g. [messenger-notify](https://gitlab.com/Schmiddiii/messenger-notify))

## Blocked features

This application uses [presage](https://github.com/whisperfish/presage) internally, the current features are not yet implemented for that.

- Listing groups
- Avatars
- Profile name instead of "You" in chats

## Translation

Flare can easily be translated for other languages, as it uses gettext. Please consider contributing translations using [Weblate](https://hosted.weblate.org/engage/flare/), as an alternative you can also open merge requests and I will notify you if updates are necessary.

## Donate

Please consider donating to [Signal](https://signal.org/donate/) first as they run the servers in use and therefore have a high cost of actually providing this free service.

After you have already donated to them and still have money left, consider donating to this Monero address:

```
86oyawuujDNVpT7jjYghhPc8xZjGB1DwQ3NX4mVhqxXZdRXjMEq7SWU3spD8L8stmYgEWV5BrAdY7X1uCKzRdrYcDwLt8cB
```

## Security

To my knowledge, any data this application uses (contacts, linking credentials, ...) are stored encrypted in `~/.local/share/flare` (path will be different in Flatpaks). Messages sent and received by this application are stored equivalently.

Even though things are encrypted, I do not guarantee for the security of your data. This application will probably worsen the security compared to official Signal products. Use this application with care when handling sensitive data.

### More detailed notes on encryption

This application stores data using [encrypted-sled](https://crates.io/crates/encrypted-sled) using [chacha20poly135](https://crates.io/crates/chacha20poly1305) encryption. The encryption key is 32 byte (= 256 bit) and is stored and retrieved using [libsecret](https://crates.io/crates/libsecret).
