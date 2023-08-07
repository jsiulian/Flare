# Flare

[![Matrix](https://img.shields.io/badge/Matrix-Join-brightgreen)](https://matrix.to/#/%23flare-signal:matrix.org)
[![Translation status](https://hosted.weblate.org/widgets/flare/-/flare/svg-badge.svg)](https://hosted.weblate.org/engage/flare/)

Chat with your friends on Signal

## Screenshot

![Overview](https://gitlab.com/schmiddi-on-mobile/flare/-/raw/master/data/screenshots/screenshot.png)

## Installation

<table>
  <tr>
    <td>Flatpak</td>
    <td>
      <a href='https://flathub.org/apps/details/de.schmidhuberj.Flare'><img width='130' alt='Download on Flathub' src='https://flathub.org/assets/badges/flathub-badge-en.png'/></a>
    </td>
  </tr>
  <tr>
    <td>Arch Linux (AUR)</td>
    <td>[flare](https://aur.archlinux.org/packages/flare)</td>
  </tr>
  <tr>
    <td>Alpine Linux (testing), Postmarket OS (edge)</td>
    <td>[flare](https://pkgs.alpinelinux.org/package/edge/testing/x86/flare)</td>
  </tr>
  <tr>
    <td>Self Compile</td>
    <td>See [Compilation](https://gitlab.com/schmiddi-on-mobile/flare/-/blob/master/CONTRIBUTING.md#compilation).</td>
  </tr>
</table>

## Dependencies

Flare uses the [libsecret](https://wiki.gnome.org/Projects/Libsecret) library to store credentials. To use the Flatpak, you must install libsecret as a system package.

If you would like sound or vibration for Flare's notifications, install [feedbackd](https://source.puri.sm/Librem5/feedbackd) as a system package.

## Features

For a list of features to be added, see [the wiki](https://gitlab.com/schmiddi-on-mobile/flare/-/wikis/Feature-roadmap)

- Linking device
- Sending a message
- Receiving messages
- Replying to a message
- Reacting to a message
- Attachments
- Message storage
- Encrypted storage (As I am not a security-expert, I do not guarantee anything. Read the `Security`-section)
- Notifications

## Translation

Flare can easily be translated for other languages, as it uses gettext. Please consider contributing translations using [Weblate](https://hosted.weblate.org/engage/flare/), as an alternative you can also open merge requests and I will notify you if updates are necessary. Thanks to Weblate for free hosting and all the translators for their great work keeping the translations up-to-date.

<a href="https://hosted.weblate.org/engage/flare/">
<img src="https://hosted.weblate.org/widgets/flare/-/flare/multi-auto.svg" alt="Translation status" />
</a>

## Contributing

This project is open to contributions. Please refer to [CONTRIBUTING.md](https://gitlab.com/schmiddi-on-mobile/flare/-/blob/master/CONTRIBUTING.md) for more information.

## Code of Conduct

This project follows [GNOME's Code of Conduct](https://wiki.gnome.org/Foundation/CodeOfConduct).

## Donate

Please consider donating to [Signal](https://signal.org/donate/) first as they run the servers in use and therefore have a high cost of actually providing this free service.

After you have already donated to them and still have money left, consider donating to this Monero address:

```
86oyawuujDNVpT7jjYghhPc8xZjGB1DwQ3NX4mVhqxXZdRXjMEq7SWU3spD8L8stmYgEWV5BrAdY7X1uCKzRdrYcDwLt8cB
```

## Security

To my knowledge, most data (see below) this application uses (contacts, linking credentials, ...) are stored encrypted in `~/.local/share/flare` (path will be different in Flatpaks). Messages sent and received by this application are stored equivalently.

Even though things are encrypted, I do not guarantee for the security of your data. This application will probably worsen the security compared to official Signal products. Use this application with care when handling sensitive data.

### Encrypted

- Linking credentials
- Contact and group details
- Message contents

### Not Encrypted

- Number of contacts and groups
- Number of messages in a chat (but without information on what specific chat; does not only include "visible" messages but also messages to synchronize between clients)
- Message timestamps

### More detailed notes on encryption

This application stores data using [sled](https://crates.io/crates/sled) using [matrix-sdk-store-encryption](https://crates.io/crates/matrix-sdk-store-encryption) encryption. The passphrase for the encryption is stored and retrieved using [libsecret](https://crates.io/crates/libsecret).
