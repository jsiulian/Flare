# Flare

[![Matrix](https://img.shields.io/badge/Matrix-Join-brightgreen)](https://matrix.to/#/%23flare-signal:matrix.org)
[![Translation status](https://hosted.weblate.org/widgets/schmiddi-on-mobile/-/flare/svg-badge.svg)](https://hosted.weblate.org/engage/schmiddi-on-mobile/)

Flare is an unofficial app that lets you chat with your friends on Signal from Linux.

## Screenshot

![Overview](https://gitlab.com/schmiddi-on-mobile/flare/-/raw/master/data/screenshots/screenshot.png)

## Installation

<table>
  <tr>
    <td>Flatpak</td>
    <td>
      <a href='https://flathub.org/apps/details/de.schmidhuberj.Flare'><img width='130' alt='Download on Flathub' src='https://flathub.org/api/badge?svg&locale=en'/></a>
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
  <td>Nix</td>
  <td>[flare-signal](https://search.nixos.org/packages?channel=23.11&show=flare-signal&from=0&size=50&sort=relevance&type=packages&query=flare)</td>
  </tr>
  <tr>
    <td>Self Compile</td>
    <td>See [Compilation](https://gitlab.com/schmiddi-on-mobile/flare/-/blob/master/doc/compilation.md).</td>
  </tr>
</table>

## Dependencies

Flare uses the [libsecret](https://wiki.gnome.org/Projects/Libsecret) library to store credentials. To use the Flatpak, you must install libsecret as a system package.

## Features

Flare is an unofficial app for Signal. It is still in development and doesn't include all the features that the official Signal apps do. More information can be found on its [feature roadmap](https://gitlab.com/schmiddi-on-mobile/flare/-/wikis/Feature-roadmap).

## Translation

Flare can easily be translated for other languages, as it uses gettext. Please consider contributing translations using [Weblate](https://hosted.weblate.org/engage/flare/), as an alternative you can also open merge requests and I will notify you if updates are necessary. Thanks to Weblate for free hosting and all the translators for their great work keeping the translations up-to-date.

<a href="https://hosted.weblate.org/engage/flare/">
<img src="https://hosted.weblate.org/widgets/schmiddi-on-mobile/-/flare/multi-auto.svg" alt="Translation status" />
</a>

## Contributing

This project is open to contributions. Please refer to [CONTRIBUTING.md](https://gitlab.com/schmiddi-on-mobile/flare/-/blob/master/CONTRIBUTING.md) for more information.
If you plan to contribute code, please also review the [developer documentation](https://gitlab.com/schmiddi-on-mobile/flare/-/tree/master/doc?ref_type=heads).

## Code of Conduct

This project follows [GNOME's Code of Conduct](https://conduct.gnome.org/).

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
